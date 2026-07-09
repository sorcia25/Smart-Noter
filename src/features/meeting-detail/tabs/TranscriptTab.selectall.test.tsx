import type { MeetingDetail, Participant } from '@/ipc/bindings';
import { store } from '@/store';
import * as tauriCore from '@tauri-apps/api/core';
import { render, screen } from '@testing-library/react';
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
  wordCount: 6,
  summary: null,
  participants: [p1, p2],
  actions: [],
  decisions: [],
  blockers: [],
  transcript: [
    { id: 1, t: '00:00', speakerId: 'p1', text: { es: 'Hola', en: null } },
    { id: 2, t: '00:05', speakerId: 'p2', text: { es: 'Qué tal', en: null } },
    { id: 3, t: '00:10', speakerId: 'p1', text: { es: 'Todo bien', en: null } },
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

describe('TranscriptTab — select all lines of a speaker', () => {
  it('selects every line belonging to the chosen speaker and leaves the others untouched', async () => {
    const invokeMock = vi.mocked(tauriCore.invoke);
    invokeMock.mockClear();
    renderTab(twoSpeakerMeeting);

    // Enter selection mode.
    await userEvent.click(screen.getByRole('button', { name: 'Seleccionar líneas' }));

    // Nothing selected yet.
    expect(screen.getByRole('checkbox', { name: 'Select line 1' })).not.toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'Select line 2' })).not.toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'Select line 3' })).not.toBeChecked();

    // Click the select-all shortcut for p1 (speakerLabel "Sujeto 1").
    await userEvent.click(screen.getByRole('button', { name: 'Sujeto 1' }));

    // Both of p1's lines (1 and 3) are now checked; p2's line (2) is not.
    expect(screen.getByRole('checkbox', { name: 'Select line 1' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'Select line 3' })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: 'Select line 2' })).not.toBeChecked();
  });
});
