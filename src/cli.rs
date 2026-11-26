//! Command-line interface using the same parsers, generator, and formats as the GUIs.
use crate::{input, scripting, shellcode, web_core};
use base64::Engine as _;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};

pub(crate) const HELP: &str = r#"Usage: sc_obf cli <SOURCE> [OPTIONS]

Sources (choose one):
  --generate                  Generate the embedded payload
  --input, --load PATH         Load a file; use - for stdin
  --text TEXT                 Parse inline base64, hex, or escaped bytes

Generation:
  --lhost IPV4                Callback address (default: 127.0.0.1)
  --lport PORT                Callback port (default: 4444)
                              Either option implies --generate

Conversion:
  --input-format FORMAT       raw, base64, hex, escaped
                              Default: raw for --input; base64 for --text
  --format FORMAT             hex (default), base64, c, python, rust, csharp,
                              psh, vba, go, custom, raw
  --template TEXT             Text with one {bytes} placeholder; implies custom
  --template-file PATH        Read a plain-text template (or legacy JSON)
  --separator TEXT            Byte separator (default: ', '; empty is allowed)
  --byte-repr REPR            0xff (default), &Hff, or \xff
  --script, --python PATH      Run a Python file defining process(bytes) -> str
                              Its returned text replaces formatted output
  --output, -o PATH            Write output to a file (default: stdout; - also works)
  -h, --help                  Show this help

Examples:
  sc_obf cli --generate --lhost 127.0.0.1 --lport 4444 --format rust
  sc_obf cli --input payload.bin --format base64 -o payload.txt
  sc_obf cli --text '0090ff' --input-format hex --format c
  sc_obf cli --input - --input-format hex --format raw -o payload.bin
  sc_obf cli --input payload.bin --script transform.py -o result.txt

Custom Templates:
  Use {bytes} for the rendered bytes and {len} for the byte count.
  Other braces, quotes, backslashes, and newlines are kept literally.
  Example:
    sc_obf cli --input payload.bin --template 'byte[] buf = { {bytes} };'
    sc_obf cli --input payload.bin --template 'buf = "{bytes}"' --byte-repr '\xff' --separator ''
  For templates with mixed quotes, save the text in format.tpl and use:
    sc_obf cli --input payload.bin --template-file format.tpl
  Existing --template template.json files remain supported.

Python Scripting:
  Requires Python 3 installed and available to the application.
  Your script must define process(shellcode: bytes) -> str:

    import sys
    sys.path.insert(0, "/path/to/venv/lib/python3.x/site-packages")

    def process(shellcode: bytes) -> str:
        from Crypto.Cipher import AES
        # Return transformed output text.
        return shellcode.hex()

  The sys.path line is optional; use it to import packages from a virtual
  environment. Install any third-party packages your script imports.
  Input is provided as raw bytes. The returned string replaces formatted
  output (stdout or --output in CLI; the Output panel in GUI/web).
  Run with --script transform.py or --python transform.py.
  In CLI mode, print() messages go to stderr.
  If Python is unavailable, CLI scripting reports an error; GUI/web scripting
  controls stay disabled and show guidance to install Python.
"#;

type Result<T> = std::result::Result<T, String>;

struct Options {
    values: HashMap<String, String>,
    generate: bool,
}

impl Options {
    fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }
}

fn parse(args: &[String]) -> Result<Options> {
    let mut options = Options {
        values: HashMap::new(),
        generate: false,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--generate" {
            if options.generate {
                return Err("Repeated option: --generate".into());
            }
            options.generate = true;
            continue;
        }
        let (flag, inline_value) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(a, b)| (a, Some(b)));
        let key = match flag {
            "--input" | "--load" => "input",
            "--text" => "text",
            "--lhost" => "lhost",
            "--lport" => "lport",
            "--input-format" => "input-format",
            "--format" => "format",
            "--template" => "template",
            "--template-file" => "template-file",
            "--separator" => "separator",
            "--byte-repr" => "byte-repr",
            "--script" | "--python" => "script",
            "--output" | "-o" => "output",
            _ => return Err(format!("Unknown option: {arg}. Use sc_obf cli --help.")),
        };
        let value = match inline_value {
            Some(value) => value,
            None => args
                .next()
                .map(String::as_str)
                .ok_or_else(|| format!("{flag} requires a value"))?,
        };
        if (value.is_empty() && key != "separator")
            || (inline_value.is_none() && value.starts_with("--"))
        {
            return Err(format!("{flag} requires a value"));
        }
        if options.values.insert(key.into(), value.into()).is_some() {
            return Err(format!("Repeated option: {flag}"));
        }
    }
    Ok(options)
}

fn read(path: &str) -> Result<Vec<u8>> {
    if path == "-" {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Cannot read stdin: {e}"))?;
        Ok(bytes)
    } else {
        fs::read(path).map_err(|e| format!("Cannot read '{path}': {e}"))
    }
}

fn decode(bytes: Vec<u8>, format: &str) -> Result<Vec<u8>> {
    if format == "raw" {
        return Ok(bytes);
    }
    let text = String::from_utf8(bytes).map_err(|_| "Encoded input must be UTF-8 text")?;
    let text = text.trim();
    match format {
        "base64" => base64::engine::general_purpose::STANDARD
            .decode(text)
            .map_err(|e| format!("Invalid base64: {e}")),
        "hex" => input::parse_hex_bytes(text),
        "escaped" => input::parse_escaped_bytes(text),
        _ => Err(format!("Unknown input format: {format}")),
    }
}

