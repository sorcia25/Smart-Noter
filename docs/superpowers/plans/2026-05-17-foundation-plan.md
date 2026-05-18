# Smart Noter Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Sub-project 1 (Foundation) of Smart Noter: a Tauri 2 desktop app that renders the nine prototype screens pixel-perfect with mock data persisted in SQLite, full primitives library, light/dark theming, ES/EN i18n, and complete IPC contract for sub-projects 2–8 to extend.

**Architecture:** Tauri 2 (Rust) + React 18 + TypeScript + Vite for the webview. Redux Toolkit + RTK Query for state; React Router for navigation; CSS Modules decomposed from the prototype's `app.css`; SQLite via `sqlx` seeded with the prototype's mock data; `tauri-specta` for end-to-end typed IPC. Cargo workspace with skeleton crates (`audio`, `whisper`, `providers`) reserved for sub-projects 2/3/6.

**Tech Stack:** Tauri 2.1, Rust 1.80+, sqlx 0.8, tauri-specta 2.0-rc, React 18.3, TypeScript 5.6, Vite 5.4, Redux Toolkit 2.3, React Router 6.27, react-i18next 15, Biome 1.9, Vitest 2.1, Playwright 1.48, Storybook 8.3, pnpm 9.12.

**Parent specs:**
- [Architecture](../specs/2026-05-17-smart-noter-architecture.md)
- [Foundation design](../specs/2026-05-17-foundation-design.md)

---

## Phase 0 — Repo bootstrap

### Task 0.1: Initialize git repo + commit specs

**Files:**
- Create: `.gitignore`, `.gitattributes`
- Existing (untouched): `docs/superpowers/specs/2026-05-17-smart-noter-architecture.md`, `docs/superpowers/specs/2026-05-17-foundation-design.md`, `handoff/**`

- [ ] **Step 1: Initialize the repo at the project root**

```bash
cd "C:\Users\erick\Projects\Smart Noter"
git init -b main
```

Expected: `Initialized empty Git repository in C:/Users/erick/Projects/Smart Noter/.git/`

- [ ] **Step 2: Write `.gitignore`**

```gitignore
# Node
node_modules/
.pnpm-store/
dist/
*.local

# Rust
target/
**/*.rs.bk

# Tauri
src-tauri/gen/
src-tauri/target/

# IDE
.idea/
.vscode/*
!.vscode/extensions.json

# OS
.DS_Store
Thumbs.db

# App data (never commit)
**/.env
**/.env.local
**/db.sqlite
**/db.sqlite-journal

# Test artifacts
coverage/
playwright-report/
test-results/

# Generated
src/ipc/bindings.ts
src/i18n/keys.ts
src-tauri/crates/db/.sqlx/

# Logs
logs/
*.log
```

- [ ] **Step 3: Write `.gitattributes`** (consistent line endings on Windows)

```gitattributes
* text=auto eol=lf
*.bat text eol=crlf
*.cmd text eol=crlf
*.ps1 text eol=crlf
*.png binary
*.jpg binary
*.woff2 binary
*.ico binary
*.zip binary
```

- [ ] **Step 4: Initial commit with specs + handoff**

```bash
git add .gitignore .gitattributes docs/ handoff/
git commit -m "chore: initial commit — specs, handoff bundle, gitignore"
```

Expected: Commit succeeds.

- [ ] **Step 5: Verify**

```bash
git log --oneline
git status
```

Expected: One commit. Working tree clean.

---

### Task 0.2: Scaffold pnpm workspace + tooling

**Files:**
- Create: `package.json`, `pnpm-workspace.yaml`, `biome.json`, `lefthook.yml`, `tsconfig.json`, `tsconfig.node.json`, `.editorconfig`, `.npmrc`
- Verify: `pnpm --version` is ≥ 9.12

- [ ] **Step 1: Verify toolchain**

```bash
pnpm --version
node --version
```

Expected: pnpm ≥ 9.12, node ≥ 20.

If pnpm is missing: `npm install -g pnpm@9.12.0`

- [ ] **Step 2: Write `.npmrc`** (pin store, hoist for Tauri)

```ini
auto-install-peers=true
strict-peer-dependencies=false
shamefully-hoist=false
```

- [ ] **Step 3: Write `package.json`**

```json
{
  "name": "smart-noter",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "tauri:dev": "tauri dev",
    "tauri:build": "tauri build",
    "test": "vitest",
    "test:run": "vitest --run",
    "test:coverage": "vitest --run --coverage",
    "test:e2e": "playwright test",
    "test:e2e:update": "playwright test --update-snapshots",
    "lint": "biome check .",
    "lint:fix": "biome check --apply .",
    "format": "biome format --write .",
    "storybook": "storybook dev -p 6006",
    "storybook:build": "storybook build",
    "extract-mocks": "node scripts/extract-mocks.mjs",
    "generate:i18n-keys": "node scripts/generate-i18n-keys.mjs",
    "generate:bindings": "cd src-tauri && cargo run --bin specta-export",
    "check:hardcoded-strings": "node scripts/check-no-hardcoded-strings.mjs",
    "check:stories": "node scripts/check-stories-coverage.mjs"
  },
  "dependencies": {
    "@reduxjs/toolkit": "^2.3.0",
    "@tauri-apps/api": "^2.1.1",
    "@tauri-apps/plugin-log": "^2.0.0",
    "date-fns": "^4.1.0",
    "i18next": "^23.16.0",
    "i18next-icu": "^2.3.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1",
    "react-hook-form": "^7.53.0",
    "react-i18next": "^15.1.0",
    "react-redux": "^9.1.2",
    "react-router-dom": "^6.27.0",
    "sonner": "^1.5.0",
    "zod": "^3.23.8"
  },
  "devDependencies": {
    "@biomejs/biome": "^1.9.0",
    "@playwright/test": "^1.48.0",
    "@storybook/addon-essentials": "^8.3.0",
    "@storybook/react-vite": "^8.3.0",
    "@tauri-apps/cli": "^2.1.0",
    "@testing-library/jest-dom": "^6.5.0",
    "@testing-library/react": "^16.0.0",
    "@testing-library/user-event": "^14.5.0",
    "@types/node": "^22.7.0",
    "@types/react": "^18.3.0",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.0",
    "@vitest/coverage-v8": "^2.1.0",
    "jsdom": "^25.0.0",
    "lefthook": "^1.7.0",
    "msw": "^2.4.0",
    "storybook": "^8.3.0",
    "typescript": "^5.6.0",
    "vite": "^5.4.0",
    "vitest": "^2.1.0"
  },
  "packageManager": "pnpm@9.12.0",
  "engines": {
    "node": ">=20"
  }
}
```

- [ ] **Step 4: Write `pnpm-workspace.yaml`** (single root, reserved for future)

```yaml
packages:
  - "."
```

- [ ] **Step 5: Write `tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedIndexedAccess": true,
    "exactOptionalPropertyTypes": false,
    "allowImportingTsExtensions": false,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"]
    },
    "types": ["vite/client", "vitest/globals", "@testing-library/jest-dom"]
  },
  "include": ["src", "tests"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

- [ ] **Step 6: Write `tsconfig.node.json`**

```json
{
  "compilerOptions": {
    "composite": true,
    "skipLibCheck": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "types": ["node"]
  },
  "include": ["vite.config.ts", "vitest.config.ts", "playwright.config.ts", "scripts/**/*.mjs"]
}
```

- [ ] **Step 7: Write `biome.json`**

```json
{
  "$schema": "https://biomejs.dev/schemas/1.9.0/schema.json",
  "vcs": { "enabled": true, "clientKind": "git", "useIgnoreFile": true },
  "files": {
    "ignore": ["node_modules", "dist", "src-tauri/target", "src/ipc/bindings.ts", "src/i18n/keys.ts", "**/*.stories.tsx"]
  },
  "organizeImports": { "enabled": true },
  "linter": {
    "enabled": true,
    "rules": {
      "recommended": true,
      "suspicious": { "noConsoleLog": "error" },
      "style": { "useImportType": "error", "useNodejsImportProtocol": "error" },
      "correctness": { "noUnusedImports": "error" }
    }
  },
  "formatter": {
    "enabled": true,
    "indentStyle": "space",
    "indentWidth": 2,
    "lineWidth": 100,
    "lineEnding": "lf"
  },
  "javascript": {
    "formatter": {
      "quoteStyle": "single",
      "jsxQuoteStyle": "double",
      "trailingCommas": "es5",
      "semicolons": "always"
    }
  }
}
```

- [ ] **Step 8: Write `lefthook.yml`**

```yaml
pre-commit:
  parallel: true
  commands:
    biome:
      glob: "*.{ts,tsx,js,jsx,json}"
      run: pnpm biome check {staged_files}
    no-hardcoded-strings:
      glob: "src/**/*.tsx"
      run: node scripts/check-no-hardcoded-strings.mjs {staged_files}
    rust-fmt:
      glob: "*.rs"
      root: src-tauri/
      run: cargo fmt --check
    rust-clippy:
      glob: "*.rs"
      root: src-tauri/
      run: cargo clippy --workspace --all-targets -- -D warnings

