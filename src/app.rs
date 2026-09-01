use arboard::Clipboard;
use base64::Engine as _;
use eframe::egui;
use egui_code_editor::{CodeEditor, ColorTheme, Syntax};

use crate::formats::{ByteRepr, FormatTemplate, OutputFormat, builtin_templates, format_bytes};
use crate::input::{InputMode, PasteFormat, parse_escaped_bytes, parse_hex_bytes};
use crate::scripting::{python_is_available, run_python_script};
use crate::shellcode::{SHELLCODE, patch_shellcode};

#[derive(Debug, Clone, PartialEq)]
pub enum UiTheme {
    Dark,
    Light,
}

impl UiTheme {
    fn editor_theme(&self) -> ColorTheme {
        match self {
            Self::Dark => ColorTheme::GRUVBOX_DARK,
            Self::Light => ColorTheme::GRUVBOX_LIGHT,
        }
    }
}

pub struct App {
    pub lhost: String,
    pub lport: String,
    pub format: OutputFormat,
    pub output: String,
    pub status: String,
    pub clipboard: Option<Clipboard>,
    pub python_available: bool,
    pub script_code: String,
    pub script_status: String,
    pub last_shellcode: Vec<u8>,
    pub input_mode: InputMode,
    pub paste_text: String,
    pub paste_format: PasteFormat,
    pub custom_template: FormatTemplate,
    pub ui_theme: UiTheme,
    pub python_syntax: Syntax,
}

