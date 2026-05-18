import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { Input } from './Input';

describe('Input', () => {
  it('renders an input element', () => {
    render(<Input placeholder="Type here" />);
    expect(screen.getByPlaceholderText('Type here')).toBeInTheDocument();
  });

  it('shows the label when provided', () => {
    render(<Input label="Email" placeholder="x" />);
    expect(screen.getByText('Email')).toBeInTheDocument();
  });

  it('forwards typing events', async () => {
    const onChange = vi.fn();
    render(<Input placeholder="x" onChange={onChange} />);
    await userEvent.type(screen.getByPlaceholderText('x'), 'a');
    expect(onChange).toHaveBeenCalled();
  });
});
