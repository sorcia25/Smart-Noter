import { store } from '@/store';
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import PreRecordPage from './PreRecordPage';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_audio_devices') return [];
    if (cmd === 'list_templates') return [];
    if (cmd === 'get_settings') return null;
    return null;
  }),
}));

function setup() {
  return render(
    <Provider store={store}>
      <MemoryRouter>
        <PreRecordPage />
      </MemoryRouter>
    </Provider>
  );
}

describe('PreRecordPage', () => {
  it('renders the pre-record title', async () => {
    setup();
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Nueva grabación' })).toBeInTheDocument();
    });
  });

  it('exposes data-screen-label', () => {
    const { container } = setup();
    expect(container.querySelector('[data-screen-label="03 Pre-record"]')).toBeTruthy();
  });
});
