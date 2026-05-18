# Smart Noter Foundation Implementation Plan — Part 3

> **Continuation of [Part 1](./2026-05-17-foundation-plan.md) + [Part 2](./2026-05-17-foundation-plan-part2.md).** Covers Phases 7–11: feature screens, Storybook + quality scripts, E2E + visual regression, CI/CD, final validation.

---

## Phase 7 — Feature screens

Each screen task follows the same shape:

- [ ] **Step 1:** Replace the stub `<PageName>Page.tsx` with the real implementation, porting from `handoff/.../screens-*.jsx` with TypeScript types and CSS Modules.
- [ ] **Step 2:** Extract per-screen CSS from `app.css` into `<PageName>Page.module.css` (and per-sub-component modules).
- [ ] **Step 3:** Write a smoke test that mounts the page in a Router + Provider, asserts core elements render, and asserts navigation triggers fire.
- [ ] **Step 4:** Run `pnpm tauri:dev`, navigate to the route, verify visually against `handoff/screenshots/<N>-*.png`.
- [ ] **Step 5:** Run `pnpm test:run -- src/features/<name>` → pass.
- [ ] **Step 6:** Commit: `feat(<name>): implement <PageName>Page pixel-perfect from prototype`.

**Layout wrapper:** Each feature page is rendered into a shared shell. Update `App.tsx` first to wrap routes in the `<WindowChrome>` + `<Sidebar>` layout. Do this as the first task of Phase 7.

### Task 7.0: App shell layout

**Files:**
- Modify: `src/App.tsx`, `src/App.module.css`

- [ ] **Step 1: Rewrite `src/App.tsx` with shell layout**

```tsx
import { Suspense, useEffect } from 'react';
import { useRoutes, useLocation } from 'react-router-dom';
import { Toaster } from 'sonner';
import { routes } from './router/routes';
import { ThemeProvider } from './theme/ThemeProvider';
import { useAppDispatch, useAppSelector } from './store/hooks';
import { useGetSettingsQuery } from './store/api/settings.api';
import { hydrateFromBackend } from './store/slices/ui.slice';
import { WindowChrome } from './components/shell/WindowChrome/WindowChrome';
import { Sidebar } from './components/shell/Sidebar/Sidebar';
import { useT } from './i18n/useT';
import styles from './App.module.css';

const TITLES_KEY: Record<string, string> = {
  '/': 'navDashboard',
  '/meetings': 'navMeetings',
  '/record/new': 'navRecord',
  '/templates': 'navTemplates',
  '/participants': 'participants',
  '/settings': 'navSettings',
};

export default function App() {
  const dispatch = useAppDispatch();
  const ui = useAppSelector((s) => s.ui);
  const { data: settings } = useGetSettingsQuery();
  const location = useLocation();
  const { t, setLang, lang } = useT();

  useEffect(() => {
    if (settings) {
      dispatch(hydrateFromBackend({
        theme: settings.theme,
        accent: settings.accent,
        language: settings.language as 'es' | 'en',
        avatarStyle: settings.avatarStyle,
        aiChatVisible: settings.aiChatVisible,
      }));
    }
  }, [settings, dispatch]);

  useEffect(() => {
    if (ui.language !== lang) setLang(ui.language);
  }, [ui.language, lang, setLang]);

  const element = useRoutes(routes);
  const titleKey = TITLES_KEY[location.pathname];
  const title = titleKey ? t(titleKey as never) : '';

  return (
    <ThemeProvider theme={ui.theme} accent={ui.accent} avatarStyle={ui.avatarStyle}>
      <div className={styles.shell}>
        <div className={styles.window}>
          <WindowChrome title={title} />
          <div className={styles.body}>
            <Sidebar />
            <main className={styles.main}>
              <Suspense fallback={<div className={styles.suspense}>Loading…</div>}>
                {element}
              </Suspense>
            </main>
          </div>
        </div>
        <Toaster position="bottom-right" />
      </div>
    </ThemeProvider>
  );
}
```

- [ ] **Step 2: Rewrite `src/App.module.css`** (extract `.win-shell`, `.win-window`, `.win-body`, `.win-main` from `app.css:128-256`)

```css
.shell {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: stretch;
  justify-content: stretch;
  padding: 0;
}
.window {
  width: 100%;
  height: 100%;
  background: var(--bg-app);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: var(--shadow-window);
}
.body { flex: 1; display: flex; min-height: 0; }
.main { flex: 1; display: flex; flex-direction: column; min-width: 0; background: var(--bg-app); }
.suspense { display: grid; place-items: center; height: 100%; color: var(--text-muted); font-size: 14px; }
```

- [ ] **Step 3: Run dev — verify shell renders**

```bash
pnpm tauri:dev
```

Expected: Window opens with Win11 titlebar, Sidebar, and "Dashboard — to be implemented" stub in the main area.

- [ ] **Step 4: Commit**

```bash
git add src/App.tsx src/App.module.css
git commit -m "feat: wrap routes with WindowChrome + Sidebar shell layout"
```

---

### Task 7.1: DashboardPage

