# Sub-project 1 — Foundation — Design Spec

**Date:** 2026-05-17
**Status:** Approved
**Parent spec:** [2026-05-17-smart-noter-architecture.md](./2026-05-17-smart-noter-architecture.md)
**Position in roadmap:** 1 of 8 sub-projects

> Foundation is the first implementable slice of Smart Noter. It produces a Tauri 2 app that renders the nine prototype screens pixel-perfect, with mock data persisted in SQLite, but without real audio capture, transcription, summarization or cloud calls. Those are explicit responsibilities of sub-projects 2–6.

---

## 1. Goal, Scope & Acceptance Criteria

### 1.1 Goal

Ship a **functional, navigable** Tauri 2 application that:

- Renders the nine prototype screens pixel-perfect against `handoff/smart-noter/project/screenshots/`.
- Persists data (meetings, participants, actions, settings) in SQLite, seeded on first launch from the prototype's mock data.
- Has a working light/dark theme switch, accent color picker, and ES/EN language toggle, all persisted across restarts.
- Ships a complete primitives library (Button, Card, Chip, Icon, Input, SearchBox, SegmentedControl, Toggle, Modal, Toast, Avatar) used by every feature screen, with Storybook stories and unit tests for each.
- Is the working substrate every later sub-project builds on. No screen, no command, no IPC pattern that lands in Foundation will need to be reworked when Sub-projects 2–8 land — they only extend it.

### 1.2 In scope

1. **Tauri shell** with custom Windows 11 titlebar (`decoration: false`), drag region on the title, working min/max/close controls.
2. **Nine pixel-perfect screens**: Dashboard, Meetings List, Pre-Record, Live Recording, Meeting Detail (4 tabs: Summary, Transcript, Actions, Audio + side panel with Participants + AI chat), Templates Gallery, Participants, Settings (including the full provider selector with Local, OpenAI, Azure, and Custom forms — all reactive but disconnected from real APIs), Export Modal.
3. **Full primitives library** under `src/components/primitives/`: Button, Card, Chip, Icon (all prototype icons), Input, SearchBox, SegmentedControl, Toggle, Modal, Toast, Avatar (SubjectAvatar + AvatarStack). Plus domain components: TemplateIcon, MeetingRow, DevicePill, EqBar, LevelBar, Waveform, LivePill, Skeleton.
4. **Light/dark theme** with working switch, accent color configurable across 5 colors, persisted in SQLite.
5. **ES/EN i18n** with react-i18next + ICU, language switch without reload.
6. **Routing** with React Router DOM, `React.lazy()` per screen, suspense fallbacks with skeleton screens.
7. **SQLite + sqlx**: migration `0001_init.sql` applied on first launch; idempotent seed with mock data if DB empty.
8. **Typed IPC**: commands `list_meetings`, `get_meeting`, `update_meeting_title`, `toggle_action`, `rename_participant`, `list_templates`, `set_default_template`, `list_audio_devices`, `get_settings`, `update_settings`, `log_frontend_error` — TypeScript bindings auto-generated via `tauri-specta`.
9. **Prototype animations**: live waveform, dashboard EQ bars, recording timer, model download progress bar — all client-side animations with no backend dependency.
10. **Error handling**: global `ErrorBoundary`, `tracing` logging to `$APPDATA/SmartNoter/logs/`, frontend errors logged via `log_frontend_error` command.

### 1.3 Out of scope (explicit non-goals for Foundation)

- ❌ Real audio capture (Sub-2)
- ❌ Real transcription (Sub-3)
- ❌ AI summary / RAG chat / automated action extraction (Sub-5)
- ❌ Real calls to OpenAI / Azure / Custom APIs (Sub-6) — "Test connection" returns mock "OK" after 900 ms
- ❌ Real export to MD / PDF / MP3 (Sub-7) — modal opens, final button closes without generating any file
- ❌ MSI / code signing / auto-update (Sub-8)
- ❌ Real Whisper model download — only the visual progress simulation
- ❌ Onboarding / welcome wizard
- ❌ Tweaks panel from prototype (Settings already exposes theme/accent/language)
- ❌ OS notifications, system tray icon, global keyboard shortcuts, drag-and-drop file import
- ❌ Full-text search in transcripts (Sub-4 once real transcripts exist)
- ❌ Telemetry, analytics, crash reporting

### 1.4 Buttons / interactions disabled in Foundation

