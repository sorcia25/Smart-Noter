import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { store } from '@/store';
import { meetingsApi } from './meetings.api';

describe('export endpoint', () => {
  beforeEach(() => invoke.mockReset());

  it('exportMeeting invokes export_meeting with all args', async () => {
    invoke.mockResolvedValueOnce(['/tmp/meeting.mp3', '/tmp/meeting.md']);
    const args = {
      meetingId: 'm1',
      formats: ['audio', 'md'],
      fileName: 'my-meeting',
      timestamps: true,
      bilingual: false,
    };
    await store.dispatch(meetingsApi.endpoints.exportMeeting.initiate(args)).unwrap();
    expect(invoke).toHaveBeenCalledWith('export_meeting', args);
  });

  it('exportMeeting returns empty array when user cancels dialog', async () => {
    invoke.mockResolvedValueOnce([]);
    const args = {
      meetingId: 'm2',
      formats: ['pdf'],
      fileName: 'cancelled',
      timestamps: false,
      bilingual: false,
    };
    const result = await store
      .dispatch(meetingsApi.endpoints.exportMeeting.initiate(args))
      .unwrap();
    expect(result).toEqual([]);
    expect(invoke).toHaveBeenCalledWith('export_meeting', args);
  });
});