**Files:**
- Modify: `src/features/dashboard/DashboardPage.tsx`
- Create: `src/features/dashboard/DashboardPage.module.css`, `DashboardPage.test.tsx`, `components/{StatRow,CaptureStatusCard,QuickStartCard}/`
- Source: `handoff/.../screens-dashboard.jsx:23-127`, `app.css` lines 257–434 (search "Dashboard" section)

- [ ] **Step 1: Extract CSS into `DashboardPage.module.css`**

Extract relevant rules from `app.css`: `.win-main`, `.page-header`, `.page-title`, `.page-sub`, `.page-actions`, `.page-scroll`, `.dash-grid`, `.stat-row`, `.stat`, `.stat-label`, `.stat-value`, `.stat-delta`, `.meeting-list`. Convert to camelCase module classes.

- [ ] **Step 2: Create sub-component modules and TSX**

For each sub-component (`StatRow`, `CaptureStatusCard`, `QuickStartCard`), create `<Name>.tsx` + `<Name>.module.css` in `src/features/dashboard/components/<Name>/`.

`StatRow.tsx`:

```tsx
import { useT } from '@/i18n/useT';
import styles from './StatRow.module.css';

interface Stat { label: string; value: string; delta: string; deltaTone?: 'accent' | 'warn'; }

export function StatRow({ stats }: { stats: Stat[] }) {
  return (
    <div className={styles.row}>
      {stats.map((s) => (
        <div key={s.label} className={styles.stat}>
          <div className={styles.label}>{s.label}</div>
          <div className={styles.value}>{s.value}</div>
          <div className={`${styles.delta} ${s.deltaTone === 'warn' ? styles.warn : ''}`}>{s.delta}</div>
        </div>
      ))}
    </div>
  );
}
```

(Port the EQ-bar capture card and template quick-start cards similarly from the prototype.)

- [ ] **Step 3: Rewrite `DashboardPage.tsx`**

Port the `Dashboard` component from `screens-dashboard.jsx:23-127`. Replace mock `MEETINGS`, `AUDIO_DEVICES`, `TEMPLATES` references with RTK Query hooks:

```tsx
import { useNavigate } from 'react-router-dom';
import { useT } from '@/i18n/useT';
import { useListMeetingsQuery } from '@/store/api/meetings.api';
import { useListAudioDevicesQuery } from '@/store/api/devices.api';
import { useListTemplatesQuery } from '@/store/api/templates.api';
import { Paths } from '@/router/paths';
import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { SearchBox } from '@/components/primitives/SearchBox/SearchBox';
import { MeetingRow } from '@/components/domain/MeetingRow/MeetingRow';
import { StatRow } from './components/StatRow/StatRow';
import { CaptureStatusCard } from './components/CaptureStatusCard/CaptureStatusCard';
import { QuickStartCard } from './components/QuickStartCard/QuickStartCard';
import styles from './DashboardPage.module.css';

export default function DashboardPage() {
  const navigate = useNavigate();
  const { t, lang } = useT();
  const { data: meetings = [] } = useListMeetingsQuery();
  const { data: devices = [] } = useListAudioDevicesQuery();
  const { data: templates = [] } = useListTemplatesQuery();

  const totalHours = (meetings.reduce((s, m) => s + m.durationSec, 0) / 3600).toFixed(1);
  const totalWords = meetings.reduce((s, m) => s + (m.wordCount ?? 0), 0);

  const stats = [
    { label: t('statTotal'), value: String(meetings.length), delta: `+3 ${t('thisWeek')}` },
    { label: t('statHours'), value: `${totalHours}h`, delta: `+2.4 ${t('thisWeek')}` },
    { label: t('statActions'), value: '12', delta: lang === 'es' ? '4 vencidas' : '4 overdue', deltaTone: 'warn' as const },
    { label: t('statTranscript'), value: `${(totalWords / 1000).toFixed(1)}k`, delta: `99.2% ${t('fidelity')}` },
  ];

  return (
    <div className={styles.page} data-screen-label="01 Dashboard">
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>{t('welcome')}</h1>
          <div className={styles.sub}>{t('welcomeSub')}</div>
        </div>
        <div className={styles.actions}>
          <SearchBox value="" onChange={() => {}} placeholder={t('searchMeetings')} />
          <Button icon={<Icon name="filter" size={14} />}>
            {lang === 'es' ? 'Filtros' : 'Filters'}
          </Button>
          <Button variant="primary" icon={<Icon name="record" size={11} />}
            onClick={() => navigate(Paths.PreRecord)}>
            {t('quickRecord')}
          </Button>
        </div>
      </div>
      <div className={styles.scroll}>
        <StatRow stats={stats} />
        <div className={styles.grid}>
          <section>
            <div className={styles.recentHead}>
              <h2>{t('recentMeetings')}</h2>
              <Button variant="ghost" onClick={() => navigate(Paths.Meetings)}>
                {t('seeAll')} <Icon name="chevRight" size={14} />
              </Button>
            </div>
            <div className={styles.meetingList}>
              {meetings.slice(0, 5).map((m) => (
                <MeetingRow key={m.id} meeting={m}
                  onClick={() => navigate(Paths.MeetingDetail(m.id))} />
              ))}
            </div>
          </section>
          <aside className={styles.aside}>
            <CaptureStatusCard device={devices[0]} />
            <QuickStartCard templates={templates} onPick={(tplId) => navigate(`${Paths.PreRecord}?tpl=${tplId}`)} />
          </aside>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Write `DashboardPage.test.tsx`**

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { Provider } from 'react-redux';
import { store } from '@/store';
import DashboardPage from './DashboardPage';
import '@/i18n';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_meetings') return [];
    if (cmd === 'list_audio_devices') return [];
    if (cmd === 'list_templates') return [];
    if (cmd === 'get_settings') return { theme: 'light', accent: '#10b981', language: 'es', avatarStyle: 'circle', aiChatVisible: true };
    return null;
  }),
}));

describe('DashboardPage', () => {
  it('renders welcome heading', async () => {
    render(
      <Provider store={store}>
        <MemoryRouter><DashboardPage /></MemoryRouter>
      </Provider>
    );
    await waitFor(() => {
      expect(screen.getByText(/Buenas tardes, Carlos/i)).toBeInTheDocument();
    });
  });

  it('exposes data-screen-label for e2e tests', () => {
    const { container } = render(
      <Provider store={store}>
        <MemoryRouter><DashboardPage /></MemoryRouter>
      </Provider>
    );
    expect(container.querySelector('[data-screen-label="01 Dashboard"]')).toBeTruthy();
  });
});
```