These render exactly as the prototype but are wired to no-ops with a "Próximamente" / "Coming soon" tooltip:

- "Iniciar grabación" on Pre-Record screen
- "Exportar" final button in Export Modal (modal still opens; cancel still works)
- "Probar conexión" on engine settings (returns mock OK after 900 ms — visual only)
- "Descargar" model on engine settings (animates progress bar but does not download anything)
- "Compartir" on Meeting Detail header
- AI chat input on Meeting Detail (disabled; suggested chips do nothing)
- "Crear plantilla" and "Importar" on Templates Gallery
- "Añadir" on Participants page

### 1.5 Acceptance criteria

**Functional:**

- ✅ App launches; Dashboard renders with no console errors (frontend or Rust log)
- ✅ Navigating between the nine screens works; no broken routes
- ✅ ES↔EN switch updates every text without reload
- ✅ Light↔Dark switch changes the theme with a smooth transition
- ✅ Accent color picker updates the five derived colors live
- ✅ Renaming a participant persists after close + reopen
- ✅ Marking an action as done persists after reload
- ✅ Reopening preserves theme, language, accent
- ✅ Closing the window kills the Tauri process (no zombies in Task Manager)

**Quality:**

- ✅ Vitest passes: ≥90% coverage on `components/primitives/*`, smoke test per feature
- ✅ `cargo test` passes: ≥80% coverage on commands + repos
- ✅ Playwright E2E passes: "navigates the nine screens without console errors"
- ✅ Playwright visual regression: 15 baselines match within ±2% pixel diff
- ✅ Biome check: zero warnings
- ✅ Clippy: zero warnings
- ✅ Storybook: 100% of primitives have at least one story
- ✅ `pnpm tauri:dev` starts in < 5 s; `pnpm tauri:build --debug` completes in < 90 s
- ✅ Zero hardcoded strings in JSX (pre-commit script passes)

---

## 2. Modules and Components

### 2.1 Per-screen deliverables

| Screen | Path | Key new components | Data consumed | Animations |
|---|---|---|---|---|
| **Dashboard** | `/` | `StatRow`, `CaptureStatusCard`, `QuickStartCard`, `MeetingRow` (shared) | `listMeetings()` (top 5), `listAudioDevices()` | EQ bars, pulsing level bar |
| **Meetings List** | `/meetings` | `MeetingFilterChips`, `MeetingsTable` | `listMeetings()`, `listTemplates()` | — |
| **Pre-Record** | `/record/new` | `DeviceCard` (radio grid), `TemplateCard` (radio grid), `AudioPreviewCard`, `AdvancedToggles` | `listAudioDevices()`, `listTemplates()` | EQ bars, level bar |
| **Live Recording** | `/record/live/:sessionId` | `LivePill`, `LiveTimer`, `Waveform`, `LiveControls`, `LiveMetaBar` | session meta from state | Waveform animated, pulsing dot, running timer |
| **Meeting Detail** | `/meetings/:id` | `MeetingDetailHeader`, `TabsBar`, four tabs (`SummaryTab`, `TranscriptTab`, `ActionsTab`, `AudioTab`), `ParticipantsBlock`, `AiChatPanel` | `getMeeting(id)` | Static waveform in AudioTab |
| **Templates Gallery** | `/templates` | `TemplateGalleryCard`, `FeatureList` | `listTemplates()`, `featuresFor(id, lang)` | — |
| **Participants** | `/participants` | `ParticipantsTable` | `listParticipantsGlobal()` (grouped by name across meetings) | — |
| **Settings** | `/settings` | `SettingGroup`, `SettingRow`, `EngineProviderGrid`, `LocalProviderConfig`, `OpenAIProviderConfig`, `AzureProviderConfig`, `CustomProviderConfig`, `ModelList`, `HardwareGrid`, `LocalStatusBanner` | `getSettings()`, `updateSettings()` | Model download (mock) animation |
| **Export Modal** | overlay on Meeting Detail | `ExportModal`, `ExportFormatRow` | current meeting | — |

### 2.2 Primitives library (`src/components/primitives/`)

Each primitive ships with **four mandatory files**: `*.tsx`, `*.module.css`, `*.test.tsx`, `*.stories.tsx`.

