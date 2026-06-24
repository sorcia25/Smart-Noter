import type { MeetingDetail, MeetingSummary, SearchHit } from '@/ipc/bindings';
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
    mergeSpeakers: b.mutation<void, { into: string; from: string }>({
      query: (args) => ({ cmd: 'merge_speakers', args }),
      invalidatesTags: ['Meeting'],
    }),
    reassignLines: b.mutation<void, { lineIds: number[]; speakerId: string }>({
      query: (args) => ({ cmd: 'reassign_lines', args }),
      invalidatesTags: ['Meeting'],
    }),
    createSpeaker: b.mutation<string, { meetingId: string }>({
      query: (args) => ({ cmd: 'create_speaker', args }),
      invalidatesTags: ['Meeting'],
    }),
    listTrashedMeetings: b.query<MeetingSummary[], void>({
      query: () => ({ cmd: 'list_trashed_meetings' }),
      providesTags: ['Trash'],
    }),
    deleteMeeting: b.mutation<void, string>({
      query: (id) => ({ cmd: 'delete_meeting', args: { id } }),
      invalidatesTags: ['Meeting', 'Trash'],
    }),
    restoreMeeting: b.mutation<void, string>({
      query: (id) => ({ cmd: 'restore_meeting', args: { id } }),
      invalidatesTags: ['Meeting', 'Trash'],
    }),
    purgeMeeting: b.mutation<void, string>({
      query: (id) => ({ cmd: 'purge_meeting', args: { id } }),
      invalidatesTags: ['Trash'],
    }),
    createAction: b.mutation<
      string,
      { meetingId: string; text: string; ownerParticipantId: string | null; due: string | null }
    >({
      query: (args) => ({ cmd: 'create_action', args }),
      invalidatesTags: (_r, _e, { meetingId }) => [{ type: 'Meeting', id: meetingId }],
    }),
    updateAction: b.mutation<
      void,
      {
        meetingId: string;
        actionId: string;
        text: string;
        ownerParticipantId: string | null;
        due: string | null;
      }
    >({
      query: ({ meetingId: _m, ...args }) => ({ cmd: 'update_action', args }),
      invalidatesTags: (_r, _e, { meetingId }) => [{ type: 'Meeting', id: meetingId }],
    }),
    deleteAction: b.mutation<void, { meetingId: string; actionId: string }>({
      query: ({ actionId }) => ({ cmd: 'delete_action', args: { actionId } }),
      invalidatesTags: (_r, _e, { meetingId }) => [{ type: 'Meeting', id: meetingId }],
    }),
    createDecision: b.mutation<number, { meetingId: string; text: string }>({
      query: (args) => ({ cmd: 'create_decision', args }),
      invalidatesTags: (_r, _e, { meetingId }) => [{ type: 'Meeting', id: meetingId }],
    }),
    updateDecision: b.mutation<void, { meetingId: string; id: number; text: string }>({
      query: ({ id, text }) => ({ cmd: 'update_decision', args: { id, text } }),
      invalidatesTags: (_r, _e, { meetingId }) => [{ type: 'Meeting', id: meetingId }],
    }),
    deleteDecision: b.mutation<void, { meetingId: string; id: number }>({
      query: ({ id }) => ({ cmd: 'delete_decision', args: { id } }),
      invalidatesTags: (_r, _e, { meetingId }) => [{ type: 'Meeting', id: meetingId }],
    }),
    createBlocker: b.mutation<number, { meetingId: string; text: string }>({
      query: (args) => ({ cmd: 'create_blocker', args }),
      invalidatesTags: (_r, _e, { meetingId }) => [{ type: 'Meeting', id: meetingId }],
    }),
    updateBlocker: b.mutation<void, { meetingId: string; id: number; text: string }>({
      query: ({ id, text }) => ({ cmd: 'update_blocker', args: { id, text } }),
      invalidatesTags: (_r, _e, { meetingId }) => [{ type: 'Meeting', id: meetingId }],
    }),
    deleteBlocker: b.mutation<void, { meetingId: string; id: number }>({
      query: ({ id }) => ({ cmd: 'delete_blocker', args: { id } }),
      invalidatesTags: (_r, _e, { meetingId }) => [{ type: 'Meeting', id: meetingId }],
    }),
    searchMeetings: b.query<SearchHit[], { query: string; template: string | null }>({
      query: (args) => ({ cmd: 'search_meetings', args }),
    }),
  }),
});

export const {
  useListMeetingsQuery,
  useGetMeetingQuery,
  useUpdateMeetingTitleMutation,
  useToggleActionMutation,
  useRenameParticipantMutation,
  useMergeSpeakersMutation,
  useReassignLinesMutation,
  useCreateSpeakerMutation,
  useListTrashedMeetingsQuery,
  useDeleteMeetingMutation,
  useRestoreMeetingMutation,
  usePurgeMeetingMutation,
  useCreateActionMutation,
  useUpdateActionMutation,
  useDeleteActionMutation,
  useCreateDecisionMutation,
  useUpdateDecisionMutation,
  useDeleteDecisionMutation,
  useCreateBlockerMutation,
  useUpdateBlockerMutation,
  useDeleteBlockerMutation,
  useSearchMeetingsQuery,
} = meetingsApi;
