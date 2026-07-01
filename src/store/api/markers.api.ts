import type { Marker } from '@/ipc/bindings';
import { baseApi } from './base';

export const markersApi = baseApi.injectEndpoints({
  endpoints: (b) => ({
    listMarkers: b.query<Marker[], string>({
      query: (meetingId) => ({ cmd: 'list_markers', args: { meetingId } }),
      providesTags: ['Marker'],
    }),
    createMarker: b.mutation<Marker, { meetingId: string; tSeconds: number; label: string }>({
      query: (args) => ({ cmd: 'create_marker', args }),
      invalidatesTags: ['Marker'],
    }),
    updateMarker: b.mutation<void, { id: string; label: string }>({
      query: (args) => ({ cmd: 'update_marker', args }),
      invalidatesTags: ['Marker'],
    }),
    deleteMarker: b.mutation<void, string>({
      query: (id) => ({ cmd: 'delete_marker', args: { id } }),
      invalidatesTags: ['Marker'],
    }),
  }),
});

export const {
  useListMarkersQuery,
  useCreateMarkerMutation,
  useUpdateMarkerMutation,
  useDeleteMarkerMutation,
} = markersApi;