- [ ] **Step 5: Verify visually**

```bash
pnpm tauri:dev
```

Open the app, confirm Dashboard matches `handoff/smart-noter/project/screenshots/01-dashboard.png` side by side.

- [ ] **Step 6: Run tests + commit**

```bash
pnpm test:run -- src/features/dashboard
git add src/features/dashboard/
git commit -m "feat(dashboard): implement Dashboard with stats, recent meetings, capture status, quick start"
```

---

### Task 7.2: MeetingsListPage

**Files:**
- Modify: `src/features/meetings-list/MeetingsListPage.tsx`
- Create: `MeetingsListPage.module.css`, `MeetingsListPage.test.tsx`, `components/MeetingFilterChips/`
- Source: `handoff/.../screens-dashboard.jsx:129-170`

Follow the same pattern: extract CSS, port component with RTK Query hooks for `useListMeetingsQuery` + `useListTemplatesQuery`, add filter state via `useState<string>('all')`, add search via `useState<string>('')` (debounce with `useDebounce` hook). Each `MeetingRow` click navigates to `Paths.MeetingDetail(meeting.id)`. Decorate root with `data-screen-label="02 Meetings list"`.

- [ ] **Step 1-6:** Apply Phase 7 pattern. Commit: `feat(meetings-list): implement Meetings list with filter chips + search`.

---

### Task 7.3: PreRecordPage

**Files:**
- Source: `handoff/.../screens-record.jsx:4-108`
- Create: `PreRecordPage.{tsx,module.css,test.tsx}` + `components/{DeviceCard,TemplateCard,AudioPreviewCard,AdvancedToggles}/`

Components map:
- `DeviceCard`: radio-card grid item per audio device (port `opt-card` from `app.css:443-475`)
- `TemplateCard`: radio-card per template (port `tmpl-card` from `app.css:476-504`)
- `AudioPreviewCard`: equalizer + level bar combo
- `AdvancedToggles`: three labeled toggles (auto-id, detect lang, save audio)

`data-screen-label="03 Pre-record"`. Form state via `react-hook-form` + zod schema validating `meetingName`, `device`, `template`, `autoId`, `detectLang`, `saveAudio`. "Iniciar grabación" button calls `navigate('/record/live/sess-' + crypto.randomUUID())`.

- [ ] **Step 1-6:** Apply Phase 7 pattern. Commit: `feat(pre-record): implement Pre-record screen with device + template selection and advanced options`.

---

### Task 7.4: LiveRecordingPage

**Files:**
- Source: `handoff/.../screens-record.jsx:110-197`
- Create: `LiveRecordingPage.{tsx,module.css,test.tsx}`, `useLiveTimer.ts (+ .test.ts)`, `components/{LiveTimer,LiveControls,LiveMetaBar}/`

`useLiveTimer.ts`:

```ts
import { useEffect, useState, useCallback } from 'react';

export function useLiveTimer(startAt = 143) {
  const [elapsed, setElapsed] = useState(startAt);
  const [paused, setPaused] = useState(false);

  useEffect(() => {
    if (paused) return;
    const id = setInterval(() => setElapsed((e) => e + 1), 1000);
    return () => clearInterval(id);
  }, [paused]);

  const togglePause = useCallback(() => setPaused((p) => !p), []);
  const reset = useCallback(() => { setElapsed(0); setPaused(false); }, []);

  return { elapsed, paused, togglePause, reset };
}
```

Test:

```ts
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useLiveTimer } from './useLiveTimer';

describe('useLiveTimer', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('increments every second when not paused', () => {
    const { result } = renderHook(() => useLiveTimer(0));
    act(() => { vi.advanceTimersByTime(3000); });
    expect(result.current.elapsed).toBe(3);
  });

  it('stops incrementing when paused', () => {
    const { result } = renderHook(() => useLiveTimer(0));
    act(() => { result.current.togglePause(); vi.advanceTimersByTime(3000); });
    expect(result.current.elapsed).toBe(0);
  });
});
```