commit-msg:
  commands:
    conventional:
      run: |
        head -1 {1} | grep -qE '^(feat|fix|chore|docs|refactor|test|style|perf|build|ci)(\(.+\))?: .+' || (echo "Commit must follow Conventional Commits (type: subject)"; exit 1)
```

- [ ] **Step 9: Write `.editorconfig`**

```ini
root = true

[*]
charset = utf-8
end_of_line = lf
indent_style = space
indent_size = 2
insert_final_newline = true
trim_trailing_whitespace = true

[*.{rs,toml}]
indent_size = 4

[*.md]
trim_trailing_whitespace = false
```

- [ ] **Step 10: Install dependencies**

```bash
pnpm install
```

Expected: All packages install. `node_modules/` and `pnpm-lock.yaml` appear.

- [ ] **Step 11: Verify Biome runs**

```bash
pnpm biome check . --no-errors-on-unmatched
```

Expected: Exits 0 with "No files were checked" or similar (no source files yet).

- [ ] **Step 12: Install lefthook hooks**

```bash
pnpm exec lefthook install
```

Expected: `sync hooks: ✔️ (pre-commit, commit-msg)`

- [ ] **Step 13: Commit**

```bash
git add .editorconfig .gitignore .gitattributes .npmrc package.json pnpm-lock.yaml pnpm-workspace.yaml biome.json lefthook.yml tsconfig.json tsconfig.node.json
git commit -m "chore: scaffold pnpm workspace + biome + lefthook + tsconfig"
```

---

### Task 0.3: Scaffold Tauri 2 + Vite + React app

**Files:**
- Create: `vite.config.ts`, `index.html`, `src/main.tsx`, `src/App.tsx`, `src/App.module.css`, `src/vite-env.d.ts`, `src/styles/reset.css`, `src/styles/globals.css`
- Create: `src-tauri/Cargo.toml` (workspace), `src-tauri/tauri.conf.json`, `src-tauri/build.rs`, `src-tauri/capabilities/default.json`, `src-tauri/icons/` (copied from prototype's SVG thumbnail)

- [ ] **Step 1: Install Tauri CLI globally if missing**

```bash
pnpm exec tauri --version
```

If error: it will install via package.json devDep when we run `pnpm install` again. Verify:

```bash
pnpm install
pnpm exec tauri --version
```

Expected: `tauri-cli 2.1.x`

- [ ] **Step 2: Write `vite.config.ts`**

```ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'node:path';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  resolve: {
    alias: { '@': path.resolve(__dirname, 'src') },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: 'ws', host, port: 1421 } : undefined,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    minify: !process.env.TAURI_ENV_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    cssMinify: true,
  },
  css: {
    modules: {
      localsConvention: 'camelCaseOnly',
      generateScopedName: process.env.TAURI_ENV_DEBUG
        ? '[name]__[local]__[hash:base64:5]'
        : '[hash:base64:8]',
    },
  },
}));
```

- [ ] **Step 3: Write `index.html`**

```html
<!doctype html>
<html lang="es">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Smart Noter</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 4: Write `src/vite-env.d.ts`**

```ts
/// <reference types="vite/client" />
```

- [ ] **Step 5: Write minimal `src/main.tsx`** (placeholder to validate scaffold)

```tsx
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './styles/reset.css';
import './styles/globals.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

- [ ] **Step 6: Write minimal `src/App.tsx`** (placeholder)

```tsx
import styles from './App.module.css';

function App() {
  return (
    <div className={styles.bootstrap}>
      <h1>Smart Noter</h1>
      <p>Foundation scaffolding online.</p>
    </div>
  );
}

