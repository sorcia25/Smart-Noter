import { type ReactNode, useEffect } from 'react';
import { applyAccent } from './accent';

export interface ThemeProviderProps {
  theme: 'light' | 'dark';
  accent: string;
  avatarStyle: 'circle' | 'square';
  children: ReactNode;
}

export function ThemeProvider({ theme, accent, avatarStyle, children }: ThemeProviderProps) {
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  useEffect(() => {
    applyAccent(accent);
  }, [accent]);

  useEffect(() => {
    const id = '__avatar-style';
    let s = document.getElementById(id);
    if (!s) {
      s = document.createElement('style');
      s.id = id;
      document.head.appendChild(s);
    }
    s.textContent = avatarStyle === 'square' ? '.avatar { border-radius: 8px !important; }' : '';
  }, [avatarStyle]);

  return <>{children}</>;
}
