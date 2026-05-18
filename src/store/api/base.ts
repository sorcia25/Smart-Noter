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
    if (typeof e === 'object' && e !== null && 'code' in e) {
      return { error: e as AppError };
    }
    return { error: { code: 'internal', message: String(e) } };
  }
};

export const baseApi = createApi({
  baseQuery: tauriBaseQuery,
  tagTypes: ['Meeting', 'Template', 'AudioDevice', 'Settings'],
  endpoints: () => ({}),
});
