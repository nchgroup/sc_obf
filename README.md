# sc_obf

Shellcode Obfuscator is a desktop GUI tool built with Rust and egui.

It can:
- Generate a patched shellcode payload using LHOST and LPORT.
- Load payload bytes directly from a raw file.
- Parse pasted payloads in base64, hex, or escaped `\\xNN` format.
- Convert payloads to multiple output representations (hex, base64, C, Python, Rust, C#, PowerShell, VBA, Go, and custom templates).
- Apply custom Python scripting (when Python 3 is available) to transform payload data.

## Project Name

- Cargo package name: `sc_obf`
- App display name: `Shellcode Obfuscator`

## Requirements

- Rust toolchain (stable)
- Cargo
- On macOS: Xcode Command Line Tools (for native compilation)
- Optional: Python 3 (to enable the Python scripting panel)

## Build

```bash
cargo build
```

## Run

```bash
cargo run
```

## How To Use

1. Open the application.
2. Choose an input mode:
   - Generate: provide LHOST and LPORT.
   - Load shellcode: open a raw file or paste payload text.
3. Select output format.
4. Click:
   - Generate (for LHOST/LPORT mode)
   - Parse & Convert (for pasted payload mode)
5. Copy output from the Output panel.

## Input Modes

### Generate

- Enter LHOST (IPv4) and LPORT.
- The app patches the embedded base payload and renders the selected output format.

### Load shellcode

- Open file: reads bytes as raw binary.
- Paste mode supports:
  - base64
  - hex (supports compact and prefixed styles like `0x90,0x90`)
  - escaped bytes like `\x90\x90\xCC`

## Python Scripting

If Python 3 is installed and available, the Python Scripting section is enabled.

Your script must define:

```python
def process(shellcode: bytes) -> str:
    # return transformed output text
    return shellcode.hex()
```

- Input is provided as raw `bytes`.
- Returned string replaces the Output panel content.

If Python is unavailable, scripting controls stay disabled and the app shows guidance to install Python.

## Output Templates

The app includes built-in templates and a custom template editor.

Custom template fields:
- Name
- Prefix
- Separator
- Suffix
- Byte representation (`0xff`, `&Hff`, or `\\xff`)

Use `{len}` in prefix/suffix to inject payload length.

## Notes

- Loaded files are read as raw bytes (`std::fs::read`).
- This project focuses on payload formatting and transformation workflows in a local GUI environment.
