#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Generate,
    Load,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PasteFormat {
    Base64,
    Hex,
    Escaped,
}

impl PasteFormat {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Base64 => "base64",
            Self::Hex => "hex",
            Self::Escaped => "\\xNN escaped",
        }
    }
}

pub fn parse_hex_bytes(s: &str) -> Result<Vec<u8>, String> {
    let s: String = s
        .replace("0x", "")
        .replace("\\x", "")
        .replace([',', ' ', '\n', '\r', '\t', '"', ';', '{', '}'], "");
    if s.is_empty() {
        return Err("no hex data found".into());
    }
    if s.len() % 2 != 0 {
        return Err(format!("hex data has odd length ({})", s.len()));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| format!("invalid hex '{}' at position {i}", &s[i..i + 2]))
        })
        .collect()
}

pub fn parse_escaped_bytes(s: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut chars = s.char_indices().peekable();
    while let Some((pos, c)) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some((_, 'x')) => {
                    let h1 = chars
                        .next()
                        .map(|(_, c)| c)
                        .ok_or_else(|| format!("unexpected end after \\x at byte {pos}"))?;
                    let h2 = chars
                        .next()
                        .map(|(_, c)| c)
                        .ok_or_else(|| format!("unexpected end at byte {pos}"))?;
                    let byte = u8::from_str_radix(&format!("{h1}{h2}"), 16)
                        .map_err(|_| format!("invalid \\x{h1}{h2} at byte {pos}"))?;
                    bytes.push(byte);
                }
                Some((_, other)) => {
                    return Err(format!("unknown escape \\{other} at byte {pos}"));
                }
                None => return Err(format!("trailing backslash at byte {pos}")),
            }
        }
        // skip surrounding syntax (quotes, variable names, etc.)
    }
    if bytes.is_empty() {
        return Err("no \\xNN sequences found \u{2014} check format".into());
    }
    Ok(bytes)
}
