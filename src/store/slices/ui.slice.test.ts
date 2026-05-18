import { describe, expect, it } from 'vitest';
import { setAccent, setTheme, uiSlice } from './ui.slice';

describe('ui.slice', () => {
  it('changes theme', () => {
    const state = uiSlice.reducer(undefined, setTheme('dark'));
    expect(state.theme).toBe('dark');
  });
  it('changes accent', () => {
    const state = uiSlice.reducer(undefined, setAccent('#3b82f6'));
    expect(state.accent).toBe('#3b82f6');
  });
});
