import { beforeEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { store } from '@/store';
import { aiApi } from './ai.api';

describe('aiApi endpoints', () => {
  beforeEach(() => invoke.mockReset());

  it('generateSummary invokes generate_summary with meetingId', async () => {
    invoke.mockResolvedValueOnce(null);
    const args = { meetingId: 'meet-1' };
    await store.dispatch(aiApi.endpoints.generateSummary.initiate(args)).unwrap();
    expect(invoke).toHaveBeenCalledWith('generate_summary', args);
  });

  it('cancelSummary invokes cancel_summary with meetingId', async () => {
    invoke.mockResolvedValueOnce(null);
    const args = { meetingId: 'meet-1' };
    await store.dispatch(aiApi.endpoints.cancelSummary.initiate(args)).unwrap();
    expect(invoke).toHaveBeenCalledWith('cancel_summary', args);
  });

  it('updateSummaryText invokes update_summary_text with meetingId and summary', async () => {
    invoke.mockResolvedValueOnce(null);
    const args = { meetingId: 'meet-2', summary: { es: 'Resumen de la reunión', en: null } };
    await store.dispatch(aiApi.endpoints.updateSummaryText.initiate(args)).unwrap();
    expect(invoke).toHaveBeenCalledWith('update_summary_text', args);
  });

  it('askMeeting invokes ask_meeting with meetingId and question', async () => {
    invoke.mockResolvedValueOnce(null);
    const args = { meetingId: 'meet-3', question: '¿Cuáles fueron los acuerdos?' };
    await store.dispatch(aiApi.endpoints.askMeeting.initiate(args)).unwrap();
    expect(invoke).toHaveBeenCalledWith('ask_meeting', args);
  });

  it('cancelChat invokes cancel_chat with meetingId', async () => {
    invoke.mockResolvedValueOnce(null);
    const args = { meetingId: 'meet-3' };
    await store.dispatch(aiApi.endpoints.cancelChat.initiate(args)).unwrap();
    expect(invoke).toHaveBeenCalledWith('cancel_chat', args);
  });

  it('downloadLlmModel invokes download_llm_model with model id', async () => {
    invoke.mockResolvedValueOnce(null);
    const args = { id: 'qwen2.5-3b-instruct-q4' };
    await store.dispatch(aiApi.endpoints.downloadLlmModel.initiate(args)).unwrap();
    expect(invoke).toHaveBeenCalledWith('download_llm_model', args);
  });

  it('cancelLlmDownload invokes cancel_llm_download with model id', async () => {
    invoke.mockResolvedValueOnce(null);
    const args = { id: 'qwen2.5-3b-instruct-q4' };
    await store.dispatch(aiApi.endpoints.cancelLlmDownload.initiate(args)).unwrap();
    expect(invoke).toHaveBeenCalledWith('cancel_llm_download', args);
  });

  it('deleteLlmModel invokes delete_llm_model with model id', async () => {
    invoke.mockResolvedValueOnce(null);
    const args = { id: 'qwen2.5-3b-instruct-q4' };
    await store.dispatch(aiApi.endpoints.deleteLlmModel.initiate(args)).unwrap();
    expect(invoke).toHaveBeenCalledWith('delete_llm_model', args);
  });
});
