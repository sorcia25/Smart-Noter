import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import './index';
import { useT } from './useT';

function Probe() {
  const { t, lang } = useT();
  return (
    <div data-testid="probe" data-lang={lang}>
      {t('navDashboard')}
    </div>
  );
}

describe('useT', () => {
  it('returns Spanish text by default', () => {
    render(<Probe />);
    const el = screen.getByTestId('probe');
    expect(el).toHaveTextContent('Inicio');
    expect(el).toHaveAttribute('data-lang', 'es');
  });
});
