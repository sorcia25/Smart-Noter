import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { store } from '@/store';
import { meetingsApi } from './meetings.api';

describe('search endpoint', () => {
  beforeEach(() => invoke.mockReset());
  it('searchMeetings invokes search_meetings with query + template', async () => {
    invoke.mockResolvedValueOnce([]);
    await store
      .dispatch(
        meetingsApi.endpoints.searchMeetings.initiate({ query: 'arq', template: 'tecnica' })
      )
      .unwrap();
    expect(invoke).toHaveBeenCalledWith('search_meetings', { query: 'arq', template: 'tecnica' });
  });
});
