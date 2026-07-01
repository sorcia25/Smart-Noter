# Sub-8: Distribution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Execution note:** This sub-project is **infrastructure** (config, build tooling, CI), not
> feature logic. Only Task 7 has classic unit tests; the rest verify via **build + inspect +
> smoke**. Tasks 4 and 10 are **MANUAL** (Windows Sandbox GUI / GitHub secrets / release tag) —
> the controller pairs with the user for those. Inline execution (executing-plans) fits better
> than a subagent fan-out because tasks are sequential and build-dependent.

**Goal:** Ship Smart Noter as a self-contained, self-updating Windows `.exe` installer at v1.0.0.

**Architecture:** A Tauri **NSIS** installer bundles the native runtime DLLs (sherpa/onnx + llama)
next to the exe via a staging script + `bundle.resources`; **auto-update** uses `tauri-plugin-updater`
fed by a `latest.json` on **GitHub Releases**, produced by a tag-triggered `release.yml`. Code
signing is deferred behind a conditional CI slot.

**Tech Stack:** Tauri 2 bundler (NSIS), `tauri-plugin-updater` + `tauri-plugin-process`,
`@tauri-apps/plugin-updater`, `tauri-apps/tauri-action`, ed25519 update signing.

**Spec:** `docs/superpowers/specs/2026-07-01-sub8-distribution-design.md`

---

## File Structure

**Create:**
- `scripts/stage-dlls.mjs` — copies runtime DLLs → `src-tauri/bundle-dlls/` (mirrors `ci.yml` copy logic)
- `src/features/updater/updater.ts` — thin wrapper over the updater plugin (`checkForUpdate`)
- `src/features/updater/useAppUpdater.ts` — React hook: check / install / status
- `src/features/updater/updater.test.ts` — vitest for `checkForUpdate`
- `.github/workflows/release.yml` — tag-triggered build + GitHub Release
- `sandbox/smart-noter.wsb` — Windows Sandbox config for the clean-install smoke

**Modify:**
- `package.json` — version 1.0.0; add `@tauri-apps/plugin-updater`, `@tauri-apps/plugin-process`
- `src-tauri/tauri.conf.json` — version; NSIS target; `resources`; `beforeBundleCommand`; `createUpdaterArtifacts`; `plugins.updater`
- `src-tauri/Cargo.toml` — version 1.0.0; add `tauri-plugin-updater`, `tauri-plugin-process`
- `src-tauri/src/lib.rs` — register the two plugins (near line 104-105)
- `src-tauri/capabilities/default.json` — add `updater:default`, `process:default`
- `src/components/shell/Sidebar/Sidebar.tsx:13` — version string → `Pro · v1.0.0`
- `src/features/settings/SettingsPage.tsx` — "Check for updates" row
- `src/i18n/locales/{es,en}.json` — updater strings
- `.gitignore` — add `src-tauri/bundle-dlls/`

---

## Task 1: Version bump to 1.0.0

**Files:**
- Modify: `package.json:2`
- Modify: `src-tauri/tauri.conf.json:4`
- Modify: `src-tauri/Cargo.toml:16`
- Modify: `src/components/shell/Sidebar/Sidebar.tsx:13`

- [ ] **Step 1: Bump the four version sites**

`package.json`: `"version": "0.4.0"` → `"version": "1.0.0"`
`src-tauri/tauri.conf.json`: `"version": "0.4.0"` → `"version": "1.0.0"`
`src-tauri/Cargo.toml` line 16: `version = "0.4.0"` → `version = "1.0.0"`
`src/components/shell/Sidebar/Sidebar.tsx` line 13: `role: 'Pro · v0.4.0',` → `role: 'Pro · v1.0.0',`

- [ ] **Step 2: Verify no stray 0.4.0 remains and the build is green**

```bash
grep -rn '0\.4\.0' package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src/components/shell/Sidebar/Sidebar.tsx
# Expected: no output
(cd src-tauri && cargo metadata --no-deps --format-version 1 | grep -o '"name":"smart-noter","version":"[^"]*"')
# Expected: ...,"version":"1.0.0"
pnpm build
# Expected: tsc + vite build succeed, exit 0
```

