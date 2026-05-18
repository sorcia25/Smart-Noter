import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ThemeProvider } from './ThemeProvider';

describe('ThemeProvider', () => {
  it('sets data-theme on documentElement', () => {
    render(
      <ThemeProvider theme="dark" accent="#10b981" avatarStyle="circle">
        <span />
      </ThemeProvider>
    );
    expect(document.documentElement.getAttribute('data-theme')).toBe('dark');
  });

  it('applies accent CSS variable', () => {
    render(
      <ThemeProvider theme="light" accent="#3b82f6" avatarStyle="circle">
        <span />
      </ThemeProvider>
    );
    expect(document.documentElement.style.getPropertyValue('--accent')).toBe('#3b82f6');
  });
});
