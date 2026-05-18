import { invoke } from '@tauri-apps/api/core';
import { commands } from './bindings';

export const tauri = {
  listMeetings: commands.listMeetings,
  getMeeting: commands.getMeeting,
  updateMeetingTitle: commands.updateMeetingTitle,
  toggleAction: commands.toggleAction,
  renameParticipant: commands.renameParticipant,
  listTemplates: commands.listTemplates,
  setDefaultTemplate: commands.setDefaultTemplate,
  listAudioDevices: commands.listAudioDevices,
  getSettings: commands.getSettings,
  updateSettings: commands.updateSettings,
  logFrontendError: (level: 'error' | 'warn' | 'info', message: string, stack?: string) =>
    invoke('log_frontend_error', { level, message, stack }),
};
