# sc_obf

Shellcode Obfuscator provides CLI, desktop GUI, and web interfaces in one Rust
binary. Generate payloads with LHOST/LPORT, load or parse bytes, convert output
formats, and apply Python transformations.

## Build

Requires stable Rust and Cargo. On macOS, install Xcode Command Line Tools.
Python 3 is optional and enables scripting.

```bash
cargo build --release
./target/release/sc_obf help
```

The examples below use `sc_obf` from your PATH. Alternatively, use
`./target/release/sc_obf` or `cargo run -- <command> <options>`.

## Usage

```text
sc_obf cli <OPTIONS>           Generate, load, and convert bytes
sc_obf gui                     Open the desktop GUI
sc_obf web [--bind IP:PORT]     Start the embedded web interface
sc_obf help                    Show complete help for all modes
```

Without arguments, the binary shows a summary. Use `cli --help` or `web --help`
for mode-specific help.

## CLI

Choose one source: `--generate`, `--input PATH`, or `--text TEXT`.

| Option | Description |
| --- | --- |
| `--generate` | Generate the embedded payload. |
| `--input PATH`, `--load PATH` | Read a file; use `-` for stdin. |
| `--text TEXT` | Parse inline bytes. |
| `--lhost IPV4` | Callback address; default `127.0.0.1`. Implies generation. |
| `--lport PORT` | Callback port; default `4444`. Implies generation. |
| `--input-format FORMAT` | `raw`, `base64`, `hex`, or `escaped`. Defaults to `raw` for files/stdin and `base64` for inline text. |
| `--format FORMAT` | Output format; default `hex`. See formats below. |
| `--template TEXT` | Inline text template with `{bytes}`; implies `--format custom`. |
| `--template-file PATH` | Load a plain-text template or legacy JSON file. |
| `--separator TEXT` | Byte separator; default `, `. Use `''` for none. |
| `--byte-repr REPR` | Byte representation: `0xff` (default), `&Hff`, or `\xff`. |
| `--script PATH`, `--python PATH` | Execute a Python transformation; its returned text replaces formatted output. |
| `--output PATH`, `-o PATH` | Save output to a file; omitted or `-` writes to stdout. |
| `--help`, `-h` | Show CLI help. |

Output formats: `hex`, `base64`, `c`, `python`, `rust`, `csharp`, `psh`, `vba`,
`go`, `custom`, and `raw` (binary bytes).

```bash
# Generate and format as Rust
sc_obf cli --generate --lhost 127.0.0.1 --lport 4444 --format rust

# Convert a binary file to base64
sc_obf cli --input payload.bin --format base64 -o payload.txt

# Parse inline hex and output C
sc_obf cli --text '0090ff' --input-format hex --format c

# Read hex from stdin and save raw bytes
printf '0090ff' | sc_obf cli --input - --input-format hex --format raw -o payload.bin

# Apply a Python script
sc_obf cli --input payload.bin --script transform.py -o result.txt
```

## GUI and Web

```bash
sc_obf gui
sc_obf web
sc_obf web --bind 0.0.0.0:8080
sc_obf web --bind '[::1]:3000'
```

The web interface defaults to **http://127.0.0.1:3000**. `--bind IP:PORT`
customizes the listener; `0.0.0.0` listens on all IPv4 interfaces.

Both interfaces offer Dark/Light themes, generation, file/text input, output
formats, Python scripting, and templates. Choose **Generate** or **Load shellcode**,
select a format, then generate or parse and convert your input.

The web interface adds drag and drop, multiple file selection, and downloads.
Files are processed on the server in memory: 5 MiB per file, 20 MiB total in the
browser list. Reloading clears the list. The server and frontend are embedded
in the binary; rebuild after editing `web/`.

## Python Scripting

Python 3 must be installed and available to the application. Define
`process(shellcode: bytes) -> str` in your script:

```python
import sys

# Optional: import packages from a virtual environment.
sys.path.insert(0, "/path/to/venv/lib/python3.x/site-packages")


def process(shellcode: bytes) -> str:
    from Crypto.Cipher import AES  # Requires a separately installed package.
    # Return transformed output text.
    return shellcode.hex()
```

Input is raw `bytes`. The returned string becomes stdout or `--output` in CLI
mode and replaces the Output panel in GUI/web. In CLI mode, `print()` goes to
stderr. Scripts replace the selected formatter and cannot be combined with raw
output or custom templates in the CLI.

If Python is unavailable, CLI scripting reports an error; GUI/web scripting
controls stay disabled and show installation guidance. GUI and web share saved
scripts in `~/.config/sc_obf/scripts`.

## Custom Templates

JSON is optional. Write the desired output with `{bytes}` where the bytes should
appear and `{len}` for the byte count:

```bash
sc_obf cli --input payload.bin --template 'byte[] buf = { {bytes} };'
sc_obf cli --input payload.bin --template 'buf = "{bytes}"' --byte-repr '\xff' --separator ''
```

For mixed single/double quotes or multiline output, save the template as plain
text in `format.tpl`. Quotes and backslashes in the file do not need escaping:

```text
# It's a byte buffer
buf = "{bytes}"  # {len} bytes
```

```bash
sc_obf cli --input payload.bin --template-file format.tpl --byte-repr '\xff' --separator ''
```

Templates require exactly one `{bytes}` placeholder. All other content is kept
literally, except `{len}`, which expands to the byte count. Defaults: `0xff`
bytes separated by `, `. `--byte-repr` also accepts `&Hff` or `\xff`.

Existing JSON templates still work with `--template template.json` or
`--template-file template.json`. GUI/web retain their prefix, separator, suffix,
and byte representation fields under **Output Templates**.