| Primitive | Key props | Notes |
|---|---|---|
| `Button` | `variant: 'default' \| 'primary' \| 'ghost' \| 'danger'`, `size: 'sm' \| 'md' \| 'icon'`, `icon?`, `disabled`, `loading?` | Maps `.btn`, `.btn-primary`, `.btn-ghost`, `.btn-icon` |
| `Card` | `padding?: boolean`, `children` | Maps `.card`, `.card-pad` |
| `Chip` | `variant: 'default' \| 'accent'`, `icon?`, `children` | Maps `.chip`, `.chip-accent` |
| `Icon` | `name: IconName` (union of `keyof typeof ICONS`), `size?`, `stroke?`, `fill?` | Migrated from `icons.jsx`. `IconName` is generated by TS from the icons object. |
| `Input` | `value`, `onChange`, `type?`, `placeholder?`, `prefix?`, `suffix?` | Supports password + eye toggle |
| `SearchBox` | `value`, `onChange`, `placeholder?` | Maps `.search-box` (composes Icon + Input) |
| `SegmentedControl` | `value`, `options: {value, label}[]`, `onChange` | Maps `.segmented` |
| `Toggle` | `on: boolean`, `onChange` | Maps `.toggle`, `.toggle.on` |
| `Modal` | `open`, `onClose`, `title`, `subtitle?`, `children`, `footer?` | Maps `.modal-backdrop`, `.modal`, `.modal-head`, `.modal-body`, `.modal-foot` |
| `Toast` | wraps `sonner` with Fluent styles | — |
| `Avatar` (`SubjectAvatar` + `AvatarStack`) | `participant`, `size?` / `participants[]`, `max?`, `size?` | Maps `.avatar`, `.avatar-stack`, colors `s-color-1..8` |

### 2.3 Domain components (`src/components/domain/`)

Reusable across screens. Tests mandatory, stories optional.

| Component | Used in |
|---|---|
| `TemplateIcon` | Dashboard, Meetings List, Meeting Detail, Templates, Live Recording |
| `MeetingRow` | Dashboard, Meetings List |
| `DevicePill` | Dashboard (capture status), Live Recording (meta) |
| `EqBar` | Dashboard, Pre-Record, Live Recording |
| `LevelBar` | Dashboard, Pre-Record |
| `Waveform` | Live Recording, Meeting Detail (AudioTab) |
| `LivePill` | Live Recording header |
| `Skeleton` | Suspense fallbacks |

### 2.4 Shell components (`src/components/shell/`)

| Component | Notes |
|---|---|
| `WindowChrome` | Custom Win11 titlebar. Drag region on `.win-title` (excludes controls). Min/Max/Close call `getCurrentWebviewWindow().minimize()` / `toggleMaximize()` / `close()` from the Tauri webview API. |
| `Sidebar` | Brand mark + "Nueva grabación" CTA + two nav sections (Workspace, Tools) + footer with user "Carlos Rivera". Active item based on `useLocation()`. |

### 2.5 IPC contract — commands shipped in Foundation

Each Rust command has a `#[tauri::command]` + `#[specta::specta]` annotation, generates a TypeScript binding via `tauri-specta`, and is wrapped in `src/ipc/commands.ts`.

```rust
// src-tauri/src/commands/meetings.rs
#[tauri::command]
#[specta::specta]
pub async fn list_meetings(state: State<'_, AppState>) -> Result<Vec<MeetingSummary>, AppError>;

#[tauri::command]
#[specta::specta]
pub async fn get_meeting(state: State<'_, AppState>, id: String) -> Result<MeetingDetail, AppError>;

#[tauri::command]
#[specta::specta]
pub async fn update_meeting_title(
    state: State<'_, AppState>,
    id: String,
    title_es: String,
    title_en: Option<String>
) -> Result<(), AppError>;

#[tauri::command]
#[specta::specta]
pub async fn toggle_action(state: State<'_, AppState>, action_id: String) -> Result<bool, AppError>;

#[tauri::command]
#[specta::specta]
pub async fn rename_participant(
    state: State<'_, AppState>,
    participant_id: String,
    name: Option<String>
) -> Result<(), AppError>;
```

```rust
// src-tauri/src/commands/templates.rs
#[tauri::command] pub async fn list_templates(state: State<'_, AppState>) -> Result<Vec<Template>, AppError>;
#[tauri::command] pub async fn set_default_template(state: State<'_, AppState>, id: String) -> Result<(), AppError>;
```

```rust
// src-tauri/src/commands/devices.rs (mock in Foundation; replaced by real WASAPI in Sub-2)
#[tauri::command] pub async fn list_audio_devices() -> Result<Vec<AudioDevice>, AppError>; // hardcoded from AUDIO_DEVICES seed
```

