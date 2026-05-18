import { Icon } from '@/components/primitives/Icon/Icon';
import { useT } from '@/i18n/useT';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import styles from './WindowChrome.module.css';

export interface WindowChromeProps {
  title?: string;
}

export function WindowChrome({ title }: WindowChromeProps) {
  const { t } = useT();
  const win = getCurrentWebviewWindow();

  return (
    <div className={styles.titlebar} data-tauri-drag-region>
      <div className={styles.title} data-tauri-drag-region>
        <div className={styles.brandMark}>
          <Icon name="mic" size={10} stroke="white" />
        </div>
        <span data-tauri-drag-region>
          {t('appName')} — {title ?? ''}
        </span>
      </div>
      <div className={styles.controls}>
        <button
          type="button"
          className={styles.ctrl}
          onClick={() => void win.minimize()}
          title={t('winMinimize')}
        >
          <svg viewBox="0 0 10 10" width="10" height="10">
            <title>{t('winMinimize')}</title>
            <rect x="1" y="5" width="8" height="1" fill="currentColor" />
          </svg>
        </button>
        <button
          type="button"
          className={styles.ctrl}
          onClick={() => void win.toggleMaximize()}
          title={t('winMaximize')}
        >
          <svg viewBox="0 0 10 10" width="10" height="10" fill="none">
            <title>{t('winMaximize')}</title>
            <rect x="1" y="1" width="8" height="8" stroke="currentColor" />
          </svg>
        </button>
        <button
          type="button"
          className={`${styles.ctrl} ${styles.close}`}
          onClick={() => void win.close()}
          title={t('winClose')}
        >
          <svg viewBox="0 0 10 10" width="10" height="10" stroke="currentColor" strokeWidth={1}>
            <title>{t('winClose')}</title>
            <line x1="1" y1="1" x2="9" y2="9" />
            <line x1="9" y1="1" x2="1" y2="9" />
          </svg>
        </button>
      </div>
    </div>
  );
}
