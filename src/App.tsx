import { Suspense, useEffect } from 'react';
import { useRoutes } from 'react-router-dom';
import styles from './App.module.css';
import { routes } from './router/routes';
import { useGetSettingsQuery } from './store/api/settings.api';
import { useAppDispatch, useAppSelector } from './store/hooks';
import { hydrateFromBackend } from './store/slices/ui.slice';
import { ThemeProvider } from './theme/ThemeProvider';

export default function App() {
  const dispatch = useAppDispatch();
  const ui = useAppSelector((s) => s.ui);
  const { data: settings } = useGetSettingsQuery();

  useEffect(() => {
    if (settings) {
      dispatch(
        hydrateFromBackend({
          theme: settings.theme,
          accent: settings.accent,
          language: settings.language as 'es' | 'en',
          avatarStyle: settings.avatarStyle,
          aiChatVisible: settings.aiChatVisible,
        })
      );
    }
  }, [settings, dispatch]);

  const element = useRoutes(routes);

  return (
    <ThemeProvider theme={ui.theme} accent={ui.accent} avatarStyle={ui.avatarStyle}>
      <div className={styles.app}>
        <Suspense fallback={<div className={styles.suspense}>Loading…</div>}>{element}</Suspense>
      </div>
    </ThemeProvider>
  );
}
