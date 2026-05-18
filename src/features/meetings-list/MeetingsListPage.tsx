import { MeetingRow } from '@/components/domain/MeetingRow/MeetingRow';
import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { SearchBox } from '@/components/primitives/SearchBox/SearchBox';
import { useT } from '@/i18n/useT';
import { Paths } from '@/router/paths';
import { useListMeetingsQuery } from '@/store/api/meetings.api';
import { useListTemplatesQuery } from '@/store/api/templates.api';
import { pickL } from '@/utils/format';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import styles from './MeetingsListPage.module.css';
import { MeetingFilterChips } from './components/MeetingFilterChips/MeetingFilterChips';

export default function MeetingsListPage() {
  const navigate = useNavigate();
  const { t, lang } = useT();
  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState('all');

  const { data: meetings = [] } = useListMeetingsQuery();
  const { data: templates = [] } = useListTemplatesQuery();

  const filtered = useMemo(() => {
    return meetings.filter((m) => {
      if (filter !== 'all' && m.template !== filter) return false;
      if (!search) return true;
      return pickL(m.title, lang).toLowerCase().includes(search.toLowerCase());
    });
  }, [meetings, filter, search, lang]);

  const subText =
    lang === 'es'
      ? 'Todas tus reuniones grabadas y transcritas.'
      : 'All your recorded and transcribed meetings.';

  return (
    <div className={styles.page} data-screen-label="02 Meetings list">
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>{t('navMeetings')}</h1>
          <div className={styles.sub}>{subText}</div>
        </div>
        <div className={styles.actions}>
          <SearchBox value={search} onChange={setSearch} placeholder={t('searchMeetings')} />
          <Button
            variant="primary"
            icon={<Icon name="plus" size={14} />}
            onClick={() => navigate(Paths.PreRecord)}
          >
            {t('quickRecord')}
          </Button>
        </div>
      </div>
      <div className={styles.scroll}>
        <MeetingFilterChips
          templates={templates}
          selected={filter}
          totalCount={meetings.length}
          onChange={setFilter}
        />
        {filtered.length > 0 ? (
          <div className={styles.list}>
            {filtered.map((m) => (
              <MeetingRow
                key={m.id}
                meeting={m}
                onClick={() => navigate(Paths.MeetingDetail(m.id))}
              />
            ))}
          </div>
        ) : (
          <div className={styles.empty}>
            {lang === 'es'
              ? 'Sin reuniones que coincidan con los filtros.'
              : 'No meetings match the filters.'}
          </div>
        )}
      </div>
    </div>
  );
}
