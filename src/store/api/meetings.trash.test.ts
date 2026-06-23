import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { store } from '@/store';
import { meetingsApi } from './meetings.api';

describe('trash endpoints', () => {
  beforeEach(() => invoke.mockReset());

  it('deleteMeeting invokes delete_meeting with id', async () => {
    invoke.mockResolvedValueOnce(undefined);
    await store.dispatch(meetingsApi.endpoints.deleteMeeting.initiate('m1')).unwrap();
    expect(invoke).toHaveBeenCalledWith('delete_meeting', { id: 'm1' });
  });

  it('listTrashedMeetings invokes list_trashed_meetings', async () => {
    invoke.mockResolvedValueOnce([]);
    await store.dispatch(meetingsApi.endpoints.listTrashedMeetings.initiate()).unwrap();
    expect(invoke).toHaveBeenCalledWith('list_trashed_meetings', {});
  });
});
