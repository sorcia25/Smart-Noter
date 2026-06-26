import type { Bilingual } from '@/ipc/bindings';
import { baseApi } from './base';

// ---------------------------------------------------------------------------
// Types (mirrored from ai.rs until bindings are regenerated in CI)
// ---------------------------------------------------------------------------

export interface LlmModelInfo {
  id: string;
  name: string;
  sizeMb: number;
  downloaded: boolean;
}

export interface ChatMessage {
  id: number;
  meetingId: string;
  role: 'user' | 'assistant';
  content: string;
  createdAt: string;
}

// ---------------------------------------------------------------------------
// RTK Query slice injected into baseApi
// ---------------------------------------------------------------------------

export const aiApi = baseApi.injectEndpoints({
  endpoints: (b) => ({
    // -- Summary commands --
    generateSummary: b.mutation<void, { meetingId: string }>({
      query: (args) => ({ cmd: 'generate_summary', args }),
    }),

    cancelSummary: b.mutation<void, { meetingId: string }>({
      query: (args) => ({ cmd: 'cancel_summary', args }),
    }),

    updateSummaryText: b.mutation<void, { meetingId: string; summary: Bilingual }>({
      query: (args) => ({ cmd: 'update_summary_text', args }),
      invalidatesTags: (_r, _e, { meetingId }) => [{ type: 'Meeting', id: meetingId }],
    }),

    getSummaryState: b.query<string | null, void>({
      query: () => ({ cmd: 'get_summary_state' }),
    }),

    // -- Chat commands --
    askMeeting: b.mutation<void, { meetingId: string; question: string }>({
      query: (args) => ({ cmd: 'ask_meeting', args }),
    }),

    cancelChat: b.mutation<void, { meetingId: string }>({
      query: (args) => ({ cmd: 'cancel_chat', args }),
    }),

    listChat: b.query<ChatMessage[], { meetingId: string }>({
      query: (args) => ({ cmd: 'list_chat', args }),
    }),

    // -- LLM model management --
    listLlmModels: b.query<LlmModelInfo[], void>({
      query: () => ({ cmd: 'list_llm_models' }),
    }),

    downloadLlmModel: b.mutation<void, { id: string }>({
      query: (args) => ({ cmd: 'download_llm_model', args }),
    }),

    cancelLlmDownload: b.mutation<void, { id: string }>({
      query: (args) => ({ cmd: 'cancel_llm_download', args }),
    }),

    deleteLlmModel: b.mutation<void, { id: string }>({
      query: (args) => ({ cmd: 'delete_llm_model', args }),
    }),
  }),
});

export const {
  useGenerateSummaryMutation,
  useCancelSummaryMutation,
  useUpdateSummaryTextMutation,
  useGetSummaryStateQuery,
  useAskMeetingMutation,
  useCancelChatMutation,
  useListChatQuery,
  useListLlmModelsQuery,
  useDownloadLlmModelMutation,
  useCancelLlmDownloadMutation,
  useDeleteLlmModelMutation,
} = aiApi;
