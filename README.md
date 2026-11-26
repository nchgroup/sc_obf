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
sc_obf web [--bind IP:PORT]    Start the embedded web interface
sc_obf help                    Show complete help for all modes
```

Without arguments, the binary shows a summary. Use `cli --help` or `web --help`
for mode-specific help.

## CLI

Choose one source: generate a payload, read a file/stdin, or parse inline text.
Output goes to stdout unless `--output` / `-o` specifies a file.

```text
--generate                     Generate the embedded payload.
--input PATH, --load PATH      Read raw bytes from a file; - reads stdin.
--text TEXT                    Parse inline text; default input format: base64.
--lhost IPV4                   Default: 127.0.0.1; implies generation.
--lport PORT                   Default: 4444; implies generation.
--input-format FORMAT          raw, base64, hex, or escaped.
--format FORMAT                Default: hex; supported formats below.
--template TEXT                Inline template or JSON file path; implies custom.
--template-file PATH           Plain-text file or .json template; implies custom.
--separator TEXT               Custom byte separator; default: ', '. Empty allowed.
--byte-repr REPR               Custom byte representation; default: 0xff.
--script PATH, --python PATH   Execute process(bytes) -> str from a Python file.
--output PATH, -o PATH         Save output; omitted or - writes to stdout.
--help, -h                     Show CLI help.
```

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

Choose inline text, a plain-text file, or a JSON file. Each selects the `custom`
output format automatically.

### Inline text

Use exactly one `{bytes}` placeholder and optionally `{len}` for the byte count:

```bash
sc_obf cli --input payload.bin --template 'byte[] buf = { {bytes} };'
sc_obf cli --input payload.bin --template 'buf = "{bytes}"' --byte-repr '\xff' --separator ''
```

### Plain-text file

For multiline output or mixed quotes, save `format.tpl`. Quotes and backslashes
inside this file are literal and do not need escaping:

```text
# It's a byte buffer
buf = "{bytes}"  # {len} bytes
```

```bash
sc_obf cli --input payload.bin --template-file format.tpl --byte-repr '\xff' --separator ''
```

### JSON file

JSON stores the template fields explicitly. Save this as `template.json`:

```json
{
  "name": "custom",
  "prefix": "byte[] buf = {",
  "separator": ", ",
  "suffix": "};",
  "byte_repr": "0xff"
}
```

```bash
sc_obf cli --input payload.bin --template-file template.json
# The original syntax also works:
sc_obf cli --input payload.bin --template template.json
```

| JSON field | Meaning |
| --- | --- |
| `name` | Optional template name; defaults to `custom`. |
| `prefix` | Text before the bytes; supports `{len}`. |
| `separator` | Text between bytes; use `""` for none. |
| `suffix` | Text after the bytes; supports `{len}`. |
| `byte_repr` | `0xff`, `&Hff`, or `\xff`. |

JSON uses prefix/suffix fields instead of a `{bytes}` placeholder. All fields
except `name` are required. Escape double quotes as `\"` and backslashes as
`\\` inside JSON strings. For example, this produces a quoted Python buffer:

```json
{
  "prefix": "buf = \"",
  "separator": "",
  "suffix": "\"",
  "byte_repr": "\\xff"
}
```

### Byte representation and separators

`--byte-repr` controls each byte; `--separator` controls the text between them.
For input bytes `90 ff`:

| Representation | With the default `, ` separator |
| --- | --- |
| `0xff` | `0x90, 0xff` |
| `&Hff` | `&H90, &Hff` |
| `\xff` | `\x90, \xff` |

Command-line `--separator` and `--byte-repr` override the corresponding JSON
fields. Use `--separator ''` to concatenate bytes without a separator.
GUI/web provide the same prefix, separator, suffix, and byte representation
fields under **Output Templates**.