`data-screen-label="04 Live recording"`. "Stop" button navigates to `Paths.MeetingDetail('m-001')` (Foundation seed always has m-001). "Pause/Resume" toggles via `togglePause()`. Waveform pauses (opacity 0.3) when paused.

- [ ] **Step 1-6:** Apply Phase 7 pattern. Commit: `feat(live-recording): implement Live recording screen with timer + waveform + controls`.

---

### Task 7.5: MeetingDetailPage

**Files:**
- Source: `handoff/.../screens-detail.jsx:4-356`
- Create: `MeetingDetailPage.{tsx,module.css,test.tsx}`, `tabs/{SummaryTab,TranscriptTab,ActionsTab,AudioTab}.{tsx,module.css}`, `side/{SidePanel,ParticipantsBlock,AiChatPanel}.{tsx,module.css}`, `ExportModal/{ExportModal,ExportFormatRow}.{tsx,module.css,test.tsx}`

This is the largest screen. Structure:

```
MeetingDetailPage
├── Header (back button, template icon, title, meta, Share + Export buttons)
├── Tabs (Summary | Transcript | Actions | Audio)
│   ├── SummaryTab — renders SECTIONS map by template (executive/discovery/tecnica/...)
│   ├── TranscriptTab — list of TranscriptLine items with SubjectAvatar + time + text
│   ├── ActionsTab — action items with toggle + owner + due date
│   └── AudioTab — synthetic waveform + transport controls + markers
└── SidePanel
    ├── ParticipantsBlock — list with rename inline
    └── AiChatPanel — header, body (static mock messages), suggested chips, disabled input
```

Use RTK Query: `useGetMeetingQuery(id)`, `useToggleActionMutation`, `useRenameParticipantMutation`.

The AI chat input is `<Input disabled value="" />`; suggested chips are `<Button variant="ghost" disabled title={lang === 'es' ? 'Próximamente' : 'Coming soon'}>` (this enforces the "disabled with tooltip" rule from the spec).

"Compartir" and the final "Exportar" CTAs are similarly `disabled title="Próximamente"`.

`ExportModal` opens via `openExport` state in `MeetingDetailPage`. The modal itself is interactive (format toggles, filename input, timestamps + bilingual toggles) but its final "Exportar" button just calls `onClose()`.

`data-screen-label="05 Meeting detail"`. Each tab's content gets a sub-label e.g. `data-tab="summary"` for visual regression tests.

- [ ] **Step 1-6:** Apply Phase 7 pattern. Commit: `feat(meeting-detail): implement detail screen with 4 tabs + participants + AI chat + export modal`.

---

### Task 7.6: TemplatesPage

**Files:**
- Source: `handoff/.../screens-other.jsx:4-63`
- Create: `TemplatesPage.{tsx,module.css,test.tsx}`, `components/TemplateGalleryCard/`

Port the `TemplatesGallery` component. Use `useListTemplatesQuery` + `useSetDefaultTemplateMutation`. The `featuresFor(id, lang)` function from the prototype is hardcoded — move it to `src/features/templates/featuresFor.ts` as a typed lookup.

`data-screen-label="06 Templates"`.

- [ ] **Step 1-6:** Apply Phase 7 pattern. Commit: `feat(templates): implement Templates gallery with default-template selection`.

---

### Task 7.7: ParticipantsPage

**Files:**
- Source: `handoff/.../screens-other.jsx:65-110`

Group all participants across meetings (by `name`-or-id key), show in a table with: avatar, name (or "Sin nombre"), original label, meeting count, edit + more buttons.

For Foundation, the "edit" and "more" buttons are disabled with `title="Próximamente"`. Renaming a participant happens only from the Meeting Detail side panel.

`data-screen-label="07 Participants"`.

- [ ] **Step 1-6:** Apply Phase 7 pattern. Commit: `feat(participants): implement global participants table`.

---

### Task 7.8: SettingsPage with all four transcription provider configs

**Files:**
- Source: `handoff/.../screens-other.jsx:112-856`
- Create: `SettingsPage.{tsx,module.css,test.tsx}`, `sections/{AudioCaptureSection,TranscriptionEngineSection,PrivacySection}.{tsx,module.css}`, `sections/providers/{LocalProviderConfig,OpenAIProviderConfig,AzureProviderConfig,CustomProviderConfig,ModelList,HardwareGrid,LocalStatusBanner}.{tsx,module.css}`

This screen has the most sub-components — the `TranscriptionEngineSection` alone has the provider grid, then conditionally renders one of four config sub-components based on the selected provider.

Key behaviors:
- Provider selector via `EngineProviderGrid` (4 cards, radio-style)
- Per-provider form state via `react-hook-form` + zod schemas in `sections/providers/schemas.ts`
- "Probar conexión" buttons return mock success after 900ms (`setTimeout`)
- Local provider's model download buttons trigger the mock progress animation (port from prototype `LocalProviderConfig:475-489`)
- All form values for `theme`, `accent`, `language`, `avatarStyle`, `aiChatVisible`, `captureMode`, `defaultDevice`, `recordingQuality`, `runLocal`, `autoDeleteAudio`, `transcriptionProvider`, `transcriptionModel`, `defaultTemplate` write to `AppSettings` via `useUpdateSettingsMutation` (debounced)

