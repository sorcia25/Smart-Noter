import { useTranslation } from 'react-i18next';
import type { TKey } from './keys';

export function useT() {
  const { t, i18n } = useTranslation();
  return {
    t: (key: TKey, opts?: Record<string, unknown>) => t(key as string, opts ?? {}),
    lang: i18n.language as 'es' | 'en',
    setLang: (l: 'es' | 'en') => i18n.changeLanguage(l),
  };
}
