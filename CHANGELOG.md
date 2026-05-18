## Changelog

All notable changes follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

### [0.1.0] — Foundation — 2026-05-18

#### Added

- Tauri 2 shell with custom Windows 11 titlebar (drag region, min/max/close controls)
- Eight feature screens ported pixel-perfect from the prototype:
  - Dashboard (stats, recent meetings, capture status, quick start)
  - Meetings List (filter chips, search, MeetingRow grid)
  - Pre-Record (device + template selection, advanced toggles)
  - Live Recording (timer, animated waveform, pause/stop)
  - Meeting Detail (Summary / Transcript / Actions / Audio tabs + Participants + AI chat + Export modal)
  - Templates Gallery (with default-template selection)
  - Participants (cross-meeting aggregation)
  - Settings (personalization, audio capture, 4 transcription providers, privacy)
- Primitives library: Button, Card, Chip, Icon, Input, SearchBox, SegmentedControl, Toggle, Modal, Avatar (+ AvatarStack)
- Domain components: TemplateIcon, MeetingRow, DevicePill, EqBar
- Light/dark theme with 5 accent colors, persisted in SQLite
- ES/EN i18n with `react-i18next` + ICU MessageFormat; 159 typed keys
- SQLite persistence (sqlx) with idempotent seed from prototype mock data
- Typed IPC via `tauri-specta` (10 commands round-trip with the React app)
- React Router 6 with lazy-loaded routes per screen
- Redux Toolkit + RTK Query for state and IPC queries
- Storybook catalog of primitives (10 stories) + Fluent theme decorator
- Vitest unit tests (84 passing) + Playwright E2E (navigation + persistence)
- Quality scripts: `check-no-hardcoded-strings` (forbid JSX text outside i18n) and `check-stories-coverage` (one story per primitive)
- GitHub Actions CI: frontend lint/test/build, backend fmt/clippy/test, Playwright E2E on Windows

#### Out of scope (deferred to sub-projects 2–8)

- Real WASAPI loopback audio capture
- Whisper local transcription
- AI summary generation and RAG chat
- Real OpenAI / Azure / Custom STT integrations
- Real export to MD / PDF / MP3
- MSI code signing + auto-update
- Participant rename CRUD endpoints beyond Meeting Detail side panel
