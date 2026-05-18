import type { TKey } from '@/i18n/keys';
import type { AppError } from '@/store/api/base';

const codeToKey: Record<string, TKey> = {};

export function errorMessage(err: AppError, t: (k: TKey) => string): string {
  const key = codeToKey[err.code];
  return key ? t(key) : err.message;
}
