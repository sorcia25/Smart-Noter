import type { Bilingual } from '@/ipc/bindings';

export function fmtDuration(sec: number): string {
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  const pad = (n: number) => String(n).padStart(2, '0');
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

export function fmtDate(iso: string, lang: 'es' | 'en' = 'es'): string {
  const d = new Date(iso);
  return d.toLocaleString(lang === 'en' ? 'en-US' : 'es-MX', {
    day: '2-digit',
    month: 'short',
    hour: '2-digit',
    minute: '2-digit',
  });
}

export function pickL(b: Bilingual | undefined | null, lang: 'es' | 'en'): string {
  if (!b) return '';
  return lang === 'en' ? (b.en ?? b.es) : b.es;
}
