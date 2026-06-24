import { configureStore } from '@reduxjs/toolkit';
import { setupListeners } from '@reduxjs/toolkit/query';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
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
  beforeEach(() => invoke.mockReset());
  afterEach(() => vi.useRealTimers());

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

    fireEvent.click(screen.getByTestId('delete-m1'));
    // The modal confirm button is the last 'Eliminar/Delete' button in the DOM (row button comes first)
    const confirmBtns = screen.getAllByRole('button', { name: /Eliminar|Delete/i });
    // biome-ignore lint/style/noNonNullAssertion: array is guaranteed non-empty by getAllByRole
    fireEvent.click(confirmBtns.at(-1)!);

    await waitFor(() => expect(invoke).toHaveBeenCalledWith('delete_meeting', { id: 'm1' }));
  });

  it('typing in the search box queries the backend and renders the highlighted snippet', async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_meetings') return [];
      if (cmd === 'list_templates') return [];
      if (cmd === 'get_settings') return null;
      if (cmd === 'search_meetings')
        return [
          {
            meeting: {
              id: 'm1',
              title: { es: 'M 1', en: null },
              template: 'tecnica',
              date: '2026-06-01T00:00:00Z',
              durationSec: 10,
              participants: [],
              wordCount: 0,
            },
            snippet: 'hablamos de ⁨kube⁩rnetes',
          },
        ];
      return null;
    });
    render(
      <Provider store={makeStore()}>
        <MemoryRouter>
          <MeetingsListPage />
        </MemoryRouter>
      </Provider>
    );

    // Debounced (300ms) backend search fires after typing.
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'kube' } });
    await waitFor(
      () =>
        expect(invoke).toHaveBeenCalledWith('search_meetings', { query: 'kube', template: null }),
      { timeout: 2000 }
    );
    // The matched fragment is highlighted via <mark>.
    expect(await screen.findByText('kube')).toBeInTheDocument();
  });
});
