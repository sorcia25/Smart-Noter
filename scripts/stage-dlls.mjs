// Stage the native runtime DLLs into src-tauri/bundle-dlls/ so Tauri's
// bundle.resources ships them next to the exe in the NSIS installer. Mirrors the
// DLL-copy logic in .github/workflows/ci.yml. Meant to run as Tauri's
// beforeBundleCommand (after cargo build, before bundling) — target/release then
// holds the artifacts. CWD-independent (resolves paths from its own location).
import { existsSync, mkdirSync, readdirSync, rmSync, copyFileSync, statSync } from 'node:fs';
import { join, dirname, basename } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const tauri = join(root, 'src-tauri');
const dest = join(tauri, 'bundle-dlls');

/** Recursively collect *.dll paths under `dir` matching `filter`. */
function findDlls(dir, filter = () => true, out = []) {
  if (!existsSync(dir)) return out;
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    let s;
    try { s = statSync(p); } catch { continue; }
    if (s.isDirectory()) findDlls(p, filter, out);
    else if (name.toLowerCase().endsWith('.dll') && filter(p)) out.push(p);
  }
  return out;
}

const sources = [];
// release is what `tauri build` produces; debug is a fallback for local dev testing.
for (const profile of ['release', 'debug']) {
  const profileDir = join(tauri, 'target', profile);
  // (a) DLLs next to the exe — sherpa-rs-sys copies these on a cache miss.
  if (existsSync(profileDir)) {
    for (const name of readdirSync(profileDir)) {
      if (name.toLowerCase().endsWith('.dll')) sources.push(join(profileDir, name));
    }
  }
  // (b) llama-cpp-sys-2 stages its ggml/llama DLLs under build/*/out/bin.
  sources.push(...findDlls(join(profileDir, 'build'),
    (p) => p.includes('llama-cpp-sys-2') && p.split(/[\\/]/).includes('bin')));
}
// (c) sherpa-rs download dir (%LOCALAPPDATA%\sherpa-rs) — authoritative on a rust-cache
// hit, when sherpa-rs-sys's build.rs (which normally copies next to the exe) is skipped.
// Copy ALL DLLs (mirrors ci.yml): the dir holds exactly the vendor runtime set
// (onnxruntime*, sherpa-onnx-*-api, cargs) whose real names a narrow filter would miss.
if (process.env.LOCALAPPDATA) {
  sources.push(...findDlls(join(process.env.LOCALAPPDATA, 'sherpa-rs')));
}

// Dedupe by filename; release before debug (first hit wins).
const byName = new Map();
for (const src of sources) if (!byName.has(basename(src))) byName.set(basename(src), src);

if (byName.size === 0) {
  console.error('[stage-dlls] No DLLs found. Build first: (cd src-tauri && cargo build --release)');
  process.exit(1);
}

rmSync(dest, { recursive: true, force: true });
mkdirSync(dest, { recursive: true });
for (const [name, src] of byName) {
  copyFileSync(src, join(dest, name));
  console.log(`[stage-dlls] ${name}  <-  ${src}`);
}
console.log(`[stage-dlls] staged ${byName.size} DLL(s) into ${dest}`);
