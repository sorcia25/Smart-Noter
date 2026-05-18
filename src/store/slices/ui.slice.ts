import { type PayloadAction, createSlice } from '@reduxjs/toolkit';

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
  } catch {
    /* ignore */
  }
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
    setTheme(state, action: PayloadAction<Theme>) {
      state.theme = action.payload;
    },
    setAccent(state, action: PayloadAction<string>) {
      state.accent = action.payload;
    },
    setLanguage(state, action: PayloadAction<Language>) {
      state.language = action.payload;
    },
    setAvatarStyle(state, action: PayloadAction<AvatarStyle>) {
      state.avatarStyle = action.payload;
    },
    toggleAiChat(state) {
      state.aiChatVisible = !state.aiChatVisible;
    },
    hydrateFromBackend(state, action: PayloadAction<Partial<UiState>>) {
      Object.assign(state, action.payload, { ready: true });
    },
  },
});

export const {
  setTheme,
  setAccent,
  setLanguage,
  setAvatarStyle,
  toggleAiChat,
  hydrateFromBackend,
} = uiSlice.actions;
