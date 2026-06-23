import { configureStore } from '@reduxjs/toolkit';
import { setupListeners } from '@reduxjs/toolkit/query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { baseApi } from '@/store/api/base';
import { uiSlice } from '@/store/slices/ui.slice';
import TrashPage from './TrashPage';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

function makeStore() {
  const s = configureStore({
    reducer: {
      ui: uiSlice.reducer,
      [baseApi.reducerPath]: baseApi.reducer,
    },
    middleware: (gdm) => gdm().concat(baseApi.middleware),
  });
  setupListeners(s.dispatch);
  return s;
}

describe('TrashPage', () => {
  beforeEach(() => invoke.mockReset());

  it('restores a trashed meeting', async () => {
    invoke.mockImplementation((cmd: string) =>
      cmd === 'list_trashed_meetings'
        ? Promise.resolve([
            {
              id: 'm1',
              title: { es: 'M 1', en: null },
              template: 'tecnica',
              date: '2026-06-01T00:00:00Z',
              durationSec: 10,
              participants: [],
              wordCount: 0,
            },
          ])
        : Promise.resolve(undefined)
    );
    render(
      <Provider store={makeStore()}>
        <MemoryRouter>
          <TrashPage />
        </MemoryRouter>
      </Provider>
    );
    await screen.findByText(/M 1/i);
    fireEvent.click(screen.getByTestId('restore-m1'));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('restore_meeting', { id: 'm1' }));
  });
});
