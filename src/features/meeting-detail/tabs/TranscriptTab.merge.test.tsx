import type { MeetingDetail, Participant } from '@/ipc/bindings';
import { store } from '@/store';
import * as tauriCore from '@tauri-apps/api/core';
import { render, screen, waitFor, within } from '@testing-library/react';
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
    if (cmd === 'get_meeting_audio') return null;
    if (cmd === 'merge_speakers') return null;
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

describe('TranscriptTab — merge speakers modal', () => {
  it('does not show the merge CTA with fewer than 2 participants', () => {
    const oneParticipant = { ...twoSpeakerMeeting, participants: [p1] };
    renderTab(oneParticipant);
    expect(screen.queryByRole('button', { name: 'Fusionar hablantes' })).not.toBeInTheDocument();
  });

  it('opens the merge modal, gates confirm on a valid distinct pair, and calls merge_speakers', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    invokeMock.mockClear();
    renderTab(twoSpeakerMeeting);

    await userEvent.click(screen.getByRole('button', { name: 'Fusionar hablantes' }));

    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText('Fusionar dos hablantes en uno')).toBeInTheDocument();

    const confirmBtn = within(dialog).getByRole('button', { name: 'Fusionar hablantes' });
    expect(confirmBtn).toBeDisabled();

    const fromSelect = within(dialog).getByLabelText('Fusionar este…');
    const intoSelect = within(dialog).getByLabelText('…dentro de este');

    // Only "from" chosen — still disabled.
    await userEvent.selectOptions(fromSelect, 'p1');
    expect(confirmBtn).toBeDisabled();

    // Same participant on both sides — still disabled.
    await userEvent.selectOptions(intoSelect, 'p1');
    expect(confirmBtn).toBeDisabled();

    // Distinct pair — now enabled.
    await userEvent.selectOptions(intoSelect, 'p2');
    expect(confirmBtn).not.toBeDisabled();

    await userEvent.click(confirmBtn);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('merge_speakers', { into: 'p2', from: 'p1' })
    );
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });
});
