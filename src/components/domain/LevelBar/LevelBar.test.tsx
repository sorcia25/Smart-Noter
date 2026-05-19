import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { LevelBar } from './LevelBar';

describe('LevelBar', () => {
  it('exposes a meter role with the clamped percentage', () => {
    render(<LevelBar level={0.42} />);
    const meter = screen.getByRole('meter');
    expect(meter).toHaveAttribute('aria-valuenow', '42');
    expect(meter).toHaveAttribute('aria-valuemax', '100');
  });

  it('clamps values outside 0..1', () => {
    render(<LevelBar level={2} />);
    expect(screen.getByRole('meter')).toHaveAttribute('aria-valuenow', '100');
  });

  it('clamps negative values to zero', () => {
    render(<LevelBar level={-0.5} />);
    expect(screen.getByRole('meter')).toHaveAttribute('aria-valuenow', '0');
  });
});
