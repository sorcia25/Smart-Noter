export const Paths = {
  Dashboard: '/',
  Meetings: '/meetings',
  MeetingDetail: (id: string) => `/meetings/${id}`,
  PreRecord: '/record/new',
  LiveRecording: (sessionId: string) => `/record/live/${sessionId}`,
  Templates: '/templates',
  Participants: '/participants',
  Settings: '/settings',
} as const;
