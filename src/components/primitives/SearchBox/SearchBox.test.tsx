import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { SearchBox } from './SearchBox';

describe('SearchBox', () => {
  it('renders with placeholder', () => {
    render(<SearchBox value="" onChange={() => {}} placeholder="Find something" />);
    expect(screen.getByPlaceholderText('Find something')).toBeInTheDocument();
  });

  it('calls onChange with the next string when user types', async () => {
    const onChange = vi.fn();
    render(<SearchBox value="" onChange={onChange} placeholder="x" />);
    await userEvent.type(screen.getByPlaceholderText('x'), 'q');
    expect(onChange).toHaveBeenLastCalledWith('q');
  });
});