export default App;
```

- [ ] **Step 7: Write `src/App.module.css`**

```css
.bootstrap {
  display: grid;
  place-items: center;
  height: 100vh;
  font-family: system-ui, sans-serif;
  color: #1a1a1a;
}
```

- [ ] **Step 8: Write `src/styles/reset.css`** (minimal — full Fluent reset lands in Phase 2)

```css
* { box-sizing: border-box; }
html, body { margin: 0; padding: 0; height: 100%; }
#root { height: 100%; }
```

- [ ] **Step 9: Write `src/styles/globals.css`** (stub — populated in Phase 2)

```css
/* Global styles will be populated when migrating from prototype/app.css */
```

- [ ] **Step 10: Initialize Tauri 2 scaffold (workspace root)**

```bash
mkdir -p src-tauri/icons src-tauri/capabilities src-tauri/src src-tauri/crates
```

- [ ] **Step 11: Write `src-tauri/Cargo.toml` (workspace root)**

```toml
[workspace]
resolver = "2"
members = [
  ".",
  "crates/core",
  "crates/db",
  "crates/audio",
  "crates/whisper",
  "crates/providers"
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.80"
license = "MIT"

[workspace.dependencies]
tauri = { version = "2.1", features = [] }
tauri-build = { version = "2.0", features = [] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.41", features = ["full"] }
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio", "macros", "migrate", "chrono"] }
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }
tracing-appender = "0.2"
specta = "2.0.0-rc.20"
specta-typescript = "0.0.7"
tauri-specta = { version = "2.0.0-rc.20", features = ["derive", "typescript"] }
uuid = { version = "1.10", features = ["v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

# Binary crate (smart-noter) inherits its own package config:
[package]
name = "smart-noter"
version.workspace = true
edition.workspace = true

[lib]
name = "smart_noter_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[[bin]]
name = "smart-noter"
path = "src/main.rs"

[[bin]]
name = "specta-export"
path = "src/bin/specta_export.rs"

[build-dependencies]
tauri-build.workspace = true

[dependencies]
tauri = { workspace = true, features = [] }
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
sqlx.workspace = true
thiserror.workspace = true
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
tracing-appender.workspace = true
specta.workspace = true
specta-typescript.workspace = true
tauri-specta.workspace = true

smart-noter-core = { path = "crates/core" }
smart-noter-db = { path = "crates/db" }
smart-noter-audio = { path = "crates/audio" }
smart-noter-whisper = { path = "crates/whisper" }
smart-noter-providers = { path = "crates/providers" }
```

- [ ] **Step 12: Write `src-tauri/build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 13: Write `src-tauri/tauri.conf.json`**

```jsonc
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Smart Noter",
  "version": "0.1.0",
  "identifier": "com.smartnoter.app",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "Smart Noter",
        "width": 1440,
        "height": 900,
        "minWidth": 1100,
        "minHeight": 700,
        "center": true,
        "decorations": false,
        "transparent": false,
        "resizable": true,
        "windowEffects": {
          "effects": ["mica"]
        }
      }
    ],
    "security": {
      "csp": "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["msi"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/icon.ico"
    ]
  }
}
```

- [ ] **Step 14: Generate Tauri icons from the prototype's SVG thumbnail**

Save the SVG below as `src-tauri/icons/icon.svg`:

```svg
<svg viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#10b981"/>
      <stop offset="100%" stop-color="#0c8a61"/>
    </linearGradient>
  </defs>
  <rect width="200" height="200" rx="40" fill="url(#bg)"/>
  <g transform="translate(70,55)" fill="white">
    <rect x="14" y="0" width="32" height="50" rx="16"/>
    <path d="M0 40 a 30 30 0 0 0 60 0" fill="none" stroke="white" stroke-width="6" stroke-linecap="round"/>
    <line x1="30" y1="70" x2="30" y2="85" stroke="white" stroke-width="6" stroke-linecap="round"/>
  </g>
</svg>
```

Then run:

```bash
cd src-tauri
pnpm exec tauri icon icons/icon.svg
cd ..
```

Expected: Generates `icon.ico`, `32x32.png`, `128x128.png`, `128x128@2x.png`, plus platform-specific icons under `src-tauri/icons/`.

- [ ] **Step 15: Write `src-tauri/capabilities/default.json`**

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capabilities for Smart Noter main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:window:allow-close",
    "core:window:allow-minimize",
    "core:window:allow-maximize",
    "core:window:allow-unmaximize",
    "core:window:allow-toggle-maximize",
    "core:window:allow-start-dragging",
    "core:window:allow-set-title",
    "core:path:default",
    "core:event:default",
    "core:app:default"
  ]
}
```

- [ ] **Step 16: Write minimal `src-tauri/src/main.rs`** (placeholder — real entrypoint in Task 1.5)

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    smart_noter_lib::run();
}
```

- [ ] **Step 17: Write minimal `src-tauri/src/lib.rs`**

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 18: Add `tauri-plugin-log` to dependencies**

Add to `src-tauri/Cargo.toml` `[dependencies]`:

```toml
tauri-plugin-log = "2.0"
```

- [ ] **Step 19: Stub `src-tauri/src/bin/specta_export.rs`** (real impl in Phase 1)

```rust
fn main() {
    println!("specta-export: stub (will be implemented in Phase 1)");
}
```

- [ ] **Step 20: Create empty stub crates**

```bash
for crate in core db audio whisper providers; do
  mkdir -p "src-tauri/crates/$crate/src"
done
```

For each crate `$NAME` in [`core`, `db`, `audio`, `whisper`, `providers`], write `src-tauri/crates/$NAME/Cargo.toml`:

```toml
[package]
name = "smart-noter-{NAME}"
version.workspace = true
edition.workspace = true

[dependencies]
```

(Substitute `{NAME}` with the actual crate name.)

And `src-tauri/crates/$NAME/src/lib.rs`:

```rust
pub fn version() -> &'static str {
    "0.1.0"
}
```

- [ ] **Step 21: Run `tauri:dev` once to validate scaffold**

```bash
pnpm tauri:dev
```

Expected: Cargo builds (first compile ~3 min), Vite starts on :1420, a 1440×900 borderless window opens displaying "Smart Noter — Foundation scaffolding online." Console has no errors.

Close the window when verified.

- [ ] **Step 22: Commit**

```bash
git add vite.config.ts index.html src/ src-tauri/
git commit -m "feat: scaffold Tauri 2 + Vite + React app with workspace crates"
```

---

## Phase 1 — Rust backbone

### Task 1.1: `core` crate — AppError + Bilingual helper

**Files:**
- Modify: `src-tauri/crates/core/Cargo.toml`
- Create: `src-tauri/crates/core/src/lib.rs`, `error.rs`, `lang.rs`
- Test: `src-tauri/crates/core/src/error.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Update `src-tauri/crates/core/Cargo.toml`**

```toml
[package]
name = "smart-noter-core"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
specta.workspace = true
chrono.workspace = true
uuid.workspace = true
```

- [ ] **Step 2: Write failing test in `src/error.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

#[derive(Debug, Error, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "code", content = "message")]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn i18n_key(&self) -> &'static str {
        match self {
            AppError::NotFound(_) => "errors.notFound",
            AppError::Database(_) => "errors.database",
            AppError::Validation(_) => "errors.validation",
            AppError::Internal(_) => "errors.internal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_tagged_json() {
        let err = AppError::NotFound("meeting m-999".into());
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"code":"notFound","message":"meeting m-999"}"#);
    }

    #[test]
    fn each_variant_has_i18n_key() {
        assert_eq!(AppError::NotFound("x".into()).i18n_key(), "errors.notFound");
        assert_eq!(AppError::Database("x".into()).i18n_key(), "errors.database");
        assert_eq!(AppError::Validation("x".into()).i18n_key(), "errors.validation");
        assert_eq!(AppError::Internal("x".into()).i18n_key(), "errors.internal");
    }
}
```

- [ ] **Step 3: Run tests (expect compile error first, then pass after we write lib.rs)**

```bash
cd src-tauri
cargo test -p smart-noter-core
cd ..
```

Expected: Compile error because `lib.rs` doesn't yet `pub mod error;`. We fix in Step 4.

- [ ] **Step 4: Write `src/lang.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

/// A bilingual ES/EN string. ES is always present; EN is optional and falls back to ES.
#[derive(Debug, Clone, Type, Serialize, Deserialize)]
pub struct Bilingual {
    pub es: String,
    pub en: Option<String>,
}

impl Bilingual {
    pub fn new(es: impl Into<String>) -> Self {
        Self { es: es.into(), en: None }
    }

    pub fn with_en(es: impl Into<String>, en: impl Into<String>) -> Self {
        Self { es: es.into(), en: Some(en.into()) }
    }

