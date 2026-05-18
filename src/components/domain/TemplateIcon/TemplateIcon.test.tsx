import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { TemplateIcon } from './TemplateIcon';

describe('TemplateIcon', () => {
  it('renders the known template with its color class', () => {
    const { container } = render(<TemplateIcon templateId="daily" />);
    expect((container.firstChild as HTMLElement)?.className).toMatch(/tDaily/);
  });

  it('falls back to default for unknown templates', () => {
    const { container } = render(<TemplateIcon templateId="unknown-xyz" />);
    expect((container.firstChild as HTMLElement)?.className).toMatch(/tDefault/);
  });

  it('respects the size prop', () => {
    const { container } = render(<TemplateIcon templateId="ejecutiva" size={32} />);
    const el = container.firstChild as HTMLElement;
    expect(el.style.width).toBe('32px');
    expect(el.style.height).toBe('32px');
  });
});
