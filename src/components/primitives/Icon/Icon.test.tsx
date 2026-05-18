import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { Icon } from './Icon';

describe('Icon', () => {
  it('renders an svg with the requested size and viewBox', () => {
    const { container } = render(<Icon name="home" size={24} />);
    const svg = container.querySelector('svg');
    expect(svg).not.toBeNull();
    expect(svg).toHaveAttribute('width', '24');
    expect(svg).toHaveAttribute('height', '24');
    expect(svg).toHaveAttribute('viewBox', '0 0 24 24');
  });

  it('uses stroke styling for outline icons', () => {
    const { container } = render(<Icon name="home" stroke="red" />);
    const svg = container.querySelector('svg');
    expect(svg).toHaveAttribute('stroke', 'red');
    expect(svg).toHaveAttribute('fill', 'none');
  });

  it('uses fill styling for filled icons (record/play/stop/...)', () => {
    const { container } = render(<Icon name="record" stroke="blue" />);
    const svg = container.querySelector('svg');
    expect(svg).toHaveAttribute('fill', 'blue');
    expect(svg).toHaveAttribute('stroke', 'none');
  });

  it('emits a path element with the icon definition', () => {
    const { container } = render(<Icon name="check" />);
    const path = container.querySelector('path');
    expect(path).not.toBeNull();
    expect(path?.getAttribute('d')).toContain('M5 12');
  });
});
