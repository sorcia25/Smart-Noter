import { store } from '@/store';
import { render, screen } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it } from 'vitest';
import '@/i18n';
import { Sidebar } from './Sidebar';

function setup() {
  return render(
    <Provider store={store}>
      <MemoryRouter initialEntries={['/']}>
        <Sidebar />
      </MemoryRouter>
    </Provider>
  );
}

describe('Sidebar', () => {
  it('shows brand name', () => {
    setup();
    expect(screen.getByText('Smart Noter')).toBeInTheDocument();
  });

  it('renders all workspace nav entries (ES default)', () => {
    setup();
    expect(screen.getByText('Inicio')).toBeInTheDocument();
    expect(screen.getByText('Reuniones')).toBeInTheDocument();
    expect(screen.getByText('Plantillas')).toBeInTheDocument();
  });

  it('renders the new-recording CTA', () => {
    setup();
    expect(screen.getByRole('button', { name: /Nueva grabación/i })).toBeInTheDocument();
  });
});
