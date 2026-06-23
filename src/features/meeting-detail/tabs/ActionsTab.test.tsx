import { store } from '@/store';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { ActionsTab } from './ActionsTab';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

const renderTab = () =>
  render(
    <Provider store={store}>
      <ActionsTab meetingId="m1" participants={[]} actions={[]} />
    </Provider>
  );

describe('ActionsTab editing', () => {
  beforeEach(() => invoke.mockReset());

  it('adds an action', async () => {
    invoke.mockResolvedValue('act-1');
    renderTab();
    fireEvent.click(screen.getByRole('button', { name: /Añadir acción|Add action/i }));
    fireEvent.change(screen.getByPlaceholderText(/Escribe aquí|Type here/i), {
      target: { value: 'New' },
    });
    fireEvent.click(screen.getByRole('button', { name: /Guardar|Save/i }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(
        'create_action',
        expect.objectContaining({ meetingId: 'm1', text: 'New' })
      )
    );
  });
});
