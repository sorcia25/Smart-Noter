import { store } from '@/store';
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import ParticipantsPage from './ParticipantsPage';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_meetings') return [];
    if (cmd === 'get_settings') return null;
    return null;
  }),
}));

function setup() {
  return render(
    <Provider store={store}>
      <MemoryRouter>
        <ParticipantsPage />
      </MemoryRouter>
    </Provider>
  );
}

describe('ParticipantsPage', () => {
  it('renders the page title', async () => {
    setup();
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Participantes' })).toBeInTheDocument();
    });
  });

  it('exposes data-screen-label and shows the empty state when no meetings exist', async () => {
    const { container } = setup();
    expect(container.querySelector('[data-screen-label="07 Participants"]')).toBeTruthy();
    await waitFor(() => {
      expect(screen.getByText(/Sin reuniones todavía/i)).toBeInTheDocument();
    });
  });
});
