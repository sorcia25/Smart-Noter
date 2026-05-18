import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { EqBar } from './EqBar';

describe('EqBar', () => {
  it('renders five spans by default', () => {
    const { container } = render(<EqBar />);
    expect(container.querySelectorAll('span').length).toBe(5);
  });

  it('honors the bars prop', () => {
    const { container } = render(<EqBar bars={8} />);
    expect(container.querySelectorAll('span').length).toBe(8);
  });
});
