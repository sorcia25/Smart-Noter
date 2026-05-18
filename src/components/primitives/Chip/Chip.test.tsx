import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { Chip } from './Chip';

describe('Chip', () => {
  it('renders children', () => {
    render(<Chip>Daily</Chip>);
    expect(screen.getByRole('button', { name: 'Daily' })).toBeInTheDocument();
  });

  it('applies accent variant class', () => {
    const { container } = render(<Chip variant="accent">A</Chip>);
    expect((container.firstChild as HTMLElement).className).toMatch(/accent/);
  });

  it('fires onClick', async () => {
    const onClick = vi.fn();
    render(<Chip onClick={onClick}>x</Chip>);
    await userEvent.click(screen.getByRole('button'));
    expect(onClick).toHaveBeenCalled();
  });
});
