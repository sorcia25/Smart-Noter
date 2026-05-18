import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { SegmentedControl } from './SegmentedControl';

const options = [
  { value: 'a', label: 'Alpha' },
  { value: 'b', label: 'Beta' },
  { value: 'c', label: 'Gamma' },
] as const;

describe('SegmentedControl', () => {
  it('marks the selected option as active', () => {
    render(<SegmentedControl value="b" options={[...options]} onChange={() => {}} />);
    expect(screen.getByRole('tab', { name: 'Beta', selected: true })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'Alpha', selected: false })).toBeInTheDocument();
  });

  it('fires onChange with the new value when a tab is clicked', async () => {
    const onChange = vi.fn();
    render(<SegmentedControl value="a" options={[...options]} onChange={onChange} />);
    await userEvent.click(screen.getByRole('tab', { name: 'Gamma' }));
    expect(onChange).toHaveBeenCalledWith('c');
  });
});
