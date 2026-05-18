# Smart Noter Foundation Implementation Plan — Part 2

> **Continuation of [foundation-plan.md](./2026-05-17-foundation-plan.md).** Covers Phases 2–6: CSS migration, frontend infrastructure (i18n, theme, store, router, IPC client, tests), primitives library, shell, domain components.
>
> Part 3 ([foundation-plan-part3.md](./2026-05-17-foundation-plan-part3.md)) covers feature screens, quality gates, and CI.

---

## Phase 2 — CSS migration from prototype

The prototype's `handoff/smart-noter/project/app.css` (40 KB) is the single source of truth for visual design. We extract it into:

- `src/theme/tokens.module.css` — `:root` + `[data-theme="*"]` CSS custom properties
- `src/styles/globals.css` — body, scrollbars, `::selection`, base resets
- One `.module.css` file per component (delivered alongside each component task in Phases 3–6)

This phase only sets up tokens + globals. Per-component CSS arrives with each component.

### Task 2.1: Extract theme tokens

**Files:**
- Create: `src/theme/tokens.module.css`
- Modify: `src/styles/globals.css`, `src/styles/reset.css`, `src/main.tsx`
- Source: `handoff/smart-noter/project/app.css:1-115`

- [ ] **Step 1: Write `src/theme/tokens.module.css`**

Copy the contents of `handoff/smart-noter/project/app.css` lines **1–84** verbatim (the `:root`, `[data-theme="light"]`, `[data-theme="dark"]` blocks). Save as `src/theme/tokens.module.css`.

This file declares CSS custom properties globally on `:root`, which is the documented exception in the architecture spec.

- [ ] **Step 2: Rewrite `src/styles/reset.css`**

Copy lines **86–118** of `app.css` (the `*`, `html`, `body`, `button`, `input` reset rules + `::selection`) into `src/styles/reset.css`.

- [ ] **Step 3: Rewrite `src/styles/globals.css`**

Copy lines **120–125** of `app.css` (the `.scroll` + scrollbar rules + `#root` rule) into `src/styles/globals.css`.

- [ ] **Step 4: Bundle Inter + JetBrains Mono fonts locally**

Download `.woff2` files for:
- Inter (400, 500, 600, 700)
- JetBrains Mono (400, 500, 600)

Place under `src/assets/fonts/`:

```
src/assets/fonts/
├── Inter-400.woff2
├── Inter-500.woff2
├── Inter-600.woff2
├── Inter-700.woff2
├── JetBrainsMono-400.woff2
├── JetBrainsMono-500.woff2
└── JetBrainsMono-600.woff2
```

Source URLs (download manually or via curl):
- https://fonts.gstatic.com/s/inter/v18/UcCo3FwrK3iLTcvneQg7Ca725JhhKnNqk4j1ebLhAm8SrXTcvfk7Hg.woff2 (Inter)
- https://fonts.gstatic.com/s/jetbrainsmono/v20/tDbY2o-flEEny0FZhsfKu5WU4xD-IQ-PuZJJXxfpAO-Lf1OQk6OThxPA.woff2 (JetBrains Mono)

Write `src/assets/fonts/fonts.css`:

```css
@font-face {
  font-family: 'Inter';
  font-weight: 400;
  font-style: normal;
  font-display: swap;
  src: url('./Inter-400.woff2') format('woff2');
}
@font-face {
  font-family: 'Inter';
  font-weight: 500;
  src: url('./Inter-500.woff2') format('woff2');
}
@font-face {
  font-family: 'Inter';
  font-weight: 600;
  src: url('./Inter-600.woff2') format('woff2');
}
@font-face {
  font-family: 'Inter';
  font-weight: 700;
  src: url('./Inter-700.woff2') format('woff2');
}
@font-face {
  font-family: 'JetBrains Mono';
  font-weight: 400;
  src: url('./JetBrainsMono-400.woff2') format('woff2');
}
@font-face {
  font-family: 'JetBrains Mono';
  font-weight: 500;
  src: url('./JetBrainsMono-500.woff2') format('woff2');
}
@font-face {
  font-family: 'JetBrains Mono';
  font-weight: 600;
  src: url('./JetBrainsMono-600.woff2') format('woff2');
}
```

- [ ] **Step 5: Update `src/main.tsx` to import in order**

```tsx
import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './assets/fonts/fonts.css';
import './theme/tokens.module.css';
import './styles/reset.css';
import './styles/globals.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

- [ ] **Step 6: Smoke test**

```bash
pnpm tauri:dev
```

Expected: Window opens with the Inter font visible in the placeholder text. `<html>` element should have `--accent`, `--text`, etc. (inspect via DevTools).

- [ ] **Step 7: Commit**

```bash
git add src/theme/tokens.module.css src/styles/ src/assets/fonts/ src/main.tsx
git commit -m "feat(theme): extract tokens + global reset + bundle Inter/JetBrains Mono fonts"
```

---

## Phase 3 — Frontend infrastructure

### Task 3.1: i18n setup with react-i18next + ICU

**Files:**
- Create: `src/i18n/locales/es.json`, `en.json`, `src/i18n/index.ts`, `src/i18n/useT.ts`, `src/i18n/keys.ts`
- Create: `scripts/generate-i18n-keys.mjs`
- Test: `src/i18n/useT.test.ts`

- [ ] **Step 1: Extract i18n dictionaries from prototype**

Open `handoff/smart-noter/project/data.js`. Locate the `const I18N = { es: {...}, en: {...} }` object (lines ~293–610).

Copy `I18N.es` to `src/i18n/locales/es.json` as valid JSON (quoted keys, escaped apostrophes). Same for `I18N.en` → `en.json`.

Example transformation:

Prototype JS:
```js
welcome: 'Buenas tardes, Carlos',
```

JSON:
```json
"welcome": "Buenas tardes, Carlos",
```

- [ ] **Step 2: Write `scripts/generate-i18n-keys.mjs`**

```js
#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');
const esPath = resolve(root, 'src/i18n/locales/es.json');
const outPath = resolve(root, 'src/i18n/keys.ts');

const dict = JSON.parse(readFileSync(esPath, 'utf-8'));
const keys = Object.keys(dict).sort().map((k) => `  | '${k}'`).join('\n');

writeFileSync(
  outPath,
  `// AUTO-GENERATED — do not edit. Run pnpm generate:i18n-keys to update.\nexport type TKey =\n${keys};\n`
);
console.log(`Wrote ${Object.keys(dict).length} keys → ${outPath}`);
```

Run it:

```bash
node scripts/generate-i18n-keys.mjs
```

- [ ] **Step 3: Write `src/i18n/index.ts`**

```ts
import i18n from 'i18next';
import ICU from 'i18next-icu';
import { initReactI18next } from 'react-i18next';
import es from './locales/es.json';
import en from './locales/en.json';

await i18n
  .use(ICU)
  .use(initReactI18next)
  .init({
    resources: {
      es: { translation: es },
      en: { translation: en },
    },
    lng: 'es',
    fallbackLng: 'es',
    interpolation: { escapeValue: false },
    returnNull: false,
  });