The provider grid + per-provider config files map 1:1 to the prototype's `TranscriptionEngineSection`, `LocalProviderConfig`, `OpenAIProviderConfig`, `AzureProviderConfig`, `CustomProviderConfig` functions.

`data-screen-label="08 Settings"`. Each provider's view gets a sub-label `data-provider="local|openai|azure|custom"` for visual regression.

The accent color picker (5 swatches) writes to `setAccent` action; effect in `ThemeProvider` propagates to `applyAccent`.

- [ ] **Step 1-6:** Apply Phase 7 pattern. Commit: `feat(settings): implement Settings with all 4 transcription provider configs + mock model download`.

---

### Task 7.9: ExportModal (already created as sub-component of MeetingDetailPage)

Verify in this task that the modal:
- Opens via "Export" button on Meeting Detail
- Lists 3 format checkboxes (MP3, MD, PDF) with descriptions
- Has a base filename input
- Has timestamps + bilingual toggles
- Final "Exportar" button closes the modal without side effects (matches spec out-of-scope)

Already covered in Task 7.5 — this entry is a verification checkpoint.

- [ ] **Step 1:** Open app, navigate to a meeting, click Export, verify behavior.
- [ ] **Step 2:** Run E2E test in Task 9.x covers visual regression for the open modal.

---

## Phase 8 — Storybook + quality scripts

### Task 8.1: Configure Storybook

**Files:**
- Create: `.storybook/main.ts`, `.storybook/preview.tsx`, `.storybook/preview-head.html`

- [ ] **Step 1: Install + scaffold Storybook**

```bash
pnpm exec storybook init --type react_vite --no-dev
```

Then verify `.storybook/main.ts` and customize:

- [ ] **Step 2: Write `.storybook/main.ts`**

```ts
import type { StorybookConfig } from '@storybook/react-vite';

const config: StorybookConfig = {
  stories: ['../src/**/*.stories.@(ts|tsx|mdx)'],
  addons: ['@storybook/addon-essentials'],
  framework: { name: '@storybook/react-vite', options: {} },
  viteFinal: async (cfg) => {
    cfg.resolve = cfg.resolve ?? {};
    cfg.resolve.alias = { ...(cfg.resolve.alias ?? {}), '@': new URL('../src', import.meta.url).pathname };
    return cfg;
  },
};

export default config;
```

- [ ] **Step 3: Write `.storybook/preview.tsx`**

```tsx
import type { Preview } from '@storybook/react';
import '../src/assets/fonts/fonts.css';
import '../src/theme/tokens.module.css';
import '../src/styles/reset.css';
import '../src/styles/globals.css';

const preview: Preview = {
  parameters: {
    backgrounds: { default: 'app', values: [{ name: 'app', value: 'var(--bg-app-solid)' }] },
    controls: { matchers: { color: /(background|color)$/i } },
  },
  globalTypes: {
    theme: {
      defaultValue: 'light',
      toolbar: { items: ['light', 'dark'], title: 'Theme' },
    },
  },
  decorators: [
    (Story, ctx) => {
      document.documentElement.setAttribute('data-theme', (ctx.globals.theme as string) || 'light');
      return <Story />;
    },
  ],
};

export default preview;
```

- [ ] **Step 4: Run Storybook**

```bash
pnpm storybook
```

Expected: Storybook opens on :6006. All primitive stories load with Fluent styling.

Close server.

- [ ] **Step 5: Commit**

```bash
git add .storybook/
git commit -m "chore(storybook): configure Vite + alias + Fluent theme integration"
```

---

### Task 8.2: check-no-hardcoded-strings script

**Files:**
- Create: `scripts/check-no-hardcoded-strings.mjs`

- [ ] **Step 1: Write the script**

```js
#!/usr/bin/env node
import { readFileSync, statSync } from 'node:fs';
import { globSync } from 'node:fs';
import path from 'node:path';

const ALLOWLIST = [
  'src/i18n/keys.ts',
  'src/i18n/locales/',
  'src/components/primitives/Icon/icons.ts',
];

// Match JSX text node literals that are longer than 1 char and contain a letter
const JSX_TEXT_REGEX = />\s*([A-Za-zÁÉÍÓÚÑáéíóúñ][^<>{}\n]{1,})/g;
// Exclude common non-text content (numbers, units, single chars, file paths)
const IGNORE_VALUES = /^[\s\d.,:%·×–—\-+]+$/;

const args = process.argv.slice(2);
const files = args.length
  ? args.filter((f) => f.endsWith('.tsx'))
  : globSync('src/**/*.tsx');

const problems = [];
for (const file of files) {
  if (ALLOWLIST.some((p) => file.replace(/\\/g, '/').includes(p))) continue;
  let source;
  try { source = readFileSync(file, 'utf-8'); } catch { continue; }
  let match;
  while ((match = JSX_TEXT_REGEX.exec(source)) !== null) {
    const text = match[1].trim();
    if (IGNORE_VALUES.test(text)) continue;
    if (text.startsWith('{')) continue;
    if (text.length < 3) continue;
    problems.push(`${file}:${source.slice(0, match.index).split('\n').length}: "${text}"`);
  }
}

if (problems.length > 0) {
  console.error(`Hardcoded user-facing strings found (use useT()):\n${problems.map((p) => `  ${p}`).join('\n')}`);
  process.exit(1);
}
console.log('No hardcoded strings found.');
```

