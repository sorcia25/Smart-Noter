import { store } from '@/store';
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import DashboardPage from './DashboardPage';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_meetings') return [];
    if (cmd === 'list_audio_devices') return [];
    if (cmd === 'list_templates') return [];
    if (cmd === 'get_settings') {
      return {
        theme: 'light',
        accent: '#10b981',
        language: 'es',
        avatarStyle: 'circle',
        aiChatVisible: true,
        captureMode: 'system',
        defaultDevice: '',
        recordingQuality: 'high',
        runLocal: true,
        autoDeleteAudio: false,
        transcriptionProvider: 'whisper',
        transcriptionModel: 'small',
        defaultTemplate: 'ejecutiva',
      };
    }
    return null;
  }),
}));

function setup() {
  return render(
    <Provider store={store}>
      <MemoryRouter>
        <DashboardPage />
      </MemoryRouter>
    </Provider>
  );
}

describe('DashboardPage', () => {
  it('renders welcome heading', async () => {
    setup();
    await waitFor(() => {
      expect(screen.getByText(/Buenas tardes, Toño/i)).toBeInTheDocument();
    });
  });

  it('exposes data-screen-label for e2e tests', () => {
    const { container } = setup();
    expect(container.querySelector('[data-screen-label="01 Dashboard"]')).toBeTruthy();
  });

  it('renders the four stat cards', async () => {
    setup();
    await waitFor(() => {
      expect(screen.getByText('Horas grabadas')).toBeInTheDocument();
    });
    expect(screen.getByText('Acciones pendientes')).toBeInTheDocument();
    expect(screen.getByText('Palabras transcritas')).toBeInTheDocument();
  });
});