export default i18n;
```

- [ ] **Step 4: Write `src/i18n/useT.ts`**

```ts
import { useTranslation } from 'react-i18next';
import type { TKey } from './keys';

export function useT() {
  const { t, i18n } = useTranslation();
  return {
    t: (key: TKey, opts?: Record<string, unknown>) => t(key as string, opts ?? {}),
    lang: i18n.language as 'es' | 'en',
    setLang: (l: 'es' | 'en') => i18n.changeLanguage(l),
  };
}
```

- [ ] **Step 5: Write failing test `src/i18n/useT.test.ts`**

```ts
import { describe, it, expect, beforeAll } from 'vitest';
import { render, screen } from '@testing-library/react';
import { useT } from './useT';
import './index';

function Probe() {
  const { t, lang } = useT();
  return <div data-testid="probe" data-lang={lang}>{t('navDashboard')}</div>;
}

describe('useT', () => {
  it('returns Spanish text by default', () => {
    render(<Probe />);
    const el = screen.getByTestId('probe');
    expect(el).toHaveTextContent('Inicio');
    expect(el).toHaveAttribute('data-lang', 'es');
  });
});
```

- [ ] **Step 6: Set up Vitest config**

Write `vitest.config.ts`:

```ts
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'node:path';

export default defineConfig({
  plugins: [react()],
  resolve: { alias: { '@': path.resolve(__dirname, 'src') } },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./tests/setup.ts'],
    css: { modules: { classNameStrategy: 'non-scoped' } },
    coverage: {
      provider: 'v8',
      include: ['src/**/*.{ts,tsx}'],
      exclude: ['src/**/*.stories.tsx', 'src/ipc/bindings.ts', 'src/i18n/keys.ts', 'src/mock/**'],
      thresholds: {
        'src/components/primitives/**': { lines: 90, statements: 90, branches: 85, functions: 90 },
      },
    },
  },
});
```

Write `tests/setup.ts`:

```ts
import '@testing-library/jest-dom/vitest';
```

- [ ] **Step 7: Run test**

```bash
pnpm test:run -- src/i18n
```

Expected: 1 passed.

- [ ] **Step 8: Commit**

```bash
git add src/i18n/ scripts/generate-i18n-keys.mjs vitest.config.ts tests/setup.ts
git commit -m "feat(i18n): react-i18next + ICU with typed useT() hook and ES/EN dictionaries"
```

---

### Task 3.2: Theme provider + accent system

**Files:**
- Create: `src/theme/ThemeProvider.tsx`, `src/theme/accent.ts`
- Test: `src/theme/accent.test.ts`, `src/theme/ThemeProvider.test.tsx`

- [ ] **Step 1: Write `src/theme/accent.ts`**

```ts
export function shade(hex: string, pct: number): string {
  const c = Number.parseInt(hex.slice(1), 16);
  let r = (c >> 16) & 0xff;
  let g = (c >> 8) & 0xff;
  let b = c & 0xff;
  const adj = (v: number) =>
    Math.max(0, Math.min(255, Math.round(v + (pct < 0 ? v * pct : (255 - v) * pct))));
  r = adj(r);
  g = adj(g);
  b = adj(b);
  return `#${[r, g, b].map((x) => x.toString(16).padStart(2, '0')).join('')}`;
}

export function applyAccent(hex: string, root: HTMLElement = document.documentElement) {
  root.style.setProperty('--accent', hex);
  root.style.setProperty('--accent-hover', shade(hex, -0.1));
  root.style.setProperty('--accent-press', shade(hex, -0.2));
  const c = Number.parseInt(hex.slice(1), 16);
  const r = (c >> 16) & 0xff;
  const g = (c >> 8) & 0xff;
  const b = c & 0xff;
  root.style.setProperty('--accent-soft', `rgba(${r},${g},${b},0.12)`);
  root.style.setProperty('--accent-ring', `rgba(${r},${g},${b},0.35)`);
}
```

- [ ] **Step 2: Write failing test `src/theme/accent.test.ts`**

```ts
import { describe, it, expect } from 'vitest';
import { shade, applyAccent } from './accent';

