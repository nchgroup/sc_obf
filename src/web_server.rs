use serde_json::{Value, json};
use std::fs;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

const MAX_REQUEST: usize = 8 * 1024 * 1024;
const MAX_RESPONSE: u64 = 64 * 1024 * 1024;
const OPERATION_TIMEOUT: Duration = Duration::from_secs(20);
const INDEX: &[u8] = include_bytes!("../web/index.html");
const JAVASCRIPT: &[u8] = include_bytes!("../web/app.js");
const STYLES: &[u8] = include_bytes!("../web/style.css");

type Result<T> = std::result::Result<T, String>;

fn header<'a>(request: &'a Request, name: &'static str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|h| h.field.equiv(name))
        .map(|h| h.value.as_str())
}

fn respond(request: Request, status: u16, body: Vec<u8>, content_type: &str) {
    let mut response = Response::from_data(body).with_status_code(StatusCode(status));
    for (key, value) in [
        ("Content-Type", content_type),
        ("X-Content-Type-Options", "nosniff"),
        ("Cache-Control", "no-store"),
        (
            "Content-Security-Policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
        ),
    ] {
        response.add_header(Header::from_bytes(key, value).expect("static HTTP header"));
    }
    let _ = request.respond(response);
}

fn json_response(request: Request, status: u16, value: Value) {
    respond(
        request,
        status,
        value.to_string().into_bytes(),
        "application/json; charset=utf-8",
    );
}

fn error(request: Request, status: u16, message: &str) {
    json_response(request, status, json!({"ok": false, "error": message}));
}

// Scripts run in another instance of this same executable. The HTTP server remains
// responsive if Python fails or does not finish; no Python server is involved.
fn run_worker(source: Vec<u8>) -> Result<Value> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut child = Command::new(executable)
        .arg("--web-worker")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().ok_or("Worker stdout unavailable")?;
    let output = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .by_ref()
            .take(MAX_RESPONSE + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_RESPONSE {
            return Err("Output exceeds 64 MiB".to_string());
        }
        Ok(bytes)
    });
    let start = Instant::now();
    let write_result = child
        .stdin
        .take()
        .ok_or("Worker stdin unavailable")?
        .write_all(&source);
    if let Err(err) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        let _ = output.join();
        return Err(err.to_string());
    }
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if start.elapsed() < OPERATION_TIMEOUT => {
                thread::sleep(Duration::from_millis(10))
            }
            Ok(None) => break Err("Operation timed out after 20 seconds".to_string()),
            Err(e) => break Err(e.to_string()),
        }
    };
    if status.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let bytes = output.join().map_err(|_| "Worker output reader failed")??;
    if !status?.success() {
        return Err("Rust worker failed to process the request".into());
    }
    // Ignore print() output from user scripts, including output flushed at exit.
    let text = String::from_utf8(bytes).map_err(|_| "Invalid worker output")?;
    let response = text
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix("SC_OBF_RESPONSE:"))
        .ok_or("Worker did not return a response")?;
    serde_json::from_str(response).map_err(|e| e.to_string())
}

fn script_path(directory: &Path, name: &str) -> Result<PathBuf> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '))
    {
        return Err("name must use only letters, digits, dash, underscore, or space".into());
    }
    let path = directory.join(format!("{name}.py"));
    if path.is_symlink() {
        return Err("Symbolic links are not supported".into());
    }
    Ok(path)
}