impl Default for App {
    fn default() -> Self {
        Self {
            lhost: "127.0.0.1".into(),
            lport: "4444".into(),
            format: OutputFormat::Hex,
            output: String::new(),
            status: String::new(),
            clipboard: Clipboard::new().ok(),
            python_available: python_is_available(),
            script_code: concat!(
                "def process(shellcode: bytes) -> str:\n",
                "    # XOR-encode with 0xAA and return as hex\n",
                "    key = 0xAA\n",
                "    return bytes(b ^ key for b in shellcode).hex()\n"
            )
            .into(),
            script_status: String::new(),
            last_shellcode: Vec::new(),
            input_mode: InputMode::Generate,
            paste_text: String::new(),
            paste_format: PasteFormat::Base64,
            custom_template: FormatTemplate::new(
                "custom",
                "byte[] buf = {",
                ", ",
                "};",
                ByteRepr::Hex0x,
            ),
            ui_theme: UiTheme::Dark,
            python_syntax: Syntax::python(),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        match self.ui_theme {
            UiTheme::Dark => ui.ctx().set_visuals(egui::Visuals::dark()),
            UiTheme::Light => ui.ctx().set_visuals(egui::Visuals::light()),
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.heading("Shellcode Obfuscator");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Theme:");
                ui.selectable_value(&mut self.ui_theme, UiTheme::Dark, "Dark");
                ui.selectable_value(&mut self.ui_theme, UiTheme::Light, "Light");
            });
            ui.add_space(6.0);

            // ── input mode toggle ─────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.label("Input:");
                ui.selectable_value(&mut self.input_mode, InputMode::Generate, "Generate");
                ui.selectable_value(&mut self.input_mode, InputMode::Load, "Load shellcode");
            });
            ui.add_space(4.0);

            // ── inputs ───────────────────────────────────────────────────────
            if self.input_mode == InputMode::Generate {
                egui::Grid::new("inputs_gen")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("LHOST:");
                        ui.text_edit_singleline(&mut self.lhost);
                        ui.end_row();

                        ui.label("LPORT:");
                        ui.text_edit_singleline(&mut self.lport);
                        ui.end_row();

                        ui.label("Output format:");
                        egui::ComboBox::from_id_salt("format_combo")
                            .selected_text(self.format.label())
                            .show_ui(ui, |ui| {
                                for fmt in OutputFormat::ALL {
                                    ui.selectable_value(&mut self.format, fmt.clone(), fmt.label());
                                }
                            });
                        ui.end_row();
                    });
            } else {
                ui.horizontal(|ui| {
                    if ui.button("Open file\u{2026}").clicked() {
                        self.on_load_file();
                    }
                    ui.label("or paste shellcode below:");
                });
                ui.add_space(2.0);
                egui::Grid::new("inputs_load")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Input format:");
                        egui::ComboBox::from_id_salt("paste_fmt_combo")
                            .selected_text(self.paste_format.label())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.paste_format,
                                    PasteFormat::Base64,
                                    PasteFormat::Base64.label(),
                                );
                                ui.selectable_value(
                                    &mut self.paste_format,
                                    PasteFormat::Hex,
                                    PasteFormat::Hex.label(),
                                );
                                ui.selectable_value(
                                    &mut self.paste_format,
                                    PasteFormat::Escaped,
                                    PasteFormat::Escaped.label(),
                                );
                            });
                        ui.end_row();

                        ui.label("Output format:");
                        egui::ComboBox::from_id_salt("format_combo")
                            .selected_text(self.format.label())
                            .show_ui(ui, |ui| {
                                for fmt in OutputFormat::ALL {
                                    ui.selectable_value(&mut self.format, fmt.clone(), fmt.label());
                                }
                            });
                        ui.end_row();
                    });
                egui::ScrollArea::vertical()
                    .id_salt("paste_scroll")
                    .max_height(80.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.paste_text)
                                .desired_width(f32::INFINITY)
                                .desired_rows(4)
                                .font(egui::TextStyle::Monospace)
                                .hint_text("paste shellcode here\u{2026}"),
                        );
                    });
            }

            ui.add_space(8.0);

            // ── action buttons ───────────────────────────────────────────────
            ui.horizontal(|ui| {
                match self.input_mode {
                    InputMode::Generate => {
                        if ui.button("Generate").clicked() {
                            self.on_generate();
                        }
                    }
                    InputMode::Load => {
                        if ui.button("Parse & Convert").clicked() {
                            self.on_parse_and_convert();
                        }
                    }
                }
                if ui.button("Copy to clipboard").clicked() {
                    self.on_copy();
                }
                if ui.button("Clear").clicked() {
                    self.output.clear();
                    self.status.clear();
                }
            });

            ui.add_space(4.0);

            // ── status label ─────────────────────────────────────────────────
            if !self.status.is_empty() {
                let color = if self.status.starts_with("Error") {
                    egui::Color32::from_rgb(220, 80, 80)
                } else {
                    egui::Color32::from_rgb(100, 200, 100)
                };
                ui.colored_label(color, &self.status);
            }

            ui.separator();

            // ── output text area ─────────────────────────────────────────────
            ui.label("Output:");
            egui::ScrollArea::vertical()
                .id_salt("output_scroll")
                .max_height(200.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.output)
                            .desired_width(f32::INFINITY)
                            .desired_rows(10)
                            .font(egui::TextStyle::Monospace),
                    );
                });

            // ── python scripting ─────────────────────────────────────────────
            ui.separator();
            ui.collapsing("Python Scripting", |ui| {
                if self.python_available {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Define a");
                        ui.code("process(shellcode: bytes) -> str");
                        ui.label(
                            "function. Output replaces the payload. Generate a payload first.",
                        );
                    });
                    let mut code_editor = CodeEditor::default()
                        .id_source("python_script_editor")
                        .with_rows(18)
                        .with_fontsize(14.0)
                        .with_theme(self.ui_theme.editor_theme())
                        .with_numlines(true)
                        .vscroll(true);
                    code_editor.show(ui, &mut self.script_code, &self.python_syntax);
                    ui.horizontal(|ui| {
                        let can_run = !self.last_shellcode.is_empty();
                        if ui
                            .add_enabled(can_run, egui::Button::new("Run Script"))
                            .clicked()
                        {
                            self.on_run_script();
                        }
                        if !self.script_status.is_empty() {
                            let color = if self.script_status.starts_with("Error") {
                                egui::Color32::from_rgb(220, 80, 80)
                            } else {
                                egui::Color32::from_rgb(100, 200, 100)
                            };
                            ui.colored_label(color, &self.script_status);
                        }
                    });
                } else {
                    ui.add_enabled_ui(false, |ui| {
                        let mut placeholder = String::new();
                        ui.add(
                            egui::TextEdit::multiline(&mut placeholder)
                                .desired_rows(2)
                                .desired_width(f32::INFINITY)
                                .hint_text("Scripting unavailable"),
                        );
                    });
                    ui.colored_label(
                        egui::Color32::from_rgb(200, 160, 60),
                        "Python 3 is not installed or could not be initialized. \
                         Install Python 3 (https://python.org) to enable scripting.",
                    );
                }
            });

            // ── output templates ─────────────────────────────────────────────
            ui.separator();
            ui.collapsing("Output Templates", |ui| {
                ui.label("Built-in templates (read-only reference):");
                egui::ScrollArea::horizontal()
                    .id_salt("tmpl_hscroll")
                    .show(ui, |ui| {
                        egui::Grid::new("builtin_tmpl")
                            .num_columns(5)
                            .spacing([12.0, 3.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("name");
                                ui.strong("prefix");
                                ui.strong("sep");
                                ui.strong("suffix");
                                ui.strong("byte");
                                ui.end_row();
                                for t in &builtin_templates() {
                                    ui.monospace(&t.name);
                                    ui.monospace(&t.prefix);
                                    ui.monospace(if t.separator.is_empty() {
                                        "(none)"
                                    } else {
                                        &t.separator
                                    });
                                    ui.monospace(&t.suffix);
                                    ui.monospace(t.byte_repr.label());
                                    ui.end_row();
                                }
                            });
                    });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                ui.horizontal_wrapped(|ui| {
                    ui.label("Custom template \u{2014} select");
                    ui.code("custom");
                    ui.label("in Output format. Use");
                    ui.code("{len}");
                    ui.label("in prefix/suffix for byte count.");
                });
                egui::Grid::new("custom_tmpl_editor")
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.custom_template.name);
                        ui.end_row();

                        ui.label("Prefix:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.custom_template.prefix)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY),
                        );
                        ui.end_row();

                        ui.label("Separator:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.custom_template.separator)
                                .font(egui::TextStyle::Monospace),
                        );
                        ui.end_row();

                        ui.label("Suffix:");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.custom_template.suffix)
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY),
                        );
                        ui.end_row();

                        ui.label("Byte repr:");
                        egui::ComboBox::from_id_salt("byte_repr_combo")
                            .selected_text(self.custom_template.byte_repr.label())
                            .show_ui(ui, |ui| {
                                for br in ByteRepr::ALL {
                                    ui.selectable_value(
                                        &mut self.custom_template.byte_repr,
                                        br.clone(),
                                        br.label(),
                                    );
                                }
                            });
                        ui.end_row();
                    });

                if !self.last_shellcode.is_empty() {
                    let n = self.last_shellcode.len().min(4);
                    let preview = self.custom_template.render(&self.last_shellcode[..n]);
                    let ellipsis = if self.last_shellcode.len() > 4 {
                        "\u{2026}"
                    } else {
                        ""
                    };
                    ui.horizontal(|ui| {
                        ui.label("Preview:");
                        ui.monospace(format!("{preview}{ellipsis}"));
                    });
                }
            });
        });
    }
}

