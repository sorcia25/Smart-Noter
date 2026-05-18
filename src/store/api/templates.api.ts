import type { Template } from '@/ipc/bindings';
import { baseApi } from './base';

export const templatesApi = baseApi.injectEndpoints({
  endpoints: (b) => ({
    listTemplates: b.query<Template[], void>({
      query: () => ({ cmd: 'list_templates' }),
      providesTags: ['Template'],
    }),
    setDefaultTemplate: b.mutation<void, string>({
      query: (id) => ({ cmd: 'set_default_template', args: { id } }),
      invalidatesTags: ['Template'],
    }),
  }),
});

export const { useListTemplatesQuery, useSetDefaultTemplateMutation } = templatesApi;
