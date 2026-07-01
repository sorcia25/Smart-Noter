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
    // Avatars read `--avatar-radius` (Avatar.module.css / Sidebar.module.css). A global
    // `.avatar` stylesheet wouldn't match — those are hashed CSS-module class names.
    document.documentElement.style.setProperty(
      '--avatar-radius',
      avatarStyle === 'square' ? '8px' : '50%'
    );
  }, [avatarStyle]);

  return <>{children}</>;
}