describe('shade', () => {
  it('darkens by negative pct', () => {
    expect(shade('#10b981', -0.1)).toMatch(/^#[0-9a-f]{6}$/);
    expect(shade('#ffffff', -0.5)).toBe('#808080');
  });
  it('lightens by positive pct toward white', () => {
    expect(shade('#000000', 0.5)).toBe('#808080');
  });
});

describe('applyAccent', () => {
  it('sets --accent and derived custom properties on the root', () => {
    const root = document.createElement('div');
    applyAccent('#10b981', root);
    expect(root.style.getPropertyValue('--accent')).toBe('#10b981');
    expect(root.style.getPropertyValue('--accent-hover')).toBeTruthy();
    expect(root.style.getPropertyValue('--accent-soft')).toMatch(/^rgba\(/);
  });
});
```

- [ ] **Step 3: Run tests — expect PASS**

```bash
pnpm test:run -- src/theme/accent
```

- [ ] **Step 4: Write `src/theme/ThemeProvider.tsx`**

```tsx
import { useEffect, type ReactNode } from 'react';
import { applyAccent } from './accent';

export interface ThemeProviderProps {
  theme: 'light' | 'dark';
  accent: string;
  avatarStyle: 'circle' | 'square';
  children: ReactNode;
}

export function ThemeProvider({ theme, accent, avatarStyle, children }: ThemeProviderProps) {
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  useEffect(() => {
    applyAccent(accent);
  }, [accent]);

  useEffect(() => {
    const id = '__avatar-style';
    let s = document.getElementById(id);
    if (!s) {
      s = document.createElement('style');
      s.id = id;
      document.head.appendChild(s);
    }
    s.textContent = avatarStyle === 'square' ? '.avatar { border-radius: 8px !important; }' : '';
  }, [avatarStyle]);

  return <>{children}</>;
}
```

- [ ] **Step 5: Write test `src/theme/ThemeProvider.test.tsx`**

```tsx
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { ThemeProvider } from './ThemeProvider';

describe('ThemeProvider', () => {
  it('sets data-theme on documentElement', () => {
    render(
      <ThemeProvider theme="dark" accent="#10b981" avatarStyle="circle">
        <span />
      </ThemeProvider>
    );
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
  });

  it('applies accent CSS variable', () => {
    render(
      <ThemeProvider theme="light" accent="#3b82f6" avatarStyle="circle">
        <span />
      </ThemeProvider>
    );
    expect(document.documentElement.style.getPropertyValue('--accent')).toBe('#3b82f6');
  });
});
```

- [ ] **Step 6: Run tests**

```bash
pnpm test:run -- src/theme
```

Expected: 4 passed.

- [ ] **Step 7: Commit**

```bash
git add src/theme/
git commit -m "feat(theme): ThemeProvider + applyAccent with derived hover/press/soft/ring colors"
```

---

### Task 3.3: Redux store + RTK Query base

**Files:**
- Create: `src/store/index.ts`, `src/store/hooks.ts`, `src/store/slices/ui.slice.ts`, `src/store/api/base.ts`

- [ ] **Step 1: Write `src/store/slices/ui.slice.ts`**

```ts
import { createSlice, type PayloadAction } from '@reduxjs/toolkit';

export type Theme = 'light' | 'dark';
export type Language = 'es' | 'en';
export type AvatarStyle = 'circle' | 'square';

export interface UiState {
  theme: Theme;
  accent: string;
  language: Language;
  avatarStyle: AvatarStyle;
  aiChatVisible: boolean;
  ready: boolean;
}

const cached = (() => {
  try {
    const raw = localStorage.getItem('smart-noter:ui');
    if (raw) return JSON.parse(raw) as Partial<UiState>;
  } catch { /* ignore */ }
  return {};
})();

const initialState: UiState = {
  theme: cached.theme ?? 'light',
  accent: cached.accent ?? '#10b981',
  language: cached.language ?? 'es',
  avatarStyle: cached.avatarStyle ?? 'circle',
  aiChatVisible: cached.aiChatVisible ?? true,
  ready: false,
};

export const uiSlice = createSlice({
  name: 'ui',
  initialState,
  reducers: {
    setTheme(state, action: PayloadAction<Theme>) { state.theme = action.payload; },
    setAccent(state, action: PayloadAction<string>) { state.accent = action.payload; },
    setLanguage(state, action: PayloadAction<Language>) { state.language = action.payload; },
    setAvatarStyle(state, action: PayloadAction<AvatarStyle>) { state.avatarStyle = action.payload; },
    toggleAiChat(state) { state.aiChatVisible = !state.aiChatVisible; },
    hydrateFromBackend(state, action: PayloadAction<Partial<UiState>>) {
      Object.assign(state, action.payload, { ready: true });
    },
  },
});

export const { setTheme, setAccent, setLanguage, setAvatarStyle, toggleAiChat, hydrateFromBackend } = uiSlice.actions;
```

- [ ] **Step 2: Write `src/store/api/base.ts`**

```ts
import { createApi } from '@reduxjs/toolkit/query/react';
import type { BaseQueryFn } from '@reduxjs/toolkit/query';
import { invoke } from '@tauri-apps/api/core';

export interface AppError {
  code: string;
  message: string;
}

export interface InvokeArgs {
  cmd: string;
  args?: Record<string, unknown>;
}

const tauriBaseQuery: BaseQueryFn<InvokeArgs, unknown, AppError> = async ({ cmd, args }) => {
  try {
    const data = await invoke(cmd, args ?? {});
    return { data };
  } catch (e) {
    if (typeof e === 'object' && e !== null && 'code' in e) {
      return { error: e as AppError };
    }
    return { error: { code: 'internal', message: String(e) } };
  }
};

export const baseApi = createApi({
  baseQuery: tauriBaseQuery,
  tagTypes: ['Meeting', 'Template', 'AudioDevice', 'Settings'],
  endpoints: () => ({}),
});
```

- [ ] **Step 3: Write `src/store/index.ts`**

```ts
import { configureStore } from '@reduxjs/toolkit';
import { setupListeners } from '@reduxjs/toolkit/query';
import { uiSlice } from './slices/ui.slice';
import { baseApi } from './api/base';

export const store = configureStore({
  reducer: {
    ui: uiSlice.reducer,
    [baseApi.reducerPath]: baseApi.reducer,
  },
  middleware: (gdm) => gdm().concat(baseApi.middleware),
});

setupListeners(store.dispatch);

// Persist a subset of UI state to localStorage for instant rehydration on next start
store.subscribe(() => {
  const { theme, accent, language, avatarStyle, aiChatVisible } = store.getState().ui;
  try {
    localStorage.setItem('smart-noter:ui', JSON.stringify({ theme, accent, language, avatarStyle, aiChatVisible }));
  } catch { /* ignore */ }
});

export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
```

- [ ] **Step 4: Write `src/store/hooks.ts`**

```ts
import { type TypedUseSelectorHook, useDispatch, useSelector } from 'react-redux';
import type { RootState, AppDispatch } from './index';

export const useAppDispatch: () => AppDispatch = useDispatch;
export const useAppSelector: TypedUseSelectorHook<RootState> = useSelector;
```

- [ ] **Step 5: Write test `src/store/slices/ui.slice.test.ts`**

```ts
import { describe, it, expect } from 'vitest';
import { uiSlice, setTheme, setAccent } from './ui.slice';

describe('ui.slice', () => {
  it('changes theme', () => {
    const state = uiSlice.reducer(undefined, setTheme('dark'));
    expect(state.theme).toBe('dark');
  });
  it('changes accent', () => {
    const state = uiSlice.reducer(undefined, setAccent('#3b82f6'));
    expect(state.accent).toBe('#3b82f6');
  });
});
```

- [ ] **Step 6: Run tests**

```bash
pnpm test:run -- src/store
```

Expected: 2 passed.

- [ ] **Step 7: Commit**

```bash
git add src/store/
git commit -m "feat(store): Redux Toolkit store + ui.slice + RTK Query tauriBaseQuery"
```

---

### Task 3.4: RTK Query API slices (meetings, templates, devices, settings)

**Files:**
- Create: `src/store/api/{meetings,templates,devices,settings}.api.ts`

- [ ] **Step 1: Write `src/store/api/meetings.api.ts`**

```ts
import { baseApi } from './base';
import type { MeetingSummary, MeetingDetail } from '@/ipc/bindings';

export const meetingsApi = baseApi.injectEndpoints({
  endpoints: (b) => ({
    listMeetings: b.query<MeetingSummary[], void>({
      query: () => ({ cmd: 'list_meetings' }),
      providesTags: ['Meeting'],
    }),
    getMeeting: b.query<MeetingDetail, string>({
      query: (id) => ({ cmd: 'get_meeting', args: { id } }),
      providesTags: (_r, _e, id) => [{ type: 'Meeting', id }],
    }),
    updateMeetingTitle: b.mutation<void, { id: string; titleEs: string; titleEn?: string | null }>({
      query: (args) => ({ cmd: 'update_meeting_title', args }),
      invalidatesTags: (_r, _e, { id }) => [{ type: 'Meeting', id }, 'Meeting'],
    }),
    toggleAction: b.mutation<boolean, string>({
      query: (actionId) => ({ cmd: 'toggle_action', args: { actionId } }),
      invalidatesTags: ['Meeting'],
    }),
    renameParticipant: b.mutation<void, { participantId: string; name: string | null }>({
      query: (args) => ({ cmd: 'rename_participant', args }),
      invalidatesTags: ['Meeting'],
    }),
  }),
});

export const {
  useListMeetingsQuery,
  useGetMeetingQuery,
  useUpdateMeetingTitleMutation,
  useToggleActionMutation,
  useRenameParticipantMutation,
} = meetingsApi;
```

- [ ] **Step 2: Write `src/store/api/templates.api.ts`**

```ts
import { baseApi } from './base';
import type { Template } from '@/ipc/bindings';

export const templatesApi = baseApi.injectEndpoints({
  endpoints: (b) => ({
    listTemplates: b.query<Template[], void>({
      query: () => ({ cmd: 'list_templates' }),
      providesTags: ['Template'],
    }),
    setDefaultTemplate: b.mutation<void, string>({
      query: (id) => ({ cmd: 'set_default_template', args: { id } }),
      invalidatesTags: ['Template'],
    }),
  }),
});

export const { useListTemplatesQuery, useSetDefaultTemplateMutation } = templatesApi;
```

- [ ] **Step 3: Write `src/store/api/devices.api.ts`**

```ts
import { baseApi } from './base';
import type { AudioDevice } from '@/ipc/bindings';

export const devicesApi = baseApi.injectEndpoints({
  endpoints: (b) => ({
    listAudioDevices: b.query<AudioDevice[], void>({
      query: () => ({ cmd: 'list_audio_devices' }),
      providesTags: ['AudioDevice'],
    }),
  }),
});

export const { useListAudioDevicesQuery } = devicesApi;
```

- [ ] **Step 4: Write `src/store/api/settings.api.ts`**

```ts
import { baseApi } from './base';
import type { AppSettings } from '@/ipc/bindings';

export const settingsApi = baseApi.injectEndpoints({
  endpoints: (b) => ({
    getSettings: b.query<AppSettings, void>({
      query: () => ({ cmd: 'get_settings' }),
      providesTags: ['Settings'],
    }),
    updateSettings: b.mutation<void, AppSettings>({
      query: (settings) => ({ cmd: 'update_settings', args: { settings } }),
      invalidatesTags: ['Settings'],
    }),
  }),
});

export const { useGetSettingsQuery, useUpdateSettingsMutation } = settingsApi;
```

- [ ] **Step 5: Commit**

```bash
git add src/store/api/
git commit -m "feat(store): RTK Query API slices for meetings, templates, devices, settings"
```

---

### Task 3.5: Router setup with lazy-loaded routes

**Files:**
- Create: `src/router/paths.ts`, `src/router/routes.tsx`, `src/router/NotFoundRedirect.tsx`

- [ ] **Step 1: Write `src/router/paths.ts`**

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
} as const;
```

- [ ] **Step 2: Write `src/router/NotFoundRedirect.tsx`**

```tsx
import { Navigate } from 'react-router-dom';
import { Paths } from './paths';

export function NotFoundRedirect() {
  return <Navigate to={Paths.Dashboard} replace />;
}
```

- [ ] **Step 3: Write `src/router/routes.tsx`** (placeholders — real components land in Phase 7)

```tsx
import { lazy } from 'react';
import { Navigate, type RouteObject } from 'react-router-dom';
import { Paths } from './paths';
import { NotFoundRedirect } from './NotFoundRedirect';

const Dashboard = lazy(() => import('@/features/dashboard/DashboardPage'));
const MeetingsList = lazy(() => import('@/features/meetings-list/MeetingsListPage'));
const PreRecord = lazy(() => import('@/features/pre-record/PreRecordPage'));
const LiveRecording = lazy(() => import('@/features/live-recording/LiveRecordingPage'));
const MeetingDetail = lazy(() => import('@/features/meeting-detail/MeetingDetailPage'));
const Templates = lazy(() => import('@/features/templates/TemplatesPage'));
const Participants = lazy(() => import('@/features/participants/ParticipantsPage'));
const Settings = lazy(() => import('@/features/settings/SettingsPage'));

export const routes: RouteObject[] = [
  { path: Paths.Dashboard, element: <Dashboard /> },
  { path: Paths.Meetings, element: <MeetingsList /> },
  { path: '/meetings/:id', element: <MeetingDetail /> },
  { path: Paths.PreRecord, element: <PreRecord /> },
  { path: '/record/live/:sessionId', element: <LiveRecording /> },
  { path: Paths.Templates, element: <Templates /> },
  { path: Paths.Participants, element: <Participants /> },
  { path: Paths.Settings, element: <Settings /> },
  { path: '*', element: <NotFoundRedirect /> },
];
```

- [ ] **Step 4: Write `src/router/paths.test.ts`**

```ts
import { describe, it, expect } from 'vitest';
import { Paths } from './paths';

describe('Paths', () => {
  it('produces detail path with id', () => {
    expect(Paths.MeetingDetail('m-001')).toBe('/meetings/m-001');
  });
  it('produces live recording path with sessionId', () => {
    expect(Paths.LiveRecording('sess-123')).toBe('/record/live/sess-123');
  });
});
```

- [ ] **Step 5: Run tests**

```bash
pnpm test:run -- src/router
```

Expected: 2 passed.

- [ ] **Step 6: Commit**

```bash
git add src/router/
git commit -m "feat(router): typed Paths + lazy-loaded route tree with NotFoundRedirect"
```

---

### Task 3.6: ErrorBoundary + IPC commands wrapper

**Files:**
- Create: `src/ErrorBoundary.tsx`, `src/ipc/commands.ts`, `src/ipc/error.ts`, `src/ipc/events.ts`

- [ ] **Step 1: Write `src/ipc/commands.ts`**

```ts
import { invoke } from '@tauri-apps/api/core';
import * as bindings from './bindings';
export { bindings };

export const tauri = {
  listMeetings: bindings.listMeetings,
  getMeeting: bindings.getMeeting,
  updateMeetingTitle: bindings.updateMeetingTitle,
  toggleAction: bindings.toggleAction,
  renameParticipant: bindings.renameParticipant,
  listTemplates: bindings.listTemplates,
  setDefaultTemplate: bindings.setDefaultTemplate,
  listAudioDevices: bindings.listAudioDevices,
  getSettings: bindings.getSettings,
  updateSettings: bindings.updateSettings,
  logFrontendError: (level: 'error' | 'warn' | 'info', message: string, stack?: string) =>
    invoke('log_frontend_error', { level, message, stack }),
};
```

- [ ] **Step 2: Write `src/ipc/error.ts`**

```ts
import type { AppError } from '@/store/api/base';
import type { TKey } from '@/i18n/keys';

const codeToKey: Record<string, TKey> = {};

export function errorMessage(err: AppError, t: (k: TKey) => string): string {
  const key = codeToKey[err.code];
  return key ? t(key) : err.message;
}
```

- [ ] **Step 3: Write `src/ipc/events.ts`** (empty registry for Foundation)

```ts
// Event registry for sub-projects 2+. Empty in Foundation.
export type AppEvent = never;
```

- [ ] **Step 4: Write `src/ErrorBoundary.tsx`**

```tsx
import { Component, type ReactNode, type ErrorInfo } from 'react';
import { tauri } from './ipc/commands';

interface State { error: Error | null; }

export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    void tauri.logFrontendError('error', error.message, error.stack ?? info.componentStack ?? '');
  }

  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: 32, fontFamily: 'system-ui' }}>
          <h1>Something went wrong</h1>
          <pre style={{ whiteSpace: 'pre-wrap' }}>{this.state.error.message}</pre>
        </div>
      );
    }
    return this.props.children;
  }
}
```

- [ ] **Step 5: Commit**

```bash
git add src/ipc/commands.ts src/ipc/error.ts src/ipc/events.ts src/ErrorBoundary.tsx
git commit -m "feat(ipc): typed command wrappers + ErrorBoundary that forwards to backend log"
```

---

### Task 3.7: Wire App.tsx with Provider + Router + Theme + Error boundary

**Files:**
- Modify: `src/App.tsx`, `src/App.module.css`, `src/main.tsx`

- [ ] **Step 1: Rewrite `src/main.tsx`**

```tsx
import React from 'react';
import ReactDOM from 'react-dom/client';
import { Provider } from 'react-redux';
import { BrowserRouter } from 'react-router-dom';
import { store } from './store';
import App from './App';
import { ErrorBoundary } from './ErrorBoundary';
import './assets/fonts/fonts.css';
import './theme/tokens.module.css';
import './styles/reset.css';
import './styles/globals.css';
import './i18n';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ErrorBoundary>
      <Provider store={store}>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </Provider>
    </ErrorBoundary>
  </React.StrictMode>
);
```

- [ ] **Step 2: Rewrite `src/App.tsx`**

```tsx
import { Suspense, useEffect } from 'react';
import { useRoutes } from 'react-router-dom';
import { routes } from './router/routes';
import { ThemeProvider } from './theme/ThemeProvider';
import { useAppSelector, useAppDispatch } from './store/hooks';
import { useGetSettingsQuery } from './store/api/settings.api';
import { hydrateFromBackend } from './store/slices/ui.slice';
import styles from './App.module.css';

export default function App() {
  const dispatch = useAppDispatch();
  const ui = useAppSelector((s) => s.ui);
  const { data: settings } = useGetSettingsQuery();

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

  const element = useRoutes(routes);

  return (
    <ThemeProvider theme={ui.theme} accent={ui.accent} avatarStyle={ui.avatarStyle}>
      <div className={styles.app}>
        <Suspense fallback={<div className={styles.suspense}>Loading…</div>}>
          {element}
        </Suspense>
      </div>
    </ThemeProvider>
  );
}
```

- [ ] **Step 3: Rewrite `src/App.module.css`**

```css
.app {
  height: 100vh;
  display: flex;
  flex-direction: column;
}

.suspense {
  display: grid;
  place-items: center;
  height: 100%;
  color: var(--text-muted);
  font-size: 14px;
}
```

- [ ] **Step 4: Run dev — expect runtime error about missing feature pages**

```bash
pnpm tauri:dev
```

Expected: Compiles. Browser shows error from lazy() — feature pages don't exist yet. That's expected; we land them in Phase 7.

- [ ] **Step 5: Add temporary stub pages so the app can run**

For each of these eight files, create a stub component:

```
src/features/dashboard/DashboardPage.tsx
src/features/meetings-list/MeetingsListPage.tsx
src/features/pre-record/PreRecordPage.tsx
src/features/live-recording/LiveRecordingPage.tsx
src/features/meeting-detail/MeetingDetailPage.tsx
src/features/templates/TemplatesPage.tsx
src/features/participants/ParticipantsPage.tsx
src/features/settings/SettingsPage.tsx
```

Each file (replace `Dashboard` with the page name):

```tsx
export default function Dashboard() {
  return <div style={{ padding: 32 }}>Dashboard — to be implemented</div>;
}
```

- [ ] **Step 6: Run dev — verify app navigates**

```bash
pnpm tauri:dev
```

Expected: Window opens, shows "Dashboard — to be implemented". Manually navigate to `/meetings`, `/settings` etc. via address bar (briefly visible) — each stub renders.

- [ ] **Step 7: Commit**

```bash
git add src/App.tsx src/App.module.css src/main.tsx src/features/
git commit -m "feat: wire App.tsx with Redux + Router + Theme + Suspense + stub feature pages"
```

---

## Phase 4 — Primitives library

Each primitive lives under `src/components/primitives/<Name>/` with four files: `<Name>.tsx`, `<Name>.module.css`, `<Name>.test.tsx`, `<Name>.stories.tsx`.

**Approach:** Each primitive task follows the same shape — write the test, run it to confirm failure, write the implementation + CSS module (extracted from the relevant section of `handoff/.../app.css`), run the test to pass, write the story, commit.

To keep the plan readable, each primitive task documents:
- Source lines in `app.css` to extract
- The component API (props)
- The full test code
- The full implementation code

### Task 4.1: Button primitive

**Files:**
- Create: `src/components/primitives/Button/{Button.tsx,Button.module.css,Button.test.tsx,Button.stories.tsx}`
- Source CSS: `handoff/.../app.css` lines 267–280 (`.btn`, `.btn-primary`, `.btn-ghost`, `.btn-icon`, `.btn-danger`)

- [ ] **Step 1: Write `Button.test.tsx`**

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Button } from './Button';

describe('Button', () => {
  it('renders children', () => {
    render(<Button>Hello</Button>);
    expect(screen.getByRole('button', { name: 'Hello' })).toBeInTheDocument();
  });

  it('fires onClick when not disabled', async () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick}>Click</Button>);
    await userEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('does not fire onClick when disabled', async () => {
    const onClick = vi.fn();
    render(<Button onClick={onClick} disabled>Click</Button>);
    await userEvent.click(screen.getByRole('button'));
    expect(onClick).not.toHaveBeenCalled();
  });

  it('applies primary variant class', () => {
    const { container } = render(<Button variant="primary">P</Button>);
    expect(container.firstChild).toHaveClass('primary');
  });

  it('shows loading state', () => {
    render(<Button loading>Save</Button>);
    expect(screen.getByRole('button')).toHaveAttribute('aria-busy', 'true');
  });
});
```

- [ ] **Step 2: Write `Button.module.css`**

Extract `app.css:267-280` and adapt class names to camelCase:

```css
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 8px 14px;
  border-radius: var(--radius);
  font-size: 13px;
  font-weight: 500;
  border: 1px solid var(--stroke);
  background: var(--bg-surface);
  color: var(--text);
  transition: background var(--speed) var(--ease), border-color var(--speed) var(--ease), transform var(--speed) var(--ease);
  cursor: pointer;
}
.btn:hover:not(:disabled) { background: var(--bg-surface-hover); }
.btn:active:not(:disabled) { transform: scale(0.98); }
.btn:disabled { opacity: 0.5; cursor: not-allowed; }
.btn:focus-visible { outline: 2px solid var(--accent-ring); outline-offset: 2px; }

