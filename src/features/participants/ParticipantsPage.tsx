import { SubjectAvatar } from '@/components/primitives/Avatar/Avatar';
import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { SearchBox } from '@/components/primitives/SearchBox/SearchBox';
import { useT } from '@/i18n/useT';
import type { Participant } from '@/ipc/bindings';
import { useListMeetingsQuery } from '@/store/api/meetings.api';
import { useMemo, useState } from 'react';
import styles from './ParticipantsPage.module.css';

interface AggregatedParticipant extends Participant {
  meetings: string[];
}

function subjectLabel(label: string, lang: 'es' | 'en'): string {
  const suffix = label.replace(/^[a-zA-Z]+/, '');
  return lang === 'es'
    ? `Sujeto${suffix ? ` ${suffix}` : ''}`
    : `Subject${suffix ? ` ${suffix}` : ''}`;
}

export default function ParticipantsPage() {
  const { t, lang } = useT();
  const [search, setSearch] = useState('');

  const { data: meetings = [] } = useListMeetingsQuery();

  const aggregated = useMemo<AggregatedParticipant[]>(() => {
    const map = new Map<string, AggregatedParticipant>();
    for (const m of meetings) {
      for (const p of m.participants) {
        const key = p.name ?? `${m.id}::${p.id}`;
        const existing = map.get(key);
        if (existing) {
          existing.meetings.push(m.id);
        } else {
          map.set(key, { ...p, meetings: [m.id] });
        }
      }
    }
    return Array.from(map.values());
  }, [meetings]);

  const filtered = useMemo(() => {
    if (!search) return aggregated;
    const needle = search.toLowerCase();
    return aggregated.filter((p) => {
      const displayName = (p.name ?? subjectLabel(p.label, lang)).toLowerCase();
      return displayName.includes(needle) || p.label.toLowerCase().includes(needle);
    });
  }, [aggregated, search, lang]);

  return (
    <div className={styles.page} data-screen-label="07 Participants">
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>{t('partTitle')}</h1>
          <div className={styles.sub}>{t('partSub')}</div>
        </div>
        <div className={styles.actions}>
          <SearchBox
            value={search}
            onChange={setSearch}
            placeholder={lang === 'es' ? 'Buscar participante…' : 'Search participant…'}
          />
          <Button
            variant="primary"
            icon={<Icon name="plus" size={14} />}
            disabled
            title={lang === 'es' ? 'Próximamente' : 'Coming soon'}
          >
            {lang === 'es' ? 'Añadir' : 'Add'}
          </Button>
        </div>
      </div>
      <div className={styles.scroll}>
        {filtered.length === 0 ? (
          <div className={styles.empty}>
            {meetings.length === 0
              ? lang === 'es'
                ? 'Sin reuniones todavía.'
                : 'No meetings yet.'
              : lang === 'es'
                ? 'Sin participantes que coincidan con la búsqueda.'
                : 'No participants match the search.'}
          </div>
        ) : (
          <div className={styles.tableCard}>
            <div className={styles.headerRow}>
              <span />
              <span>{lang === 'es' ? 'Nombre' : 'Name'}</span>
              <span>{lang === 'es' ? 'Etiqueta original' : 'Original label'}</span>
              <span>{lang === 'es' ? 'Reuniones' : 'Meetings'}</span>
              <span />
            </div>
            {filtered.map((p) => (
              <div key={`${p.id}-${p.meetings[0] ?? ''}`} className={styles.row}>
                <SubjectAvatar participant={p} size={36} />
                <div>
                  <div className={styles.name}>{p.name ?? t('unnamed')}</div>
                  <div className={styles.subjectFallback}>{subjectLabel(p.label, lang)}</div>
                </div>
                <span className={styles.label}>{p.label}</span>
                <span className={styles.meetings}>
                  {p.meetings.length} {lang === 'es' ? 'reuniones' : 'meetings'}
                </span>
                <div className={styles.rowActions}>
                  <Button
                    size="icon"
                    variant="ghost"
                    disabled
                    title={lang === 'es' ? 'Próximamente' : 'Coming soon'}
                  >
                    <Icon name="edit" size={14} />
                  </Button>
                  <Button
                    size="icon"
                    variant="ghost"
                    disabled
                    title={lang === 'es' ? 'Próximamente' : 'Coming soon'}
                  >
                    <Icon name="more" size={16} />
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
