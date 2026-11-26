# sc_obf

sc_obf (Shellcode Obfuscator) provides a Rust/egui desktop GUI and a local web
interface served by a Rust binary.

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

## Web Interface

Run the same application in your browser:

```bash
cargo run -- --web
```

Open **http://127.0.0.1:3000**. To customize the host and port:

```bash
cargo run -- --web 127.0.0.1:8080
cargo run -- --web 0.0.0.0:3000
```

Use `0.0.0.0` to listen on all network interfaces. Without `--web`, the
application opens the desktop GUI.

To build and run the binary directly:

```bash
cargo build --release
./target/release/sc_obf --web
```

The web interface includes the desktop features plus drag and drop, multiple
file selection, and output downloads. The server and frontend are embedded in
the Rust binary; rebuild after changing the frontend. Python is only used for
optional scripting.

Files are processed on the machine running the server and kept in memory
(5 MiB per file, 20 MiB total). Reloading clears the file list. Both interfaces
share saved scripts in `~/.config/sc_obf/scripts`.

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
import sys
sys.path.insert(0, "/path/to/venv/lib/python3.x/site-packages")

def process(shellcode: bytes) -> str:
    from Crypto.Cipher import AES
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