.primary { background: var(--accent); border-color: transparent; color: var(--text-on-accent); }
.primary:hover:not(:disabled) { background: var(--accent-hover); }
.primary:active:not(:disabled) { background: var(--accent-press); }

.ghost { background: transparent; border-color: transparent; }
.ghost:hover:not(:disabled) { background: var(--bg-surface-hover); }

.danger { color: #d4183d; }

.icon { padding: 8px; width: 34px; height: 34px; }

.sm { padding: 5px 10px; font-size: 12px; }
```

- [ ] **Step 3: Write `Button.tsx`**

```tsx
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from 'react';
import styles from './Button.module.css';

type Variant = 'default' | 'primary' | 'ghost' | 'danger';
type Size = 'sm' | 'md' | 'icon';

export interface ButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'children'> {
  variant?: Variant;
  size?: Size;
  loading?: boolean;
  icon?: ReactNode;
  children?: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = 'default', size = 'md', loading = false, icon, children, className, disabled, ...rest },
  ref,
) {
  const classes = [
    styles.btn,
    variant !== 'default' && styles[variant],
    size === 'icon' && styles.icon,
    size === 'sm' && styles.sm,
    className,
  ].filter(Boolean).join(' ');

  return (
    <button
      ref={ref}
      className={classes}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {icon}
      {children}
    </button>
  );
});
```

- [ ] **Step 4: Run test — expect PASS**

```bash
pnpm test:run -- src/components/primitives/Button
```

Expected: 5 passed.

- [ ] **Step 5: Write `Button.stories.tsx`**

```tsx
import type { Meta, StoryObj } from '@storybook/react';
import { Button } from './Button';

