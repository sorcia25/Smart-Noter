import type { MeetingDetail } from '@/ipc/bindings';
import { store } from '@/store';
import { render, screen } from '@testing-library/react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { AudioTab } from './AudioTab';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...a: unknown[]) => invoke(...a),
  convertFileSrc: (path: string) => path,
}));

const meeting = {
  id: 'm1',
  title: { es: 'M', en: null },
  template: 'tecnica',
  date: '2026-06-01T00:00:00Z',
  durationSec: 120,
  deviceUsed: null,
  wordCount: 0,
  summary: null,
  participants: [],
  actions: [],
  decisions: [],
  blockers: [],
  transcript: [],
} as unknown as MeetingDetail;

const markers = [
  {
    id: 'mk-1',
    meetingId: 'm1',
    tSeconds: 84,
    kind: 'decision',
    label: 'D1',
    source: 'manual',
    createdAt: '2026-06-01T00:00:00Z',
  },
  {
    id: 'mk-2',
    meetingId: 'm1',
    tSeconds: 10,
    kind: 'manual',
    label: 'mía',
    source: 'manual',
    createdAt: '2026-06-01T00:00:00Z',
  },
];

const renderTab = () =>
  render(
    <Provider store={store}>
      <AudioTab meeting={meeting} onExport={() => {}} />
    </Provider>
  );

describe('AudioTab markers', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_meeting_audio') return Promise.resolve(null);
      if (cmd === 'list_markers') return Promise.resolve(markers);
      return Promise.resolve(undefined);
    });
  });

  it('renders marker labels and type chips', async () => {
    renderTab();
    expect(await screen.findByText('D1')).toBeInTheDocument();
    expect(screen.getByText('mía')).toBeInTheDocument();
    expect(screen.getByText('Decisión')).toBeInTheDocument();
    expect(screen.getByText('Manual')).toBeInTheDocument();
  });

  it('shows the "Marcar aquí" button', async () => {
    renderTab();
    expect(await screen.findByRole('button', { name: /Marcar aquí/i })).toBeInTheDocument();
  });
});
