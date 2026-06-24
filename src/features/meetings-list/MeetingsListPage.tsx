import { MeetingRow } from '@/components/domain/MeetingRow/MeetingRow';
import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { Modal } from '@/components/primitives/Modal/Modal';
import { SearchBox } from '@/components/primitives/SearchBox/SearchBox';
import { toast } from '@/components/primitives/Toast/Toast';
import { useT } from '@/i18n/useT';
import type { MeetingSummary, SearchHit } from '@/ipc/bindings';
import { Paths } from '@/router/paths';
import {
  useDeleteMeetingMutation,
  useListMeetingsQuery,
  useSearchMeetingsQuery,
} from '@/store/api/meetings.api';
import { useListTemplatesQuery } from '@/store/api/templates.api';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import styles from './MeetingsListPage.module.css';
import { MeetingFilterChips } from './components/MeetingFilterChips/MeetingFilterChips';
import { SearchSnippet } from './components/SearchSnippet/SearchSnippet';

export default function MeetingsListPage() {
  const navigate = useNavigate();
  const { t, lang } = useT();
  const [search, setSearch] = useState('');
  const [debouncedSearch, setDebouncedSearch] = useState('');
  const [filter, setFilter] = useState('all');
  const [pendingDelete, setPendingDelete] = useState<string | null>(null);

  // Debounce 300ms
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(search), 300);
    return () => clearTimeout(timer);
  }, [search]);

  const isSearching = debouncedSearch.trim().length > 0;

  const { data: meetings = [] } = useListMeetingsQuery();
  const { data: templates = [] } = useListTemplatesQuery();
  const [deleteMeeting] = useDeleteMeetingMutation();

  const { data: hits = [], isFetching } = useSearchMeetingsQuery(
    { query: debouncedSearch, template: filter === 'all' ? null : filter },
    { skip: !isSearching }
  );

  // When NOT searching: filter only by template chip (no client-side title filter)
  const filtered = useMemo(() => {
    return meetings.filter((m) => {
      if (filter !== 'all' && m.template !== filter) return false;
      return true;
    });
  }, [meetings, filter]);

  const subText =
    lang === 'es'
      ? 'Todas tus reuniones grabadas y transcritas.'
      : 'All your recorded and transcribed meetings.';

  // Shared row renderer: MeetingRow + delete button (avoids duplication between branches)
  function renderRow(meeting: MeetingSummary, snippet?: string) {
    return (
      <div key={meeting.id} className={styles.rowWrap}>
        <div>
          <MeetingRow meeting={meeting} onClick={() => navigate(Paths.MeetingDetail(meeting.id))} />
          {snippet !== undefined && <SearchSnippet text={snippet} />}
        </div>
        <button
          type="button"
          aria-label={t('deleteMeeting')}
          data-testid={`delete-${meeting.id}`}
          className={styles.deleteBtn}
          onClick={() => setPendingDelete(meeting.id)}
        >
          <Icon name="trash" size={16} />
        </button>
      </div>
    );
  }

  function renderList() {
    if (isSearching) {
      if (isFetching && hits.length === 0) {
        return <div className={styles.empty}>{t('searchingHint')}</div>;
      }
      if (hits.length === 0) {
        return <div className={styles.empty}>{t('searchNoResults')}</div>;
      }
      return (
        <div className={styles.list}>
          {hits.map((hit: SearchHit) => renderRow(hit.meeting, hit.snippet))}
        </div>
      );
    }

    if (filtered.length === 0) {
      return (
        <div className={styles.empty}>
          {lang === 'es'
            ? 'Sin reuniones que coincidan con los filtros.'
            : 'No meetings match the filters.'}
        </div>
      );
    }
    return <div className={styles.list}>{filtered.map((m) => renderRow(m))}</div>;
  }

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
        {renderList()}
      </div>

      <Modal
        open={pendingDelete !== null}
        onClose={() => setPendingDelete(null)}
        title={t('confirmDeleteTitle')}
        subtitle={t('confirmDeleteBody')}
        footer={
          <>
            <Button variant="ghost" onClick={() => setPendingDelete(null)}>
              {t('cancel')}
            </Button>
            <Button
              variant="primary"
              onClick={async () => {
                if (!pendingDelete) return;
                try {
                  await deleteMeeting(pendingDelete).unwrap();
                } catch {
                  toast.error(t('errorTitle'));
                  return;
                }
                setPendingDelete(null);
              }}
            >
              {t('deleteMeeting')}
            </Button>
          </>
        }
      >
        {null}
      </Modal>
    </div>
  );
}