const meta: Meta<typeof Button> = { component: Button, title: 'Primitives/Button' };
export default meta;

type Story = StoryObj<typeof Button>;

export const Default: Story = { args: { children: 'Default' } };
export const Primary: Story = { args: { children: 'Primary', variant: 'primary' } };
export const Ghost: Story = { args: { children: 'Ghost', variant: 'ghost' } };
export const Danger: Story = { args: { children: 'Danger', variant: 'danger' } };
export const Loading: Story = { args: { children: 'Loading…', loading: true } };
export const Disabled: Story = { args: { children: 'Disabled', disabled: true } };
```

- [ ] **Step 6: Commit**

```bash
git add src/components/primitives/Button/
git commit -m "feat(primitives): Button — variants, sizes, loading, disabled states"
```

---

### Task 4.2–4.11: Remaining primitives

For each remaining primitive, follow the same pattern as Task 4.1. The source CSS sections, file names, and key props are tabulated:

| # | Primitive | `app.css` source | Files | Key API |
|---|---|---|---|---|
| 4.2 | `Card` | `.card`, `.card-pad` (lines 281–287) | `Card/{tsx, module.css, test.tsx, stories.tsx}` | `padding?: boolean`, `children` |
| 4.3 | `Chip` | `.chip`, `.chip-accent` (lines 289–296) | `Chip/...` | `variant?: 'default' \| 'accent'`, `icon?`, `children` |
| 4.4 | `Icon` | n/a (uses inline SVG) | `Icon/{Icon.tsx, icons.ts, Icon.module.css, Icon.test.tsx, Icon.stories.tsx}` | `name: IconName`, `size?: number`, `stroke?: string`, `fill?: string`. `icons.ts` exports the full `ICONS` map from `handoff/.../icons.jsx` and the `IconName = keyof typeof ICONS` type. |
| 4.5 | `Input` | `.input`, `.field-label` (lines 299–311) | `Input/...` | `value`, `onChange`, `type?`, `placeholder?`, `prefix?`, `suffix?` |
| 4.6 | `SearchBox` | `.search-box` (lines 314–325) | `SearchBox/...` | `value`, `onChange`, `placeholder?` (composes `Icon` + `Input`) |
| 4.7 | `SegmentedControl` | `.segmented` (search `app.css` for `.segmented`) | `SegmentedControl/...` | `value`, `options: {value, label}[]`, `onChange` |
| 4.8 | `Toggle` | `.toggle`, `.toggle.on` (search `app.css`) | `Toggle/...` | `on: boolean`, `onChange: (next: boolean) => void` |
| 4.9 | `Modal` | `.modal-backdrop`, `.modal`, `.modal-head`, `.modal-body`, `.modal-foot` | `Modal/...` | `open`, `onClose`, `title`, `subtitle?`, `children`, `footer?`. Uses `createPortal`. |
| 4.10 | `Toast` | n/a (wraps `sonner`) | `Toast/{tsx, module.css, test.tsx, stories.tsx}` | Re-export `<Toaster>` + a typed `toast.success/info/error` helper |
| 4.11 | `Avatar` | `.avatar`, `.avatar-stack`, `.s-color-1..8` (lines 370–390) | `Avatar/{SubjectAvatar.tsx, AvatarStack.tsx, Avatar.module.css, Avatar.test.tsx, Avatar.stories.tsx}` | `SubjectAvatar`: `participant`, `size?` / `AvatarStack`: `participants[]`, `max?: 4`, `size?: 26` |

For each primitive, the per-step pattern is identical to Task 4.1:

- [ ] **Step 1:** Write `<Name>.test.tsx` — test rendering, key props, accessibility (≥3 assertions).
- [ ] **Step 2:** Write `<Name>.module.css` — extract from the noted `app.css` lines, convert classes to camelCase, replace global selectors with module-scoped ones.
- [ ] **Step 3:** Write `<Name>.tsx` — TypeScript-typed props, `forwardRef` if applicable, no inline styles for layout.
- [ ] **Step 4:** Run `pnpm test:run -- src/components/primitives/<Name>` — verify pass.
- [ ] **Step 5:** Write `<Name>.stories.tsx` — at least one story per relevant variant.
- [ ] **Step 6:** Commit with message `feat(primitives): <Name> — <brief>`.

**Concrete examples for the less obvious ones:**

#### Icon (Task 4.4) — `icons.ts`

Port the entire `ICONS` object from `handoff/smart-noter/project/icons.jsx:4-59` to `src/components/primitives/Icon/icons.ts`:

```ts
export const ICONS = {
  home: 'M3 11.5 12 4l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z',
  // ... copy the entire object (50+ icons) from icons.jsx
  external: 'M14 4h6v6M20 4l-9 9M9 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3',
} as const;

