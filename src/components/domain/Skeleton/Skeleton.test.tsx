import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { Skeleton } from './Skeleton';

describe('Skeleton', () => {
  it('renders with the requested width/height', () => {
    const { container } = render(<Skeleton width={120} height={16} />);
    const el = container.firstChild as HTMLElement;
    expect(el.style.width).toBe('120px');
    expect(el.style.height).toBe('16px');
  });

  it('adds the round modifier when round prop is true', () => {
    const { container } = render(<Skeleton width={32} height={32} round />);
    expect((container.firstChild as HTMLElement).className).toMatch(/round/);
  });
});
