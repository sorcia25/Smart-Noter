import type { AppSettings, WhisperModelInfo } from '@/ipc/bindings';
import * as tauriCore from '@tauri-apps/api/core';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { TranscriptionPanel } from './TranscriptionPanel';

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const fakeModels: WhisperModelInfo[] = [
  { id: 'base', name: 'Whisper Base', sizeMb: 148, downloaded: false },
  { id: 'large-v3', name: 'Whisper Large v3', sizeMb: 3094, downloaded: true },
];

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_whisper_models') return fakeModels;
    return null;
  }),
}));

const baseDraft: AppSettings = {
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
  transcriptionModel: 'large-v3',
  autoTranscribe: false,
  nativeLanguage: 'es',
  defaultTemplate: 'tecnica',
};

describe('TranscriptionPanel', () => {
  it('shows a Download button for models not yet downloaded', async () => {
    render(<TranscriptionPanel draft={baseDraft} patch={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByRole('button').length).toBeGreaterThan(0));
    const buttons = screen.getAllByRole('button');
    const downloadBtns = buttons.filter((b) => b.textContent?.includes('Descargar'));
    expect(downloadBtns.length).toBeGreaterThan(0);
  });

  it('clicking Download invokes download_whisper_model with the model id', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    invokeMock.mockClear();
    render(<TranscriptionPanel draft={baseDraft} patch={vi.fn()} />);
    const downloadBtn = await screen.findByRole('button', { name: /descargar/i });
    await userEvent.click(downloadBtn);
    expect(invokeMock.mock.calls.some((c) => c[0] === 'download_whisper_model')).toBe(true);
  });

  it('toggling autoTranscribe calls patch with autoTranscribe toggled', async () => {
    const patchMock = vi.fn();
    render(<TranscriptionPanel draft={baseDraft} patch={patchMock} />);
    const toggle = await screen.findByRole('switch');
    await userEvent.click(toggle);
    expect(patchMock).toHaveBeenCalledWith({ autoTranscribe: true });
  });

  it('renders the provider selector with local selected by default', async () => {
    render(<TranscriptionPanel draft={baseDraft} patch={vi.fn()} />);
    const select = await screen.findByRole('combobox', { name: /proveedor/i });
    expect((select as HTMLSelectElement).value).toBe('local');
  });

  it('switching provider to openai calls patch with transcriptionProvider openai', async () => {
    const patchMock = vi.fn();
    render(<TranscriptionPanel draft={baseDraft} patch={patchMock} />);
    const select = await screen.findByRole('combobox', { name: /proveedor/i });
    await userEvent.selectOptions(select, 'openai');
    expect(patchMock).toHaveBeenCalledWith({ transcriptionProvider: 'openai' });
  });

  it('selecting azure shows the Whisper deployment input', async () => {
    const azureDraft: AppSettings = {
      ...baseDraft,
      transcriptionProvider: 'azure',
      transcriptionModels: { azure: 'my-whisper-deployment' },
    };
    render(<TranscriptionPanel draft={azureDraft} patch={vi.fn()} />);
    const input = await screen.findByPlaceholderText(/deployment/i);
    expect(input).toBeInTheDocument();
  });
});