export type IconName = keyof typeof ICONS;
export const FILLED_ICONS: IconName[] = ['record', 'play', 'stop', 'pause', 'send', 'briefcase'];
```

`Icon.tsx`:

```tsx
import type { SVGProps } from 'react';
import { ICONS, FILLED_ICONS, type IconName } from './icons';

export interface IconProps extends Omit<SVGProps<SVGSVGElement>, 'name'> {
  name: IconName;
  size?: number;
  stroke?: string;
  fill?: string;
  strokeWidth?: number;
}

export function Icon({
  name, size = 18, stroke = 'currentColor', fill = 'none',
  strokeWidth = 1.7, className, ...rest
}: IconProps) {
  const d = ICONS[name];
  const filled = FILLED_ICONS.includes(name);
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={filled ? stroke : fill}
      stroke={filled ? 'none' : stroke}
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
      {...rest}
    >
      <path d={d} />
    </svg>
  );
}
```

#### Modal (Task 4.9) — uses portal and keyboard handling

`Modal.tsx`:

```tsx
import { useEffect, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import styles from './Modal.module.css';

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  subtitle?: string;
  children: ReactNode;
  footer?: ReactNode;
}

export function Modal({ open, onClose, title, subtitle, children, footer }: ModalProps) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [open, onClose]);

  if (!open) return null;
  return createPortal(
    <div className={styles.backdrop} role="dialog" aria-modal="true" onClick={onClose}>
      <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
        <div className={styles.head}>
          <h2 className={styles.title}>{title}</h2>
          {subtitle && <p className={styles.subtitle}>{subtitle}</p>}
        </div>
        <div className={styles.body}>{children}</div>
        {footer && <div className={styles.foot}>{footer}</div>}
      </div>
    </div>,
    document.body,
  );
}
```

After completing all 11 primitives, run:

```bash
pnpm test:run -- src/components/primitives --coverage
```

Expected: All tests pass; coverage on `components/primitives/**` ≥ 90% (enforced by `vitest.config.ts` thresholds).

---

## Phase 5 — Shell components

### Task 5.1: WindowChrome

**Files:**
- Create: `src/components/shell/WindowChrome/{WindowChrome.tsx,WindowChrome.module.css,WindowChrome.test.tsx,WindowChrome.stories.tsx}`
- Source: `handoff/.../chrome.jsx:4-26`, `app.css:127-163`

- [ ] **Step 1: Write `WindowChrome.module.css`**

Extract `app.css` lines 127–163 (`.win-shell`, `.win-window`, `.win-titlebar`, `.win-title`, `.win-controls`, `.win-ctrl`). Convert global selectors to module-scoped:

```css
.shell { position: absolute; inset: 0; }
.window { /* ...from app.css... */ }
.titlebar { /* ...from app.css... */ -webkit-app-region: drag; }
.title { display: flex; align-items: center; gap: 8px; font-size: 12px; color: var(--text-muted); font-weight: 500; padding: 0 12px; }
.controls { display: flex; -webkit-app-region: no-drag; height: 100%; }
.ctrl { width: 46px; height: 100%; display: grid; place-items: center; color: var(--text-muted); transition: background var(--speed) var(--ease); background: transparent; border: 0; }
.ctrl:hover { background: var(--bg-surface-hover); }
.ctrl.close:hover { background: #e81123; color: white; }
.brandMark { width: 16px; height: 16px; border-radius: 4px; background: var(--accent); display: grid; place-items: center; }
```

- [ ] **Step 2: Write `WindowChrome.tsx`**

```tsx
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { Icon } from '@/components/primitives/Icon/Icon';
import { useT } from '@/i18n/useT';
import styles from './WindowChrome.module.css';

export interface WindowChromeProps {
  title?: string;
}

export function WindowChrome({ title }: WindowChromeProps) {
  const { t } = useT();
  const win = getCurrentWebviewWindow();

  return (
    <div className={styles.titlebar} data-tauri-drag-region>
      <div className={styles.title} data-tauri-drag-region>
        <div className={styles.brandMark}>
          <Icon name="mic" size={10} stroke="white" />
        </div>
        <span data-tauri-drag-region>{t('appName')} — {title ?? ''}</span>
      </div>
      <div className={styles.controls}>
        <button className={styles.ctrl} onClick={() => win.minimize()} title="Minimize">
          <svg viewBox="0 0 10 10" width="10" height="10"><rect x="1" y="5" width="8" height="1" fill="currentColor" /></svg>
        </button>
        <button className={styles.ctrl} onClick={() => win.toggleMaximize()} title="Maximize">
          <svg viewBox="0 0 10 10" width="10" height="10" fill="none"><rect x="1" y="1" width="8" height="8" stroke="currentColor" /></svg>
        </button>
        <button className={`${styles.ctrl} ${styles.close}`} onClick={() => win.close()} title="Close">
          <svg viewBox="0 0 10 10" width="10" height="10" stroke="currentColor" strokeWidth={1}>
            <line x1="1" y1="1" x2="9" y2="9" />
            <line x1="9" y1="1" x2="1" y2="9" />
          </svg>
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Write `WindowChrome.test.tsx`**

```tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { WindowChrome } from './WindowChrome';

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
  }),
}));

describe('WindowChrome', () => {
  it('renders app name + provided title', () => {
    render(<WindowChrome title="Dashboard" />);
    expect(screen.getByText(/Smart Noter — Dashboard/)).toBeInTheDocument();
  });

  it('renders three window controls', () => {
    render(<WindowChrome title="X" />);
    expect(screen.getByTitle('Minimize')).toBeInTheDocument();
    expect(screen.getByTitle('Maximize')).toBeInTheDocument();
    expect(screen.getByTitle('Close')).toBeInTheDocument();
  });
});
```

- [ ] **Step 4: Run test + commit**

```bash
pnpm test:run -- src/components/shell/WindowChrome
git add src/components/shell/WindowChrome/
git commit -m "feat(shell): WindowChrome with custom Win11 titlebar + drag region"
```

---

### Task 5.2: Sidebar

**Files:**
- Create: `src/components/shell/Sidebar/{Sidebar.tsx,Sidebar.module.css,Sidebar.test.tsx,Sidebar.stories.tsx}`
- Source: `handoff/.../chrome.jsx:28-73`, `app.css:165-247`

- [ ] **Step 1: Write `Sidebar.module.css`**

Extract `app.css:165-247` (`.win-sidebar`, `.brand`, `.brand-mark`, `.nav-section`, `.nav-section-label`, `.nav-item`, `.cta-record`, `.sidebar-footer`, `.count`). Convert classes to camelCase as module rules.

- [ ] **Step 2: Write `Sidebar.tsx`**

```tsx
import { NavLink, useNavigate, useLocation } from 'react-router-dom';
import { Icon, type IconName } from '@/components/primitives/Icon/Icon';
import { Paths } from '@/router/paths';
import { useT } from '@/i18n/useT';
import { useListMeetingsQuery } from '@/store/api/meetings.api';
import styles from './Sidebar.module.css';

interface NavEntry {
  to: string;
  icon: IconName;
  label: string;
  count?: number;
}

export function Sidebar() {
  const navigate = useNavigate();
  const location = useLocation();
  const { t } = useT();
  const { data: meetings } = useListMeetingsQuery();

  const workspace: NavEntry[] = [
    { to: Paths.Dashboard, icon: 'home', label: t('navDashboard') },
    { to: Paths.Meetings, icon: 'list', label: t('navMeetings'), count: meetings?.length },
    { to: Paths.Templates, icon: 'templates', label: t('navTemplates') },
  ];
  const tools: NavEntry[] = [
    { to: Paths.Participants, icon: 'user', label: t('participants') },
    { to: Paths.Settings, icon: 'settings', label: t('navSettings') },
  ];

  const isMeetingDetail = location.pathname.startsWith('/meetings/');

  return (
    <aside className={styles.sidebar}>
      <div className={styles.brand}>
        <div className={styles.brandMark}><Icon name="mic" size={16} stroke="white" /></div>
        <div>
          <div className={styles.brandName}>{t('appName')}</div>
          <div className={styles.brandSub}>{t('appTag')}</div>
        </div>
      </div>
      <button className={styles.ctaRecord} onClick={() => navigate(Paths.PreRecord)}>
        <div className={styles.dot} />
        {t('navRecord')}
      </button>
      <div className={styles.navSection}>
        <div className={styles.navSectionLabel}>{t('navWorkspace')}</div>
        {workspace.map((it) => (
          <NavLink
            key={it.to}
            to={it.to}
            className={({ isActive }) =>
              `${styles.navItem} ${isActive || (it.to === Paths.Meetings && isMeetingDetail) ? styles.active : ''}`}
          >
            <Icon name={it.icon} size={18} />
            <span>{it.label}</span>
            {it.count != null && <span className={styles.count}>{it.count}</span>}
          </NavLink>
        ))}
      </div>
      <div className={styles.navSection}>
        <div className={styles.navSectionLabel}>{t('navTools')}</div>
        {tools.map((it) => (
          <NavLink key={it.to} to={it.to}
            className={({ isActive }) => `${styles.navItem} ${isActive ? styles.active : ''}`}>
            <Icon name={it.icon} size={18} />
            <span>{it.label}</span>
          </NavLink>
        ))}
      </div>
      <div className={styles.sidebarFooter}>
        <div className={styles.avatar}>CR</div>
        <div>
          <div className={styles.name}>Carlos Rivera</div>
          <div className={styles.role}>Pro · v0.1.0</div>
        </div>
      </div>
    </aside>
  );
}
```

- [ ] **Step 3: Write `Sidebar.test.tsx`**

```tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { Provider } from 'react-redux';
import { store } from '@/store';
import { Sidebar } from './Sidebar';
import '@/i18n';

function setup() {
  return render(
    <Provider store={store}>
      <MemoryRouter initialEntries={['/']}>
        <Sidebar />
      </MemoryRouter>
    </Provider>
  );
}

describe('Sidebar', () => {
  it('shows brand name', () => {
    setup();
    expect(screen.getByText('Smart Noter')).toBeInTheDocument();
  });
  it('renders all workspace nav entries', () => {
    setup();
    expect(screen.getByText('Inicio')).toBeInTheDocument();
    expect(screen.getByText('Reuniones')).toBeInTheDocument();
    expect(screen.getByText('Plantillas')).toBeInTheDocument();
  });
});
```

- [ ] **Step 4: Run + commit**

```bash
pnpm test:run -- src/components/shell/Sidebar
git add src/components/shell/Sidebar/
git commit -m "feat(shell): Sidebar with brand, CTA, two nav sections, user footer"
```

---

## Phase 6 — Domain components

Same shape as primitives — test, CSS module, component, commit. Per-component summary:

| # | Domain component | Source | Files |
|---|---|---|---|
| 6.1 | `TemplateIcon` | `primitives.jsx:31-37`, `app.css:393-401` | `TemplateIcon/{tsx, module.css, test.tsx}` |
| 6.2 | `MeetingRow` | `screens-dashboard.jsx:4-21`, `app.css:347-367` | `MeetingRow/...` |
| 6.3 | `DevicePill` | extracted from `screens-dashboard.jsx:82-90`, `app.css:413-420` | `DevicePill/...` |
| 6.4 | `EqBar` | `app.css:422-434` | `EqBar/...` (5 or 8 animated spans) |
| 6.5 | `LevelBar` | `app.css:513-521` | `LevelBar/...` (level prop 0..1) |
| 6.6 | `Waveform` | `screens-record.jsx:152-160`, `app.css:567-577` | `Waveform/...` (animated bars) |
| 6.7 | `LivePill` | `screens-record.jsx:132-133`, `app.css:536-549` | `LivePill/...` |
| 6.8 | `Skeleton` | new (Fluent shimmer) | `Skeleton/...` (used in Suspense fallbacks) |

For each one:

- [ ] **Step 1:** Write `<Name>.test.tsx` (≥2 assertions — what it renders + key prop variation).
- [ ] **Step 2:** Write `<Name>.module.css` from referenced `app.css` lines.
- [ ] **Step 3:** Write `<Name>.tsx` (typed props, ~30–80 LOC).
- [ ] **Step 4:** Run `pnpm test:run -- src/components/domain/<Name>` → pass.
- [ ] **Step 5:** Commit: `feat(domain): <Name>`.

**Example skeleton — `MeetingRow.tsx`** (since it's used by Dashboard and Meetings List):

```tsx
import { useT } from '@/i18n/useT';
import { TemplateIcon } from '@/components/domain/TemplateIcon/TemplateIcon';
import { AvatarStack } from '@/components/primitives/Avatar/AvatarStack';
import { fmtDate, fmtDuration, pickL } from '@/utils/format';
import type { MeetingSummary } from '@/ipc/bindings';
import styles from './MeetingRow.module.css';

export interface MeetingRowProps {
  meeting: MeetingSummary;
  onClick?: () => void;
}

export function MeetingRow({ meeting, onClick }: MeetingRowProps) {
  const { lang } = useT();
  return (
    <button className={styles.row} onClick={onClick} type="button">
      <TemplateIcon templateId={meeting.template} size={44} />
      <div className={styles.meta}>
        <div className={styles.title}>{pickL(meeting.title, lang)}</div>
        <div className={styles.sub}>
          <span>{meeting.template}</span>
          <span className={styles.sep} />
          <span>{fmtDate(meeting.date, lang)}</span>
          <span className={styles.sep} />
          <span>{meeting.participants.length} {lang === 'es' ? 'participantes' : 'participants'}</span>
        </div>
      </div>
      <AvatarStack participants={meeting.participants} size={26} max={4} />
      <div className={styles.duration}>{fmtDuration(meeting.durationSec)}</div>
    </button>
  );
}
```

**Also write `src/utils/format.ts`** (referenced above) at the start of Phase 6 — port from `data.js:623-637`:

```ts
import type { Bilingual } from '@/ipc/bindings';

export function fmtDuration(sec: number): string {
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  const pad = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

export function fmtDate(iso: string, lang: 'es' | 'en' = 'es'): string {
  const d = new Date(iso);
  return d.toLocaleString(lang === 'en' ? 'en-US' : 'es-MX',
    { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' });
}

export function pickL(b: Bilingual | undefined | null, lang: 'es' | 'en'): string {
  if (!b) return '';
  return lang === 'en' ? (b.en ?? b.es) : b.es;
}
```

With unit tests in `src/utils/format.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { fmtDuration, fmtDate, pickL } from './format';

describe('fmtDuration', () => {
  it('formats minutes:seconds when under an hour', () => {
    expect(fmtDuration(125)).toBe('2:05');
  });
  it('formats hours:minutes:seconds when over an hour', () => {
    expect(fmtDuration(3725)).toBe('1:02:05');
  });
});

describe('pickL', () => {
  it('returns es by default', () => {
    expect(pickL({ es: 'Hola', en: 'Hi' }, 'es')).toBe('Hola');
  });
  it('returns en when lang en', () => {
    expect(pickL({ es: 'Hola', en: 'Hi' }, 'en')).toBe('Hi');
  });
  it('falls back to es when en missing', () => {
    expect(pickL({ es: 'Hola' } as any, 'en')).toBe('Hola');
  });
});
```

After all eight domain components + format util are done:

```bash
pnpm test:run -- src/components/domain src/utils
git add src/components/domain/ src/utils/
git commit -m "feat(domain): TemplateIcon, MeetingRow, DevicePill, EqBar, LevelBar, Waveform, LivePill, Skeleton + format utils"
```

---

> **Plan continues in Part 3:** [foundation-plan-part3.md](./2026-05-17-foundation-plan-part3.md) covers Phase 7 (feature screens), Phase 8 (Storybook + quality gates), Phase 9 (E2E + visual regression), Phase 10 (CI), and Phase 11 (final validation + DoD checklist).
