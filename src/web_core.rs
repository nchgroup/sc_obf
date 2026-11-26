//! Operations shared by the embedded web API worker.
use crate::{formats, input, scripting, shellcode};
use base64::Engine as _;
use serde_json::{Value, json};
use std::io::{self, Read};

pub(crate) fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .ok_or_else(|| format!("Missing string: {key}"))
}

fn convert(request: &Value, bytes: &[u8]) -> Result<String, String> {
    use formats::{ByteRepr, FormatTemplate, OutputFormat};
    let format = match text(request, "format")? {
        "hex" => OutputFormat::Hex,
        "base64" => OutputFormat::Base64,
        "c" => OutputFormat::C,
        "python" => OutputFormat::Python,
        "rust" => OutputFormat::Rust,
        "csharp" => OutputFormat::CSharp,
        "psh" => OutputFormat::PowerShell,
        "vba" => OutputFormat::VBA,
        "go" => OutputFormat::Go,
        "custom" => {
            let template = &request["template"];
            let repr = match text(template, "byte_repr")? {
                "0xff" => ByteRepr::Hex0x,
                "&Hff" => ByteRepr::HexAmpersand,
                "\\xff" => ByteRepr::EscapedX,
                _ => return Err("Invalid byte representation".into()),
            };
            return Ok(FormatTemplate::new(
                text(template, "name")?,
                text(template, "prefix")?,
                text(template, "separator")?,
                text(template, "suffix")?,
                repr,
            )
            .render(bytes));
        }
        _ => return Err("Unknown output format".into()),
    };
    Ok(formats::format_bytes(bytes, &format))
}

fn handle(request: &Value) -> Result<Value, String> {
    let base64 = base64::engine::general_purpose::STANDARD;
    let action = text(request, "action")?;
    if action == "info" {
        let templates: Vec<Value> = formats::builtin_templates()
            .iter()
            .map(|t| {
                json!({
                    "name": t.name, "prefix": t.prefix, "separator": t.separator,
                    "suffix": t.suffix, "byte_repr": t.byte_repr.label(),
                })
            })
            .collect();
        return Ok(
            json!({"python_available": scripting::python_is_available(), "templates": templates}),
        );
    }
    let bytes = match action {
        "generate" => {
            let port: u16 = text(request, "lport")?
                .trim()
                .parse()
                .map_err(|_| "invalid port (must be 0-65535)".to_string())?;
            shellcode::patch_shellcode(text(request, "lhost")?.trim(), port)?
        }
        "parse" => {
            let data = text(request, "text")?.trim();
            if data.is_empty() {
                return Err("paste area is empty".into());
            }
            match text(request, "input_format")? {
                "base64" => base64.decode(data).map_err(|e| e.to_string())?,
                "hex" => input::parse_hex_bytes(data)?,
                "escaped" => input::parse_escaped_bytes(data)?,
                _ => return Err("Unknown input format".into()),
            }
        }
        "format" | "script" => base64
            .decode(text(request, "bytes")?)
            .map_err(|e| e.to_string())?,
        _ => return Err("Unknown action".into()),
    };
    if bytes.len() > 5 * 1024 * 1024 {
        return Err("File exceeds 5 MiB".into());
    }
    let output = if action == "script" {
        if bytes.is_empty() {
            return Err("generate or load a payload first".into());
        }
        scripting::run_python_script(text(request, "script")?, &bytes)?
    } else {
        convert(request, &bytes)?
    };
    Ok(json!({"output": output, "bytes": base64.encode(&bytes), "count": bytes.len()}))
}

pub(crate) fn worker() {
    let mut source = String::new();
    let result = io::stdin()
        .take(8 * 1024 * 1024 + 1)
        .read_to_string(&mut source)
        .map_err(|e| e.to_string())
        .and_then(|_| serde_json::from_str::<Value>(&source).map_err(|e| e.to_string()))
        .and_then(|request| handle(&request));
    let response = match result {
        Ok(data) => json!({"ok": true, "data": data}),
        Err(error) => json!({"ok": false, "error": error}),
    };
    println!("\nSC_OBF_RESPONSE:{response}");
}
