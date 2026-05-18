import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { Card } from './Card';

describe('Card', () => {
  it('renders children', () => {
    render(<Card>hello</Card>);
    expect(screen.getByText('hello')).toBeInTheDocument();
  });

  it('applies padding when padded prop is true', () => {
    const { container, rerender } = render(<Card>x</Card>);
    const cls = container.firstChild?.className ?? '';
    rerender(<Card padded>x</Card>);
    expect(container.firstChild?.className).not.toBe(cls);
    expect((container.firstChild as HTMLElement).className).toMatch(/pad/);
  });
});
