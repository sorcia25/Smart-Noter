import { describe, expect, it } from 'vitest';
import { fmtDuration, pickL } from './format';

describe('fmtDuration', () => {
  it('formats minutes:seconds when under an hour', () => {
    expect(fmtDuration(125)).toBe('2:05');
  });
  it('formats hours:minutes:seconds when over an hour', () => {
    expect(fmtDuration(3725)).toBe('1:02:05');
  });
  it('handles zero', () => {
    expect(fmtDuration(0)).toBe('0:00');
  });
});

describe('pickL', () => {
  it('returns es by default', () => {
    expect(pickL({ es: 'Hola', en: 'Hi' }, 'es')).toBe('Hola');
  });
  it('returns en when lang is en', () => {
    expect(pickL({ es: 'Hola', en: 'Hi' }, 'en')).toBe('Hi');
  });
  it('falls back to es when en is null', () => {
    expect(pickL({ es: 'Hola', en: null }, 'en')).toBe('Hola');
  });
  it('returns empty string when bilingual is nullish', () => {
    expect(pickL(undefined, 'es')).toBe('');
    expect(pickL(null, 'en')).toBe('');
  });
});
