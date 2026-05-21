import { store } from '@/store';
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import PreRecordPage from './PreRecordPage';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_audio_devices') {
      return [{
        id: 'd-L-test',
        name: 'Test Speakers',
        kind: 'loopback',
        sampleRate: 48000,
        channels: 2,
        isDefault: true,
        recommended: true,
      }];
    }
    if (cmd === 'start_preview') return null;
    if (cmd === 'stop_preview') return null;
    if (cmd === 'list_templates') return [];
    if (cmd === 'get_settings') return null;
    return null;
  }),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
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
