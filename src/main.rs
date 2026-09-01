//! Shellcode Obfuscator — egui/eframe GUI
//!
//! Patches the embedded windows/x64/shell_reverse_tcp shellcode with the
//! user-supplied LHOST and LPORT and outputs the result in the chosen format.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod formats;
mod input;
mod scripting;
mod shellcode;

use app::App;
use eframe::egui;
// ── entry point ──────────────────────────────────────────────────────────────

fn main() -> eframe::Result {
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