- [ ] **Step 3: Check e2e visual baselines (the sidebar version text changed)**

```bash
pnpm exec playwright install chromium   # once, if not present
pnpm test:e2e
```
Expected: either **19 passed** (the few-char version diff stayed under `maxDiffPixelRatio 0.02`),
or the 8 visual specs fail on a tiny diff. If they fail:
```bash
pnpm test:e2e:update
```
Then Read `tests/e2e/visual.spec.ts-snapshots/dashboard-light-chromium-win32.png` and confirm the
sidebar shows `Pro · v1.0.0` and nothing else changed. (See the pre-Sub-8 baseline gotcha.)

- [ ] **Step 4: Commit**

```bash
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src/components/shell/Sidebar/Sidebar.tsx tests/e2e/
git commit -m "chore(sub8): bump version to 1.0.0"
```

---

## Task 2: DLL staging script

**Files:**
- Create: `scripts/stage-dlls.mjs`
- Modify: `.gitignore`

- [ ] **Step 1: Write the staging script**

Create `scripts/stage-dlls.mjs`:

```js
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
if (process.env.LOCALAPPDATA) {
  sources.push(...findDlls(join(process.env.LOCALAPPDATA, 'sherpa-rs'),
    (p) => /onnxruntime\.dll|sherpa-onnx\.dll/i.test(basename(p))));
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
```

- [ ] **Step 2: Gitignore the staging dir**

Append to `.gitignore`:
```
# Sub-8: native DLLs staged into the installer at bundle time (generated)
src-tauri/bundle-dlls/
```

- [ ] **Step 3: Materialize a release build so the DLLs exist, then run the script**

> This release build is the long pole (whisper.cpp + llama + sherpa compile). Use the env preamble.
```bash
export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
(cd src-tauri && cargo build --release --bin smart-noter)
node scripts/stage-dlls.mjs
```
Expected: `[stage-dlls] staged N DLL(s)` with N ≥ 3, listing at minimum `onnxruntime.dll`,
`sherpa-onnx.dll`, and the `ggml*`/`llama` DLLs. **This N-and-names list is the authoritative
bundle set (spec Step 0).** Record it.

- [ ] **Step 4: Verify the staged set**

```bash
ls src-tauri/bundle-dlls/
```
Expected: the DLLs from Step 3 (e.g. `onnxruntime.dll  sherpa-onnx.dll  ggml.dll  ggml-base.dll  ggml-cpu.dll  llama.dll`). Confirm no `.dll` the app needs is missing (compare against what sits next to `src-tauri/target/release/smart-noter.exe`: `ls src-tauri/target/release/*.dll`).

- [ ] **Step 5: Commit** (the script + gitignore only; `bundle-dlls/` is ignored)

```bash
git add scripts/stage-dlls.mjs .gitignore
git commit -m "build(sub8): DLL staging script for the installer bundle"
```

---

## Task 3: NSIS installer that bundles the DLLs

**Files:**
- Modify: `src-tauri/tauri.conf.json` (`build.beforeBundleCommand`, `bundle.targets`, `bundle.resources`)

- [ ] **Step 1: Switch to NSIS, wire the staging hook, add the DLL resources**

In `src-tauri/tauri.conf.json`:

Under `"build"`, add `beforeBundleCommand` (runs after cargo build, before bundling):
```jsonc
"build": {
  "beforeDevCommand": "pnpm dev",
  "devUrl": "http://localhost:1420",
  "beforeBuildCommand": "pnpm build",
  "beforeBundleCommand": "node scripts/stage-dlls.mjs",
  "frontendDist": "../dist"
},
```

Replace the `"bundle"` block:
```jsonc
"bundle": {
  "active": true,
  "targets": ["nsis"],
  "icon": ["icons/32x32.png", "icons/128x128.png", "icons/icon.ico"],
  "resources": { "bundle-dlls/*.dll": "./" }
}
```

- [ ] **Step 2: Build the installer**

```bash
export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
pnpm tauri build
```
Expected: build succeeds; NSIS downloads on first run; final line points to
`src-tauri/target/release/bundle/nsis/Smart Noter_1.0.0_x64-setup.exe`.
> If `beforeBundleCommand` is not honored by this Tauri version (R2), fall back to the two-phase
> build and re-run bundling: `node scripts/stage-dlls.mjs && pnpm tauri build`.