```rust
// src-tauri/src/commands/settings.rs
#[tauri::command] pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, AppError>;
#[tauri::command] pub async fn update_settings(state: State<'_, AppState>, settings: AppSettings) -> Result<(), AppError>;
```

```rust
// src-tauri/src/commands/log.rs
#[tauri::command]
#[specta::specta]
pub fn log_frontend_error(
    level: String,
    message: String,
    stack: Option<String>
) -> Result<(), AppError>;
```

> All commands return `Result<T, AppError>` per the architecture spec contract — even fire-and-forget logging, to preserve a uniform error path in the frontend wrappers.

No backend events are emitted in Foundation — `src/ipc/events.ts` exists but is empty. The audio level event registry lands with Sub-2.

### 2.6 Mocks → seed flow

```
handoff/smart-noter/project/data.js (MEETINGS, TEMPLATES, AUDIO_DEVICES)
        │
        │  scripts/extract-mocks.mjs (run once during initial setup)
        ▼
src-tauri/crates/db/seed_data.json (committed)
        │
        │  src-tauri/crates/db/src/seed.rs
        │     deserialize → INSERT
        ▼
$APPDATA/SmartNoter/db.sqlite
        │
        │  sqlx commands invoked via Tauri commands
        ▼
React UI (RTK Query cache)
```

- The seed function checks if the `meetings` table is empty. If yes, it inserts all six meetings + their participants, actions, transcripts, decisions, blockers.
- Idempotent: re-running on a populated DB is a no-op.
- `seed_data.json` is the single source of truth for mock data in Foundation. The original `data.js` from the handoff is not read at runtime.

### 2.7 Animations

| Animation | Implementation | Trigger |
|---|---|---|
| EQ bars (dashboard, pre-record, live) | `@keyframes eq` (from `app.css`) | always on |
| Live waveform | `@keyframes wave` + random per-bar delay | always on during recording, paused state freezes |
| Recording dot | `@keyframes pulse` | always on |
| Live timer | `setInterval(1000)` in `useLiveTimer` with `useEffect` cleanup | start on mount, pause on pause |
| Pulsing level bar | width animated via `requestAnimationFrame`, sinusoidal 40–80% over 2s | useEffect mount/unmount |
| Model download progress | `setInterval(80ms)` increments `progress` until 100, marks model installed | click on "Descargar" button |
| Theme transition | `transition: background 180ms, color 180ms` global | change of `data-theme` attr on `<html>` |

---

## 3. File Structure and Dependencies

### 3.1 Frontend tree (`src/`)

