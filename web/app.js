'use strict';
const $ = id => document.getElementById(id);
let mode = 'generate';
let connected = false;
let pythonAvailable = false;
let lastBytes = '';
let lastCount = 0;
let nextFileId = 0;
let selectedFile = null;
let working = false;
let queue = Promise.resolve();
const loadedFiles = [];
const MAX_FILE = 5 * 1024 * 1024;
const MAX_TOTAL = 20 * 1024 * 1024;
const apiButtons = ['convert', 'open-file', 'dropzone', 'load-script', 'delete-script', 'save-script', 'run-script', 'remove-file'];

function status(message, script = false, error = false) {
  const target = $(script ? 'script-status' : 'status');
  target.textContent = message;
  target.classList.toggle('error', error);
}
function updateControls() {
  for (const id of apiButtons) $(id).disabled = !connected || working;
  $('run-script').disabled = !connected || working || !pythonAvailable || !lastCount;
  $('load-script').disabled = $('delete-script').disabled = !connected || working || !$('saved-scripts').value;
  $('remove-file').disabled = !connected || working || selectedFile === null;
  $('busy').hidden = !working;
  $('files').disabled = working;
}
async function api(request) {
  const response = await fetch('/api', {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(request),
  });
  let result;
  try { result = await response.json(); }
  catch { throw new Error('Invalid server response. Start cargo run -- --web.'); }
  if (!response.ok || !result.ok) throw new Error(result.error || `HTTP ${response.status}`);
  return result.data;
}
function operation(task, script = false) {
  queue = queue.then(async () => {
    working = true;
    updateControls();
    try { await task(); }
    catch (error) { status(`Error: ${error.message}`, script, true); }
    finally { working = false; updateControls(); }
  });
  return queue;
}
function template() {
  return { name: $('template-name').value, prefix: $('prefix').value, separator: $('separator').value,
    suffix: $('suffix').value, byte_repr: $('byte-repr').value };
}
function outputOptions() { return { format: $('format').value, template: template() }; }
function setOutput(data, message) {
  lastBytes = data.bytes;
  lastCount = data.count;
  $('output').value = data.output;
  $('byte-count').textContent = `${data.count} bytes`;
  status(message);
  updatePreview();
  updateControls();
}
function updatePreview() {
  $('preview-row').hidden = !lastCount;
  if (!lastCount) return;
  const bytes = Array.from(atob(lastBytes.slice(0, 8)), c => c.charCodeAt(0)).slice(0, 4);
  const t = template();
  const prefix = t.byte_repr === '0xff' ? '0x' : t.byte_repr === '&Hff' ? '&H' : '\\x';
  const output = t.prefix + bytes.map(b => prefix + b.toString(16).padStart(2, '0')).join(t.separator) + t.suffix;
  $('template-preview').textContent = output.replaceAll('{len}', String(bytes.length)) + (lastCount > 4 ? '…' : '');
}
function setMode(value) {
  mode = value;
  $('mode-generate').setAttribute('aria-pressed', String(mode === 'generate'));
  $('mode-load').setAttribute('aria-pressed', String(mode === 'load'));
  $('generate-fields').hidden = mode !== 'generate';
  $('load-fields').hidden = $('paste-text').hidden = mode !== 'load';
  $('convert').textContent = mode === 'generate' ? 'Generate' : 'Parse & Convert';
}
function theme(value) {
  document.documentElement.dataset.theme = value;
  $('dark').setAttribute('aria-pressed', String(value === 'dark'));
  $('light').setAttribute('aria-pressed', String(value === 'light'));
  try { localStorage.setItem('sc_obf_theme', value); } catch { /* Theme still works without storage. */ }
}
function renderFiles() {
  $('files').replaceChildren(...loadedFiles.map(file => {
    const option = document.createElement('option');
    option.value = String(file.id);
    option.textContent = `${file.name} (${file.size.toLocaleString()} bytes)`;
    return option;
  }));
  if (selectedFile !== null) $('files').value = String(selectedFile);
  $('file-toolbar').hidden = !loadedFiles.length;
  updateControls();
}
function encodeBytes(bytes) {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 32768) binary += String.fromCharCode(...bytes.subarray(i, i + 32768));
  return btoa(binary);
}
async function convertFile(file) {
  const data = await api({ action: 'format', bytes: file.bytes, ...outputOptions() });
  setOutput(data, `Loaded ${data.count} bytes from '${file.name}', output format: ${$('format').value}`);
}
function upload(files) {
  if (!connected) { status('Error: start cargo run -- --web to load files.', false, true); return; }
  operation(async () => {
    setMode('load');
    const errors = [];
    for (const file of files) {
      if (file.size > MAX_FILE) { errors.push(`${file.name}: exceeds 5 MiB`); continue; }
      if (loadedFiles.reduce((sum, f) => sum + f.size, 0) + file.size > MAX_TOTAL) {
        errors.push(`${file.name}: exceeds the 20 MiB total limit`); continue;
      }
      try {
        const entry = { id: nextFileId++, name: file.name, size: file.size,
          bytes: encodeBytes(new Uint8Array(await file.arrayBuffer())) };
        loadedFiles.push(entry);
        selectedFile = entry.id;
      } catch { errors.push(`${file.name}: could not read file`); }
    }
    renderFiles();
    const selected = loadedFiles.find(f => f.id === selectedFile);
    if (selected) { $('paste-text').value = ''; await convertFile(selected); }
    if (errors.length) status(`Error: ${errors.join('; ')}`, false, true);
  });
}
function renderScripts(names, selected = $('saved-scripts').value) {
  const none = document.createElement('option');
  none.value = ''; none.textContent = '(none)';
  $('saved-scripts').replaceChildren(none, ...names.map(name => {
    const option = document.createElement('option'); option.value = option.textContent = name; return option;
  }));
  $('saved-scripts').value = names.includes(selected) ? selected : '';
  updateControls();
}
function lineNumbers() {
  $('line-numbers').textContent = Array.from({ length: $('script').value.split('\n').length }, (_, i) => i + 1).join('\n');
  const source = $('script').value;
  const tokens = /#[^\n]*|"(?:\\.|[^"\\\n])*"|'(?:\\.|[^'\\\n])*'|\b(?:def|return|import|from|as|if|else|elif|for|in|while|try|except|with|class|pass|raise|True|False|None|and|or|not|lambda)\b|\b(?:0x[\da-fA-F]+|\d+)\b/g;
  const fragment = document.createDocumentFragment();
  let start = 0;
  for (const match of source.matchAll(tokens)) {
    fragment.append(document.createTextNode(source.slice(start, match.index)));
    const token = document.createElement('span');
    token.className = match[0].startsWith('#') ? 'syntax-comment' : /^["']/.test(match[0]) ? 'syntax-string' : /^\d/.test(match[0]) ? 'syntax-number' : 'syntax-keyword';
    token.textContent = match[0];
    fragment.append(token);
    start = match.index + match[0].length;
  }
  fragment.append(document.createTextNode(source.slice(start) + '\n'));
  $('highlighted-code').replaceChildren(fragment);
  syncEditorScroll();
}
function syncEditorScroll() {
  $('line-numbers').scrollTop = $('script').scrollTop;
  $('highlighted-code').style.transform = `translate(${-$('script').scrollLeft}px, ${-$('script').scrollTop}px)`;
}
$('dark').onclick = () => theme('dark');
$('light').onclick = () => theme('light');
try { theme(localStorage.getItem('sc_obf_theme') === 'light' ? 'light' : 'dark'); } catch { theme('dark'); }
$('mode-generate').onclick = () => setMode('generate');
$('mode-load').onclick = () => setMode('load');
$('open-file').onclick = $('dropzone').onclick = () => $('file-input').click();
$('file-input').onchange = event => { upload(Array.from(event.target.files)); event.target.value = ''; };
let dragDepth = 0;
window.addEventListener('dragover', event => event.preventDefault());
window.addEventListener('drop', event => event.preventDefault());
$('dropzone').addEventListener('dragenter', event => { event.preventDefault(); dragDepth++; $('dropzone').classList.add('dragover'); });
$('dropzone').addEventListener('dragleave', () => { if (--dragDepth <= 0) { dragDepth = 0; $('dropzone').classList.remove('dragover'); } });
$('dropzone').addEventListener('drop', event => {
  event.preventDefault(); dragDepth = 0; $('dropzone').classList.remove('dragover'); upload(Array.from(event.dataTransfer.files));
});
$('files').onchange = () => {
  selectedFile = Number($('files').value);
  const file = loadedFiles.find(f => f.id === selectedFile);
  operation(async () => { $('paste-text').value = ''; await convertFile(file); });
};
$('remove-file').onclick = () => operation(async () => {
  const index = loadedFiles.findIndex(f => f.id === selectedFile);
  if (index >= 0) loadedFiles.splice(index, 1);
  selectedFile = loadedFiles[0]?.id ?? null;
  renderFiles();
  if (loadedFiles.length) await convertFile(loadedFiles[0]);
  else { lastBytes = ''; lastCount = 0; $('output').value = ''; $('byte-count').textContent = '0 bytes'; status('File removed'); updatePreview(); }
});
$('convert').onclick = () => operation(async () => {
  if (mode === 'generate') {
    const data = await api({ action: 'generate', lhost: $('lhost').value, lport: $('lport').value, ...outputOptions() });
    setOutput(data, `Payload generated — ${data.count} bytes, format: ${$('format').value}`);
  } else if ($('paste-text').value.trim()) {
    const data = await api({ action: 'parse', text: $('paste-text').value, input_format: $('input-format').value, ...outputOptions() });
    setOutput(data, `Parsed ${data.count} bytes, output format: ${$('format').value}`);
  } else {
    const file = loadedFiles.find(f => f.id === selectedFile);
    if (!file) throw new Error('paste area is empty — paste data or open a file');
    await convertFile(file);
  }
});
$('copy').onclick = async () => {
  if (!$('output').value) { status('Nothing to copy'); return; }
  try { await navigator.clipboard.writeText($('output').value); status('Copied to clipboard'); }
  catch { $('output').focus(); $('output').select(); status('Use Ctrl+C or ⌘C to copy the selected output'); }
};
$('clear').onclick = () => { $('output').value = ''; status(''); };
$('download').onclick = () => {
  if (!$('output').value) { status('Nothing to download'); return; }
  const url = URL.createObjectURL(new Blob([$('output').value], { type: 'text/plain;charset=utf-8' }));
  const anchor = document.createElement('a'); anchor.href = url;
  anchor.download = `sc_obf.${$('format').value}.txt`;
  document.body.append(anchor); anchor.click(); anchor.remove(); setTimeout(() => URL.revokeObjectURL(url), 1000);
};
$('saved-scripts').onchange = updateControls;
$('save-script').onclick = () => operation(async () => {
  const name = $('save-name').value.trim();
  const data = await api({ action: 'save_script', name, code: $('script').value });
  renderScripts(data.scripts, name); status('Script saved', true);
}, true);
$('load-script').onclick = () => operation(async () => {
  const name = $('saved-scripts').value;
  const data = await api({ action: 'load_script', name });
  $('script').value = data.code; $('save-name').value = name; lineNumbers(); status(`Loaded '${name}'`, true);
}, true);
$('delete-script').onclick = () => operation(async () => {
  const name = $('saved-scripts').value;
  const data = await api({ action: 'delete_script', name });
  renderScripts(data.scripts); status(`Deleted '${name}'`, true);
}, true);
$('run-script').onclick = () => operation(async () => {
  const data = await api({ action: 'script', script: $('script').value, bytes: lastBytes });
  $('output').value = data.output; status('Script executed successfully', true);
}, true);
$('script').addEventListener('input', lineNumbers);
$('script').addEventListener('scroll', syncEditorScroll);
$('script').addEventListener('keydown', event => {
  if (event.key !== 'Tab') return;
  event.preventDefault();
  const editor = $('script'); editor.setRangeText('    ', editor.selectionStart, editor.selectionEnd, 'end'); lineNumbers();
});
for (const id of ['template-name', 'prefix', 'separator', 'suffix', 'byte-repr']) $(id).addEventListener('input', updatePreview);
lineNumbers(); updateControls();
operation(async () => {
  try {
    const [info, saved] = await Promise.all([api({ action: 'info' }), api({ action: 'scripts' })]);
    connected = true; pythonAvailable = info.python_available;
    $('connection').textContent = '● Rust core connected'; $('connection').classList.add('ready');
    $('python-controls').hidden = !pythonAvailable;
    $('python-unavailable').hidden = pythonAvailable;
    $('python-unavailable').textContent = 'Python 3 is not installed or could not be initialized. Install Python 3 to enable scripting.';
    renderScripts(saved.scripts);
    $('templates').replaceChildren(...info.templates.map(t => {
      const row = document.createElement('tr');
      for (const value of [t.name, t.prefix, t.separator || '(none)', t.suffix, t.byte_repr]) {
        const cell = document.createElement('td'); cell.textContent = value; row.append(cell);
      }
      return row;
    }));
  } catch {
    $('connection').textContent = '● Disconnected';
    $('python-unavailable').textContent = 'Start the Rust web server to enable Python scripting.';
    throw new Error('Start cargo run -- --web and open the configured address (default: http://127.0.0.1:3000).');
  }
});
