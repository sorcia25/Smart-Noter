import { store } from '@/store';
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import MeetingsListPage from './MeetingsListPage';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_meetings') return [];
    if (cmd === 'list_templates') return [];
    if (cmd === 'get_settings') return null;
    return null;
  }),
}));

function setup() {
  return render(
    <Provider store={store}>
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
});