```
src/
├── main.tsx
├── App.tsx
├── App.module.css
├── vite-env.d.ts
├── ErrorBoundary.tsx
│
├── theme/
│   ├── tokens.module.css                # light/dark/accent CSS vars
│   ├── ThemeProvider.tsx                # applies data-theme + accent to <html>
│   ├── accent.ts                        # applyAccent (from app.jsx)
│   └── theme.test.ts
│
├── i18n/
│   ├── index.ts                         # i18next.init() + ICU + detect from settings
│   ├── locales/{es,en}.json             # migrated from I18N
│   ├── keys.ts                          # GENERATED type TKey
│   ├── useT.ts
│   └── useT.test.ts
│
├── store/
│   ├── index.ts                         # configureStore + setupListeners
│   ├── hooks.ts                         # typed useAppDispatch/useAppSelector
│   ├── slices/{ui,session}.slice.ts (+ .test.ts)
│   └── api/
│       ├── base.ts                      # tauriBaseQuery
│       ├── {meetings,templates,devices,settings}.api.ts
│
├── ipc/
│   ├── bindings.ts                      # GENERATED by tauri-specta
│   ├── commands.ts                      # typed wrappers + error mapping
│   ├── events.ts                        # listener registry (empty in Foundation)
│   └── error.ts                         # AppError → i18n message mapper
│
├── router/
│   ├── routes.tsx                       # tree with React.lazy()
│   ├── paths.ts                         # typed paths
│   └── NotFoundRedirect.tsx
│
├── components/
│   ├── primitives/
│   │   ├── Button/ {tsx, module.css, test.tsx, stories.tsx}
│   │   ├── Card/ (×4)
│   │   ├── Chip/ (×4)
│   │   ├── Icon/ {tsx, icons.ts, module.css, test.tsx, stories.tsx}
│   │   ├── Input/ (×4)
│   │   ├── SearchBox/ (×4)
│   │   ├── SegmentedControl/ (×4)
│   │   ├── Toggle/ (×4)
│   │   ├── Modal/ (×4)
│   │   ├── Toast/ (×4)
│   │   └── Avatar/ {SubjectAvatar.tsx, AvatarStack.tsx, module.css, test.tsx, stories.tsx}
│   ├── shell/
│   │   ├── WindowChrome/ (×4 + drag region)
│   │   └── Sidebar/ (×4)
│   └── domain/
│       ├── {TemplateIcon, MeetingRow, DevicePill, EqBar, LevelBar, Waveform, LivePill, Skeleton}/ (×3: tsx, module.css, test.tsx)
│
├── features/
│   ├── dashboard/
│   │   ├── DashboardPage.{tsx,module.css,test.tsx}
│   │   └── components/{StatRow, CaptureStatusCard, QuickStartCard}/
│   ├── meetings-list/{MeetingsListPage.*, components/MeetingFilterChips/}
│   ├── pre-record/{PreRecordPage.*, components/{DeviceCard, TemplateCard, AudioPreviewCard, AdvancedToggles}/}
│   ├── live-recording/
│   │   ├── LiveRecordingPage.*
│   │   ├── useLiveTimer.ts (+ .test.ts)
│   │   └── components/{LiveTimer, LiveControls, LiveMetaBar}/
│   ├── meeting-detail/
│   │   ├── MeetingDetailPage.*
│   │   ├── tabs/{SummaryTab, TranscriptTab, ActionsTab, AudioTab}.{tsx,module.css}
│   │   ├── side/{SidePanel, ParticipantsBlock, AiChatPanel}.{tsx,module.css}
│   │   └── ExportModal/{ExportModal, ExportFormatRow}.{tsx,module.css,test.tsx}
│   ├── templates/{TemplatesPage.*, components/TemplateGalleryCard/}
│   ├── participants/ParticipantsPage.{tsx,module.css,test.tsx}
│   └── settings/
│       ├── SettingsPage.{tsx,module.css,test.tsx}
│       └── sections/
│           ├── {AudioCaptureSection, TranscriptionEngineSection, PrivacySection}.{tsx,module.css}
│           └── providers/
│               ├── {LocalProviderConfig, OpenAIProviderConfig, AzureProviderConfig, CustomProviderConfig}.{tsx,module.css}
│               └── {ModelList, HardwareGrid, LocalStatusBanner}.{tsx,module.css}
│
├── utils/
│   ├── format.ts (+ .test.ts)
│   └── hooks/{useDebounce.ts (+ .test.ts), useElementSize.ts, useEventListener.ts}
│
└── styles/{reset.css, globals.css, window-bg.module.css}
```

### 3.2 Backend tree (`src-tauri/`)

```
src-tauri/
├── Cargo.toml                           # [workspace] members
├── tauri.conf.json
├── build.rs
├── icons/
├── capabilities/default.json
│
├── crates/
│   ├── core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs                 # AppError + Specta + serde
│   │       ├── models/{meeting, participant, action, template, audio_device, settings}.rs
│   │       └── lang.rs                  # Bilingual<T> helper
│   │
│   ├── db/
│   │   ├── Cargo.toml
│   │   ├── migrations/0001_init.sql
│   │   ├── seed_data.json               # generated by scripts/extract-mocks.mjs
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── connection.rs            # init_pool + run migrations
│   │       ├── seed.rs                  # seed_if_empty
│   │       └── repos/{meetings, participants, actions, templates, settings}_repo.rs
│   │
│   ├── audio/                           # SKELETON
│   ├── whisper/                         # SKELETON
│   └── providers/                       # SKELETON
│
└── src/                                 # binary crate
    ├── main.rs
    ├── state.rs                         # AppState { pool: SqlitePool, ... }
    ├── error.rs                         # impl IntoResponse for AppError
    ├── commands/{mod, meetings, templates, devices, settings, log}.rs
    ├── events/mod.rs                    # empty registry in Foundation
    └── specta_export.rs                 # writes bindings.ts to ../../src/ipc/
```

### 3.3 Root files