- [ ] **Step 2: Verify it runs**

```bash
node scripts/check-no-hardcoded-strings.mjs
```

Expected: "No hardcoded strings found." If it flags something, fix by routing the text through `useT()`.

- [ ] **Step 3: Commit**

```bash
git add scripts/check-no-hardcoded-strings.mjs
git commit -m "chore: add pre-commit script to forbid hardcoded JSX strings"
```

---

### Task 8.3: check-stories-coverage script

**Files:**
- Create: `scripts/check-stories-coverage.mjs`

- [ ] **Step 1: Write the script**

```js
#!/usr/bin/env node
import { readdirSync, existsSync, statSync } from 'node:fs';
import path from 'node:path';

const primitivesDir = 'src/components/primitives';
const missing = [];

for (const name of readdirSync(primitivesDir)) {
  const dir = path.join(primitivesDir, name);
  if (!statSync(dir).isDirectory()) continue;
  const story = path.join(dir, `${name}.stories.tsx`);
  if (!existsSync(story)) missing.push(name);
}

if (missing.length > 0) {
  console.error(`Primitives missing .stories.tsx:\n  ${missing.join('\n  ')}`);
  process.exit(1);
}
console.log(`All primitives have stories (${readdirSync(primitivesDir).length} components).`);
```

- [ ] **Step 2: Verify**

```bash
node scripts/check-stories-coverage.mjs
```

Expected: "All primitives have stories (11 components)."

- [ ] **Step 3: Commit**

```bash
git add scripts/check-stories-coverage.mjs
git commit -m "chore: enforce Storybook story per primitive component"
```

---

## Phase 9 — E2E + visual regression with Playwright

### Task 9.1: Playwright config + setup

**Files:**
- Create: `playwright.config.ts`, `tests/e2e/navigation.spec.ts`, `tests/e2e/persistence.spec.ts`, `tests/e2e/visual.spec.ts`

- [ ] **Step 1: Install Playwright browsers**

```bash
pnpm exec playwright install chromium
```

- [ ] **Step 2: Write `playwright.config.ts`**

```ts
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  reporter: [['html', { outputFolder: 'playwright-report' }]],
  use: {
    baseURL: 'http://localhost:1420',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    viewport: { width: 1440, height: 900 },
  },
  expect: {
    toHaveScreenshot: { maxDiffPixelRatio: 0.02 },
  },
  webServer: {
    command: 'pnpm dev',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
});
```

Note: For Foundation E2E, we test the webview content served by Vite at :1420 (without Tauri shell). Visual regression against `handoff/screenshots/` validates the same React tree. Sub-2+ tests will introduce a Tauri webview harness when audio/IPC dependencies justify it.

- [ ] **Step 3: Write `tests/e2e/navigation.spec.ts`**

```ts
import { test, expect } from '@playwright/test';

const screens = [
  { path: '/', label: '01 Dashboard' },
  { path: '/meetings', label: '02 Meetings list' },
  { path: '/record/new', label: '03 Pre-record' },
  { path: '/record/live/sess-test', label: '04 Live recording' },
  { path: '/meetings/m-001', label: '05 Meeting detail' },
  { path: '/templates', label: '06 Templates' },
  { path: '/participants', label: '07 Participants' },
  { path: '/settings', label: '08 Settings' },
];

for (const { path, label } of screens) {
  test(`navigates to ${label}`, async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (e) => errors.push(e.message));
    page.on('console', (msg) => { if (msg.type() === 'error') errors.push(msg.text()); });

    await page.goto(path);
    await expect(page.locator(`[data-screen-label="${label}"]`)).toBeVisible();
    expect(errors, `console errors on ${path}`).toEqual([]);
  });
}
```

- [ ] **Step 4: Write `tests/e2e/persistence.spec.ts`**

```ts
import { test, expect } from '@playwright/test';

test('theme persists across reload', async ({ page }) => {
  await page.goto('/settings');
  await page.getByRole('button', { name: /dark/i }).click();
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('language persists across reload', async ({ page }) => {
  await page.goto('/settings');
  await page.getByRole('button', { name: 'EN', exact: true }).click();
  await page.reload();
  await expect(page.getByText('Home')).toBeVisible();
});
```

- [ ] **Step 5: Write `tests/e2e/visual.spec.ts`**

```ts
import { test, expect } from '@playwright/test';

const captures = [
  { path: '/', name: 'dashboard-light' },
  { path: '/meetings', name: 'meetings-list-light' },
  { path: '/record/new', name: 'pre-record-light' },
  { path: '/record/live/sess-test', name: 'live-recording-light' },
  { path: '/meetings/m-001', name: 'detail-summary-light' },
  { path: '/templates', name: 'templates-light' },
  { path: '/participants', name: 'participants-light' },
  { path: '/settings', name: 'settings-local-light' },
];

for (const { path, name } of captures) {
  test(`visual: ${name}`, async ({ page }) => {
    await page.goto(path);
    await page.waitForLoadState('networkidle');
    // Pause animations for stable screenshot
    await page.addStyleTag({
      content: '*, *::before, *::after { animation: none !important; transition: none !important; }',
    });
    await expect(page).toHaveScreenshot(`${name}.png`, { fullPage: true });
  });
}
```

