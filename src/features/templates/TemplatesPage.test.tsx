import { store } from '@/store';
import { render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import TemplatesPage from './TemplatesPage';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'list_templates') return [];
    if (cmd === 'get_settings') return null;
    return null;
  }),
}));

function setup() {
  return render(
    <Provider store={store}>
      <MemoryRouter>
        <TemplatesPage />
      </MemoryRouter>
    </Provider>
  );
}

describe('TemplatesPage', () => {
  it('renders the gallery title', async () => {
    setup();
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Galería de plantillas' })).toBeInTheDocument();
    });
  });

  it('exposes data-screen-label', () => {
    const { container } = setup();
    expect(container.querySelector('[data-screen-label="06 Templates"]')).toBeTruthy();
  });
});
