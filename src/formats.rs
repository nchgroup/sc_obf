use base64::Engine as _;

pub fn format_bytes(buf: &[u8], format: &OutputFormat) -> String {
    match format {
        OutputFormat::Hex => hex::encode(buf),
        OutputFormat::Base64 => base64::engine::general_purpose::STANDARD.encode(buf),
        OutputFormat::C => {
            let hex_list: Vec<String> = buf.iter().map(|b| format!("0x{b:02x}")).collect();
            format!("unsigned char buf[] = {{{}}};", hex_list.join(", "))
        }
        OutputFormat::Python => {
            let escaped: String = buf.iter().map(|b| format!("\\x{b:02x}")).collect();
            format!("buf = \"{escaped}\"")
        }
        OutputFormat::Rust => {
            let hex_list: Vec<String> = buf.iter().map(|b| format!("0x{b:02x}")).collect();
            format!("let buf: [u8; {}] = [{}];", buf.len(), hex_list.join(", "))
        }
        OutputFormat::CSharp => {
            let hex_list: Vec<String> = buf.iter().map(|b| format!("0x{b:02x}")).collect();
            format!("byte[] buf = new byte[] {{{}}};", hex_list.join(", "))
        }
        OutputFormat::PowerShell => {
            let hex_list: Vec<String> = buf.iter().map(|b| format!("0x{b:02x}")).collect();
            format!("$buf = @({});", hex_list.join(", "))
        }
        OutputFormat::VBA => {
            let hex_list: Vec<String> = buf.iter().map(|b| format!("&H{b:02x}")).collect();
            format!("Dim buf() As Byte\nbuf = Array({});", hex_list.join(", "))
        }
        OutputFormat::Go => {
            let hex_list: Vec<String> = buf.iter().map(|b| format!("0x{b:02x}")).collect();
            format!("shellcode := []byte{{{}}}", hex_list.join(", "))
        }
        OutputFormat::Custom => String::new(), // handled by App::format_output
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ByteRepr {
    Hex0x,
    HexAmpersand,
    EscapedX,
}

impl ByteRepr {
    pub const ALL: &'static [Self] = &[Self::Hex0x, Self::HexAmpersand, Self::EscapedX];

    pub fn render(&self, b: u8) -> String {
        match self {
            Self::Hex0x => format!("0x{b:02x}"),
            Self::HexAmpersand => format!("&H{b:02x}"),
            Self::EscapedX => format!("\\x{b:02x}"),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Hex0x => "0xff",
            Self::HexAmpersand => "&Hff",
            Self::EscapedX => "\\xff",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormatTemplate {
    pub name: String,
    pub prefix: String,
    pub separator: String,
    pub suffix: String,
    pub byte_repr: ByteRepr,
}

impl FormatTemplate {
    pub fn new(
        name: &str,
        prefix: &str,
        separator: &str,
        suffix: &str,
        byte_repr: ByteRepr,
    ) -> Self {
        Self {
            name: name.into(),
            prefix: prefix.into(),
            separator: separator.into(),
            suffix: suffix.into(),
            byte_repr,
        }
    }

    // {len} in prefix/suffix is replaced with the byte count
    pub fn render(&self, buf: &[u8]) -> String {
        let bytes_str = buf
            .iter()
            .map(|b| self.byte_repr.render(*b))
            .collect::<Vec<_>>()
            .join(&self.separator);
        format!("{}{}{}", self.prefix, bytes_str, self.suffix)
            .replace("{len}", &buf.len().to_string())
    }
}

pub fn builtin_templates() -> Vec<FormatTemplate> {
    vec![
        FormatTemplate::new("c", "unsigned char buf[] = {", ", ", "};", ByteRepr::Hex0x),
        FormatTemplate::new("python", "buf = \"", "", "\"", ByteRepr::EscapedX),
        FormatTemplate::new(
            "rust",
            "let buf: [u8; {len}] = [",
            ", ",
            "];",
            ByteRepr::Hex0x,
        ),
        FormatTemplate::new(
            "csharp",
            "byte[] buf = new byte[] {",
            ", ",
            "};",
            ByteRepr::Hex0x,
        ),
        FormatTemplate::new("psh", "$buf = @(", ", ", ");", ByteRepr::Hex0x),
        FormatTemplate::new(
            "vba",
            "Dim buf() As Byte\nbuf = Array(",
            ", ",
            ");",
            ByteRepr::HexAmpersand,
        ),
        FormatTemplate::new("go", "shellcode := []byte{", ", ", "}", ByteRepr::Hex0x),
    ]
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    Hex,
    Base64,
    C,
    Python,
    Rust,
    CSharp,
    PowerShell,
    VBA,
    Go,
    Custom,
}

impl OutputFormat {
    pub const ALL: &'static [Self] = &[
        Self::Hex,
        Self::Base64,
        Self::C,
        Self::Python,
        Self::Rust,
        Self::CSharp,
        Self::PowerShell,
        Self::VBA,
        Self::Go,
        Self::Custom,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Hex => "hex",
            Self::Base64 => "base64",
            Self::C => "c",
            Self::Python => "python",
            Self::Rust => "rust",
            Self::CSharp => "csharp",
            Self::PowerShell => "psh",
            Self::VBA => "vba",
            Self::Go => "go",
            Self::Custom => "custom",
        }
    }
}
