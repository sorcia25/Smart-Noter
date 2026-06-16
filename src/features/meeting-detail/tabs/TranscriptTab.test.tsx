import type { MeetingDetail } from '@/ipc/bindings';
import { store } from '@/store';
import * as tauriCore from '@tauri-apps/api/core';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { TranscriptTab } from './TranscriptTab';

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'get_settings')
      return { autoTranscribe: true, transcriptionModel: 'large-v3', nativeLanguage: 'es' };
    if (cmd === 'get_transcription_state') return null;
    return null;
  }),
}));

const baseMeeting: MeetingDetail = {
  id: 'm-1',
  title: { es: 'Test', en: null },
  template: 'tecnica',
  date: '2026-06-16',
  durationSec: 10,
  deviceUsed: null,
  wordCount: 0,
  summary: null,
  participants: [],
  actions: [],
  decisions: [],
  blockers: [],
  transcript: [],
};

function renderTab(meeting: MeetingDetail, state?: object) {
  return render(
    <Provider store={store}>
      <MemoryRouter initialEntries={[{ pathname: '/m', state }]}>
        <TranscriptTab meeting={meeting} />
      </MemoryRouter>
    </Provider>
  );
}

describe('TranscriptTab', () => {
  it('auto-transcribes a just-recorded meeting when autoTranscribe is on', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    invokeMock.mockClear();
    renderTab(baseMeeting, { justRecorded: true });
    await waitFor(() =>
      expect(invokeMock.mock.calls.some((c) => c[0] === 'transcribe_meeting')).toBe(true)
    );
  });

  it('does NOT auto-transcribe on a plain visit (no justRecorded)', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    invokeMock.mockClear();
    renderTab(baseMeeting);
    await new Promise((r) => setTimeout(r, 20));
    expect(invokeMock.mock.calls.some((c) => c[0] === 'transcribe_meeting')).toBe(false);
    expect(screen.getByText(/Transcribir/i)).toBeInTheDocument();
  });

  it('clicking Transcribe invokes transcribe_meeting', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    invokeMock.mockClear();
    renderTab(baseMeeting);
    await userEvent.click(await screen.findByText(/Transcribir/i));
    expect(invokeMock.mock.calls.some((c) => c[0] === 'transcribe_meeting')).toBe(true);
  });
});
