import type { MeetingDetail } from '@/ipc/bindings';
import { store } from '@/store';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@/i18n';
import { SummaryTab } from './SummaryTab';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

const meeting = {
  id: 'm1',
  title: { es: 'M', en: null },
  template: 'tecnica',
  date: '2026-06-01T00:00:00Z',
  durationSec: 1,
  deviceUsed: null,
  wordCount: 0,
  summary: { es: 'S', en: null },
  participants: [],
  actions: [],
  decisions: [{ id: 1, text: { es: 'Dec one', en: null } }],
  blockers: [],
  transcript: [],
} as unknown as MeetingDetail;

describe('SummaryTab decisions editing', () => {
  beforeEach(() => invoke.mockReset());

  it('deletes a decision', async () => {
    invoke.mockResolvedValue(undefined);
    render(
      <Provider store={store}>
        <SummaryTab meeting={meeting} template={undefined} />
      </Provider>
    );
    fireEvent.click(screen.getByLabelText('delete-decision-1'));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('delete_decision', { id: 1 }));
  });
});
