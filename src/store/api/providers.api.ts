import type { ProviderConfig } from '@/ipc/bindings';
import { baseApi } from './base';

export const providersApi = baseApi.injectEndpoints({
  endpoints: (b) => ({
    getProviderConfig: b.query<ProviderConfig[], void>({
      query: () => ({ cmd: 'get_provider_config' }),
      providesTags: ['ProviderConfig'],
    }),

    updateProviderConfig: b.mutation<
      null,
      { provider: string; key?: string | null; model?: string | null }
    >({
      query: ({ provider, key, model }) => ({
        cmd: 'update_provider_config',
        args: { provider, key: key ?? null, model: model ?? null },
      }),
      invalidatesTags: ['ProviderConfig'],
    }),

    testApiKey: b.mutation<null, { provider: string }>({
      query: ({ provider }) => ({ cmd: 'test_api_key', args: { provider } }),
    }),
  }),
});

export const { useGetProviderConfigQuery, useUpdateProviderConfigMutation, useTestApiKeyMutation } =
  providersApi;
