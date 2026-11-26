//! Shellcode Obfuscator — egui/eframe GUI
//!
//! Patches the embedded windows/x64/shell_reverse_tcp shellcode with the
//! user-supplied LHOST and LPORT and outputs the result in the chosen format.

mod app;
mod formats;
mod input;
mod scripting;
mod shellcode;
mod web_core;
mod web_server;

use app::App;
use eframe::egui;

#[derive(Debug, PartialEq)]
enum Launch {
    Gui,
    Web { host: String, port: u16 },
    WebWorker,
    Help,
}

fn parse_args(args: &[String]) -> Result<Launch, String> {
    if args == ["--web-worker"] {
        return Ok(Launch::WebWorker);
    }
    let mut web = false;
    let mut port = None;
    let mut address = None;
    let mut args = args.iter().peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--web" if !web => {
                web = true;
                if args.peek().is_some_and(|value| !value.starts_with('-')) {
                    address = args.next().cloned();
                }
            }
            value if value.starts_with("--web=") && !web => {
                web = true;
                address = Some(value[6..].to_string());
            }
            "--port" if port.is_none() => {
                port = Some(
                    args.next()
                        .ok_or("--port requires a value")?
                        .parse::<u16>()
                        .map_err(|_| "Invalid port (0-65535)")?,
                );
            }
            "--help" | "-h" => return Ok(Launch::Help),
            _ => return Err(format!("Unknown or repeated argument: {arg}")),
        }
    }
    if web {
        if port.is_some() && address.is_some() {
            return Err("Use either --web HOST:PORT or --port, not both".into());
        }
        let (host, address_port) = match address {
            Some(address) => parse_web_address(&address)?,
            None => ("127.0.0.1".to_string(), 3000),
        };
        Ok(Launch::Web {
            host,
            port: port.unwrap_or(address_port),
        })
    } else if port.is_some() {
        Err("--port requires --web".into())
    } else {
        Ok(Launch::Gui)
    }
}
fn parse_web_address(address: &str) -> Result<(String, u16), String> {
    if let Ok(socket) = address.parse::<std::net::SocketAddr>() {
        return Ok((socket.ip().to_string(), socket.port()));
    }
    let (host, port) = address
        .rsplit_once(':')
        .ok_or("Expected HOST:PORT after --web (example: 127.0.0.1:3000)")?;
    if host.is_empty()
        || !host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'.')
    {
        return Err("Invalid web host; use an IP address or hostname (IPv6: [::1]:3000)".into());
    }
    let port = port.parse::<u16>().map_err(|_| "Invalid port (0-65535)")?;
    Ok((host.to_string(), port))
}

// ── entry point ──────────────────────────────────────────────────────────────

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_args(&args) {
        Ok(Launch::Gui) => {}
        Ok(Launch::Web { host, port }) => {
            if let Err(error) = web_server::run(&host, port) {
                eprintln!("Error: {error}");
                std::process::exit(1);
            }
            return Ok(());
        }
        Ok(Launch::WebWorker) => {
            web_core::worker();
            return Ok(());
        }
        Ok(Launch::Help) => {
            println!(
                "Usage: sc_obf [--web [HOST:PORT]]\n\nWithout arguments: open the desktop GUI.\n--web [HOST:PORT]: start the web interface (default: 127.0.0.1:3000).\nExamples: --web 0.0.0.0:8080 | --web localhost:4000 | --web [::1]:3000\n--port PORT: override the web server port (0-65535).\n-h, --help: show this help."
            );
            return Ok(());
        }
        Err(error) => {
            eprintln!("Error: {error}\nUsage: sc_obf [--web [HOST:PORT]]");
            std::process::exit(2);
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Shellcode Obfuscator")
            .with_inner_size([700.0, 860.0])
            .with_min_inner_size([500.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Shellcode Obfuscator",
        options,
        Box::new(|_cc| Ok(Box::new(App::default()))),
    )
}