| File | Purpose |
|---|---|
| `package.json` | scripts: `dev`, `build`, `test`, `lint`, `storybook`, `tauri:dev`, `tauri:build`, `extract-mocks`, `generate:i18n-keys`, `generate:bindings`, `check:hardcoded-strings`, `test:e2e`, `test:e2e:update` |
| `pnpm-workspace.yaml` | declares `src/` (no internal workspaces in Foundation; reserved for future) |
| `tsconfig.json` | strict: true, alias `@/*` → `src/*` |
| `vite.config.ts` | React plugin + Tauri plugin + CSS Modules + alias |
| `biome.json` | linter + formatter config |
| `lefthook.yml` | pre-commit: biome check, vitest --run --changed, cargo fmt --check, hardcoded strings check |
| `.gitignore` | node_modules, dist, target, *.local |
| `.github/workflows/ci.yml` | jobs: frontend, backend, e2e |
| `playwright.config.ts` | launches `tauri build --debug` as webServer |
| `vitest.config.ts` | jsdom env, setupFiles for i18n + redux |
| `scripts/extract-mocks.mjs` | parses `handoff/.../data.js` → `seed_data.json` |
| `scripts/check-no-hardcoded-strings.mjs` | pre-commit grep guard |
| `scripts/generate-i18n-keys.mjs` | generates `src/i18n/keys.ts` from `es.json` |
| `scripts/check-stories-coverage.mjs` | enforces story per primitive |
| `README.md` | quickstart, scripts, screenshots |
| `CHANGELOG.md` | `0.1.0 — Foundation` |

### 3.4 `package.json` dependencies

```json
{
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
    "@testing-library/react": "^16.0.0",
    "@testing-library/user-event": "^14.5.0",
    "@types/react": "^18.3.0",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.0",
    "jsdom": "^25.0.0",
    "lefthook": "^1.7.0",
    "msw": "^2.4.0",
    "typescript": "^5.6.0",
    "vite": "^5.4.0",
    "vitest": "^2.1.0"
  },
  "packageManager": "pnpm@9.12.0"
}
```

### 3.5 Workspace `Cargo.toml`

```toml
[workspace]
resolver = "2"
members = ["src", "crates/core", "crates/db", "crates/audio", "crates/whisper", "crates/providers"]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.80"

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
```

### 3.6 Binary crate `Cargo.toml` (`src-tauri/src/Cargo.toml`)

```toml
[package]
name = "smart-noter"
version.workspace = true
edition.workspace = true

[build-dependencies]
tauri-build.workspace = true

[dependencies]
tauri.workspace = true
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

smart-noter-core = { path = "../crates/core" }
smart-noter-db = { path = "../crates/db" }
smart-noter-audio = { path = "../crates/audio" }
smart-noter-whisper = { path = "../crates/whisper" }
smart-noter-providers = { path = "../crates/providers" }
```

### 3.7 `tauri.conf.json` (excerpt)

```jsonc
{
  "productName": "Smart Noter",
  "version": "0.1.0",
  "identifier": "com.smartnoter.app",
  "app": {
    "windows": [{
      "title": "Smart Noter",
      "width": 1440,
      "height": 900,
      "minWidth": 1100,
      "minHeight": 700,
      "center": true,
      "decorations": false,
      "transparent": false,
      "resizable": true
    }],
    "security": {
      "csp": "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; connect-src 'self' ipc: http://ipc.localhost"
    }
  },
  "bundle": {
    "active": true,
    "targets": ["msi"],
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/icon.ico"]
  }
}
```

### 3.8 Size estimate

| Category | Files | Approx LOC |
|---|---|---|
| Frontend `.tsx` | ~75 | ~4500 |
| Frontend `.module.css` | ~70 | ~3500 (decomposition of `app.css`) |
| Frontend `.test.tsx` | ~30 | ~1500 |
| Frontend `.stories.tsx` | ~11 | ~400 |
| Frontend utils / i18n / store / ipc / router | ~25 | ~1200 |
| Backend Rust commands | 6 | ~300 |
| Backend Rust repos | 5 | ~400 |
| Backend Rust models / error / state | ~12 | ~500 |
| Backend Rust seed + connection | 3 | ~200 |
| **Total new** | **~240 files** | **~12,500 LOC** |

---

## 4. Validation, Risks, Definition of Done

### 4.1 Validation plan (how every acceptance criterion is verified)