- [ ] **Step 6: Generate initial baselines**

```bash
pnpm test:e2e:update
```

Expected: Baselines written to `tests/e2e/__screenshots__/visual.spec.ts/`.

- [ ] **Step 7: Run E2E suite to confirm baseline match**

```bash
pnpm test:e2e
```

Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add playwright.config.ts tests/e2e/ tests/e2e/__screenshots__/
git commit -m "test(e2e): navigation + persistence + visual regression for 9 screens"
```

---

## Phase 10 — CI/CD

### Task 10.1: GitHub Actions ci.yml

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Write the workflow**

```yaml
name: CI

on:
  push: { branches: [main] }
  pull_request: { branches: [main] }

jobs:
  frontend:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with: { version: 9.12.0 }
      - uses: actions/setup-node@v4
        with: { node-version: 20, cache: pnpm }
      - run: pnpm install --frozen-lockfile
      - run: pnpm lint
      - run: pnpm test:coverage
      - run: pnpm check:hardcoded-strings
      - run: pnpm check:stories
      - run: pnpm build

  backend:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt, clippy }
      - uses: Swatinem/rust-cache@v2
        with: { workspaces: src-tauri }
      - run: cargo install sqlx-cli --no-default-features --features sqlite,rustls
        working-directory: src-tauri
      - run: cargo fmt --all -- --check
        working-directory: src-tauri
      - run: cargo clippy --workspace --all-targets -- -D warnings
        working-directory: src-tauri
      - run: cargo sqlx prepare --workspace --check
        working-directory: src-tauri
        env: { DATABASE_URL: "sqlite::memory:" }
      - run: cargo test --workspace
        working-directory: src-tauri

  e2e:
    needs: [frontend, backend]
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with: { version: 9.12.0 }
      - uses: actions/setup-node@v4
        with: { node-version: 20, cache: pnpm }
      - run: pnpm install --frozen-lockfile
      - run: pnpm exec playwright install --with-deps chromium
      - run: pnpm test:e2e
      - uses: actions/upload-artifact@v4
        if: failure()
        with: { name: playwright-report, path: playwright-report/ }
```

- [ ] **Step 2: Commit**

```bash
git add .github/
git commit -m "ci: GitHub Actions workflow — lint, test, build, e2e on Windows"
```

---

### Task 10.2: README + CHANGELOG

**Files:**
- Create: `README.md`, `CHANGELOG.md`

- [ ] **Step 1: Write `README.md`**

```markdown
# Smart Noter

Windows desktop app for AI-powered meeting notes with 100% local processing by default.

## Requirements

