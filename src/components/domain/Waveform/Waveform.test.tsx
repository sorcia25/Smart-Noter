import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { Waveform } from './Waveform';

describe('Waveform', () => {
  it('renders the default 36 bars', () => {
    const { container } = render(<Waveform />);
    expect(container.querySelectorAll('span').length).toBe(36);
  });

  it('honors the bars prop', () => {
    const { container } = render(<Waveform bars={12} />);
    expect(container.querySelectorAll('span').length).toBe(12);
  });

  it('applies the paused class when paused', () => {
    const { container } = render(<Waveform paused />);
    expect((container.firstChild as HTMLElement).className).toMatch(/paused/);
  });
});
