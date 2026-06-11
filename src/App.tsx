import { listen } from '@tauri-apps/api/event';
import { Suspense, useEffect } from 'react';
import { useLocation, useRoutes } from 'react-router-dom';
import styles from './App.module.css';
import { ToastProvider, toast } from './components/primitives/Toast/Toast';
import { Sidebar } from './components/shell/Sidebar/Sidebar';
import { WindowChrome } from './components/shell/WindowChrome/WindowChrome';
import { useT } from './i18n/useT';
import type { AudioErrorCode } from './ipc/bindings';
import { errorMessage } from './ipc/error';
import { routes } from './router/routes';
import { useGetSettingsQuery } from './store/api/settings.api';
import { useAppDispatch, useAppSelector } from './store/hooks';
import { hydrateFromBackend } from './store/slices/ui.slice';
import { ThemeProvider } from './theme/ThemeProvider';

const TITLES_KEY: Record<string, string> = {
  '/': 'navDashboard',
  '/meetings': 'navMeetings',
  '/record/new': 'navRecord',
  '/templates': 'navTemplates',
  '/participants': 'participants',
  '/settings': 'navSettings',
};

export default function App() {
  const dispatch = useAppDispatch();
  const ui = useAppSelector((s) => s.ui);
  const { data: settings } = useGetSettingsQuery();
  const location = useLocation();
  const { t, setLang, lang } = useT();

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

  useEffect(() => {
    if (ui.language !== lang) setLang(ui.language);
  }, [ui.language, lang, setLang]);

  useEffect(() => {
    let cancelled = false;
    let un: (() => void) | null = null;
    listen<{ code: AudioErrorCode; message: string }>('audio:error', (e) => {
      if (cancelled) return;
      toast.error(t('audioErrorTitle'), { description: errorMessage(e.payload, t) });
    }).then((fn) => {
      if (cancelled) fn();
      else un = fn;
    });
    return () => {
      cancelled = true;
      un?.();
    };
  }, [t]);

  const element = useRoutes(routes);
  const titleKey = TITLES_KEY[location.pathname];
  const title = titleKey ? t(titleKey as never) : '';

  return (
    <ThemeProvider theme={ui.theme} accent={ui.accent} avatarStyle={ui.avatarStyle}>
      <div className={styles.shell}>
        <div className={styles.window}>
          <WindowChrome title={title} />
          <div className={styles.body}>
            <Sidebar />
            <main className={styles.main}>
              <Suspense fallback={<div className={styles.suspense}>{t('loading')}</div>}>
                {element}
              </Suspense>
            </main>
          </div>
        </div>
        <ToastProvider position="bottom-right" />
      </div>
    </ThemeProvider>
  );
}
