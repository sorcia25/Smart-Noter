import type { AudioDevice } from '@/ipc/bindings';
import { baseApi } from './base';

export const devicesApi = baseApi.injectEndpoints({
  endpoints: (b) => ({
    listAudioDevices: b.query<AudioDevice[], void>({
      query: () => ({ cmd: 'list_audio_devices' }),
      providesTags: ['AudioDevice'],
    }),
  }),
});

export const { useListAudioDevicesQuery } = devicesApi;
