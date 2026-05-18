import { configureStore } from '@reduxjs/toolkit';
import { setupListeners } from '@reduxjs/toolkit/query';
import { baseApi } from './api/base';
import { uiSlice } from './slices/ui.slice';

export const store = configureStore({
  reducer: {
    ui: uiSlice.reducer,
    [baseApi.reducerPath]: baseApi.reducer,
  },
  middleware: (gdm) => gdm().concat(baseApi.middleware),
});

setupListeners(store.dispatch);

store.subscribe(() => {
  const { theme, accent, language, avatarStyle, aiChatVisible } = store.getState().ui;
  try {
    localStorage.setItem(
      'smart-noter:ui',
      JSON.stringify({ theme, accent, language, avatarStyle, aiChatVisible })
    );
  } catch {
    /* ignore */
  }
});

export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
