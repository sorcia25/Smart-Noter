import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { Toggle } from './Toggle';

describe('Toggle', () => {
  it('renders with aria-checked reflecting `on`', () => {
    render(<Toggle on={true} onChange={() => {}} aria-label="t" />);
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
  });

  it('fires onChange with the toggled value', async () => {
    const onChange = vi.fn();
    render(<Toggle on={false} onChange={onChange} aria-label="t" />);
    await userEvent.click(screen.getByRole('switch'));
    expect(onChange).toHaveBeenCalledWith(true);
  });
});
