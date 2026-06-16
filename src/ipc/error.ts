import type { TKey } from '@/i18n/keys';
import type { AppError } from '@/store/api/base';

const codeToKey: Record<string, TKey> = {
  DeviceNotFound: 'audioError.DeviceNotFound',
  WasapiInit: 'audioError.WasapiInit',
  FormatUnsupported: 'audioError.FormatUnsupported',
  DiskFull: 'audioError.DiskFull',
  MixerOverflow: 'audioError.MixerOverflow',
  ModelNotDownloaded: 'transcriptionError.ModelNotDownloaded',
  TranscriptionBusy: 'transcriptionError.TranscriptionBusy',
  DecodeFailed: 'transcriptionError.DecodeFailed',
  ModelLoadFailed: 'transcriptionError.ModelLoadFailed',
  InferenceFailed: 'transcriptionError.InferenceFailed',
  DownloadBusy: 'transcriptionError.DownloadBusy',
  DownloadFailed: 'transcriptionError.DownloadFailed',
};

export function errorMessage(err: AppError, t: (k: TKey) => string): string {
  const key = codeToKey[err.code];
  return key ? t(key) : err.message;
}

/**
 * Normalizes any thrown/rejected value from a Tauri invoke into a flat AppError.
 *
 * Handles the adjacently-tagged AppError::Audio shape where the outer message
 * is a nested object `{ code: AudioErrorCode; message: string }` — those are
 * flattened so `err.code` is the AudioErrorCode and `err.message` is the string.
 */
export function toAppError(e: unknown): AppError {
  if (typeof e === 'object' && e !== null && 'code' in e) {
    const outer = e as Record<string, unknown>;
    if (
      (outer.code === 'audio' || outer.code === 'transcription') &&
      typeof outer.message === 'object' &&
      outer.message !== null &&
      typeof (outer.message as Record<string, unknown>).code === 'string' &&
      typeof (outer.message as Record<string, unknown>).message === 'string'
    ) {
      const inner = outer.message as { code: string; message: string };
      return { code: inner.code, message: inner.message };
    }
    return {
      code: String(outer.code),
      message: typeof outer.message === 'string' ? outer.message : String(outer.message),
    };
  }
  return { code: 'internal', message: String(e) };
}
