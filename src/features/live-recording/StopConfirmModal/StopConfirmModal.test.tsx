import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@/i18n';
import type { CaptureResult } from '@/ipc/bindings';
import * as tauriCore from '@tauri-apps/api/core';
import { StopConfirmModal } from './StopConfirmModal';

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
    <MemoryRouter>
      <StopConfirmModal
        open
        onClose={onClose ?? (() => {})}
        capture={capture}
        suggestedTitle="Q4 review"
        templateId="tecnica"
      />
    </MemoryRouter>
  );
}

describe('StopConfirmModal', () => {
  beforeEach(() => {
    vi.mocked(tauriCore.invoke).mockImplementation(async (cmd: string) => {
      if (cmd === 'finalize_recording') return { id: 'm-new', title: { es: 'X', en: null } };
      if (cmd === 'discard_recording') return null;
      return null;
    });
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
    vi.mocked(tauriCore.invoke).mockRejectedValueOnce(new Error('no finished session'));
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
  });
});
