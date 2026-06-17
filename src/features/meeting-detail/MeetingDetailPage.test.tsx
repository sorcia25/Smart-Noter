import { store } from '@/store';
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import MeetingDetailPage from './MeetingDetailPage';

const fakeMeeting = {
  id: 'm-001',
  title: { es: 'Comité directivo', en: 'Steering committee' },
  template: 'ejecutiva',
  date: '2026-05-01T15:30:00.000Z',
  durationSec: 3600,
  deviceUsed: null,
  wordCount: 1234,
  summary: { es: 'Resumen mock.', en: 'Mock summary.' },
  participants: [
    {
      id: 'p1',
      meetingId: 'm-001',
      label: 'Sujeto 1',
      name: 'Carlos R',
      colorClass: 's-color-1',
      wordCount: 600,
      talkPct: 40,
    },
  ],
  actions: [],
  decisions: [],
  blockers: [],
  transcript: [],
};

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'get_meeting') return fakeMeeting;
    if (cmd === 'list_templates') return [];
    if (cmd === 'get_settings') return null;
    return null;
  }),
}));

function setup() {
  return render(
    <Provider store={store}>
      <MemoryRouter initialEntries={['/meetings/m-001']}>
        <Routes>
          <Route path="/meetings/:id" element={<MeetingDetailPage />} />
        </Routes>
      </MemoryRouter>
    </Provider>
  );
}

describe('MeetingDetailPage', () => {
  it('renders meeting title once the query resolves', async () => {
    setup();
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Comité directivo' })).toBeInTheDocument();
    });
  });

  it('exposes data-screen-label', () => {
    const { container } = setup();
    expect(container.querySelector('[data-screen-label="05 Meeting detail"]')).toBeTruthy();
  });

  it('renders participant talk percentage as an integer (talkPct is already 0-100)', async () => {
    setup();
    // fakeMeeting's participant has talkPct: 40 → must render "40%", not "4000%".
    await waitFor(() => {
      expect(screen.getByText('40%')).toBeInTheDocument();
    });
  });
});
