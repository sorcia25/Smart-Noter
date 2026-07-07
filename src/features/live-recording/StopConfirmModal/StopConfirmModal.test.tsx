import { store } from '@/store';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { toast } from '@/components/primitives/Toast/Toast';
import type { CaptureResult } from '@/ipc/bindings';
import * as tauriCore from '@tauri-apps/api/core';
import { StopConfirmModal } from './StopConfirmModal';

// Spy on navigate calls so tests can assert on the state passed to navigate().
const navigateSpy = vi.fn();
vi.mock('react-router-dom', async (importOriginal) => {
  const actual = await importOriginal<typeof import('react-router-dom')>();
  return {
    ...actual,
    useNavigate: () => navigateSpy,
  };
});

vi.mock('@/components/primitives/Toast/Toast', () => ({
  toast: { error: vi.fn() },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === 'finalize_recording') return { id: 'm-new', title: { es: 'X', en: null } };
    if (cmd === 'discard_recording') return null;
    return null;
  }),
}));

const capture: CaptureResult = {
  sessionId: 'sess-1',
  path: 'C:/tmp.wav',
  bytes: 1024,
  durationSec: 5,
};

function setup(onClose?: () => void) {
  return render(
    <Provider store={store}>
      <MemoryRouter>
        <StopConfirmModal
          open
          onClose={onClose ?? (() => {})}
          capture={capture}
          suggestedTitle="Q4 review"
          templateId="tecnica"
          speakerHint={null}
        />
      </MemoryRouter>
    </Provider>
  );
}

describe('StopConfirmModal', () => {
  beforeEach(() => {
    vi.mocked(tauriCore.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === 'finalize_recording') return { id: 'm-new', title: { es: 'X', en: null } };
      if (cmd === 'discard_recording') return null;
      return null;
    });
    vi.mocked(toast.error).mockClear();
    navigateSpy.mockClear();
  });

  it('renders title input pre-filled and Save enabled', () => {
    setup();
    expect(screen.getByDisplayValue('Q4 review')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Guardar/i })).not.toBeDisabled();
  });

  it('disables Save when title is blank', async () => {
    setup();
    const input = screen.getByDisplayValue('Q4 review');
    await userEvent.clear(input);
    expect(screen.getByRole('button', { name: /Guardar/i })).toBeDisabled();
  });

  it('save flow: trims title and calls finalize_recording, then onClose', async () => {
    const onClose = vi.fn();
    setup(onClose);
    const input = screen.getByDisplayValue('Q4 review');
    await userEvent.clear(input);
    await userEvent.type(input, '  Demo  ');
    await userEvent.click(screen.getByRole('button', { name: /Guardar/i }));
    await waitFor(() =>
      expect(vi.mocked(tauriCore.invoke)).toHaveBeenCalledWith('finalize_recording', {
        sessionId: 'sess-1',
        title: 'Demo',
        templateId: 'tecnica',
      })
    );
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it('discard flow: calls discard_recording and onClose', async () => {
    const onClose = vi.fn();
    setup(onClose);
    await userEvent.click(screen.getByRole('button', { name: /Descartar/i }));
    await waitFor(() =>
      expect(vi.mocked(tauriCore.invoke)).toHaveBeenCalledWith('discard_recording')
    );
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it('Esc key triggers discard_recording', async () => {
    setup();
    await userEvent.keyboard('{Escape}');
    await waitFor(() =>
      expect(vi.mocked(tauriCore.invoke)).toHaveBeenCalledWith('discard_recording')
    );
  });

  it('save failure keeps modal open and does not call onClose', async () => {
    vi.mocked(tauriCore.invoke).mockRejectedValueOnce({
      code: 'internal',
      message: 'no finished session to finalize',
    });
    const onClose = vi.fn();
    setup(onClose);
    await userEvent.click(screen.getByRole('button', { name: /Guardar/i }));
    await waitFor(() =>
      expect(vi.mocked(tauriCore.invoke)).toHaveBeenCalledWith(
        'finalize_recording',
        expect.any(Object)
      )
    );
    expect(screen.getByDisplayValue('Q4 review')).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
    // Toast should fire with the audio error title (es locale)
    await waitFor(() => expect(toast.error).toHaveBeenCalled());
    expect(vi.mocked(toast.error).mock.calls[0]?.[0]).toBe('Error de captura de audio');
    expect(vi.mocked(toast.error).mock.calls[0]?.[1]).toEqual(
      expect.objectContaining({
        id: expect.stringMatching(/^audio-error:/),
        description: 'no finished session to finalize',
      })
    );
  });

  it('typing a participant count navigates with that speakerHint', async () => {
    setup();
    const countInput = screen.getByLabelText('Participantes (opcional)');
    await userEvent.type(countInput, '3');
    await userEvent.click(screen.getByRole('button', { name: /Guardar|Save/i }));
    await waitFor(() => expect(navigateSpy).toHaveBeenCalled());
    const [, navOptions] = navigateSpy.mock.calls[0] as [
      string,
      { state: Record<string, unknown> },
    ];
    expect(navOptions.state.speakerHint).toBe(3);
  });
});