fn custom_template(options: &Options) -> Result<Value> {
    if options.get("template").is_some() && options.get("template-file").is_some() {
        return Err("Choose --template TEXT or --template-file PATH, not both".into());
    }
    let (source, legacy_json) = if let Some(path) = options.get("template-file") {
        (
            fs::read_to_string(path).map_err(|e| format!("Cannot read template '{path}': {e}"))?,
            std::path::Path::new(path)
                .extension()
                .is_some_and(|ext| ext == "json"),
        )
    } else if let Some(text) = options.get("template") {
        if text.contains("{bytes}") {
            (text.to_string(), false)
        } else {
            // Keep the previous --template PATH syntax for JSON templates.
            (fs::read_to_string(text).map_err(|_| "Inline templates need one {bytes} placeholder; use --template-file PATH for a text file".to_string())?, true)
        }
    } else {
        return Err("Custom output requires --template TEXT or --template-file PATH".into());
    };
    let mut template = if legacy_json {
        let mut template: Value =
            serde_json::from_str(&source).map_err(|e| format!("Invalid template JSON: {e}"))?;
        if !template.is_object() {
            return Err("Template must be a JSON object".into());
        }
        if template.get("name").is_none() {
            template["name"] = json!("custom");
        }
        template
    } else {
        let (prefix, suffix) = source
            .split_once("{bytes}")
            .ok_or("Text templates need one {bytes} placeholder")?;
        if suffix.contains("{bytes}") {
            return Err("Text templates must contain exactly one {bytes} placeholder".into());
        }
        json!({"name":"custom", "prefix":prefix, "suffix":suffix, "separator":", ", "byte_repr":"0xff"})
    };
    if let Some(separator) = options.get("separator") {
        template["separator"] = json!(separator);
    }
    if let Some(repr) = options.get("byte-repr") {
        template["byte_repr"] = json!(repr);
    }
    web_core::convert(&json!({"format":"custom", "template":template}), &[])?;
    Ok(template)
}

pub fn run(args: &[String]) -> Result<()> {
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return Ok(());
    }
    let options = parse(args)?;
    let generate =
        options.generate || options.get("lhost").is_some() || options.get("lport").is_some();
    let sources = usize::from(generate)
        + usize::from(options.get("input").is_some())
        + usize::from(options.get("text").is_some());
    if sources != 1 {
        return Err(
            "Choose one source: --generate (or --lhost/--lport), --input PATH, or --text TEXT"
                .into(),
        );
    }
    if generate && options.get("input-format").is_some() {
        return Err("--input-format requires --input or --text".into());
    }
    let has_template = options.get("template").is_some() || options.get("template-file").is_some();
    let format = options
        .get("format")
        .unwrap_or(if has_template { "custom" } else { "hex" });
    if ![
        "hex", "base64", "c", "python", "rust", "csharp", "psh", "vba", "go", "custom", "raw",
    ]
    .contains(&format)
    {
        return Err(format!("Unknown output format: {format}"));
    }
    if let Some(input_format) = options.get("input-format")
        && !["raw", "base64", "hex", "escaped"].contains(&input_format)
    {
        return Err(format!("Unknown input format: {input_format}"));
    }
    if has_template && format != "custom" {
        return Err("Templates require --format custom".into());
    }
    if (options.get("separator").is_some() || options.get("byte-repr").is_some())
        && format != "custom"
    {
        return Err("--separator and --byte-repr require a custom template".into());
    }
    if options.get("script").is_some() && (format == "raw" || format == "custom") {
        return Err(
            "A Python script returns text; do not combine --script with raw output or --template"
                .into(),
        );
    }
    let template = if format == "custom" {
        custom_template(&options)?
    } else {
        Value::Null
    };
    let script = options
        .get("script")
        .map(|path| {
            fs::read_to_string(path).map_err(|e| format!("Cannot read script '{path}': {e}"))
        })
        .transpose()?;
    let bytes = if generate {
        let port = options
            .get("lport")
            .unwrap_or("4444")
            .parse::<u16>()
            .map_err(|_| "Invalid LPORT (0-65535)")?;
        shellcode::patch_shellcode(options.get("lhost").unwrap_or("127.0.0.1"), port)?
    } else if let Some(path) = options.get("input") {
        decode(read(path)?, options.get("input-format").unwrap_or("raw"))?
    } else {
        decode(
            options.get("text").unwrap().as_bytes().to_vec(),
            options.get("input-format").unwrap_or("base64"),
        )?
    };
    let (output, text_output) = if let Some(script) = script {
        (
            scripting::run_python_script_cli(&script, &bytes)?.into_bytes(),
            true,
        )
    } else if format == "raw" {
        (bytes, false)
    } else {
        (
            web_core::convert(&json!({"format": format, "template": template}), &bytes)?
                .into_bytes(),
            true,
        )
    };
    match options.get("output").unwrap_or("-") {
        "-" => {
            let mut stdout = io::stdout().lock();
            stdout
                .write_all(&output)
                .map_err(|e| format!("Cannot write stdout: {e}"))?;
            if text_output && !output.ends_with(b"\n") {
                stdout
                    .write_all(b"\n")
                    .map_err(|e| format!("Cannot write stdout: {e}"))?;
            }
            stdout
                .flush()
                .map_err(|e| format!("Cannot flush stdout: {e}"))?;
        }
        path => fs::write(path, output).map_err(|e| format!("Cannot write '{path}': {e}"))?,
    }
    Ok(())
}
