import { configureStore } from '@reduxjs/toolkit';
import { setupListeners } from '@reduxjs/toolkit/query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { baseApi } from '@/store/api/base';
import { uiSlice } from '@/store/slices/ui.slice';
import MeetingsListPage from './MeetingsListPage';

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

function setup() {
  invoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'list_meetings') return [];
    if (cmd === 'list_templates') return [];
    if (cmd === 'get_settings') return null;
    return null;
  });
  return render(
    <Provider store={makeStore()}>
      <MemoryRouter>
        <MeetingsListPage />
      </MemoryRouter>
    </Provider>
  );
}

function renderPage() {
  invoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'list_meetings')
      return [
        {
          id: 'm1',
          title: { es: 'M 1', en: null },
          template: 'tecnica',
          date: '2026-06-01T00:00:00Z',
          durationSec: 10,
          participants: [],
          wordCount: 0,
        },
      ];
    if (cmd === 'list_templates') return [];
    if (cmd === 'get_settings') return null;
    return undefined;
  });
  render(
    <Provider store={makeStore()}>
      <MemoryRouter>
        <MeetingsListPage />
      </MemoryRouter>
    </Provider>
  );
}

describe('MeetingsListPage', () => {
  it('renders heading and subtitle', async () => {
    setup();
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Reuniones' })).toBeInTheDocument();
    });
  });

  it('exposes data-screen-label', () => {
    const { container } = setup();
    expect(container.querySelector('[data-screen-label="02 Meetings list"]')).toBeTruthy();
  });

  it('shows the empty state when there are no meetings', async () => {
    setup();
    await waitFor(() => {
      expect(screen.getByText(/Sin reuniones que coincidan/i)).toBeInTheDocument();
    });
  });

  it('deleting a meeting calls delete_meeting after confirm', async () => {
    renderPage();
    await screen.findByText(/M 1/i);

    fireEvent.click(screen.getByLabelText('delete-m1'));
    // Click the confirm button in the modal footer (role=button, same text as title but is a button)
    fireEvent.click(screen.getByRole('button', { name: /Mover a la papelera|Move to Trash/i }));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('delete_meeting', { id: 'm1' }));
  });
});
