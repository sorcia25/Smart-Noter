import { describe, expect, it } from 'vitest';
import { Paths } from './paths';

describe('Paths', () => {
  it('produces detail path with id', () => {
    expect(Paths.MeetingDetail('m-001')).toBe('/meetings/m-001');
  });
  it('produces live recording path with sessionId', () => {
    expect(Paths.LiveRecording('sess-123')).toBe('/record/live/sess-123');
  });
});
