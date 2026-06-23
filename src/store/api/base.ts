import { toAppError } from '@/ipc/error';
import type { BaseQueryFn } from '@reduxjs/toolkit/query';
import { createApi } from '@reduxjs/toolkit/query/react';
import { invoke } from '@tauri-apps/api/core';

export interface AppError {
  code: string;
  message: string;
}

export interface InvokeArgs {
  cmd: string;
  args?: Record<string, unknown>;
}

const tauriBaseQuery: BaseQueryFn<InvokeArgs, unknown, AppError> = async ({ cmd, args }) => {
  try {
    const data = await invoke(cmd, args ?? {});
    return { data };
  } catch (e) {
    return { error: toAppError(e) };
  }
};

export const baseApi = createApi({
  baseQuery: tauriBaseQuery,
  tagTypes: ['Meeting', 'Template', 'AudioDevice', 'Settings', 'Trash'],
  endpoints: () => ({}),
});
