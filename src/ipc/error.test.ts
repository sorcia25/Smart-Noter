import { describe, expect, it } from 'vitest';
import { errorMessage, toAppError } from './error';

// Stub t that echoes keys — makes assertions readable without loading i18n
const t = (k: string) => k;

describe('toAppError', () => {
  it('flattens nested audio rejection (adjacently-tagged AppError::Audio)', () => {
    const raw = {
      code: 'audio',
      message: { code: 'DiskFull', message: 'disk is full' },
    };
    expect(toAppError(raw)).toEqual({ code: 'DiskFull', message: 'disk is full' });
  });

  it('passes through plain non-audio rejection unchanged', () => {
    const raw = { code: 'internal', message: 'something went wrong' };
    expect(toAppError(raw)).toEqual({ code: 'internal', message: 'something went wrong' });
  });

  it('normalises non-object rejection to internal', () => {
    expect(toAppError('boom')).toEqual({ code: 'internal', message: 'boom' });
    expect(toAppError(42)).toEqual({ code: 'internal', message: '42' });
    expect(toAppError(null)).toEqual({ code: 'internal', message: 'null' });
    expect(toAppError(undefined)).toEqual({ code: 'internal', message: 'undefined' });
  });

  it('handles object without string message gracefully', () => {
    const raw = { code: 'validation', message: 123 };
    expect(toAppError(raw)).toEqual({ code: 'validation', message: '123' });
  });
});

describe('errorMessage', () => {
  it('returns translated key for a mapped audio code (DiskFull)', () => {
    const err = toAppError({ code: 'audio', message: { code: 'DiskFull', message: 'fallback' } });
    expect(errorMessage(err, t as (k: import('@/i18n/keys').TKey) => string)).toBe(
      'audioError.DiskFull'
    );
  });

  it('falls back to raw message for unmapped code AlreadyRecording', () => {
    const err = toAppError({
      code: 'audio',
      message: { code: 'AlreadyRecording', message: 'already active' },
    });
    expect(errorMessage(err, t as (k: import('@/i18n/keys').TKey) => string)).toBe(
      'already active'
    );
  });

  it('falls back to raw message for outer internal error', () => {
    const err = toAppError({ code: 'internal', message: 'db locked' });
    expect(errorMessage(err, t as (k: import('@/i18n/keys').TKey) => string)).toBe('db locked');
  });

  it('covers all 5 mapped codes', () => {
    const codes = [
      'DeviceNotFound',
      'WasapiInit',
      'FormatUnsupported',
      'DiskFull',
      'MixerOverflow',
    ] as const;
    for (const code of codes) {
      const err = { code, message: 'fallback' };
      expect(errorMessage(err, t as (k: import('@/i18n/keys').TKey) => string)).toBe(
        `audioError.${code}`
      );
    }
  });
});
