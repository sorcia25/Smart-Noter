import type { Participant } from '@/ipc/bindings';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { AvatarStack, SubjectAvatar } from './Avatar';

function mkP(overrides: Partial<Participant> = {}): Participant {
  return {
    id: 'p1',
    meetingId: 'm1',
    label: 'Sujeto 1',
    name: 'Carlos Rivera',
    colorClass: 's-color-1',
    wordCount: 0,
    talkPct: 0,
    ...overrides,
  };
}

describe('SubjectAvatar', () => {
  it('renders initials from name when present', () => {
    render(<SubjectAvatar participant={mkP({ name: 'Carlos Rivera' })} />);
    expect(screen.getByText('CR')).toBeInTheDocument();
  });

  it('renders initials from label when name is null', () => {
    render(<SubjectAvatar participant={mkP({ name: null, label: 'Diego Perez' })} />);
    expect(screen.getByText('DP')).toBeInTheDocument();
  });
});

describe('AvatarStack', () => {
  it('shows up to max avatars', () => {
    const ps = Array.from({ length: 3 }, (_, i) => mkP({ id: `p${i}`, name: `User ${i}` }));
    const { container } = render(<AvatarStack participants={ps} max={4} />);
    expect(container.querySelectorAll('div[title]').length).toBe(3);
  });

  it('shows +N overflow when participants exceed max', () => {
    const ps = Array.from({ length: 7 }, (_, i) => mkP({ id: `p${i}`, name: `User ${i}` }));
    render(<AvatarStack participants={ps} max={4} />);
    expect(screen.getByText('+3')).toBeInTheDocument();
  });
});