| AC | How it is validated | Where it runs |
|---|---|---|
| App launches without errors | `pnpm tauri:dev` shows no errors in webview console nor Rust log | local + CI |
| Navigates the nine screens | Playwright `e2e/navigation.spec.ts` walks every path, asserts the `data-screen-label` on each page | CI |
| ES↔EN switch | Vitest `i18n.test.tsx` renders App, changes `language`, asserts Sidebar text changes | CI |
| Light↔Dark switch | Vitest `theme.test.tsx` asserts `<html data-theme>` changes and `--accent` is preserved | CI |
| Live accent color | Unit test on `applyAccent()` asserts derived `--accent-hover` and `--accent-soft`; e2e opens Settings, changes color, snapshots | CI + Playwright |
| Rename participant persistence | Playwright: rename → `page.reload()` → assert new name | CI |
| Action toggle persistence | Idem | CI |
| Theme / lang / accent persistence | Playwright: open Settings, change three things, reload, assert preserved | CI |
| Clean shutdown | Manual check at PR end: close window, verify Task Manager has no `smart-noter.exe` (not CI-automatable) | manual reviewer |
| Frontend primitives coverage ≥ 90% | `vitest run --coverage` with threshold in `vitest.config.ts` | CI fails if below |
| Rust coverage ≥ 80% | `cargo llvm-cov --workspace --fail-under-lines 80` | CI fails if below |
| Visual regression ±2% | Playwright `expect(page).toHaveScreenshot({ maxDiffPixelRatio: 0.02 })` | CI |
| Biome zero warnings | `pnpm biome check .` | pre-commit + CI |
| Clippy zero warnings | `cargo clippy --workspace --all-targets -- -D warnings` | pre-commit + CI |
| Storybook 100% primitives | `scripts/check-stories-coverage.mjs` enumerates `components/primitives/*` and asserts each has `.stories.tsx` | CI |
| Dev startup < 5 s | Smoke benchmark at PR end (not reliably automatable) | manual reviewer |
| Build debug < 90 s | Idem | manual reviewer |
| Zero hardcoded strings | `pnpm check:hardcoded-strings` | pre-commit + CI |

### 4.2 Visual regression baselines

The images under `handoff/smart-noter/project/screenshots/` were rendered by the designer's browser (typical resolution ~1280×800). To make the match tractable:

1. Before the first run of visual tests, regenerate baselines by running Playwright against our implementation with `--update-snapshots`. This produces baselines under `tests/e2e/__screenshots__/` specific to the Tauri webview on Windows.
2. The handoff screenshots serve as a **human reference** during development (side-by-side visual comparison), not as automatic baselines — different browsers have small rendering deltas.
3. After regeneration, any PR that changes UI must update baselines explicitly with `pnpm test:e2e:update`.

Mandatory baselines (15 total):

| Source label in handoff | Test name |
|---|---|
| `01 Dashboard` (light + dark) | `dashboard-light.png`, `dashboard-dark.png` |
| `02 Meetings list` | `meetings-list-light.png` |
| `03 Pre-record` | `pre-record-light.png` |
| `04 Live recording` | `live-recording-light.png` |
| `05 Meeting detail` (4 tabs) | `detail-{summary,transcript,actions,audio}-light.png` |
| `06 Templates` | `templates-light.png` |
| `07 Participants` | `participants-light.png` |
| `08 Settings` (4 provider tabs) | `settings-{local,openai,azure,custom}-light.png` |
| Export modal open | `export-modal-light.png` |