- [ ] **Step 3: Verify the DLLs are inside the installer**

```bash
SETUP=$(ls -1 "src-tauri/target/release/bundle/nsis/"*-setup.exe | head -1)
"/c/Program Files/7-Zip/7z.exe" l "$SETUP" | grep -iE 'onnxruntime|sherpa-onnx|ggml|llama|smart-noter\.exe'
```
Expected: `smart-noter.exe` plus each staged DLL is listed. **Verify the DLL paths are at the same
level as `smart-noter.exe`, not nested in a `resources/` subfolder** (R1). If nested, change the
`resources` target or add an NSIS template hook, rebuild, re-verify.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "build(sub8): NSIS installer bundling the native DLLs"
```

---

## Task 4: Windows Sandbox clean-install smoke  **[MANUAL — user]**

**Files:**
- Create: `sandbox/smart-noter.wsb`

This is the gold-standard proof that the bundle is self-contained (no dev env).

- [ ] **Step 1: Write a Sandbox config that maps the installer folder in read-only**

Create `sandbox/smart-noter.wsb`:
```xml
<Configuration>
  <MappedFolders>
    <MappedFolder>
      <HostFolder>C:\Users\erick\Projects\Smart Noter\src-tauri\target\release\bundle\nsis</HostFolder>
      <SandboxFolder>C:\installer</SandboxFolder>
      <ReadOnly>true</ReadOnly>
    </MappedFolder>
  </MappedFolders>
  <Networking>Enable</Networking>
