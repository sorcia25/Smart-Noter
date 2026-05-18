# Smart Noter — Architecture Spec

**Date:** 2026-05-17
**Status:** Approved
**Scope:** Cross-cutting architecture for the Smart Noter desktop application. Decomposes the product into 8 implementation sub-projects and defines the tech stack, project structure, and cross-cutting decisions (IPC, schema, state, routing, i18n, CSS, security, testing, CI) shared by all of them.

> Each sub-project gets its own spec → plan → implementation cycle. Only **Sub-project 1 — Foundation** will be specced immediately after this document is approved.

---

## 1. Goal, Scope & Sub-projects

### 1.1 Goal

Build **Smart Noter** as a native Windows desktop application for meeting capture, transcription, and AI summarization, with **100% local processing by default** (Whisper on the user's GPU/CPU) and optional cloud transcription providers (OpenAI, Azure, custom OpenAI-compatible endpoints).

The product is a faithful, pixel-perfect implementation of the HTML/CSS/JS prototype in `handoff/smart-noter/project/` (Windows 11 Fluent design language).

### 1.2 Non-goals

- **Not multi-platform** — Windows 10/11 only (WASAPI loopback is Windows-specific).
- **Does not replace Teams/Zoom** — Smart Noter listens to system audio, it does not connect to or integrate with conferencing apps.
- **No cloud sync of meetings** — all meetings are stored locally. Cloud providers are used only for STT (when chosen), not for storage.
- **Single-user only** — personal desktop app, no multi-tenancy.
- **No mobile, no web** version.

### 1.3 Constraints

- **Privacy first:** audio never leaves the device in Local mode. Cloud API keys are encrypted with Windows DPAPI under the user's profile.
- **Pixel-perfect parity** with the prototype: each completed screen is validated against the corresponding screenshot in `handoff/smart-noter/project/screenshots/`.
- **i18n ES/EN obligatory** from day 1 — full parity with the prototype's `I18N` dictionary.
- **NVIDIA GPU support** (CUDA) with CPU fallback for transcription (Sub-3).
- **Bundle size:** final MSI < 30 MB (excluding Whisper model weights, which are downloaded on demand from HuggingFace).

### 1.4 Sub-project map

| # | Sub-project | Verifiable deliverable | Effort |
|---|---|---|---|
| **1** | **Foundation** | Tauri shell + 9 pixel-perfect screens with mock data, navigation, theming, i18n, primitives library | M (initial session) |
| 2 | **Audio Capture** | Devices detected, WASAPI loopback running, `.wav` file generated on stop | M |
| 3 | **Whisper Local** | Model selection/download, transcribes `.wav` → text with timestamps + basic diarization | L |
| 4 | **Persistence** | SQLite schema with migrations, CRUD for meetings/participants/actions, audio file storage | S |
| 5 | **AI Summary + Chat** | Template-based summary generation, action/decision extraction, RAG chat over a transcript | L |
| 6 | **Cloud Providers** | OpenAI/Azure/Custom as alternative STT engines, DPAPI-encrypted key storage | M |
| 7 | **Export** | MD, PDF, MP3 export from the detail screen | S |
| 8 | **Distribution** | MSI bundler + code signing + auto-update via GitHub Releases | S |

Sub-projects 2–8 are listed here so their dependencies and scope are visible; they will each be brainstormed and specced separately when their turn comes.

---

## 2. Tech Stack

### 2.1 Frontend (webview)

| Layer | Choice | Version | Rationale |
|---|---|---|---|
| Build tool | **Vite** | ^5.4 | Fast HMR, official Tauri plugin |
| Language | **TypeScript** | ^5.6 | End-to-end types via `tauri-specta` |
| UI library | **React** | ^18.3 | Matches prototype, ecosystem |
| Routing | **React Router DOM** | ^6.27 | Declarative routes, lazy loading |
| Global state | **Redux Toolkit** + **RTK Query** | ^2.3 | Typed, slices, caching of Tauri queries |
| Local ephemeral state | `useState` / `useReducer` | native | UI only |
| Styles | **CSS Modules** | native | Decomposed from `app.css` per component |
| Icons | **Inline SVG** (from prototype) | — | No extra lib; tree-shakeable |
| i18n | **react-i18next** + **i18next-icu** | ^15 / ^2 | ICU MessageFormat (plurals, gender) |
| Forms | **react-hook-form** + **zod** | ^7 / ^3 | Typed validation (settings, API keys) |
| Dates | **date-fns** | ^4 | Tree-shakeable, native i18n |
| Notifications | **sonner** | ^1.5 | Minimalist toast compatible with Fluent |
| Charts (Sub-3+) | **Recharts** | ^2.13 | "Talk time", metrics |
| Unit tests | **Vitest** + **React Testing Library** + **MSW** | — | Unit + component with mocked IPC |
| E2E | **Playwright** | ^1.48 | Webview tests inside Tauri |
| Component catalog | **Storybook** + Vite addon | ^8.3 | Catalog of Fluent primitives |

### 2.2 Backend (Rust / Tauri 2)

| Layer | Crate / choice | Version | Rationale |
|---|---|---|---|
| Shell | **Tauri** | ^2.1 | Stable since Oct 2024, improved IPC |
| HTTP/serial | **reqwest** + **serde** + **serde_json** | ^0.12 / ^1 | Cloud API calls |
| DB | **sqlx** (SQLite) | ^0.8 | Async, built-in migrations, compile-time SQL check |
| Shared types | **tauri-specta** + **specta-typescript** | ^2 / ^0.0.7 | Generates `bindings.ts` from Rust commands |
| Audio (Sub-2) | **cpal** + **wasapi** | ^0.15 / ^0.16 | Windows loopback, multi-device |
| Whisper (Sub-3) | **whisper-rs** (with `cuda` feature) | ^0.13 | whisper.cpp binding with GPU |
| VAD (Sub-3) | **voice_activity_detector** | ^0.2 | Silero ONNX for basic diarization |
| Key encryption (Sub-6) | **windows** crate (DPAPI) | ^0.58 | Encrypts API keys with Windows profile |
| Logging | **tracing** + **tracing-subscriber** | ^0.1 / ^0.3 | Structured logging |
| Errors | **thiserror** + **anyhow** | ^1 / ^1 | Typed (lib) + dynamic (app) errors |
| Async runtime | **tokio** (multi-thread) | ^1.41 | Tauri 2 default |
| Workspace | **cargo workspace** | — | Separate crates: `core`, `db`, `audio`, `whisper`, `providers` |
| Rust tests | **cargo test** + **mockall** + **rstest** | — | Unit + parameterized |

### 2.3 Tooling / DevEx

| Tool | Use |
|---|---|
| **pnpm** | Package manager |
| **Biome** | Frontend linter + formatter (replaces ESLint + Prettier) |
| **rustfmt** + **clippy** | Rust formatter + lints |
| **lefthook** | Git hooks (lint + test pre-commit) |
| **GitHub Actions** | CI: Windows build matrix, tests, code signing |
| **conventional-commits** | Commit message convention for auto-changelog |

### 2.4 Distribution

- **MSI installer** via Tauri bundler
- **Code signing** with an EV certificate (procured in Sub-8)
- **Auto-update** via Tauri updater plugin + GitHub Releases
- **Whisper models** downloaded separately (not in bundle) from HuggingFace

---

## 3. Project Structure

### 3.1 Top-level

```
smart-noter/
├── .github/
│   └── workflows/
│       ├── ci.yml                 # Windows build matrix, tests, biome, clippy
│       └── release.yml            # tag → MSI build + code-sign + upload
├── docs/
│   └── superpowers/
│       ├── specs/                 # specs per sub-project (this file lives here)
│       └── plans/                 # implementation plans (from writing-plans)
├── handoff/                       # original designer bundle (read-only)
├── src/                           # FRONTEND (Vite root)
├── src-tauri/                     # BACKEND (Cargo workspace root)
├── .gitignore
├── biome.json
├── lefthook.yml
├── package.json                   # pnpm workspace root
├── pnpm-workspace.yaml
├── tsconfig.json
└── README.md
```

### 3.2 Frontend (`src/`) — feature-first

```
src/
├── main.tsx                       # React entrypoint + Redux Provider + Router
├── App.tsx                        # shell (WindowChrome + Sidebar + Outlet)
├── theme/
│   ├── tokens.module.css          # CSS variables (the :root tokens from the prototype)
│   ├── light.module.css
│   ├── dark.module.css
│   └── accent.ts                  # applyAccent function (from app.jsx)
├── i18n/
│   ├── index.ts                   # i18next configuration
│   ├── locales/
│   │   ├── es.json                # migrated from I18N.es
│   │   └── en.json                # migrated from I18N.en
│   └── useT.ts                    # typed hook: useT('navDashboard')
├── store/
│   ├── index.ts                   # configureStore + RTK setupListeners
│   ├── slices/
│   │   ├── ui.slice.ts            # theme, accent, language, tweaks panel
│   │   └── session.slice.ts       # current user
│   └── api/
│       ├── meetings.api.ts        # RTK Query: list/get/create/update
│       ├── templates.api.ts
│       ├── devices.api.ts
│       ├── transcription.api.ts
│       └── base.ts                # custom baseQuery wrapping Tauri commands
├── ipc/
│   ├── bindings.ts                # GENERATED by tauri-specta (do not edit)
│   ├── commands.ts                # typed wrappers (invoke + error mapping)
│   └── events.ts                  # listeners for backend events
├── components/
│   ├── primitives/                # atoms
│   │   ├── Button/                # Button.tsx + .module.css + .test.tsx + .stories.tsx
│   │   ├── Card/
│   │   ├── Chip/
│   │   ├── Icon/                  # from icons.jsx — IconName type
│   │   ├── Input/
│   │   ├── SegmentedControl/
│   │   ├── Toggle/
│   │   └── Avatar/                # SubjectAvatar + AvatarStack
│   ├── shell/
│   │   ├── WindowChrome/          # Windows 11 titlebar
│   │   └── Sidebar/
│   └── domain/
│       ├── TemplateIcon/
│       ├── MeetingRow/
│       ├── DevicePill/
│       ├── EqBar/                 # animated equalizer
│       └── LevelBar/
├── features/
│   ├── dashboard/
│   │   ├── DashboardPage.tsx
│   │   ├── DashboardPage.module.css
│   │   ├── DashboardPage.test.tsx
│   │   └── components/            # StatRow, QuickStart, CaptureStatusCard
│   ├── meetings-list/
│   ├── pre-record/
│   ├── live-recording/
│   ├── meeting-detail/
│   │   ├── MeetingDetailPage.tsx
│   │   ├── tabs/                  # SummaryTab, TranscriptTab, ActionsTab, AudioTab
│   │   ├── side/                  # ParticipantsBlock, AiChatPanel
│   │   └── ExportModal/
│   ├── templates/
│   ├── participants/
│   ├── settings/
│   │   └── sections/              # AudioCapture, TranscriptionEngine, Privacy
│   └── tweaks-panel/              # hidden dev panel from prototype
├── router/
│   ├── routes.tsx                 # route object + lazy imports
│   └── paths.ts                   # typed constants (Paths.MeetingDetail(id))
├── utils/
│   ├── format.ts                  # fmtDuration, fmtDate, pickL
│   └── hooks/                     # useTheme, useDebounce, useElementSize
├── mock/                          # Foundation only: mock data from prototype
│   ├── meetings.ts                # MEETINGS from data.js
│   ├── templates.ts               # TEMPLATES
│   ├── devices.ts                 # AUDIO_DEVICES
│   └── server.ts                  # MSW handlers for tests
├── styles/
│   ├── reset.css
│   ├── globals.css                # body, scrollbars, ::selection
│   └── window-bg.module.css       # mica gradients
└── vite-env.d.ts
```

### 3.3 Backend (`src-tauri/`) — Cargo workspace

```
src-tauri/
├── Cargo.toml                     # [workspace] members
├── tauri.conf.json                # Tauri config (windows, icons, updater)
├── build.rs
├── icons/
├── capabilities/
│   └── default.json               # permissions (FS, audio, etc.)
├── crates/
│   ├── core/                      # shared types + traits + errors
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs           # AppError (thiserror) + IntoResponse
│   │       ├── models/            # Meeting, Participant, Action, Template (derive Specta)
│   │       └── traits/            # AudioCapture trait, Transcriber trait
│   ├── db/                        # SQLite + migrations + queries
│   │   ├── Cargo.toml
│   │   ├── migrations/
│   │   │   └── 0001_init.sql      # schema (extended by Sub-4)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── connection.rs      # sqlx pool
│   │       └── repos/             # MeetingsRepo, ParticipantsRepo, etc.
│   ├── audio/                     # WASAPI + cpal (skeleton in Foundation, filled in Sub-2)
│   ├── whisper/                   # whisper-rs wrapper (skeleton, filled in Sub-3)
│   └── providers/                 # cloud providers (skeleton, filled in Sub-6)
└── src/                           # Tauri binary crate
    ├── main.rs                    # tauri::Builder setup
    ├── commands/                  # typed commands (#[tauri::command])
    │   ├── mod.rs
    │   ├── meetings.rs            # list_meetings, get_meeting, …
    │   ├── templates.rs
    │   ├── devices.rs             # list_audio_devices (mock in Foundation)
    │   ├── settings.rs
    │   └── transcription.rs       # skeleton
    ├── events/
    │   └── mod.rs                 # AudioLevelEvent, TranscriptChunkEvent
    ├── specta_bindings.rs         # exports bindings.ts into src/ipc/
    └── state.rs                   # AppState (Arc<DbPool>, etc.)
```

### 3.4 Conventions

- One component = one folder with `Component.tsx`, `.module.css`, `.test.tsx`, `.stories.tsx` (all four mandatory for `components/primitives/*`, optional for `features/*`).
- Naming: `PascalCase.tsx` for components, `camelCase.ts` for utilities, `kebab-case` for folders.
- Absolute imports via `@/` alias pointing to `src/`.
- No barrel `index.ts` files (better tree-shaking and direct navigation).
- CSS Modules: `.module.css` strict; classes in `camelCase`.

---

## 4. Cross-cutting Decisions

### 4.1 IPC: Commands vs Events vs Channels

| Pattern | When | Examples |
|---|---|---|
| **Command** (`#[tauri::command]`) | One-shot request/response, returns `Result<T, AppError>` | `list_meetings`, `get_meeting`, `start_recording`, `update_settings` |
| **Event** (`emit_to`) | Async push backend→frontend, **N→1** | `audio:level` (60Hz), `transcription:chunk`, `model:download-progress`, `recording:state-changed` |
| **Channel** (Tauri 2 IpcChannel) | Stream **1→1** dedicated to a single invocation | `transcribe_file` with a channel of progress chunks (Sub-3) |

**Rules:**

- Every command returns `Result<T, AppError>` — errors serialize to `{ code: "DEVICE_NOT_FOUND", message: "…", i18nKey: "errors.deviceNotFound" }`.
- Events use a `domain:action` namespace.
- `tauri-specta` exports command types; events are typed via a manual registry (`src/ipc/events.ts`).

### 4.2 SQLite schema — migration `0001_init.sql`

Skeleton for Foundation. Sub-4 extends with additional indexes, audit fields, and any new tables needed for AI/cloud features (e.g. summaries history, chat threads).

```sql
CREATE TABLE meetings (
  id TEXT PRIMARY KEY,                   -- UUID v7
  title_es TEXT NOT NULL,
  title_en TEXT,
  template_id TEXT NOT NULL,
  date TEXT NOT NULL,                    -- ISO8601
  duration_sec INTEGER NOT NULL,
  word_count INTEGER NOT NULL DEFAULT 0,
  device_used TEXT,
  summary_es TEXT,
  summary_en TEXT,
  audio_path TEXT,                       -- relative path under storage root
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE participants (
  id TEXT PRIMARY KEY,
  meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  label TEXT NOT NULL,                   -- "S1", "S2"…
  name TEXT,                             -- nullable
  color_class TEXT NOT NULL,
  word_count INTEGER NOT NULL DEFAULT 0,
  talk_pct INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE actions (
  id TEXT PRIMARY KEY,
  meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  text_es TEXT NOT NULL,
  text_en TEXT,
  owner_participant_id TEXT REFERENCES participants(id) ON DELETE SET NULL,
  due TEXT,                              -- ISO8601 date
  done INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE transcript_lines (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  meeting_id TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
  t_seconds INTEGER NOT NULL,            -- offset from start
  speaker_id TEXT REFERENCES participants(id) ON DELETE SET NULL,
  text_es TEXT NOT NULL,
  text_en TEXT
);

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

CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL                    -- JSON
);
```

**Decisions:**

- Bilingual columns (`*_es`, `*_en`) instead of a `translations` table — simpler and i18n is fixed to ES/EN.
- UUID v7 (time-ordered) for `meetings.id`.
- `ON DELETE CASCADE` on direct children.
- `settings` is key/value JSON — frontend types it with zod.
- Foundation **seeds** the DB with the prototype's mock data on first launch.

### 4.3 State management (Redux Toolkit)

| Slice / API | Responsibility |
|---|---|
| `ui.slice` | `theme`, `accentColor`, `language`, `avatarStyle`, `aiChatVisible`, `tweaksPanelOpen` — persisted to `localStorage` |
| `session.slice` | Current user (mock in Foundation: "Carlos Rivera Pro") |
| `meetings.api` (RTK Query) | `listMeetings`, `getMeeting`, `updateMeeting`, `deleteMeeting`, `toggleAction` |
| `templates.api` | `listTemplates`, `setDefaultTemplate` |
| `devices.api` | `listAudioDevices`, `getDeviceLevel` (via event subscription) |
| `transcription.api` | `getProviderConfig`, `updateProviderConfig`, `testApiKey` |

**Custom `tauriBaseQuery`:** a single function that invokes commands and converts `AppError` to `FetchBaseQueryError` with code + message.

### 4.4 Routing & URLs

```ts
export const Paths = {
  Dashboard: '/',
  Meetings: '/meetings',
  MeetingDetail: (id: string) => `/meetings/${id}`,
  PreRecord: '/record/new',
  LiveRecording: (sessionId: string) => `/record/live/${sessionId}`,
  Templates: '/templates',
  Participants: '/participants',
  Settings: '/settings',
  SettingsSection: (s: 'audio' | 'engine' | 'privacy') => `/settings/${s}`
};
```

- Typed routes object (IDE autocomplete).
- Each feature loaded via `React.lazy()` → per-screen bundles.
- `Outlet` from React Router inside `App.tsx`.
- Suspense boundary with Fluent-styled skeleton screens.
- 404 → redirect to `/`.

### 4.5 i18n with react-i18next + ICU

- `locales/es.json` and `locales/en.json` mirror the prototype's `I18N` dictionary (flat structure).
- ICU for plurals: `"meetingCount": "{count, plural, one {# reunión} other {# reuniones}}"`.
- Typed `useT()` hook: TypeScript validates that the key exists (`t('navDashbord')` ❌).
- Language switch without reload (automatic re-render).
- Hardcoded user-facing strings forbidden — enforced by a pre-commit script (`scripts/check-no-hardcoded-strings.mjs`) that greps `.tsx` files for literal strings inside JSX text nodes that aren't wrapped in `useT()`. Reviewed in code-review for false negatives.

### 4.6 CSS Modules — decomposition strategy

`app.css` (40 KB) decomposed progressively per the table below.

| Source (in `app.css`) | Destination |
|---|---|
| `:root` + `[data-theme="*"]` | `src/theme/tokens.module.css` (global CSS variables) |
| `body`, `*`, `.scroll`, scrollbars | `src/styles/globals.css` |
| `.win-shell`, `.win-window`, `.win-titlebar`, `.win-body` | `WindowChrome.module.css` |
| `.win-sidebar`, `.nav-*`, `.brand`, `.cta-record` | `Sidebar.module.css` |
| `.btn*`, `.chip*` | `Button.module.css`, `Chip.module.css` |
| `.card`, `.card-pad` | `Card.module.css` |
| `.s-color-*`, `.t-color-*`, `.avatar*` | `Avatar.module.css` + `TemplateIcon.module.css` |
| `.meeting-row`, `.meeting-list`, `.stat*`, `.dash-grid`, `.quick-card` | `DashboardPage.module.css` |
| Screen-specific (`.prerec`, `.live-*`, `.detail-*`, `.tmpl-*`, etc.) | Each `*Page.module.css` |
| `.modal*`, `.export-*` | `ExportModal.module.css` |
| `.engine-*`, `.model-*`, `.local-*`, `.key-*`, `.field-*`, `.pill-*`, `.hardware-*` | `TranscriptionEngineSection.module.css` |

**Rule:** zero cross-component overrides. If a parent needs to customize a primitive, it passes props (`variant`, `size`).

**Exception:** `src/styles/globals.css` is a plain global stylesheet (not a module) reserved for browser resets, scrollbars, and `body`/`*` selectors. Likewise `src/theme/tokens.module.css` declares CSS custom properties on `:root` — those leak globally by design and that is the only legitimate use of `:root` outside `globals.css`.

### 4.7 Security & Privacy (Foundation-level)

- **API keys never in Redux state** — only in backend. The `getProviderConfig` command returns whether a key is set + the last 4 chars only. *(This rule activates with Sub-6 — Foundation has no API keys yet, but the contract is established now so the UI doesn't accidentally pull keys into memory later.)*
- **Strict CSP** in `tauri.conf.json`: `default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; connect-src 'self'`.
- **No remote URLs in webview**: all resources bundled (Inter / JetBrains Mono fonts included in `src/assets/fonts/`).
- **Tauri capabilities**: only strictly necessary permissions (FS limited to `$APPDATA/SmartNoter/`, no shell, no direct HTTP from frontend).
- **Logs without PII**: meeting names and transcripts are not logged; only IDs and metrics.

### 4.8 Testing strategy

| Layer | Tool | Foundation target |
|---|---|---|
| Rust unit (commands, mappers) | `cargo test` + `rstest` + `mockall` | 80% |
| Frontend primitives | Vitest + RTL | 90% (`.test.tsx` per primitive) |
| Frontend features (smoke) | Vitest + RTL + MSW | 1 happy path per feature |
| E2E webview | Playwright | 1 test: "navigates the 9 screens without errors" |
| Visual regression | Playwright + screenshot vs `handoff/screenshots/` | 9 captures (one per screen) |
| Storybook stories | mandatory for primitives | 100% of primitives |

### 4.9 CI/CD pipeline (`.github/workflows/ci.yml` — skeleton)

```yaml
on: [push, pull_request]
jobs:
  frontend:
    runs-on: windows-latest
    steps: [checkout, setup-node, pnpm install, biome check, vitest, build]
  backend:
    runs-on: windows-latest
    steps: [checkout, setup-rust, cargo fmt --check, clippy -- -D warnings, cargo test]
  e2e:
    needs: [frontend, backend]
    steps: [tauri build --debug, playwright test]
```

### 4.10 Errors & observability (minimum viable)

- `tracing` with `tracing-subscriber` set up in `main.rs` — rotating file log under `$APPDATA/SmartNoter/logs/`.
- Frontend: a global `ErrorBoundary` captures React errors and emits them to the backend via the `log_frontend_error` command.
- In dev mode: the prototype's hidden "Tweaks" panel includes a "Show logs" button that opens the logs folder.
- **No external telemetry** (privacy first).

---

## 5. Glossary

| Term | Meaning |
|---|---|
| **WASAPI loopback** | Windows Audio Session API mode that captures whatever the system is *playing* (not from a microphone). |
| **Mica** | Windows 11 background material — semi-transparent, picks up desktop tint. |
| **DPAPI** | Windows Data Protection API — encrypts data with the current user's profile. |
| **Diarization** | Speaker segmentation of an audio stream ("who spoke when"). |
| **RAG** | Retrieval-Augmented Generation — answer questions grounded on a corpus (here: a single transcript). |
| **Fluent** | Microsoft's Windows 11 design language. |
| **Specta** | Rust framework that derives TypeScript types from Rust structs/functions. |

---

## 6. Next steps

1. **Architecture spec self-review** (placeholder/contradiction/ambiguity pass).
2. **User reviews this document** and approves or requests changes.
3. **Brainstorm Sub-project 1 — Foundation**, producing its own design spec at `docs/superpowers/specs/YYYY-MM-DD-foundation-design.md`.
4. **Hand off Foundation spec to the `writing-plans` skill** for the actual implementation plan.

Sub-projects 2–8 are not specced in this document. Each will be brainstormed and specced individually when its turn comes.
