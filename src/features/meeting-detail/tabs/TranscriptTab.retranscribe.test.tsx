import type { MeetingAudioInfo, MeetingDetail, Participant } from '@/ipc/bindings';
import { store } from '@/store';
import * as tauriCore from '@tauri-apps/api/core';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { TranscriptTab } from './TranscriptTab';

// Mutable per-test response for `get_meeting_audio` — defaults to a saved
// audio file so `hasAudio` resolves true; the "no audio" test overrides it.
let audioResponse: MeetingAudioInfo | null = {
  path: 'C:/meetings/m-2/audio.wav',
  sizeBytes: 123456,
  mimeType: 'audio/wav',
};

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'get_settings')
      return { autoTranscribe: true, transcriptionModel: 'large-v3', nativeLanguage: 'es' };
    if (cmd === 'get_transcription_state') return null;
    if (cmd === 'get_meeting_audio') return audioResponse;
    if (cmd === 'transcribe_meeting') return null;
    return null;
  }),
}));

const p1: Participant = {
  id: 'p1',
  meetingId: 'm-2',
  label: 'S1',
  name: null,
  colorClass: 'c1',
  wordCount: 2,
  talkPct: 50,
};

const p2: Participant = {
  id: 'p2',
  meetingId: 'm-2',
  label: 'S2',
  name: null,
  colorClass: 'c2',
  wordCount: 2,
  talkPct: 50,
};

const twoSpeakerMeeting: MeetingDetail = {
  id: 'm-2',
  title: { es: 'Reunión con dos hablantes', en: null },
  template: 'tecnica',
  date: '2026-07-08',
  durationSec: 60,
  deviceUsed: null,
  wordCount: 4,
  summary: null,
  participants: [p1, p2],
  actions: [],
  decisions: [],
  blockers: [],
  transcript: [
    { id: 1, t: '00:00', speakerId: 'p1', text: { es: 'Hola', en: null } },
    { id: 2, t: '00:05', speakerId: 'p2', text: { es: 'Qué tal', en: null } },
  ],
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

describe('TranscriptTab — re-transcribe', () => {
  it('opens the confirm modal and calls transcribe_meeting with the current speaker count hint', async () => {
    audioResponse = {
      path: 'C:/meetings/m-2/audio.wav',
      sizeBytes: 123456,
      mimeType: 'audio/wav',
    };
    const invokeMock = vi.mocked(tauriCore.invoke);
    invokeMock.mockClear();
    renderTab(twoSpeakerMeeting);

    const cta = await screen.findByRole('button', { name: 'Re-transcribir' });
    await waitFor(() => expect(cta).not.toBeDisabled());

    await userEvent.click(cta);

    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText('Volver a transcribir')).toBeInTheDocument();

    const confirmBtn = within(dialog).getByRole('button', { name: 'Re-transcribir' });
    await userEvent.click(confirmBtn);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('transcribe_meeting', {
        meetingId: 'm-2',
        speakerCountHint: 2,
      })
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });

  it('disables the CTA when the meeting has no saved audio', async () => {
    audioResponse = null;
    renderTab(twoSpeakerMeeting);

    const cta = await screen.findByRole('button', { name: 'Re-transcribir' });
    await waitFor(() => expect(cta).toBeDisabled());
  });
});