</Configuration>
```

- [ ] **Step 2: (once) Enable Windows Sandbox if needed**

In an elevated PowerShell:
```powershell
Enable-WindowsOptionalFeature -FeatureName "Containers-DisposableClientVM" -Online -NoRestart
```
Reboot if it was newly enabled.

- [ ] **Step 3: Run the smoke (user, in the GUI)**

Double-click `sandbox/smart-noter.wsb` → inside the sandbox, run `C:\installer\Smart Noter_1.0.0_x64-setup.exe` → install → launch Smart Noter.
Expected: **the app window opens and the UI loads** with no "missing DLL" / `0xc0000135` error.
Navigate to Settings → the model-download UI appears (models aren't installed — expected;
downloading needs internet). Closing the sandbox discards everything.

- [ ] **Step 4: Commit the sandbox config**

```bash
git add sandbox/smart-noter.wsb
git commit -m "test(sub8): Windows Sandbox config for clean-install smoke"
```

> If the app fails to start in the sandbox, a DLL is missing → revisit Task 2's staging set (add
> the missing source dir) and Task 3's `resources` placement, rebuild, re-smoke. **Do not proceed
> until this passes** — it is the whole point of 8A.

---

## Task 5: Add the updater + process plugins (plumbing)

**Files:**
- Modify: `src-tauri/Cargo.toml` (deps)
- Modify: `src-tauri/src/lib.rs:104-105`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `package.json` (JS deps)

- [ ] **Step 1: Add the Rust deps**

In `src-tauri/Cargo.toml` `[dependencies]`, after `tauri-plugin-dialog = "2"` (line 97):
```toml
tauri-plugin-updater = "2"
tauri-plugin-process = "2"
```

- [ ] **Step 2: Register the plugins**

In `src-tauri/src/lib.rs`, in `run()` (lines 104-105), add after the dialog plugin:
```rust
        .plugin(tauri_plugin_log::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
```

- [ ] **Step 3: Grant the capabilities**

In `src-tauri/capabilities/default.json`, add to the `"permissions"` array (after `"dialog:allow-open"`):
```json
    "dialog:allow-open",
    "updater:default",
    "process:default"
```

- [ ] **Step 4: Add the JS deps**

```bash
pnpm add @tauri-apps/plugin-updater @tauri-apps/plugin-process
```

- [ ] **Step 5: Verify it compiles**

```bash
export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
(cd src-tauri && cargo build)
```
Expected: compiles clean (the plugins are inert until configured in Task 6).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs src-tauri/capabilities/default.json package.json pnpm-lock.yaml
git commit -m "feat(sub8): register updater + process plugins"
```

---

## Task 6: Generate the updater keypair + configure the updater

**Files:**
- Modify: `src-tauri/tauri.conf.json` (`bundle.createUpdaterArtifacts`, `plugins.updater`)

- [ ] **Step 1: Generate the ed25519 keypair**  **[user owns the secret]**

```bash
pnpm tauri signer generate -w "$HOME/.tauri/smart-noter-updater.key"
```
Enter a password when prompted (remember it). This prints a **public key** (a base64 blob) and
writes the private key to `~/.tauri/smart-noter-updater.key`. Copy the public key for Step 2.
Keep the private key file + password for Task 8's GitHub secrets (never commit them).

- [ ] **Step 2: Add the updater config**

In `src-tauri/tauri.conf.json`, set `createUpdaterArtifacts` in the bundle block:
```jsonc
"bundle": {
  "active": true,
  "targets": ["nsis"],
  "createUpdaterArtifacts": true,
  "icon": ["icons/32x32.png", "icons/128x128.png", "icons/icon.ico"],
  "resources": { "bundle-dlls/*.dll": "./" }
}
```
And add a top-level `"plugins"` block (paste the real public key from Step 1):
```jsonc
"plugins": {
  "updater": {
    "pubkey": "PASTE_THE_PUBLIC_KEY_FROM_STEP_1",
    "endpoints": ["https://github.com/sorcia25/Smart-Noter/releases/latest/download/latest.json"],
    "windows": { "installMode": "passive" }
  }
}
```

- [ ] **Step 3: Verify config validity + updater-artifact build**

```bash
export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
export TAURI_SIGNING_PRIVATE_KEY="$(cat "$HOME/.tauri/smart-noter-updater.key")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="<the password from Step 1>"
pnpm tauri build
```
Expected: build succeeds and additionally emits the signed updater artifacts next to the installer:
`Smart Noter_1.0.0_x64-setup.exe` **plus** a `.sig` file (and, with tauri-action later, `latest.json`).
```bash
ls "src-tauri/target/release/bundle/nsis/"*.sig
# Expected: a .sig file exists
```

- [ ] **Step 4: Commit** (config only — the pubkey is safe to commit; the private key is NOT)

```bash
git add src-tauri/tauri.conf.json
git commit -m "feat(sub8): updater config (pubkey + GitHub Releases endpoint)"
```

---

## Task 7: Frontend updater module + Settings UI

**Files:**
- Create: `src/features/updater/updater.ts`
- Create: `src/features/updater/useAppUpdater.ts`
- Create: `src/features/updater/updater.test.ts`
- Modify: `src/features/settings/SettingsPage.tsx`
- Modify: `src/i18n/locales/es.json`, `src/i18n/locales/en.json`

- [ ] **Step 1: Write the failing test**

Create `src/features/updater/updater.test.ts`:
```ts
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/plugin-updater', () => ({ check: vi.fn() }));
import { check } from '@tauri-apps/plugin-updater';
import { checkForUpdate } from './updater';

describe('checkForUpdate', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
  });

  it('returns null outside a Tauri context (and never calls check)', async () => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    expect(await checkForUpdate()).toBeNull();
    expect(check).not.toHaveBeenCalled();
  });

  it('returns null when up to date', async () => {
    (check as ReturnType<typeof vi.fn>).mockResolvedValue(null);
    expect(await checkForUpdate()).toBeNull();
  });

  it('returns the update handle when one is available', async () => {
    const fake = { version: '1.0.1', body: 'notes' };
    (check as ReturnType<typeof vi.fn>).mockResolvedValue(fake);
    expect(await checkForUpdate()).toBe(fake);
  });
});
```

- [ ] **Step 2: Run it — verify it fails**

```bash
pnpm test:run src/features/updater/updater.test.ts
```
Expected: FAIL — `Cannot find module './updater'`.

- [ ] **Step 3: Implement the module**

Create `src/features/updater/updater.ts`:
```ts
import { check, type Update } from '@tauri-apps/plugin-updater';

/**
 * Check GitHub Releases for a newer version. Returns the Update handle when one
 * is available, null when up to date. Never throws outside a Tauri context
 * (vitest / e2e run in a plain browser with no IPC) — returns null instead.
 */
export async function checkForUpdate(): Promise<Update | null> {
  if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window)) return null;
  return (await check()) ?? null;
}
```

- [ ] **Step 4: Run it — verify it passes**

```bash
pnpm test:run src/features/updater/updater.test.ts
```
Expected: PASS (3 tests).

- [ ] **Step 5: Write the hook**

Create `src/features/updater/useAppUpdater.ts`:
```ts
import { relaunch } from '@tauri-apps/plugin-process';
import type { Update } from '@tauri-apps/plugin-updater';
import { useCallback, useState } from 'react';
import { checkForUpdate } from './updater';

export type UpdateStatus =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'upToDate' }
  | { kind: 'available'; version: string; notes: string; update: Update }
  | { kind: 'downloading' }
  | { kind: 'error'; message: string };

export function useAppUpdater() {
  const [status, setStatus] = useState<UpdateStatus>({ kind: 'idle' });

  const check = useCallback(async () => {
    setStatus({ kind: 'checking' });
    try {
      const update = await checkForUpdate();
      if (!update) {
        setStatus({ kind: 'upToDate' });
        return;
      }
      setStatus({ kind: 'available', version: update.version, notes: update.body ?? '', update });
    } catch (e) {
      setStatus({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
    }
  }, []);

  const install = useCallback(async (update: Update) => {
    setStatus({ kind: 'downloading' });
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (e) {
      setStatus({ kind: 'error', message: e instanceof Error ? e.message : String(e) });
    }
  }, []);

  return { status, check, install };
}
```

- [ ] **Step 6: Add i18n strings**

Add to BOTH `src/i18n/locales/es.json` and `src/i18n/locales/en.json` (Spanish shown; use the English equivalents in en.json):
```jsonc
  "updateSection": "Actualizaciones",
  "updateCheck": "Buscar actualizaciones",
  "updateChecking": "Buscando…",
  "updateUpToDate": "Estás en la última versión.",
  "updateAvailable": "Versión {version} disponible",
  "updateDownloading": "Descargando e instalando…",
  "updateInstall": "Actualizar ahora",
  "updateError": "No se pudo comprobar. Inténtalo más tarde."
```
en.json values: `"Updates"`, `"Check for updates"`, `"Checking…"`, `"You're on the latest version."`, `"Version {version} available"`, `"Downloading and installing…"`, `"Update now"`, `"Couldn't check. Try again later."`.

- [ ] **Step 7: Regenerate i18n keys**

```bash
pnpm generate:i18n-keys
```
Expected: `src/i18n/keys.ts` gains the 8 `update*` keys; exit 0.

- [ ] **Step 8: Wire a "Check for updates" row into Settings**

In `src/features/settings/SettingsPage.tsx`, import the hook and render a row (place it in an
existing settings card near the bottom; adapt the JSX to the file's card/row primitives):
```tsx
import { useAppUpdater } from '../updater/useAppUpdater';
// …inside the component:
const updater = useAppUpdater();
// …in JSX (use the page's existing Row/Button components; illustrative markup):
<section>
  <h2>{t('updateSection')}</h2>
  <button type="button" onClick={updater.check} disabled={updater.status.kind === 'checking'}>
    {t('updateCheck')}
  </button>
  {updater.status.kind === 'checking' && <span>{t('updateChecking')}</span>}
  {updater.status.kind === 'upToDate' && <span>{t('updateUpToDate')}</span>}
  {updater.status.kind === 'available' && (
    <>
      <span>{t('updateAvailable', { version: updater.status.version })}</span>
      <button type="button" onClick={() => updater.install(updater.status.update)}>
        {t('updateInstall')}
      </button>
    </>
  )}
  {updater.status.kind === 'downloading' && <span>{t('updateDownloading')}</span>}
  {updater.status.kind === 'error' && <span>{t('updateError')}</span>}
</section>
```

- [ ] **Step 9: Verify the whole frontend is green**

```bash
pnpm check:node-types && pnpm lint && pnpm test:run && pnpm build
```
Expected: all pass (tsc 0 errors, biome clean, vitest green incl. the 3 updater tests, vite build ok).

- [ ] **Step 10: Commit**

```bash
git add src/features/updater/ src/features/settings/SettingsPage.tsx src/i18n/locales/es.json src/i18n/locales/en.json src/i18n/keys.ts
git commit -m "feat(sub8): in-app update check in Settings"
```

---

## Task 8: Release workflow (release.yml)

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Write the workflow** (mirrors `ci.yml` setup + `tauri-action`)

Create `.github/workflows/release.yml`:
```yaml
name: Release

on:
  push:
    tags: ["v*"]

permissions:
  contents: write   # create the GitHub Release + upload assets

jobs:
  release:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with:
          version: 9.12.0
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: pnpm
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri
          prefix-key: v1-rust
          cache-directories: ~/AppData/Local/sherpa-rs
      - run: pnpm install --frozen-lockfile
      - name: Set up LLVM (libclang for whisper-rs bindgen)
        shell: bash
        run: |
          if [ ! -f "/c/Program Files/LLVM/bin/libclang.dll" ]; then
            choco install llvm -y --no-progress
          fi
          echo "LIBCLANG_PATH=C:\\Program Files\\LLVM\\bin" >> "$GITHUB_ENV"
      - name: Generate IPC bindings + i18n keys
        working-directory: src-tauri
        shell: bash
        run: |
          cargo build --bin specta-export
          find ~/AppData/Local/sherpa-rs -name '*.dll' -exec cp -f {} target/debug/ \;
          find target/debug/build -path '*llama-cpp-sys-2-*/out/bin/*.dll' -exec cp -f {} target/debug/ \; 2>/dev/null || true
          ./target/debug/specta-export.exe
      - run: pnpm generate:i18n-keys
      - name: Build + release
        uses: tauri-apps/tauri-action@v0
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: "Smart Noter ${{ github.ref_name }}"
          releaseBody: "See the assets to download and install."
          releaseDraft: false
          prerelease: false
          includeUpdaterJson: true
```

> `tauri-action` runs `pnpm tauri build`, which triggers `beforeBundleCommand` (stage-dlls) — so
> the DLL sources must be present on the runner. The `find … -exec cp` copies in the bindings step
> prime `target/debug`; for the **release** build the stage-dlls script itself pulls from
> `~/AppData/Local/sherpa-rs` and `target/release/build/*/out/bin` (both cached). If a release DLL
> is missing on CI, extend `scripts/stage-dlls.mjs` sources — do not special-case CI here.

- [ ] **Step 2: Add the GitHub secrets**  **[user — repo admin]**

In the GitHub repo → Settings → Secrets and variables → Actions → New repository secret:
- `TAURI_SIGNING_PRIVATE_KEY` = the full contents of `~/.tauri/smart-noter-updater.key`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = the password from Task 6 Step 1

- [ ] **Step 3: Lint the workflow locally (syntax)**

```bash
node -e "const y=require('fs').readFileSync('.github/workflows/release.yml','utf8'); require('yaml') ? null : null; console.log('read ok, bytes', y.length)"
```
(Or eyeball it — there is no local runner. Real validation is the tag push in Task 10.)

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci(sub8): tag-triggered release workflow (NSIS + updater + GitHub Release)"
```

---

## Task 9: Code-signing slot (deferred, skipped by default)

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Add a gate variable + a conditional signing step**

In `release.yml`, add near the top of the `release` job's steps a step that decides whether signing
is enabled (based on whether a cert secret is set), then a conditional sign step **after** the build.
Add just after `- uses: actions/checkout@v4`:
```yaml
      - name: Decide code-signing (skipped until a cert secret exists)
        id: signing
        shell: bash
        env:
          CERT: ${{ secrets.CODE_SIGN_CERT_BASE64 }}
        run: echo "enabled=$([ -n "$CERT" ] && echo true || echo false)" >> "$GITHUB_OUTPUT"
```
And after the `tauri-apps/tauri-action` step:
```yaml
      - name: Sign the installer (deferred — runs only when a cert secret is configured)
        if: steps.signing.outputs.enabled == 'true'
        shell: pwsh
        run: |
          # PLACEHOLDER wiring for when a certificate is procured:
          #  - decode ${{ secrets.CODE_SIGN_CERT_BASE64 }} to a .pfx (or use Azure Trusted Signing)
          #  - signtool sign /fd SHA256 /tr <timestamp-url> /td SHA256 /f cert.pfx /p $env:CODE_SIGN_PASSWORD `
          #      "src-tauri/target/release/bundle/nsis/*-setup.exe"
          Write-Host "Code-signing enabled but not yet implemented — see spec §3D."
```

> This step is **skipped** today (no `CODE_SIGN_CERT_BASE64` secret). Enabling later: add the secret
> and fill in the signtool/Azure Trusted Signing command. No other change needed.

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci(sub8): conditional code-signing slot (deferred)"
```

---

## Task 10: Final validation + first release  **[MANUAL — controller + user]**

- [ ] **Step 1: Full local regression**

```bash
pnpm check:node-types && pnpm lint && pnpm check:hardcoded-strings && pnpm check:stories && pnpm test:run
export PATH="$HOME/.cargo/bin:/c/Program Files (x86)/Microsoft Visual Studio/2022/BuildTools/Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin:$PATH"
export LIBCLANG_PATH="C:/Program Files/LLVM/bin"
(cd src-tauri && cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace)
pnpm test:e2e
```
Expected: everything green. (Regen e2e baselines only if a visual spec fails — see Task 1 Step 3.)

- [ ] **Step 2: Finish the branch (merge to main via superpowers:finishing-a-development-branch)**

Merge `feat/sub8-distribution` → `main`, push. Wait for `ci.yml` to go green on `main` (the config
changes must not break the main pipeline; the version bump + NSIS target don't touch tested code).

- [ ] **Step 3: Confirm the GitHub secrets are set** (Task 8 Step 2). Without them, `release.yml`
      fails at signing the updater artifacts.

- [ ] **Step 4: Tag v1.0.0 → trigger the release**

```bash
git tag v1.0.0
git push origin v1.0.0
gh run watch "$(gh run list --workflow release.yml --limit 1 --json databaseId -q '.[0].databaseId')" --exit-status
```
Expected: `release.yml` builds and creates the **v1.0.0** GitHub Release with `*-setup.exe`,
its `.sig`, and `latest.json` attached.
```bash
gh release view v1.0.0 --json assets -q '.assets[].name'
# Expected: Smart Noter_1.0.0_x64-setup.exe, ...-setup.exe.sig, latest.json
```

- [ ] **Step 5: Auto-update smoke** *(after a second release exists)*

Install v1.0.0 (from the release) on a test machine / Sandbox. Later, bump to `1.0.1`, tag
`v1.0.1`, push. Open the installed v1.0.0 → Settings → **Buscar actualizaciones** → expect
"Versión 1.0.1 disponible" → **Actualizar ahora** → it downloads, installs, relaunches into 1.0.1.

- [ ] **Step 6: Update memory + close**

Record the Sub-8 outcome (SHIP SHA, the DLL bundle set from Task 2 Step 3, the beforeBundleCommand
vs two-phase resolution, NSIS-not-MSI, deferred signing) in the project memory. Sub-8 done → the
roadmap is complete; Smart Noter is a distributable v1.0.0.

---

## Self-Review

**Spec coverage:** 3A DLL bundling → T2+T3; R1 placement → T3 S3 + T4; 3B auto-update → T5+T6+T7;
3C release CI → T8; 3D signing slot → T9; 3E version bump → T1; §6 verification (Sandbox, inspect,
update smoke, baselines) → T4, T3 S3, T10 S5, T1 S3; §8 setup (keypair, secrets) → T6 S1, T8 S2. All covered.

**Placeholder scan:** The only intentional placeholders are the pubkey (pasted from the generated
key, T6) and the signtool command (T9 — the deferred slot, by design). No unfilled logic.

**Type consistency:** `checkForUpdate(): Promise<Update | null>` (T7 S3) matches the test (T7 S1)
and the hook's use (T7 S5). `UpdateStatus` union used consistently in the hook + Settings JSX.
`stage-dlls.mjs` output dir `src-tauri/bundle-dlls/` matches `.gitignore` (T2), `resources`
(T3), and `beforeBundleCommand` (T3). Secrets `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)` consistent
across T6 (local build), T8 (workflow env + repo secret).
