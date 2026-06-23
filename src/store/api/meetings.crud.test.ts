import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { store } from '@/store';
import { meetingsApi } from './meetings.api';

describe('crud endpoints', () => {
  beforeEach(() => invoke.mockReset());

  it('createAction invokes create_action', async () => {
    invoke.mockResolvedValueOnce('act-1');
    await store
      .dispatch(
        meetingsApi.endpoints.createAction.initiate({
          meetingId: 'm1',
          text: 'X',
          ownerParticipantId: null,
          due: null,
        })
      )
      .unwrap();
    expect(invoke).toHaveBeenCalledWith('create_action', {
      meetingId: 'm1',
      text: 'X',
      ownerParticipantId: null,
      due: null,
    });
  });

  it('updateAction strips meetingId from the command args', async () => {
    invoke.mockResolvedValueOnce(undefined);
    await store
      .dispatch(
        meetingsApi.endpoints.updateAction.initiate({
          meetingId: 'm1',
          actionId: 'a1',
          text: 'Y',
          ownerParticipantId: null,
          due: null,
        })
      )
      .unwrap();
    expect(invoke).toHaveBeenCalledWith('update_action', {
      actionId: 'a1',
      text: 'Y',
      ownerParticipantId: null,
      due: null,
    });
  });

  it('deleteDecision invokes delete_decision', async () => {
    invoke.mockResolvedValueOnce(undefined);
    await store
      .dispatch(meetingsApi.endpoints.deleteDecision.initiate({ id: 7, meetingId: 'm1' }))
      .unwrap();
    expect(invoke).toHaveBeenCalledWith('delete_decision', { id: 7 });
  });

  it('createBlocker invokes create_blocker', async () => {
    invoke.mockResolvedValueOnce(3);
    await store
      .dispatch(meetingsApi.endpoints.createBlocker.initiate({ meetingId: 'm1', text: 'B' }))
      .unwrap();
    expect(invoke).toHaveBeenCalledWith('create_blocker', { meetingId: 'm1', text: 'B' });
  });
});