### 4.3 Identified risks and mitigations

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| `tauri-specta` still in RC, API changes | Medium | M | Pin exact version + `pnpm generate:bindings` runs before each build. If a bump breaks it, generate `bindings.ts` manually as temporary fallback. |
| Decomposition of `app.css` (40 KB) introduces subtle regressions (specificity, cascade) | High | M | Do the decomposition in an isolated sub-phase at the start of Foundation (step 1 of the plan). After decomposing, capture 9 screenshots from the original prototype HTML and from the Tauri app, compare side-by-side before continuing with logic. |
| Strict CSP blocks Inter / JetBrains Mono fonts | Low | S | Bundle fonts under `src/assets/fonts/` as `.woff2` and load them with `@font-face` from a local CSS. `font-src 'self' data:` covers it. |
| Tauri 2 + Windows 11 custom titlebar (`decoration: false`) loses Snap Layouts / drop shadow | Medium | M | Tauri 2 has the `additional` flag for `WindowEffect::Mica` that restores shadow and mica on Windows 11. Document fallback if it fails. Sub-1 includes a visual check vs Windows 11 native. |
| Visual regression flaky due to font hinting differences across machines | High | S | `maxDiffPixelRatio: 0.02` is already tolerant. If it still flakes, pin visual tests to a single fixed CI runner. |
| Storybook does not compile with Vite + CSS Modules + `@/` alias | Medium | S | Known issue; configure `staticDirs` and `viteFinal` in `.storybook/main.ts`. Documented. |
| `sqlx` macros need an existing DB at compile time | High | M | Use `sqlx::query!` with `SQLX_OFFLINE=true` + checked-in `cargo sqlx prepare`. CI runs `sqlx prepare --check`. |
| `audio` / `whisper` / `providers` skeleton crates fail to compile in Foundation for lack of deps planned for Sub-2/3/6 | Medium | S | Skeletons expose only `pub fn version() -> &'static str`. No conditional dependencies for now. |
| Seed data with bilingual characters (`'`, `—`, `é`) corrupts in JSON serialization | Low | S | Use `serde_json::to_string_pretty` with default `ensure_ascii=false`. Roundtrip ES↔EN tests included. |
| `react-hook-form` + zod overlap with RTK Query mutations (double validation) | Low | S | Convention: zod validates on the client before invoking the mutation; the Rust backend re-validates with `serde` (defense in depth, no duplicated schemas). |

### 4.4 Definition of Done (merge-to-main criteria)

**Foundation is ready to merge when ALL of the following are true:**

#### Functionality

- [ ] The nine screens render without console errors
- [ ] Navigation between all routes works (including back/forward)
- [ ] Prototype animations (waveform, EQ bars, timer, progress) work
- [ ] Reactive forms (theme, accent, language, toggles) work
- [ ] Disabled "Próximamente" interactions show tooltip / disabled correctly
- [ ] Participant rename persists across restart
- [ ] Action toggle persists across restart
- [ ] Settings persist across restart
- [ ] SQLite DB is created at `$APPDATA/SmartNoter/db.sqlite` on first launch
- [ ] Mock seed runs only when DB is empty (idempotent)

#### Quality

- [ ] `pnpm biome check` passes with zero warnings
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `pnpm test` (Vitest) passes with ≥90% coverage on frontend primitives
- [ ] `cargo test --workspace` passes with ≥80% coverage on backend
- [ ] `pnpm test:e2e` passes the nine navigation tests + 15 visual regression baselines
- [ ] No `// TODO`, `// FIXME`, `// XXX` in merged code
- [ ] No hardcoded strings (pre-commit script passes)
- [ ] Storybook builds with zero errors; every primitive has ≥1 story

#### Documentation

- [ ] `README.md` with: requirements, quickstart, scripts, screenshots of the nine screens
- [ ] Comments only where the "why" is non-obvious (system-prompt rule)
- [ ] Initial `CHANGELOG.md` with entry `0.1.0 — Foundation`
- [ ] This spec linked in README

#### Manual review at end of Foundation

- [ ] Reviewer opens the app on Windows 11, navigates the nine screens, visually confirms vs `handoff/screenshots/`
- [ ] Reviewer changes theme, accent, language, closes and reopens — verifies persistence
- [ ] Reviewer closes the window, verifies in Task Manager that `smart-noter.exe` is gone
- [ ] Reviewer runs `pnpm tauri:build --debug` and opens the generated MSI, verifies it launches

### 4.5 Out of scope, restated

These are not Foundation responsibilities and should not be suggested in code review:

- ❌ Onboarding / welcome wizard
- ❌ Tweaks panel from prototype (Settings exposes theme/accent/language)
- ❌ OS notifications (Sub-2 if applicable)
- ❌ System tray icon
- ❌ Global keyboard shortcuts (e.g. F9 to start recording)
- ❌ Drag & drop import of audio files (Sub-2)
- ❌ Full-text search of transcripts (Sub-4, once real transcripts exist)
- ❌ Telemetry / analytics
- ❌ Sentry / crash reporting

### 4.6 Next step after this spec is approved

The next skill in the pipeline is **`superpowers:writing-plans`**, which will take this spec and produce a step-by-step implementation plan (with verification commands between steps) under `docs/superpowers/plans/2026-05-17-foundation-plan.md`.

Because the working directory is **not currently a git repo** (`git status` is unavailable), the implementation plan will include `git init` as Step 0 and commit this spec and the architecture spec before any code lands.
