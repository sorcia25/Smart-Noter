# Sub-8: Distribution — Design

> **Status:** DESIGN — brainstormed 2026-07-01. Roadmap item **8 of 8** (final sub-project).
> When this ships, Smart Noter is a complete, installable, self-updating Windows app at **v1.0.0**.

**Goal:** Make Smart Noter installable and runnable on a clean Windows machine (no dev
environment) and self-updating, shipped as **v1.0.0**.

**Architecture:** A Tauri **NSIS (`.exe`) installer** that bundles the native runtime DLLs next
to the executable, plus **auto-update** via the Tauri updater plugin fed by **GitHub Releases**,
produced by a tag-triggered `release.yml` workflow. Code signing is **deferred** but the pipeline
has a ready-to-enable signing slot.

**Tech stack:** Tauri 2 bundler (NSIS), `tauri-plugin-updater` + `@tauri-apps/plugin-updater`,
`tauri-apps/tauri-action` (GitHub Actions), ed25519 update signing (`tauri signer`).

---

## 1. Scope decisions (from brainstorming 2026-07-01)

| Decision | Choice | Rationale |
|---|---|---|
| Installer format | **NSIS (`.exe`)** only | Per-user install, no elevation prompt, smoother Tauri auto-update than MSI. (Deviates from the roadmap's "MSI"; user preference + better updater fit. MSI can be added alongside later if wanted.) |
| Code signing | **Deferred**, pipeline signing-ready | No certificate yet. Unsigned installer triggers SmartScreen "unknown publisher"; acceptable for a personal/portfolio v1.0. A conditional signing step is wired but skipped until a cert secret exists. |
| Auto-update | **In scope now** | Tauri updater + GitHub Releases. Repo is **public** → the update endpoint is reachable without auth. |
| Target version | **1.0.0** | Sub-8 is the last roadmap item; closing it = complete, distributable app. |

## 2. Non-goals

- **Bundling ML models** (whisper / diarization / LLM GGUF) — they are downloaded at runtime from
  Settings. The installer stays small (< 30 MB), per the architecture doc.
- **Executing code signing** — no certificate procured yet. The slot is built; enabling it is a
  one-line secret away and out of scope for the code work.
- **macOS / Linux packaging** — Windows only.
- **Portable (no-install) build.**
- **CUDA/GPU build** — CPU-only, unchanged.
- **Wiring the sidebar version label to the real app version** — kept a static string for
  deterministic e2e snapshots (see §7). Nice-to-have, deferred.

## 3. Architecture

### 3A. Native DLL bundling — *the must-have*

Today `bundle.targets` produces an installer that ships **only** `smart-noter.exe`. The exe
dynamically loads native DLLs that are absent on any machine but the developer's, so a clean
install fails to start. The DLLs (from the dynamic-linked crates):

- **sherpa** (`sherpa-rs` `download-binaries`, dynamic): `onnxruntime.dll`, `sherpa-onnx.dll`
- **llama** (`llama-cpp-2` `dynamic-link`): the `ggml*` / `llama` DLLs (~5)
- **whisper** (`whisper-rs`, default): **static** → compiled into the exe, **no DLL**
  *(confirmed empirically in Step 0 below — do not assume)*

**Step 0 (empirical enumeration):** run a release build (`pnpm tauri build`) and list every `*.dll`
that ends up next to `target/release/smart-noter.exe`. **That list is authoritative** — the bundle
must include exactly those. This avoids guessing DLL names/counts that drift with crate versions.

**Mechanism:** a **staging step** copies the runtime DLLs into a stable, gitignored folder
(`src-tauri/bundle-dlls/`), and `tauri.conf.json` `bundle.resources` maps them next to the exe in
the installed app:

```jsonc
"bundle": {
  "targets": ["nsis"],
  "resources": { "bundle-dlls/*.dll": "./" }   // land next to smart-noter.exe
}
```

The staging reuses the **proven CI copy logic** (mirrors `ci.yml`): copy the sherpa DLLs from the
build output / `%LOCALAPPDATA%\sherpa-rs`, and the llama DLLs from
`target/release/build/*llama-cpp-sys-2-*/out/bin/*.dll`.

The DLLs are **produced by** `cargo build` (the crates' `build.rs` stages them into `target/release`),
so staging must run **after** the Rust build but **before** bundling. Two ways, in preference order:

1. **Tauri `beforeBundleCommand` hook** — runs after `cargo build`, before the bundler. `target/release`
   already holds the DLLs; the script copies them into `bundle-dlls/`.
2. **Fallback — explicit two-phase build:** `cargo build --release` (materializes the DLLs) →
   stage into `bundle-dlls/` → `tauri build` (reuses the already-built artifacts and bundles).
   A pre-`tauri build` script **alone** cannot work on a clean `target/` — the DLLs don't exist
   until cargo has built.

> **Risk R1 (verify first):** Tauri `resources` must land the DLLs **beside** `smart-noter.exe`,
> not in a `resources/` subfolder — otherwise the Windows loader won't find them. Verified in the
> Windows Sandbox smoke (§6). If Tauri nests them, adjust the resource target or use an NSIS
> template hook so the DLLs sit next to the exe.

### 3B. Auto-update (Tauri updater + GitHub Releases)

- **Plugin:** add `tauri-plugin-updater` (Rust) + `@tauri-apps/plugin-updater` (JS). Register the
  plugin in `lib.rs`.
- **Signing keys (ed25519):** `tauri signer generate` → a **private key** (CI secret, signs the
  update artifacts) + a **public key** (embedded in `tauri.conf.json`). This is the updater's
  integrity signature — **separate from and independent of** code signing (§3D).
- **Config** (`tauri.conf.json`):
  ```jsonc
  "bundle": { "createUpdaterArtifacts": true },
  "plugins": {
    "updater": {
      "pubkey": "<ed25519 public key>",
      "endpoints": ["https://github.com/sorcia25/Smart-Noter/releases/latest/download/latest.json"],
      "windows": { "installMode": "passive" }
    }
  }
  ```
- **Manifest:** `tauri-action` generates `latest.json` (version, notes, `pub_date`, and per-platform
  `{ signature, url }` pointing at the `-setup.exe` update artifact) and uploads it to the Release.
- **Frontend:** a small updater module checks for an update (on startup + a manual "Check for
  updates" button in Settings), and on a found update shows a prompt → download → install →
  relaunch. Uses the plugin's `check()` / `downloadAndInstall()`.
- **Endpoint reachability:** the repo is **public**, so `releases/latest/download/latest.json`
  is fetchable without a token.

### 3C. Release CI — `release.yml`

- **Trigger:** push of a tag matching `v*` (e.g. `v1.0.0`).
- **Runner/setup:** `windows-latest`, reusing `ci.yml`'s setup verbatim — pnpm, Rust toolchain,
  `rust-cache` (with the `~/AppData/Local/sherpa-rs` cache dir + `v1-rust` prefix), the LLVM/libclang
  step, `generate:bindings` + `generate:i18n-keys`, and the **release-profile** DLL copy (the same
  `find` copies as `ci.yml`, but from `target/release/build/...`).
- **Build + release:** `tauri-apps/tauri-action` — builds the NSIS installer, signs the update
  artifacts with the ed25519 key, creates/updates the GitHub Release for the tag, uploads the
  `-setup.exe` + `latest.json` + `.sig`.
- **Secrets:** `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
- `ci.yml` (push/PR on `main`) is unchanged; `release.yml` is additive and tag-scoped.

### 3D. Code-signing slot (deferred, but ready)

A **conditional step** in `release.yml`, after the installer is built and before/at upload, that
signs the `-setup.exe` with `signtool` (traditional cert) **or** Azure Trusted Signing — gated on a
`secrets.CODE_SIGN_*` presence check. With no cert secret it is **skipped** (today's state).
Enabling later = add the secret; **no redesign**. This signature is what clears SmartScreen and is
orthogonal to the ed25519 update signature.

### 3E. Version bump 0.4.0 → 1.0.0

Update in lockstep:

- `package.json` `"version"`
- `src-tauri/tauri.conf.json` `"version"`
- `src-tauri/Cargo.toml` `[workspace.package] version` (line 16; crates inherit via
  `version.workspace = true`)
- `src/components/shell/Sidebar/Sidebar.tsx:13` — `role: 'Pro · v1.0.0'` (static string)

## 4. First-run experience (clean machine)

Install the `.exe` → launch → the app runs (DLLs bundled) → go to **Settings** and download the
whisper / diarization / LLM models (existing UX). First model use needs internet. This is
unchanged from current behavior; the installer simply makes the app *reachable* on a fresh machine.

## 5. Error handling / edge cases

- **SmartScreen "unknown publisher"** (unsigned) — expected; documented for users ("More info →
  Run anyway"). Removed once §3D is enabled.
- **Missing DLL on clean machine** — the exact failure 3A prevents; caught by the Sandbox smoke.
- **Update endpoint unreachable / offline** — updater fails silently/gracefully (no crash); the app
  keeps working. Manual "Check for updates" surfaces a friendly error.
- **Update signature mismatch / tampered artifact** — the updater rejects it (ed25519 check).
- **NSIS not installed locally** — `tauri build` auto-downloads NSIS on first run; `tauri-action`
  handles it in CI.

## 6. Testing / verification

- **Step 0** — release build; enumerate the DLLs next to the exe (the authoritative bundle list).
- **Windows Sandbox** (available on the user's Win 11 Pro) — copy the built `-setup.exe` into a
  fresh Sandbox, install, launch. Confirms the bundle is **self-contained** (no dev env). *Gold
  standard for R1.*
- **Installer content inspection** — `lessmsi` / 7-Zip on the `-setup.exe` (or extract) to confirm
  the DLLs are present and placed beside the exe.
- **Auto-update smoke** — publish `v1.0.0`, then a `v1.0.1` test tag; confirm a running v1.0.0
  detects, downloads, installs, and relaunches into v1.0.1.
- **Regression** — `pnpm test:coverage` (frontend), `cargo test --workspace` (backend), and the
  e2e suite stay green. The sidebar version text changes → **run e2e; if the diff exceeds
  `maxDiffPixelRatio 0.02`, regenerate the visual baselines** (`pnpm test:e2e:update` + spot-check +
  commit). The change is a few characters so it likely stays under threshold, but verify — this bit
  us in the pre-Sub-8 merge.

## 7. Risks

| # | Risk | Mitigation |
|---|---|---|
| R1 | `resources` may nest DLLs in a subfolder, not beside the exe | Verify first via Sandbox smoke; adjust resource target / NSIS template if nested |
| R2 | `beforeBundleCommand` timing/availability in this Tauri version | Verify it runs after `cargo build`; fall back to the two-phase build (`cargo build --release` → stage → `tauri build`) |
| R3 | Release-profile DLL paths differ from debug (`target/release/build/...`) | Adapt the `ci.yml` `find` copies to the release profile |
| R4 | Release build is heavier/slower in CI | Acceptable; `rust-cache` + sherpa cache mitigate |
| R5 | Keypair + secret setup is a manual one-time step | Documented in §8; done once before the first release tag |

## 8. External setup steps (user, one-time)

1. **Generate the updater keypair:** `pnpm tauri signer generate -w ~/.tauri/smart-noter.key`
   → embed the **public** key in `tauri.conf.json`; add the **private** key +
   password as GitHub repo secrets `TAURI_SIGNING_PRIVATE_KEY` /
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
2. **Repo is public** — the update endpoint works as-is (no action).
3. **(Later, to enable signing)** procure a certificate (Azure Trusted Signing ≈ $10/mo, or a
   traditional EV/OV cert) → add the `CODE_SIGN_*` secret; the §3D step activates automatically.

## 9. Module breakdown (for the implementation plan)

Roughly sequential; each is independently verifiable:

1. **8A — Working installer (DLL bundling)** — the must-have; Step 0 + staging + `resources` +
   Sandbox smoke. Switch target to `nsis`.
2. **8E — Version bump** to 1.0.0 (small; do early so builds/artifacts carry 1.0.0).
3. **8B — Auto-update** — plugin, keys, config, frontend check/prompt.
4. **8C — Release CI** (`release.yml`) with `tauri-action` + secrets + the DLL copy.
5. **8D — Code-signing slot** — the conditional, currently-skipped signing step.

Verification threads through: Sandbox clean-install (8A), auto-update smoke (8B+8C), regression +
baselines (8E).
