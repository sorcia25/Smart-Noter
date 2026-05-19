import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import '@/i18n';
import { LivePill } from './LivePill';

describe('LivePill', () => {
  it('renders GRABANDO by default (ES)', () => {
    render(<LivePill />);
    expect(screen.getByText('GRABANDO')).toBeInTheDocument();
  });

  it('renders PAUSADO when paused', () => {
    render(<LivePill paused />);
    expect(screen.getByText('PAUSADO')).toBeInTheDocument();
  });

  it('adds the paused class on the root', () => {
    const { container } = render(<LivePill paused />);
    expect((container.firstChild as HTMLElement).className).toMatch(/paused/);
  });
});
