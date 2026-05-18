import type { MeetingDetail, MeetingSummary } from '@/ipc/bindings';
import { baseApi } from './base';

export const meetingsApi = baseApi.injectEndpoints({
  endpoints: (b) => ({
    listMeetings: b.query<MeetingSummary[], void>({
      query: () => ({ cmd: 'list_meetings' }),
      providesTags: ['Meeting'],
    }),
    getMeeting: b.query<MeetingDetail, string>({
      query: (id) => ({ cmd: 'get_meeting', args: { id } }),
      providesTags: (_r, _e, id) => [{ type: 'Meeting', id }],
    }),
    updateMeetingTitle: b.mutation<void, { id: string; titleEs: string; titleEn?: string | null }>({
      query: (args) => ({ cmd: 'update_meeting_title', args }),
      invalidatesTags: (_r, _e, { id }) => [{ type: 'Meeting', id }, 'Meeting'],
    }),
    toggleAction: b.mutation<boolean, string>({
      query: (actionId) => ({ cmd: 'toggle_action', args: { actionId } }),
      invalidatesTags: ['Meeting'],
    }),
    renameParticipant: b.mutation<void, { participantId: string; name: string | null }>({
      query: (args) => ({ cmd: 'rename_participant', args }),
      invalidatesTags: ['Meeting'],
    }),
  }),
});

export const {
  useListMeetingsQuery,
  useGetMeetingQuery,
  useUpdateMeetingTitleMutation,
  useToggleActionMutation,
  useRenameParticipantMutation,
} = meetingsApi;
