import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';
import '@/i18n';
import type { CaptureResult } from '@/ipc/bindings';
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

function setup() {
  return render(
    <MemoryRouter>
      <StopConfirmModal
        open
        onClose={() => {}}
        capture={capture}
        suggestedTitle="Q4 review"
        templateId="tecnica"
      />
    </MemoryRouter>
  );
}

describe('StopConfirmModal', () => {
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
});
