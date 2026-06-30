import { store } from '@/store';
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import SettingsPage from './SettingsPage';

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_audio_devices') return [];
    if (cmd === 'list_templates') return [];
    if (cmd === 'list_whisper_models') return [];
    if (cmd === 'list_diarization_models') return [];
    if (cmd === 'list_llm_models') return [];
    if (cmd === 'get_provider_config') return [];
    if (cmd === 'get_settings') {
      return {
        theme: 'light',
        accent: '#10b981',
        language: 'es',
        avatarStyle: 'circle',
        aiChatVisible: true,
        captureMode: 'system',
        defaultDevice: 'system-loopback',
        recordingQuality: 'WAV 48k',
        runLocal: true,
        autoDeleteAudio: false,
        transcriptionProvider: 'local',
        transcriptionModel: 'Whisper Large v3',
        autoTranscribe: false,
        nativeLanguage: 'es',
        defaultTemplate: 'tecnica',
      };
    }
    return null;
  }),
}));

function setup() {
  return render(
    <Provider store={store}>
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>
    </Provider>
  );
}

describe('SettingsPage', () => {
  it('renders the title', async () => {
    setup();
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Configuración' })).toBeInTheDocument();
    });
  });

  it('exposes data-screen-label', () => {
    const { container } = setup();
    expect(container.querySelector('[data-screen-label="08 Settings"]')).toBeTruthy();
  });

  it('shows the TranscriptionPanel provider selector defaulting to local', async () => {
    setup();
    await waitFor(() => {
      const sel = screen.getByRole('combobox', { name: /proveedor de transcripción/i });
      expect(sel).toBeInTheDocument();
      expect((sel as HTMLSelectElement).value).toBe('local');
    });
  });

  it('renders MP3 quality options as disabled (deferred to Sub-7 Export)', async () => {
    setup();
    await waitFor(() => {
      const mp3192 = screen.getByRole('tab', { name: 'MP3 192k' });
      const mp3320 = screen.getByRole('tab', { name: 'MP3 320k' });
      expect(mp3192).toBeDisabled();
      expect(mp3320).toBeDisabled();
      expect(screen.getByRole('tab', { name: 'WAV 48k' })).toBeEnabled();
    });
  });
});
