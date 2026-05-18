## Smart Noter

Windows desktop app for AI-powered meeting notes with 100% local processing by default.

This release (**Foundation, v0.1.0**) covers the full UI, the typed IPC contract, and SQLite persistence. Real audio capture, transcription, AI summarization, and cloud providers ship in subsequent sub-projects.

### Requirements

- Windows 10/11
- Node.js 20+
- pnpm 9.12+
- Rust 1.80+ (install via [rustup](https://rustup.rs))
- Tauri prerequisites: <https://tauri.app/start/prerequisites/>

### Quickstart

```bash
pnpm install
pnpm generate:bindings      # Rust -> src/ipc/bindings.ts
pnpm generate:i18n-keys     # es.json -> src/i18n/keys.ts
pnpm tauri:dev
```

The app opens in a 1440×900 window. SQLite database initializes at `%APPDATA%/com.smartnoter.app/db.sqlite` and is seeded with mock meetings on first launch.

The two `generate:*` scripts produce gitignored files — re-run them after pulling changes that touch Rust commands or the ES dictionary.

### Scripts

| Command | Purpose |
|---|---|
| `pnpm dev` | Vite dev server only (no Tauri shell — for component dev / E2E) |
| `pnpm tauri:dev` | Full app (Tauri + Vite + Rust backend) |
| `pnpm tauri:build` | Build MSI installer (release) |
| `pnpm test` | Vitest watch mode |
| `pnpm test:run` | Run Vitest once |
| `pnpm test:coverage` | Run tests with V8 coverage report |
| `pnpm test:e2e` | Playwright E2E (navigation + persistence) |
| `pnpm test:e2e:update` | Regenerate visual baselines |
| `pnpm lint` | Biome lint + format check |
| `pnpm lint:fix` | Biome auto-fix |
| `pnpm format` | Biome format only |
| `pnpm storybook` | Component catalog (port 6006) |
| `pnpm storybook:build` | Build static Storybook |
| `pnpm extract-mocks` | Regenerate `seed_data.json` from prototype |
| `pnpm generate:i18n-keys` | Refresh `src/i18n/keys.ts` from `es.json` |
| `pnpm generate:bindings` | Refresh `src/ipc/bindings.ts` from Rust |
| `pnpm check:hardcoded-strings` | Forbid hardcoded JSX text (use `useT()`) |
| `pnpm check:stories` | Enforce one `.stories.tsx` per primitive |

### Architecture

See `docs/superpowers/specs/2026-05-17-smart-noter-architecture.md` and `2026-05-17-foundation-design.md` for the full design.

Quick map:

- `src/i18n/` — react-i18next + ICU; ES default, EN fallback; 159 keys
- `src/theme/` — design tokens (`tokens.module.css`), `ThemeProvider`, `applyAccent`
- `src/store/` — Redux Toolkit slice (`ui`) + RTK Query (`base` tauriBaseQuery + per-entity slices)
- `src/router/` — typed `Paths` + lazy-loaded route tree
- `src/ipc/` — typed wrappers around the specta-generated `bindings.ts`
- `src/components/primitives/` — Button, Card, Chip, Icon, Input, SearchBox, SegmentedControl, Toggle, Modal, Avatar
- `src/components/domain/` — TemplateIcon, MeetingRow, DevicePill, EqBar
- `src/components/shell/` — WindowChrome, Sidebar
- `src/features/<screen>/` — Dashboard, MeetingsList, PreRecord, LiveRecording, MeetingDetail (+ tabs/side/ExportModal), Templates, Participants, Settings
- `src-tauri/` — Tauri 2 app + workspace crates (`core` / `db` / `audio` / `providers` / `whisper`)

### Screens (data-screen-label markers)

| Label | Route | Module |
|---|---|---|
| `01 Dashboard` | `/` | `src/features/dashboard` |
| `02 Meetings list` | `/meetings` | `src/features/meetings-list` |
| `03 Pre-record` | `/record/new` | `src/features/pre-record` |
| `04 Live recording` | `/record/live/:sessionId` | `src/features/live-recording` |
| `05 Meeting detail` | `/meetings/:id` | `src/features/meeting-detail` |
| `06 Templates` | `/templates` | `src/features/templates` |
| `07 Participants` | `/participants` | `src/features/participants` |
| `08 Settings` | `/settings` | `src/features/settings` |

### Screenshots

The prototype reference renders live under `handoff/smart-noter/project/screenshots/`. The Foundation UI is a pixel-faithful port of those screens.

### License

MIT
