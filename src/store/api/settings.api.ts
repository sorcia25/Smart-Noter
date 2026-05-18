import type { AppSettings } from '@/ipc/bindings';
import { baseApi } from './base';

export const settingsApi = baseApi.injectEndpoints({
  endpoints: (b) => ({
    getSettings: b.query<AppSettings, void>({
      query: () => ({ cmd: 'get_settings' }),
      providesTags: ['Settings'],
    }),
    updateSettings: b.mutation<void, AppSettings>({
      query: (settings) => ({ cmd: 'update_settings', args: { settings } }),
      invalidatesTags: ['Settings'],
    }),
  }),
});

export const { useGetSettingsQuery, useUpdateSettingsMutation } = settingsApi;
