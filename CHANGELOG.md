## Changelog

All notable changes follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

### [0.2.0] — Sub-2 Audio Capture — 2026-06-10

#### Added

- Real audio capture via WASAPI loopback (system) + cpal input (mic) + rubato-based mix
- 3 capture modes: System / Mic / Mix
- WAV (default) and FLAC formats; MP3 options remain in Settings as `disabled` placeholders (Sub-7)
- Real-time level meter and waveform driven by Tauri events (`audio:level @20Hz`, `audio:waveform-bin @10Hz`, `audio:elapsed @1Hz`)
- Pause/Resume that omits the paused span from the resulting file
- `StopConfirmModal` with Save (commits meeting + asset) and Discard (deletes tmp)
- New `meeting_assets` table (migration 0002) — 1:N relation, prepared for Sub-3/Sub-7 future assets
- 9 new Tauri commands (`enumerate_audio_devices` reworked, `start/stop_preview`, `start/pause/resume/stop_recording`, `finalize/discard_recording`)
- Global `audio:error` event listener routing to translated Toast in App.tsx
- Startup sweep of orphan `tmp-*` files in `%APPDATA%\com.smartnoter.app\audio\`

#### Changed

- `AudioDevice` shape: `name` is now plain string, `kind` enum replaces `icon: string` (UI derives the icon)
- The Foundation seed for `audio_devices` is no longer used (table orphaned, kept for backward compatibility)
- `SegmentedControl` primitive supports per-option `disabled` with "Próximamente" tooltip

#### Out of scope (still)

- Whisper transcription (Sub-3)
- AI summaries / RAG (Sub-5)
- MP3 export (Sub-7)
- Multiple parallel sessions
- Hot-plug device detection

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
