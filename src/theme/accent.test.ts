import { describe, expect, it } from 'vitest';
import { applyAccent, shade } from './accent';

describe('shade', () => {
  it('darkens by negative pct', () => {
    expect(shade('#10b981', -0.1)).toMatch(/^#[0-9a-f]{6}$/);
    expect(shade('#ffffff', -0.5)).toBe('#808080');
  });
  it('lightens by positive pct toward white', () => {
    expect(shade('#000000', 0.5)).toBe('#808080');
  });
});

describe('applyAccent', () => {
  it('sets --accent and derived custom properties on the root', () => {
    const root = document.createElement('div');
    applyAccent('#10b981', root);
    expect(root.style.getPropertyValue('--accent')).toBe('#10b981');
    expect(root.style.getPropertyValue('--accent-hover')).toBeTruthy();
    expect(root.style.getPropertyValue('--accent-soft')).toMatch(/^rgba\(/);
  });
});