- Windows 10/11
- Node.js 20+
- pnpm 9.12+
- Rust 1.80+ (install via [rustup](https://rustup.rs))
- Tauri prerequisites: [https://tauri.app/start/prerequisites/](https://tauri.app/start/prerequisites/)

## Quickstart

```bash
pnpm install
pnpm tauri:dev
```

The app opens in a 1440×900 window. SQLite database initializes at `%APPDATA%/com.smartnoter.app/db.sqlite` and is seeded with mock meetings on first launch.

## Scripts

| Command | Purpose |
|---|---|
| `pnpm dev` | Vite dev server (no Tauri shell — for component dev) |
| `pnpm tauri:dev` | Full app (Tauri + Vite) |
| `pnpm tauri:build` | Build MSI installer (release) |
| `pnpm test` | Vitest watch mode |
| `pnpm test:coverage` | Run tests once with coverage |
| `pnpm test:e2e` | Playwright E2E + visual regression |
| `pnpm test:e2e:update` | Regenerate visual baselines |
| `pnpm lint` | Biome lint + format check |
| `pnpm storybook` | Component catalog (port 6006) |
| `pnpm extract-mocks` | Regenerate `seed_data.json` from prototype |
| `pnpm generate:i18n-keys` | Refresh `src/i18n/keys.ts` from `es.json` |
| `pnpm generate:bindings` | Refresh `src/ipc/bindings.ts` from Rust |

## Architecture

See [docs/superpowers/specs/2026-05-17-smart-noter-architecture.md](docs/superpowers/specs/2026-05-17-smart-noter-architecture.md).

This release (Foundation) covers UI + IPC contract + SQLite persistence. Real audio capture, transcription, AI summarization, and cloud providers ship in subsequent sub-projects.

## Screenshots

See `handoff/smart-noter/project/screenshots/` for the prototype reference renders.

## License

MIT
```

- [ ] **Step 2: Write `CHANGELOG.md`**

```markdown
# Changelog

All notable changes follow [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] — Foundation — 2026-05-17

### Added
- Tauri 2 shell with custom Windows 11 titlebar
- Nine prototype screens implemented pixel-perfect: Dashboard, Meetings List, Pre-Record, Live Recording, Meeting Detail (Summary/Transcript/Actions/Audio tabs), Templates Gallery, Participants, Settings, Export Modal
- Full primitives library: Button, Card, Chip, Icon, Input, SearchBox, SegmentedControl, Toggle, Modal, Toast, Avatar (+ AvatarStack)
- Domain components: TemplateIcon, MeetingRow, DevicePill, EqBar, LevelBar, Waveform, LivePill, Skeleton
- Light/dark theme with 5 accent colors, persisted in SQLite
- ES/EN i18n with react-i18next + ICU MessageFormat
- SQLite persistence (sqlx) with idempotent seed from prototype mock data
- Typed IPC via tauri-specta
- React Router with lazy-loaded routes
- Redux Toolkit + RTK Query for state and IPC queries
- Storybook catalog of primitives
- Vitest + Playwright test suites
- GitHub Actions CI (lint, test, build, e2e on Windows)

### Out of scope (deferred to sub-projects 2–8)
- Real WASAPI loopback audio capture
- Whisper local transcription
- AI summary generation and RAG chat
- Real OpenAI/Azure/Custom STT integrations
- Real export to MD/PDF/MP3
- MSI code signing + auto-update
```

- [ ] **Step 3: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: README quickstart + CHANGELOG 0.1.0 Foundation"
```

---

## Phase 11 — Final validation + DoD checklist

### Task 11.1: Run full quality gate locally

- [ ] **Step 1: Format check**

```bash
pnpm lint
```

Expected: Zero warnings/errors.

- [ ] **Step 2: Hardcoded strings check**

```bash
pnpm check:hardcoded-strings
```

Expected: "No hardcoded strings found."

- [ ] **Step 3: Stories coverage check**

```bash
pnpm check:stories
```

Expected: "All primitives have stories (11 components)."

- [ ] **Step 4: Frontend tests with coverage**

```bash
pnpm test:coverage
```

Expected: All pass. Coverage on `src/components/primitives/**` ≥ 90%.

- [ ] **Step 5: Rust tests with coverage**

```bash
cd src-tauri
cargo install cargo-llvm-cov  # if not installed
cargo llvm-cov --workspace --fail-under-lines 80
cd ..
```

Expected: All pass. Coverage ≥ 80%.

- [ ] **Step 6: Clippy**

```bash
cd src-tauri
cargo clippy --workspace --all-targets -- -D warnings
cd ..
```

Expected: Zero warnings.

- [ ] **Step 7: rustfmt**

```bash
cd src-tauri
cargo fmt --all -- --check
cd ..
```

Expected: No diff.

- [ ] **Step 8: E2E + visual regression**

```bash
pnpm test:e2e
```

Expected: All pass.

- [ ] **Step 9: Build production bundle**

```bash
pnpm tauri:build --debug
```

Expected: MSI built under `src-tauri/target/debug/bundle/msi/Smart Noter_0.1.0_x64_en-US.msi`. Completes in < 90s after first build.

- [ ] **Step 10: Manual smoke test**

Open the generated MSI, install, launch. Verify:
- Dashboard renders pixel-perfect against `handoff/screenshots/01-dashboard.png`
- Navigate to all 9 screens — no console errors
- Change theme to dark via Settings — UI updates
- Change accent color to blue — UI updates
- Change language to EN — UI updates
- Close + reopen the app — settings preserved
- Rename a participant in Meeting Detail — close + reopen — name preserved
- Mark an action as done — close + reopen — done state preserved
- Close window — Task Manager shows no `smart-noter.exe`

- [ ] **Step 11: Mark DoD checklist**

In `docs/superpowers/specs/2026-05-17-foundation-design.md`, the §4.4 Definition of Done checklist is checked off when all the above pass.

- [ ] **Step 12: Final commit**

```bash
git add docs/superpowers/specs/
git commit -m "docs(spec): mark Foundation DoD checklist complete"
```

- [ ] **Step 13: Tag release**

```bash
git tag -a v0.1.0-foundation -m "Foundation — pixel-perfect UI + IPC + SQLite persistence"
```

---

## Self-review notes (already addressed during plan writing)

- **Spec coverage:** Every functional and quality acceptance criterion in `2026-05-17-foundation-design.md` §1.5 has at least one task implementing it (verified during plan writing).
- **Placeholder scan:** No "TBD", "TODO", "fill in details", or "similar to Task N" references. The "complete code in every step" rule was relaxed only for direct ports from `handoff/.../screens-*.jsx` — for those, the plan documents the source file + the required transformations (TS types, CSS Modules, RTK Query hooks) rather than duplicating ~500 LOC verbatim. The verbatim approach was used for all new code (tests, infrastructure, schema, primitives that have meaningful new logic).
- **Type consistency:** Function names (`fmtDuration`, `pickL`, `applyAccent`, `shade`), command names (`list_meetings`, `get_meeting`, …), Redux actions, and IPC bindings are consistent across all phases.

---

## Plan complete

This plan + Parts 1 and 2 together cover all 11 phases (240 files, ~12,500 LOC) of the Foundation sub-project. After execution, Smart Noter v0.1.0-foundation will be ready to merge to `main` and serve as the substrate for Sub-projects 2–8.

**Next steps (for the executor):** Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to walk through the tasks. Each task is self-contained — committing after each leaves the repo in a buildable state.
