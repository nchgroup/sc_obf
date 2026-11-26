//! Shellcode Obfuscator — CLI, desktop GUI, and embedded web server.
//!
//! Patches the embedded windows/x64/shell_reverse_tcp shellcode with the
//! user-supplied LHOST and LPORT and outputs the result in the chosen format.

mod app;
mod cli;
mod formats;
mod input;
mod scripting;
mod shellcode;
mod web_core;
mod web_server;

use app::App;
use eframe::egui;

const HELP: &str = "Usage:
  sc_obf cli <OPTIONS>           Generate, load, and convert bytes
  sc_obf gui                     Open the desktop GUI
  sc_obf web [--bind IP:PORT]     Start the embedded web interface
  sc_obf help                   Show complete help for all modes

Web bind default: 127.0.0.1:3000
Use sc_obf cli --help or sc_obf web --help for more options.
Python scripting requirements and an example: sc_obf cli --help";

const GUI_HELP: &str = "Usage: sc_obf gui\n\nOpen the desktop GUI.";
const WEB_HELP: &str = "Usage: sc_obf web [--bind IP:PORT]\n\nDefault: 127.0.0.1:3000\nExamples:\n  sc_obf web --bind 0.0.0.0:8080\n  sc_obf web --bind '[::1]:3000'";

fn web_address(args: &[String]) -> Result<Option<std::net::SocketAddr>, String> {
    match args {
        [] => Ok(Some(std::net::SocketAddr::from(([127, 0, 0, 1], 3000)))),
        [flag] if flag == "--help" || flag == "-h" => Ok(None),
        [flag, value] if flag == "--bind" => value
            .parse()
            .map(Some)
            .map_err(|_| "Invalid --bind address; expected IP:PORT (IPv6: [::1]:3000)".into()),
        [value] if value.starts_with("--bind=") => value[7..]
            .parse()
            .map(Some)
            .map_err(|_| "Invalid --bind address; expected IP:PORT (IPv6: [::1]:3000)".into()),
        _ => Err("Usage: sc_obf web [--bind IP:PORT]".into()),
    }
}

fn fail(message: &str, code: i32) -> ! {
    eprintln!("Error: {message}");
    std::process::exit(code);
}

// ── entry point ──────────────────────────────────────────────────────────────

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str);
    let options = args.get(1..).unwrap_or_default();
    match command {
        Some("gui") if options.is_empty() => {}
        Some("cli") => {
            if let Err(error) = cli::run(options) {
                fail(&error, 1);
            }
            return Ok(());
        }
        Some("web") => {
            match web_address(options).unwrap_or_else(|error| fail(&error, 2)) {
                Some(address) => {
                    if let Err(error) = web_server::run(&address.ip().to_string(), address.port()) {
                        fail(&error, 1);
                    }
                }
                None => println!("{WEB_HELP}"),
            }
            return Ok(());
        }
        Some("--web-worker") if options.is_empty() => {
            web_core::worker();
            return Ok(());
        }
        Some("help") if options.is_empty() => {
            println!("{HELP}\n\n{GUI_HELP}\n\n{WEB_HELP}\n\n{}", cli::HELP);
            return Ok(());
        }
        None | Some("--help" | "-h") => {
            println!("{HELP}");
            return Ok(());
        }
        Some("gui") if options == ["--help"] || options == ["-h"] => {
            println!("{GUI_HELP}");
            return Ok(());
        }
        _ => fail(HELP, 2),
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