fn scripts(request: &Value, directory: &Path) -> Result<Value> {
    let action = crate::web_core::text(request, "action")?;
    let mut data = json!({});
    if action != "scripts" {
        let path = script_path(directory, crate::web_core::text(request, "name")?)?;
        match action {
            "save_script" => {
                fs::create_dir_all(directory).map_err(|e| e.to_string())?;
                fs::write(path, crate::web_core::text(request, "code")?)
                    .map_err(|e| e.to_string())?;
            }
            "load_script" => {
                data["code"] = json!(fs::read_to_string(path).map_err(|e| e.to_string())?)
            }
            "delete_script" => fs::remove_file(path).map_err(|e| e.to_string())?,
            _ => return Err("Unknown script action".into()),
        }
    }
    let mut names = Vec::new();
    if directory.exists() {
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if entry.file_type().map_err(|e| e.to_string())?.is_file()
                && path.extension().and_then(|s| s.to_str()) == Some("py")
                && let Some(name) = path.file_stem().and_then(|s| s.to_str())
            {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    data["scripts"] = json!(names);
    Ok(json!({"ok": true, "data": data}))
}

fn api(request: &mut Request, directory: &Path) -> Result<Value> {
    if header(request, "Content-Type")
        .and_then(|v| v.split(';').next())
        .map(str::trim)
        != Some("application/json")
    {
        return Err("Expected application/json".into());
    }
    if request
        .body_length()
        .is_some_and(|length| length > MAX_REQUEST)
    {
        return Err("Request exceeds 8 MiB".into());
    }
    let mut source = Vec::new();
    request
        .as_reader()
        .take(MAX_REQUEST as u64 + 1)
        .read_to_end(&mut source)
        .map_err(|e| e.to_string())?;
    if source.is_empty() || source.len() > MAX_REQUEST {
        return Err("Request exceeds 8 MiB or is empty".into());
    }
    let value: Value = serde_json::from_slice(&source).map_err(|e| e.to_string())?;
    match crate::web_core::text(&value, "action")? {
        "scripts" | "save_script" | "load_script" | "delete_script" => scripts(&value, directory),
        _ => run_worker(source),
    }
}

fn valid_host(authority: &str, address: SocketAddr, configured_host: &str) -> bool {
    let authority_with_port = if address.port() == 80 && !authority.contains(':') {
        format!("{authority}:80")
    } else {
        authority.to_string()
    };
    if authority_with_port.eq_ignore_ascii_case(&format!("{configured_host}:{}", address.port()))
        || authority_with_port == address.to_string()
        || ((address.ip().is_loopback() || address.ip().is_unspecified())
            && authority_with_port.eq_ignore_ascii_case(&format!("localhost:{}", address.port())))
    {
        return true;
    }
    // Wildcard listeners accept numeric interface addresses, while rejecting
    // arbitrary domain names. Origin must still match the request's authority.
    address.ip().is_unspecified()
        && authority_with_port
            .parse::<SocketAddr>()
            .is_ok_and(|host| host.port() == address.port() && host.is_ipv4() == address.is_ipv4())
}

fn serve(mut request: Request, address: SocketAddr, configured_host: &str, directory: &Path) {
    let authority = header(&request, "Host").unwrap_or("").to_string();
    if !valid_host(&authority, address, configured_host) {
        error(request, 403, "Invalid host");
        return;
    }
    if let Some(origin) = header(&request, "Origin")
        && origin != format!("http://{authority}")
    {
        error(request, 403, "Invalid origin");
        return;
    }
    match (
        request.method(),
        request.url().split('?').next().unwrap_or(""),
    ) {
        (&Method::Get, "/" | "/index.html") => {
            respond(request, 200, INDEX.to_vec(), "text/html; charset=utf-8")
        }
        (&Method::Get, "/app.js") => respond(
            request,
            200,
            JAVASCRIPT.to_vec(),
            "text/javascript; charset=utf-8",
        ),
        (&Method::Get, "/style.css") => {
            respond(request, 200, STYLES.to_vec(), "text/css; charset=utf-8")
        }
        (&Method::Post, "/api") => match api(&mut request, directory) {
            Ok(value) => json_response(request, 200, value),
            Err(message) => error(
                request,
                if message.contains("timed out") {
                    408
                } else {
                    400
                },
                &message,
            ),
        },
        _ => error(request, 404, "Not found"),
    }
}

pub fn run(host: &str, port: u16) -> Result<()> {
    let directory = std::env::var_os("SC_OBF_SCRIPTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
                .join("sc_obf")
                .join("scripts")
        });
    let server = Arc::new(Server::http((host, port)).map_err(|e| e.to_string())?);
    let address = server
        .server_addr()
        .to_ip()
        .ok_or("Expected TCP listener")?;
    println!("Shellcode Obfuscator · http://{address}");
    let _ = std::io::stdout().flush();
    let mut workers = Vec::new();
    for _ in 0..4 {
        let server = Arc::clone(&server);
        let directory = directory.clone();
        let host = host.to_string();
        workers.push(thread::spawn(move || {
            for request in server.incoming_requests() {
                serve(request, address, &host, &directory);
            }
        }));
    }
    for worker in workers {
        worker.join().map_err(|_| "HTTP worker failed")?;
    }
    Ok(())
}