    pub fn pick(&self, lang: &str) -> &str {
        match lang {
            "en" => self.en.as_deref().unwrap_or(&self.es),
            _ => &self.es,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_returns_en_when_lang_en_and_en_present() {
        let b = Bilingual::with_en("Hola", "Hello");
        assert_eq!(b.pick("en"), "Hello");
    }

    #[test]
    fn pick_falls_back_to_es_when_en_missing() {
        let b = Bilingual::new("Hola");
        assert_eq!(b.pick("en"), "Hola");
    }

    #[test]
    fn pick_returns_es_for_lang_es() {
        let b = Bilingual::with_en("Hola", "Hello");
        assert_eq!(b.pick("es"), "Hola");
    }
}
```

- [ ] **Step 5: Write `src/lib.rs`**

```rust
pub mod error;
pub mod lang;
pub mod models;

pub use error::AppError;
pub use lang::Bilingual;
```

- [ ] **Step 6: Stub `src/models/mod.rs` (real types come in Task 1.2)**

```bash
mkdir -p src-tauri/crates/core/src/models
```

Then write `src-tauri/crates/core/src/models/mod.rs`:

```rust
// Model definitions land in Task 1.2.
```

- [ ] **Step 7: Run tests — expect PASS**

```bash
cd src-tauri
cargo test -p smart-noter-core
cd ..
```

Expected: `5 passed; 0 failed`

- [ ] **Step 8: Commit**

```bash
git add src-tauri/crates/core/
git commit -m "feat(core): AppError + Bilingual helper with Specta types"
```

---

### Task 1.2: `core` crate — domain models

**Files:**
- Create: `src-tauri/crates/core/src/models/{meeting,participant,action,template,audio_device,settings}.rs`
- Modify: `src-tauri/crates/core/src/models/mod.rs`

- [ ] **Step 1: Write `src/models/participant.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub id: String,
    pub meeting_id: String,
    pub label: String,
    pub name: Option<String>,
    pub color_class: String,
    pub word_count: i64,
    pub talk_pct: i64,
}
```

- [ ] **Step 2: Write `src/models/action.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use crate::Bilingual;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub id: String,
    pub meeting_id: String,
    pub text: Bilingual,
    pub owner_participant_id: Option<String>,
    pub due: Option<String>, // ISO8601 date
    pub done: bool,
}
```

- [ ] **Step 3: Write `src/models/template.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use crate::Bilingual;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub color_class: String,
    pub icon: String,
    pub name: Bilingual,
    pub desc: Bilingual,
    pub sections: Vec<String>,
    pub is_default: bool,
}
```

- [ ] **Step 4: Write `src/models/audio_device.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use crate::Bilingual;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: Bilingual,
    pub desc: Bilingual,
    pub icon: String,
    pub recommended: bool,
    pub active: bool,
}
```

- [ ] **Step 5: Write `src/models/meeting.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;
use crate::Bilingual;
use super::{Action, Participant};

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingSummary {
    pub id: String,
    pub title: Bilingual,
    pub template: String,
    pub date: String,
    pub duration_sec: i64,
    pub participants: Vec<Participant>,
    pub word_count: i64,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptLine {
    pub t: String,
    pub speaker_id: String,
    pub text: Bilingual,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingDetail {
    pub id: String,
    pub title: Bilingual,
    pub template: String,
    pub date: String,
    pub duration_sec: i64,
    pub device_used: Option<String>,
    pub word_count: i64,
    pub summary: Option<Bilingual>,
    pub participants: Vec<Participant>,
    pub actions: Vec<Action>,
    pub decisions: Vec<Bilingual>,
    pub blockers: Vec<Bilingual>,
    pub transcript: Vec<TranscriptLine>,
}
```

- [ ] **Step 6: Write `src/models/settings.rs`**

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Type, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: Theme,
    pub accent: String,
    pub language: Language,
    pub avatar_style: AvatarStyle,
    pub ai_chat_visible: bool,
    pub capture_mode: CaptureMode,
    pub default_device: String,
    pub recording_quality: String,
    pub run_local: bool,
    pub auto_delete_audio: bool,
    pub transcription_provider: String,
    pub transcription_model: String,
    pub default_template: String,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Es,
    En,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AvatarStyle {
    Circle,
    Square,
}

#[derive(Debug, Clone, Type, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CaptureMode {
    System,
    Mic,
    Mix,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::Light,
            accent: "#10b981".into(),
            language: Language::Es,
            avatar_style: AvatarStyle::Circle,
            ai_chat_visible: true,
            capture_mode: CaptureMode::System,
            default_device: "system-loopback".into(),
            recording_quality: "WAV 48k".into(),
            run_local: true,
            auto_delete_audio: false,
            transcription_provider: "local".into(),
            transcription_model: "Whisper Large v3".into(),
            default_template: "tecnica".into(),
        }
    }
}
```

- [ ] **Step 7: Update `src/models/mod.rs`**

```rust
pub mod meeting;
pub mod participant;
pub mod action;
pub mod template;
pub mod audio_device;
pub mod settings;

pub use meeting::{MeetingSummary, MeetingDetail, TranscriptLine};
pub use participant::Participant;
pub use action::Action;
pub use template::Template;
pub use audio_device::AudioDevice;
pub use settings::{AppSettings, Theme, Language, AvatarStyle, CaptureMode};
```

- [ ] **Step 8: Add roundtrip serialization test in `src/models/settings.rs`**

Add at the bottom of `settings.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_roundtrip_through_json() {
        let original = AppSettings::default();
        let json = serde_json::to_string(&original).unwrap();
        let parsed: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.theme, original.theme);
        assert_eq!(parsed.language, original.language);
        assert_eq!(parsed.accent, original.accent);
    }

    #[test]
    fn theme_serializes_lowercase() {
        let json = serde_json::to_string(&Theme::Dark).unwrap();
        assert_eq!(json, r#""dark""#);
    }
}
```

- [ ] **Step 9: Run tests**

```bash
cd src-tauri
cargo test -p smart-noter-core
cd ..
```

Expected: All tests pass.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/crates/core/
git commit -m "feat(core): domain models — Meeting, Participant, Action, Template, AudioDevice, AppSettings"
```

---

### Task 1.3: `db` crate — connection + migrations + repos skeleton

**Files:**
- Modify: `src-tauri/crates/db/Cargo.toml`
- Create: `src-tauri/crates/db/migrations/0001_init.sql`
- Create: `src-tauri/crates/db/src/lib.rs`, `connection.rs`, `repos/mod.rs`

- [ ] **Step 1: Update `src-tauri/crates/db/Cargo.toml`**

```toml
[package]
name = "smart-noter-db"
version.workspace = true
edition.workspace = true

[dependencies]
sqlx.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tracing.workspace = true
tokio.workspace = true
uuid.workspace = true
chrono.workspace = true
smart-noter-core = { path = "../core" }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util", "macros", "rt-multi-thread"] }
tempfile = "3.13"
```

- [ ] **Step 2: Write `migrations/0001_init.sql`**

```sql
CREATE TABLE meetings (
    id TEXT PRIMARY KEY,
    title_es TEXT NOT NULL,
    title_en TEXT,
    template_id TEXT NOT NULL,
    date TEXT NOT NULL,
    duration_sec INTEGER NOT NULL,
    word_count INTEGER NOT NULL DEFAULT 0,
    device_used TEXT,
    summary_es TEXT,
    summary_en TEXT,
    audio_path TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_meetings_date ON meetings(date DESC);
CREATE INDEX idx_meetings_template ON meetings(template_id);

CREATE TABLE participants (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    name TEXT,
    color_class TEXT NOT NULL,
    word_count INTEGER NOT NULL DEFAULT 0,
    talk_pct INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_participants_meeting ON participants(meeting_id);

CREATE TABLE actions (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    text_es TEXT NOT NULL,
    text_en TEXT,
    owner_participant_id TEXT REFERENCES participants(id) ON DELETE SET NULL,
    due TEXT,
    done INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_actions_meeting ON actions(meeting_id);

CREATE TABLE transcript_lines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    t_seconds INTEGER NOT NULL,
    t_display TEXT NOT NULL,
    speaker_id TEXT REFERENCES participants(id) ON DELETE SET NULL,
    text_es TEXT NOT NULL,
    text_en TEXT
);

CREATE INDEX idx_transcript_meeting ON transcript_lines(meeting_id, t_seconds);

CREATE TABLE decisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    text_es TEXT NOT NULL,
    text_en TEXT
);

CREATE TABLE blockers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    text_es TEXT NOT NULL,
    text_en TEXT
);

CREATE TABLE templates (
    id TEXT PRIMARY KEY,
    color_class TEXT NOT NULL,
    icon TEXT NOT NULL,
    name_es TEXT NOT NULL,
    name_en TEXT NOT NULL,
    desc_es TEXT NOT NULL,
    desc_en TEXT NOT NULL,
    sections_json TEXT NOT NULL,
    is_default INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

- [ ] **Step 3: Write `src/connection.rs`**

```rust
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

pub async fn init_pool(db_path: &Path) -> Result<SqlitePool, DbError> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let url = format!("sqlite://{}", db_path.display());
    let options = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .pragma("journal_mode", "WAL");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn init_pool_in_memory() -> Result<SqlitePool, DbError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
```

- [ ] **Step 4: Stub `src/repos/mod.rs`** (repos populate in Task 1.4)

```rust
pub mod meetings_repo;
pub mod participants_repo;
pub mod actions_repo;
pub mod templates_repo;
pub mod settings_repo;
```

For now, write each of those five files as just:

```rust
// Implementation lands in Task 1.4
```

- [ ] **Step 5: Write `src/lib.rs`**

```rust
pub mod connection;
pub mod repos;
pub mod seed;

pub use connection::{init_pool, init_pool_in_memory, DbError};
```

- [ ] **Step 6: Stub `src/seed.rs`** (populated in Task 1.6)

```rust
use sqlx::SqlitePool;
use crate::DbError;

pub async fn seed_if_empty(_pool: &SqlitePool, _json_path: &std::path::Path) -> Result<(), DbError> {
    // Implementation lands in Task 1.6
    Ok(())
}
```

- [ ] **Step 7: Write integration test in `tests/migration.rs`**

```bash
mkdir -p src-tauri/crates/db/tests
```

Then `src-tauri/crates/db/tests/migration.rs`:

```rust
use smart_noter_db::init_pool_in_memory;

#[tokio::test]
async fn migration_creates_expected_tables() {
    let pool = init_pool_in_memory().await.expect("pool");

    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlx_%' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    )
    .fetch_all(&pool)
    .await
    .expect("query");

    let names: Vec<String> = tables.into_iter().map(|(n,)| n).collect();
    assert_eq!(
        names,
        vec![
            "actions", "blockers", "decisions", "meetings",
            "participants", "settings", "templates", "transcript_lines"
        ]
    );
}

#[tokio::test]
async fn foreign_keys_are_enabled() {
    let pool = init_pool_in_memory().await.expect("pool");
    let fk: (i64,) = sqlx::query_as("PRAGMA foreign_keys")
        .fetch_one(&pool).await.expect("query");
    assert_eq!(fk.0, 1);
}
```

- [ ] **Step 8: Run tests**

```bash
cd src-tauri
cargo test -p smart-noter-db
cd ..
```

Expected: 2 tests pass.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/crates/db/
git commit -m "feat(db): SQLite connection + initial schema (0001_init)"
```

---

### Task 1.4: `db` crate — repository implementations

**Files:**
- Modify: `src-tauri/crates/db/src/repos/{meetings,participants,actions,templates,settings}_repo.rs`
- Test: inline `#[cfg(test)]` in each repo file

- [ ] **Step 1: Write `repos/templates_repo.rs`** (simplest — start here)

```rust
use smart_noter_core::{models::Template, Bilingual};
use sqlx::SqlitePool;
use crate::DbError;

pub async fn list_all(pool: &SqlitePool) -> Result<Vec<Template>, DbError> {
    let rows = sqlx::query!(
        r#"SELECT id, color_class, icon, name_es, name_en, desc_es, desc_en, sections_json, is_default
           FROM templates ORDER BY id"#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| Template {
            id: r.id,
            color_class: r.color_class,
            icon: r.icon,
            name: Bilingual::with_en(r.name_es, r.name_en),
            desc: Bilingual::with_en(r.desc_es, r.desc_en),
            sections: serde_json::from_str(&r.sections_json).unwrap_or_default(),
            is_default: r.is_default != 0,
        })
        .collect())
}

pub async fn set_default(pool: &SqlitePool, id: &str) -> Result<(), DbError> {
    let mut tx = pool.begin().await?;
    sqlx::query!("UPDATE templates SET is_default = 0").execute(&mut *tx).await?;
    sqlx::query!("UPDATE templates SET is_default = 1 WHERE id = ?", id)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    #[tokio::test]
    async fn list_all_returns_empty_on_fresh_db() {
        let pool = init_pool_in_memory().await.unwrap();
        let templates = list_all(&pool).await.unwrap();
        assert!(templates.is_empty());
    }

    #[tokio::test]
    async fn set_default_flips_flag() {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query!(
            "INSERT INTO templates VALUES ('a','c','i','na','ne','da','de','[]',1),
                                          ('b','c','i','na','ne','da','de','[]',0)"
        ).execute(&pool).await.unwrap();
        set_default(&pool, "b").await.unwrap();
        let templates = list_all(&pool).await.unwrap();
        assert!(templates.iter().find(|t| t.id == "a").unwrap().is_default == false);
        assert!(templates.iter().find(|t| t.id == "b").unwrap().is_default == true);
    }
}
```

- [ ] **Step 2: Write `repos/participants_repo.rs`**

```rust
use smart_noter_core::models::Participant;
use sqlx::SqlitePool;
use crate::DbError;

pub async fn list_by_meeting(pool: &SqlitePool, meeting_id: &str) -> Result<Vec<Participant>, DbError> {
    let rows = sqlx::query_as!(
        Participant,
        r#"SELECT id, meeting_id, label, name, color_class, word_count, talk_pct
           FROM participants WHERE meeting_id = ? ORDER BY label"#,
        meeting_id
    )
    .fetch_all(pool).await?;
    Ok(rows)
}

pub async fn rename(pool: &SqlitePool, participant_id: &str, name: Option<&str>) -> Result<(), DbError> {
    sqlx::query!("UPDATE participants SET name = ? WHERE id = ?", name, participant_id)
        .execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    async fn setup() -> SqlitePool {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query!(
            "INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES ('m1', 'M1', 'tecnica', '2025-01-01T00:00:00', 100)"
        ).execute(&pool).await.unwrap();
        sqlx::query!(
            "INSERT INTO participants (id, meeting_id, label, color_class) VALUES ('p1', 'm1', 'S1', 's-color-1')"
        ).execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn rename_persists() {
        let pool = setup().await;
        rename(&pool, "p1", Some("Alice")).await.unwrap();
        let parts = list_by_meeting(&pool, "m1").await.unwrap();
        assert_eq!(parts[0].name.as_deref(), Some("Alice"));
    }

    #[tokio::test]
    async fn rename_to_none_clears_name() {
        let pool = setup().await;
        rename(&pool, "p1", Some("Alice")).await.unwrap();
        rename(&pool, "p1", None).await.unwrap();
        let parts = list_by_meeting(&pool, "m1").await.unwrap();
        assert_eq!(parts[0].name, None);
    }
}
```

- [ ] **Step 3: Write `repos/actions_repo.rs`**

```rust
use smart_noter_core::{models::Action, Bilingual};
use sqlx::SqlitePool;
use crate::DbError;

pub async fn list_by_meeting(pool: &SqlitePool, meeting_id: &str) -> Result<Vec<Action>, DbError> {
    let rows = sqlx::query!(
        r#"SELECT id, meeting_id, text_es, text_en, owner_participant_id, due, done
           FROM actions WHERE meeting_id = ?"#,
        meeting_id
    ).fetch_all(pool).await?;

    Ok(rows.into_iter().map(|r| Action {
        id: r.id,
        meeting_id: r.meeting_id,
        text: Bilingual { es: r.text_es, en: r.text_en },
        owner_participant_id: r.owner_participant_id,
        due: r.due,
        done: r.done != 0,
    }).collect())
}

pub async fn toggle(pool: &SqlitePool, action_id: &str) -> Result<bool, DbError> {
    let row = sqlx::query!("SELECT done FROM actions WHERE id = ?", action_id)
        .fetch_one(pool).await?;
    let new_done = if row.done == 0 { 1 } else { 0 };
    sqlx::query!("UPDATE actions SET done = ? WHERE id = ?", new_done, action_id)
        .execute(pool).await?;
    Ok(new_done != 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    async fn setup() -> SqlitePool {
        let pool = init_pool_in_memory().await.unwrap();
        sqlx::query!("INSERT INTO meetings (id, title_es, template_id, date, duration_sec) VALUES ('m1', 'M1', 't', '2025-01-01', 100)")
            .execute(&pool).await.unwrap();
        sqlx::query!("INSERT INTO actions (id, meeting_id, text_es, done) VALUES ('a1', 'm1', 'Do thing', 0)")
            .execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn toggle_flips_done() {
        let pool = setup().await;
        let after_first = toggle(&pool, "a1").await.unwrap();
        assert!(after_first);
        let after_second = toggle(&pool, "a1").await.unwrap();
        assert!(!after_second);
    }
}
```

- [ ] **Step 4: Write `repos/meetings_repo.rs`**

```rust
use smart_noter_core::{models::{MeetingSummary, MeetingDetail, TranscriptLine}, Bilingual};
use sqlx::SqlitePool;
use crate::DbError;
use crate::repos::{participants_repo, actions_repo};

pub async fn list_summaries(pool: &SqlitePool) -> Result<Vec<MeetingSummary>, DbError> {
    let rows = sqlx::query!(
        r#"SELECT id, title_es, title_en, template_id, date, duration_sec, word_count
           FROM meetings ORDER BY date DESC"#
    ).fetch_all(pool).await?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let participants = participants_repo::list_by_meeting(pool, &r.id).await?;
        out.push(MeetingSummary {
            id: r.id,
            title: Bilingual { es: r.title_es, en: r.title_en },
            template: r.template_id,
            date: r.date,
            duration_sec: r.duration_sec,
            participants,
            word_count: r.word_count,
        });
    }
    Ok(out)
}

pub async fn get_detail(pool: &SqlitePool, id: &str) -> Result<MeetingDetail, DbError> {
    let m = sqlx::query!(
        r#"SELECT id, title_es, title_en, template_id, date, duration_sec, word_count,
                  device_used, summary_es, summary_en
           FROM meetings WHERE id = ?"#,
        id
    ).fetch_one(pool).await?;

    let participants = participants_repo::list_by_meeting(pool, id).await?;
    let actions = actions_repo::list_by_meeting(pool, id).await?;

    let decisions = sqlx::query!("SELECT text_es, text_en FROM decisions WHERE meeting_id = ?", id)
        .fetch_all(pool).await?
        .into_iter().map(|r| Bilingual { es: r.text_es, en: r.text_en }).collect();

    let blockers = sqlx::query!("SELECT text_es, text_en FROM blockers WHERE meeting_id = ?", id)
        .fetch_all(pool).await?
        .into_iter().map(|r| Bilingual { es: r.text_es, en: r.text_en }).collect();

    let transcript = sqlx::query!(
        "SELECT t_display, speaker_id, text_es, text_en FROM transcript_lines WHERE meeting_id = ? ORDER BY t_seconds",
        id
    ).fetch_all(pool).await?
    .into_iter().map(|r| TranscriptLine {
        t: r.t_display,
        speaker_id: r.speaker_id.unwrap_or_default(),
        text: Bilingual { es: r.text_es, en: r.text_en },
    }).collect();

    let summary = match (m.summary_es, m.summary_en) {
        (Some(es), en) => Some(Bilingual { es, en }),
        _ => None,
    };

    Ok(MeetingDetail {
        id: m.id,
        title: Bilingual { es: m.title_es, en: m.title_en },
        template: m.template_id,
        date: m.date,
        duration_sec: m.duration_sec,
        device_used: m.device_used,
        word_count: m.word_count,
        summary,
        participants,
        actions,
        decisions,
        blockers,
        transcript,
    })
}

pub async fn update_title(
    pool: &SqlitePool,
    id: &str,
    title_es: &str,
    title_en: Option<&str>
) -> Result<(), DbError> {
    sqlx::query!(
        "UPDATE meetings SET title_es = ?, title_en = ?, updated_at = datetime('now') WHERE id = ?",
        title_es, title_en, id
    ).execute(pool).await?;
    Ok(())
}

pub async fn count(pool: &SqlitePool) -> Result<i64, DbError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM meetings").fetch_one(pool).await?;
    Ok(row.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;

    #[tokio::test]
    async fn list_summaries_empty_on_fresh_db() {
        let pool = init_pool_in_memory().await.unwrap();
        assert!(list_summaries(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn count_zero_on_fresh_db() {
        let pool = init_pool_in_memory().await.unwrap();
        assert_eq!(count(&pool).await.unwrap(), 0);
    }
}
```

- [ ] **Step 5: Write `repos/settings_repo.rs`**

```rust
use smart_noter_core::models::AppSettings;
use sqlx::SqlitePool;
use crate::DbError;

const KEY: &str = "app";

pub async fn get(pool: &SqlitePool) -> Result<AppSettings, DbError> {
    let row = sqlx::query!("SELECT value FROM settings WHERE key = ?", KEY)
        .fetch_optional(pool).await?;
    match row {
        Some(r) => Ok(serde_json::from_str(&r.value).unwrap_or_default()),
        None => Ok(AppSettings::default()),
    }
}

pub async fn upsert(pool: &SqlitePool, settings: &AppSettings) -> Result<(), DbError> {
    let value = serde_json::to_string(settings).unwrap();
    sqlx::query!(
        "INSERT INTO settings (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        KEY, value
    ).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;
    use smart_noter_core::models::Theme;

    #[tokio::test]
    async fn get_returns_default_when_empty() {
        let pool = init_pool_in_memory().await.unwrap();
        let s = get(&pool).await.unwrap();
        assert_eq!(s.theme, Theme::Light);
    }

    #[tokio::test]
    async fn upsert_then_get_roundtrips() {
        let pool = init_pool_in_memory().await.unwrap();
        let mut s = AppSettings::default();
        s.theme = Theme::Dark;
        upsert(&pool, &s).await.unwrap();
        let loaded = get(&pool).await.unwrap();
        assert_eq!(loaded.theme, Theme::Dark);
    }
}
```

- [ ] **Step 6: Prepare sqlx offline metadata**

```bash
cd src-tauri
DATABASE_URL=sqlite::memory: cargo sqlx prepare --workspace
cd ..
```

If `cargo sqlx` is missing: `cargo install sqlx-cli --no-default-features --features sqlite,rustls`

Expected: `.sqlx/` directories appear under affected crates.

- [ ] **Step 7: Run all db tests**

```bash
cd src-tauri
cargo test -p smart-noter-db
cd ..
```

Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/crates/db/
git commit -m "feat(db): repository implementations for meetings, participants, actions, templates, settings"
```

---

### Task 1.5: AppState + tauri-specta export binary

**Files:**
- Create: `src-tauri/src/state.rs`, `src-tauri/src/error.rs`, `src-tauri/src/commands/mod.rs`, `src-tauri/src/commands/{meetings,templates,devices,settings,log}.rs`, `src-tauri/src/events/mod.rs`
- Create: `src-tauri/src/bin/specta_export.rs` (replaces stub)
- Modify: `src-tauri/src/lib.rs`, `src-tauri/Cargo.toml`

- [ ] **Step 1: Add `tauri-plugin-log` config + plugin deps**

Update `src-tauri/Cargo.toml` `[dependencies]`:

```toml
tauri-plugin-log = "2.0"
```

- [ ] **Step 2: Write `src-tauri/src/state.rs`**

```rust
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
}
```

- [ ] **Step 3: Write `src-tauri/src/error.rs`**

```rust
use smart_noter_core::AppError;
use smart_noter_db::DbError;

pub fn from_db(e: DbError) -> AppError {
    AppError::Database(e.to_string())
}
```

- [ ] **Step 4: Write each command file (Foundation IPC)**

`src-tauri/src/commands/mod.rs`:

```rust
pub mod meetings;
pub mod templates;
pub mod devices;
pub mod settings;
pub mod log;
```

`src-tauri/src/commands/meetings.rs`:

```rust
use crate::state::AppState;
use crate::error::from_db;
use smart_noter_core::{AppError, models::{MeetingSummary, MeetingDetail}};
use smart_noter_db::repos::{meetings_repo, participants_repo, actions_repo};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn list_meetings(state: State<'_, AppState>) -> Result<Vec<MeetingSummary>, AppError> {
    meetings_repo::list_summaries(&state.pool).await.map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn get_meeting(state: State<'_, AppState>, id: String) -> Result<MeetingDetail, AppError> {
    meetings_repo::get_detail(&state.pool, &id).await.map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn update_meeting_title(
    state: State<'_, AppState>,
    id: String,
    title_es: String,
    title_en: Option<String>,
) -> Result<(), AppError> {
    meetings_repo::update_title(&state.pool, &id, &title_es, title_en.as_deref())
        .await.map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn toggle_action(state: State<'_, AppState>, action_id: String) -> Result<bool, AppError> {
    actions_repo::toggle(&state.pool, &action_id).await.map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn rename_participant(
    state: State<'_, AppState>,
    participant_id: String,
    name: Option<String>,
) -> Result<(), AppError> {
    participants_repo::rename(&state.pool, &participant_id, name.as_deref())
        .await.map_err(from_db)
}
```

`src-tauri/src/commands/templates.rs`:

```rust
use crate::state::AppState;
use crate::error::from_db;
use smart_noter_core::{AppError, models::Template};
use smart_noter_db::repos::templates_repo;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn list_templates(state: State<'_, AppState>) -> Result<Vec<Template>, AppError> {
    templates_repo::list_all(&state.pool).await.map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn set_default_template(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    templates_repo::set_default(&state.pool, &id).await.map_err(from_db)
}
```

`src-tauri/src/commands/devices.rs`:

```rust
use smart_noter_core::{AppError, Bilingual, models::AudioDevice};

#[tauri::command]
#[specta::specta]
pub async fn list_audio_devices() -> Result<Vec<AudioDevice>, AppError> {
    Ok(vec![
        AudioDevice {
            id: "system-loopback".into(),
            name: Bilingual::with_en("Audio del sistema (Loopback)", "System Audio (Loopback)"),
            desc: Bilingual::with_en(
                "Captura todo el audio que reproduce la PC — recomendado para Teams/Zoom.",
                "Captures all audio playing on this PC — recommended for Teams/Zoom.",
            ),
            icon: "monitor".into(),
            recommended: true,
            active: true,
        },
        AudioDevice {
            id: "realtek-mic".into(),
            name: Bilingual::with_en("Micrófono — Realtek HD Audio", "Microphone — Realtek HD Audio"),
            desc: Bilingual::with_en(
                "Sólo capturará tu voz local, no la de los demás participantes.",
                "Will only capture your local voice, not other participants.",
            ),
            icon: "mic".into(),
            recommended: false,
            active: false,
        },
        AudioDevice {
            id: "jabra-evolve".into(),
            name: Bilingual::with_en("Jabra Evolve2 75 — Headset", "Jabra Evolve2 75 — Headset"),
            desc: Bilingual::with_en(
                "Audio del headset USB. Captura el lado del usuario.",
                "USB headset audio. Captures the user side.",
            ),
            icon: "headphones".into(),
            recommended: false,
            active: false,
        },
        AudioDevice {
            id: "stereo-mix".into(),
            name: Bilingual::with_en("Mezcla estéreo (Stereo Mix)", "Stereo Mix"),
            desc: Bilingual::with_en(
                "Combina entrada y salida del sistema. Alternativa al loopback.",
                "Combines system input and output. Alternative to loopback.",
            ),
            icon: "sliders".into(),
            recommended: false,
            active: false,
        },
    ])
}
```

`src-tauri/src/commands/settings.rs`:

```rust
use crate::state::AppState;
use crate::error::from_db;
use smart_noter_core::{AppError, models::AppSettings};
use smart_noter_db::repos::settings_repo;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, AppError> {
    settings_repo::get(&state.pool).await.map_err(from_db)
}

#[tauri::command]
#[specta::specta]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<(), AppError> {
    settings_repo::upsert(&state.pool, &settings).await.map_err(from_db)
}
```

`src-tauri/src/commands/log.rs`:

```rust
use smart_noter_core::AppError;
use tracing::{error, info, warn};

#[tauri::command]
#[specta::specta]
pub fn log_frontend_error(
    level: String,
    message: String,
    stack: Option<String>,
) -> Result<(), AppError> {
    match level.as_str() {
        "error" => error!(target: "frontend", "{message}\n{}", stack.unwrap_or_default()),
        "warn" => warn!(target: "frontend", "{message}"),
        _ => info!(target: "frontend", "{message}"),
    }
    Ok(())
}
```

- [ ] **Step 5: Write `src-tauri/src/events/mod.rs`** (empty registry)

```rust
// Backend events for sub-projects 2+. Foundation has no events.
```

- [ ] **Step 6: Rewrite `src-tauri/src/lib.rs`**

```rust
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

pub mod commands;
pub mod events;
pub mod state;
pub mod error;

use crate::state::AppState;

fn specta_builder() -> Builder {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::meetings::list_meetings,
            commands::meetings::get_meeting,
            commands::meetings::update_meeting_title,
            commands::meetings::toggle_action,
            commands::meetings::rename_participant,
            commands::templates::list_templates,
            commands::templates::set_default_template,
            commands::devices::list_audio_devices,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::log::log_frontend_error,
        ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let app_handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let app_data = app_handle.path().app_data_dir().expect("app_data_dir");
                std::fs::create_dir_all(&app_data).ok();
                let db_path = app_data.join("db.sqlite");
                let pool = smart_noter_db::init_pool(&db_path)
                    .await.expect("init pool");
                app_handle.manage(AppState { pool });
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 7: Write `src-tauri/src/bin/specta_export.rs`**

```rust
use specta_typescript::Typescript;

fn main() {
    smart_noter_lib::specta_builder()
        .export(
            Typescript::default().header("// AUTO-GENERATED by tauri-specta — do not edit.\n"),
            "../src/ipc/bindings.ts",
        )
        .expect("export bindings");
    println!("bindings.ts exported");
}
```

You'll also need to expose `specta_builder` from `lib.rs`. Change `fn specta_builder() -> Builder` to `pub fn specta_builder() -> Builder`.

- [ ] **Step 8: Create `src/ipc/` directory and generate bindings**

```bash
mkdir -p src/ipc
cd src-tauri
cargo run --bin specta-export
cd ..
```

Expected: `src/ipc/bindings.ts` created with typed wrappers.

- [ ] **Step 9: Build full project**

```bash
cd src-tauri
cargo build --workspace
cd ..
```

Expected: Clean build.

- [ ] **Step 10: Run all backend tests**

```bash
cd src-tauri
cargo test --workspace
cd ..
```

Expected: All tests pass.

- [ ] **Step 11: Commit**

```bash
git add src-tauri/src/ src-tauri/Cargo.toml src/ipc/bindings.ts
git commit -m "feat: AppState + Tauri commands + specta-export binary for typed IPC"
```

---

### Task 1.6: Extract mocks script + seed implementation

**Files:**
- Create: `scripts/extract-mocks.mjs`
- Create: `src-tauri/crates/db/seed_data.json` (generated by script)
- Modify: `src-tauri/crates/db/src/seed.rs`
- Modify: `src-tauri/src/lib.rs` (call seed after pool init)

- [ ] **Step 1: Write `scripts/extract-mocks.mjs`**

```js
#!/usr/bin/env node
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');
const dataJsPath = resolve(root, 'handoff/smart-noter/project/data.js');
const outPath = resolve(root, 'src-tauri/crates/db/seed_data.json');

const source = readFileSync(dataJsPath, 'utf-8');

// The prototype's data.js attaches symbols to `window`. Run it in a sandbox with a fake window.
const sandbox = { window: {} };
vm.createContext(sandbox);
vm.runInContext(source, sandbox);

const win = sandbox.window;
const out = {
  templates: win.TEMPLATES,
  meetings: win.MEETINGS,
  audioDevices: win.AUDIO_DEVICES,
};

mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, JSON.stringify(out, null, 2));
console.log(`Wrote ${out.meetings.length} meetings, ${out.templates.length} templates → ${outPath}`);
```

- [ ] **Step 2: Run extraction**

```bash
node scripts/extract-mocks.mjs
```

Expected: `Wrote 6 meetings, 9 templates → .../seed_data.json`

- [ ] **Step 3: Write `src-tauri/crates/db/src/seed.rs`**

```rust
use serde::Deserialize;
use sqlx::SqlitePool;
use std::path::Path;
use crate::{DbError, repos::meetings_repo};

#[derive(Deserialize)]
struct SeedData {
    templates: Vec<SeedTemplate>,
    meetings: Vec<SeedMeeting>,
    #[serde(rename = "audioDevices")]
    _audio_devices: serde_json::Value, // unused — devices are hardcoded in command
}

#[derive(Deserialize)]
struct SeedTemplate {
    id: String,
    #[serde(rename = "colorClass")]
    color_class: String,
    icon: String,
    name: BilingualSeed,
    desc: BilingualSeed,
    sections: Vec<String>,
}

#[derive(Deserialize)]
struct BilingualSeed {
    es: String,
    en: String,
}

#[derive(Deserialize)]
struct SeedMeeting {
    id: String,
    title: BilingualSeed,
    template: String,
    date: String,
    #[serde(rename = "durationSec")]
    duration_sec: i64,
    participants: Vec<SeedParticipant>,
    #[serde(rename = "deviceUsed")]
    device_used: Option<String>,
    #[serde(rename = "wordCount", default)]
    word_count: i64,
    #[serde(default)]
    summary: Option<BilingualSeed>,
    #[serde(default)]
    decisions: Vec<BilingualSeed>,
    #[serde(default)]
    blockers: Vec<BilingualSeed>,
    #[serde(default)]
    actions: Vec<SeedAction>,
    #[serde(default)]
    transcript: Vec<SeedTranscriptLine>,
}

#[derive(Deserialize)]
struct SeedParticipant {
    id: String,
    label: String,
    name: Option<String>,
    #[serde(rename = "colorClass")]
    color_class: String,
    #[serde(rename = "wordCount", default)]
    word_count: i64,
    #[serde(rename = "talkPct", default)]
    talk_pct: i64,
}

#[derive(Deserialize)]
struct SeedAction {
    id: String,
    text: BilingualSeed,
    owner: Option<String>,
    due: Option<String>,
    #[serde(default)]
    done: bool,
}

#[derive(Deserialize)]
struct SeedTranscriptLine {
    t: String,
    #[serde(rename = "speakerId")]
    speaker_id: String,
    text: BilingualSeed,
}

fn t_seconds(t: &str) -> i64 {
    let parts: Vec<&str> = t.split(':').collect();
    match parts.as_slice() {
        [h, m, s] => h.parse::<i64>().unwrap_or(0) * 3600
                  + m.parse::<i64>().unwrap_or(0) * 60
                  + s.parse::<i64>().unwrap_or(0),
        [m, s] => m.parse::<i64>().unwrap_or(0) * 60 + s.parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

pub async fn seed_if_empty(pool: &SqlitePool, json_path: &Path) -> Result<(), DbError> {
    let count = meetings_repo::count(pool).await?;
    if count > 0 {
        return Ok(());
    }

    let bytes = std::fs::read(json_path).map_err(|e| DbError::Sqlx(sqlx::Error::Io(e)))?;
    let data: SeedData = serde_json::from_slice(&bytes)
        .map_err(|e| DbError::Sqlx(sqlx::Error::Decode(Box::new(e))))?;

    let mut tx = pool.begin().await?;

    for (i, t) in data.templates.iter().enumerate() {
        let sections = serde_json::to_string(&t.sections).unwrap();
        let is_default: i64 = if t.id == "tecnica" { 1 } else { 0 };
        let _ = i;
        sqlx::query!(
            "INSERT INTO templates (id, color_class, icon, name_es, name_en, desc_es, desc_en, sections_json, is_default)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            t.id, t.color_class, t.icon, t.name.es, t.name.en, t.desc.es, t.desc.en, sections, is_default
        ).execute(&mut *tx).await?;
    }

    for m in &data.meetings {
        let title_en = Some(m.title.en.clone());
        let summary_es = m.summary.as_ref().map(|s| s.es.clone());
        let summary_en = m.summary.as_ref().map(|s| s.en.clone());
        sqlx::query!(
            "INSERT INTO meetings (id, title_es, title_en, template_id, date, duration_sec, word_count, device_used, summary_es, summary_en)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            m.id, m.title.es, title_en, m.template, m.date, m.duration_sec,
            m.word_count, m.device_used, summary_es, summary_en
        ).execute(&mut *tx).await?;

        for p in &m.participants {
            let unique_id = format!("{}-{}", m.id, p.id);
            sqlx::query!(
                "INSERT INTO participants (id, meeting_id, label, name, color_class, word_count, talk_pct)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                unique_id, m.id, p.label, p.name, p.color_class, p.word_count, p.talk_pct
            ).execute(&mut *tx).await?;
        }

        for d in &m.decisions {
            let en = Some(d.en.clone());
            sqlx::query!(
                "INSERT INTO decisions (meeting_id, text_es, text_en) VALUES (?, ?, ?)",
                m.id, d.es, en
            ).execute(&mut *tx).await?;
        }

        for b in &m.blockers {
            let en = Some(b.en.clone());
            sqlx::query!(
                "INSERT INTO blockers (meeting_id, text_es, text_en) VALUES (?, ?, ?)",
                m.id, b.es, en
            ).execute(&mut *tx).await?;
        }

        for a in &m.actions {
            let unique_action_id = format!("{}-{}", m.id, a.id);
            let owner_id = a.owner.as_ref().map(|o| format!("{}-{}", m.id, o));
            let text_en = Some(a.text.en.clone());
            let done_i: i64 = if a.done { 1 } else { 0 };
            sqlx::query!(
                "INSERT INTO actions (id, meeting_id, text_es, text_en, owner_participant_id, due, done)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
                unique_action_id, m.id, a.text.es, text_en, owner_id, a.due, done_i
            ).execute(&mut *tx).await?;
        }

        for line in &m.transcript {
            let seconds = t_seconds(&line.t);
            let speaker_unique = format!("{}-{}", m.id, line.speaker_id);
            let text_en = Some(line.text.en.clone());
            sqlx::query!(
                "INSERT INTO transcript_lines (meeting_id, t_seconds, t_display, speaker_id, text_es, text_en)
                 VALUES (?, ?, ?, ?, ?, ?)",
                m.id, seconds, line.t, speaker_unique, line.text.es, text_en
            ).execute(&mut *tx).await?;
        }
    }

    tx.commit().await?;
    tracing::info!("seeded database with {} meetings, {} templates", data.meetings.len(), data.templates.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_pool_in_memory;
    use std::io::Write;

    #[tokio::test]
    async fn seed_is_idempotent() {
        let pool = init_pool_in_memory().await.unwrap();
        let mut f = tempfile::NamedTempFile::new().unwrap();
        let minimal = serde_json::json!({
            "templates": [{
                "id": "tecnica", "colorClass": "t-color-tecnica", "icon": "cpu",
                "name": {"es": "Técnica", "en": "Technical"},
                "desc": {"es": "X", "en": "X"}, "sections": ["actions"]
            }],
            "meetings": [{
                "id": "m1", "title": {"es": "T", "en": "T"}, "template": "tecnica",
                "date": "2025-01-01T00:00:00", "durationSec": 60,
                "participants": [], "actions": []
            }],
            "audioDevices": []
        });
        f.write_all(serde_json::to_string(&minimal).unwrap().as_bytes()).unwrap();

        seed_if_empty(&pool, f.path()).await.unwrap();
        let after_first = meetings_repo::count(&pool).await.unwrap();
        seed_if_empty(&pool, f.path()).await.unwrap();
        let after_second = meetings_repo::count(&pool).await.unwrap();

        assert_eq!(after_first, 1);
        assert_eq!(after_second, 1, "seed should be idempotent");
    }
}
```

- [ ] **Step 4: Embed `seed_data.json` and wire seed into `lib.rs`**

Modify `src-tauri/src/lib.rs` — update the `.setup()` closure to call seed:

Replace the `let pool = ...` block with:

```rust
            tauri::async_runtime::block_on(async move {
                let app_data = app_handle.path().app_data_dir().expect("app_data_dir");
                std::fs::create_dir_all(&app_data).ok();
                let db_path = app_data.join("db.sqlite");
                let pool = smart_noter_db::init_pool(&db_path).await.expect("init pool");

                // Write embedded seed to disk and seed if empty
                let seed_path = app_data.join("seed_data.json");
                if !seed_path.exists() {
                    let bytes = include_bytes!("../crates/db/seed_data.json");
                    std::fs::write(&seed_path, bytes).expect("write seed json");
                }
                smart_noter_db::seed::seed_if_empty(&pool, &seed_path)
                    .await.expect("seed");

                app_handle.manage(AppState { pool });
            });
```

- [ ] **Step 5: Re-prepare sqlx metadata** (new INSERT queries)

```bash
cd src-tauri
DATABASE_URL=sqlite::memory: cargo sqlx prepare --workspace
cd ..
```

- [ ] **Step 6: Run all tests**

```bash
cd src-tauri
cargo test --workspace
cd ..
```

Expected: All pass (including idempotent seed test).

- [ ] **Step 7: Run app and verify seed**

```bash
pnpm tauri:dev
```

Expected: Window opens. Close it. Then inspect:

```bash
ls "$env:APPDATA/com.smartnoter.app/" 2>$null || ls "$HOME/AppData/Roaming/com.smartnoter.app/"
```

Should show `db.sqlite` and `seed_data.json`.

- [ ] **Step 8: Commit**

```bash
git add scripts/extract-mocks.mjs src-tauri/crates/db/
git commit -m "feat(db): mock extraction script + idempotent seed from prototype data"
```

---

> **Plan continues in subsequent phases.** The remaining phases (CSS migration, primitives, shell, domain components, infrastructure, IPC client, feature screens, quality gates, CI) are documented in [foundation-plan-part2.md](./2026-05-17-foundation-plan-part2.md).
