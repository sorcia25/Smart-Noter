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
    const initial = container.firstChild as HTMLElement;
    const cls = initial.className;
    rerender(<Card padded>x</Card>);
    const after = container.firstChild as HTMLElement;
    expect(after.className).not.toBe(cls);
    expect(after.className).toMatch(/pad/);
  });
});
