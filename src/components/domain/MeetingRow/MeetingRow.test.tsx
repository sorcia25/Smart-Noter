import type { MeetingSummary } from '@/ipc/bindings';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { MeetingRow } from './MeetingRow';

const meeting: MeetingSummary = {
  id: 'm-001',
  title: { es: 'Comité directivo', en: 'Steering committee' },
  template: 'ejecutiva',
  date: '2026-05-01T15:30:00.000Z',
  durationSec: 3600,
  wordCount: 1234,
  participants: [
    {
      id: 'p1',
      meetingId: 'm-001',
      label: 'Sujeto 1',
      name: 'Carlos R',
      colorClass: 's-color-1',
      wordCount: 600,
      talkPct: 0.4,
    },
    {
      id: 'p2',
      meetingId: 'm-001',
      label: 'Sujeto 2',
      name: 'Diego P',
      colorClass: 's-color-2',
      wordCount: 634,
      talkPct: 0.6,
    },
  ],
};

describe('MeetingRow', () => {
  it('renders title and template', () => {
    render(<MeetingRow meeting={meeting} />);
    expect(screen.getByText('Comité directivo')).toBeInTheDocument();
    expect(screen.getByText('ejecutiva')).toBeInTheDocument();
  });

  it('renders formatted duration', () => {
    render(<MeetingRow meeting={meeting} />);
    expect(screen.getByText('1:00:00')).toBeInTheDocument();
  });

  it('fires onClick', async () => {
    const onClick = vi.fn();
    render(<MeetingRow meeting={meeting} onClick={onClick} />);
    await userEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalled();
  });
});