impl App {
    pub fn format_output(&self, buf: &[u8]) -> String {
        if self.format == OutputFormat::Custom {
            self.custom_template.render(buf)
        } else {
            format_bytes(buf, &self.format)
        }
    }

    pub fn on_generate(&mut self) {
        let lhost = self.lhost.trim().to_string();
        if lhost.is_empty() {
            self.status = "Error: LHOST cannot be empty".into();
            return;
        }
        let port: u16 = match self.lport.trim().parse() {
            Ok(p) => p,
            Err(_) => {
                self.status = "Error: invalid port (must be 0-65535)".into();
                return;
            }
        };
        match patch_shellcode(&lhost, port) {
            Ok(buf) => {
                self.output = self.format_output(&buf);
                self.last_shellcode = buf;
                self.status = format!(
                    "Payload generated \u{2014} {} bytes, format: {}",
                    SHELLCODE.len(),
                    self.format.label()
                );
            }
            Err(e) => {
                self.output.clear();
                self.last_shellcode.clear();
                self.status = format!("Error: {e}");
            }
        }
    }

    pub fn on_copy(&mut self) {
        if self.output.is_empty() {
            self.status = "Nothing to copy".into();
            return;
        }
        match &mut self.clipboard {
            Some(cb) => match cb.set_text(self.output.clone()) {
                Ok(_) => self.status = "Copied to clipboard".into(),
                Err(e) => self.status = format!("Error copying: {e}"),
            },
            None => self.status = "Error: clipboard not available".into(),
        }
    }

    pub fn on_run_script(&mut self) {
        if self.last_shellcode.is_empty() {
            self.script_status = "Error: generate a payload first".into();
            return;
        }
        match run_python_script(&self.script_code, &self.last_shellcode) {
            Ok(result) => {
                self.output = result;
                self.script_status = "Script executed successfully".into();
            }
            Err(e) => {
                self.script_status = format!("Error: {e}");
            }
        }
    }

    pub fn on_load_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Binary", &["bin", "raw", "sc"])
            .add_filter("All files", &["*"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read(&path) {
            Ok(bytes) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                let count = bytes.len();
                self.last_shellcode = bytes;
                self.output = self.format_output(&self.last_shellcode);
                self.status = format!(
                    "Loaded {count} bytes from '{name}', output format: {}",
                    self.format.label()
                );
            }
            Err(e) => {
                self.status = format!("Error reading file: {e}");
            }
        }
    }

    pub fn on_parse_and_convert(&mut self) {
        let text = self.paste_text.trim().to_string();
        if text.is_empty() {
            self.status = "Error: paste area is empty".into();
            return;
        }
        let result = match self.paste_format {
            PasteFormat::Base64 => base64::engine::general_purpose::STANDARD
                .decode(&text)
                .map_err(|e| format!("base64 decode error: {e}")),
            PasteFormat::Hex => parse_hex_bytes(&text),
            PasteFormat::Escaped => parse_escaped_bytes(&text),
        };
        match result {
            Ok(bytes) => {
                let count = bytes.len();
                self.last_shellcode = bytes;
                self.output = self.format_output(&self.last_shellcode);
                self.status = format!(
                    "Parsed {count} bytes, output format: {}",
                    self.format.label()
                );
            }
            Err(e) => {
                self.status = format!("Error: {e}");
            }
        }
    }
}
