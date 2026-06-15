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

  it('applies the live class (which disables the CSS animation) when externalBins is provided', () => {
    const { container } = render(<Waveform bars={3} externalBins={[0.1, 0.5, 0.9]} />);
    expect((container.firstChild as HTMLElement).className).toMatch(/live/);
  });

  it('drives bar heights from externalBins so real audio data shows', () => {
    const { container } = render(<Waveform bars={3} externalBins={[0, 0.5, 1]} />);
    const spans = container.querySelectorAll('span');
    expect((spans[0] as HTMLElement).style.height).toBe('0%');
    expect((spans[2] as HTMLElement).style.height).toBe('100%');
  });
});
