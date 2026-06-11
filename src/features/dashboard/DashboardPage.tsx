import { MeetingRow } from '@/components/domain/MeetingRow/MeetingRow';
import { Button } from '@/components/primitives/Button/Button';
import { Icon } from '@/components/primitives/Icon/Icon';
import { SearchBox } from '@/components/primitives/SearchBox/SearchBox';
import { useT } from '@/i18n/useT';
import { Paths } from '@/router/paths';
import { useListAudioDevicesQuery } from '@/store/api/devices.api';
import { useListMeetingsQuery } from '@/store/api/meetings.api';
import { useListTemplatesQuery } from '@/store/api/templates.api';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import styles from './DashboardPage.module.css';
import { CaptureStatusCard } from './components/CaptureStatusCard/CaptureStatusCard';
import { QuickStartCard } from './components/QuickStartCard/QuickStartCard';
import { type Stat, StatRow } from './components/StatRow/StatRow';

export default function DashboardPage() {
  const navigate = useNavigate();
  const { t, lang } = useT();
  const [search, setSearch] = useState('');

  const { data: meetings = [] } = useListMeetingsQuery();
  const { data: devices = [] } = useListAudioDevicesQuery();
  const { data: templates = [] } = useListTemplatesQuery();

  const totalHours = (meetings.reduce((s, m) => s + m.durationSec, 0) / 3600).toFixed(1);
  const totalWords = meetings.reduce((s, m) => s + (m.wordCount ?? 0), 0);
  const activeDevice = devices.find((d) => d.isDefault) ?? devices[0];

  const stats: Stat[] = [
    { label: t('statTotal'), value: String(meetings.length), delta: `+3 ${t('thisWeek')}` },
    { label: t('statHours'), value: `${totalHours}h`, delta: `+2.4 ${t('thisWeek')}` },
    {
      label: t('statActions'),
      value: '12',
      delta: lang === 'es' ? '4 vencidas' : '4 overdue',
      deltaTone: 'warn',
    },
    {
      label: t('statTranscript'),
      value: `${(totalWords / 1000).toFixed(1)}k`,
      delta: `99.2% ${t('fidelity')}`,
    },
  ];

  return (
    <div className={styles.page} data-screen-label="01 Dashboard">
      <div className={styles.header}>
        <div>
          <h1 className={styles.title}>{t('welcome')}</h1>
          <div className={styles.sub}>{t('welcomeSub')}</div>
        </div>
        <div className={styles.actions}>
          <SearchBox value={search} onChange={setSearch} placeholder={t('searchMeetings')} />
          <Button icon={<Icon name="filter" size={14} />}>
            {lang === 'es' ? 'Filtros' : 'Filters'}
          </Button>
          <Button
            variant="primary"
            icon={<Icon name="record" size={11} />}
            onClick={() => navigate(Paths.PreRecord)}
          >
            {t('quickRecord')}
          </Button>
        </div>
      </div>
      <div className={styles.scroll}>
        <StatRow stats={stats} />
        <div className={styles.grid}>
          <section>
            <div className={styles.recentHead}>
              <h2>{t('recentMeetings')}</h2>
              <Button variant="ghost" onClick={() => navigate(Paths.Meetings)}>
                {t('seeAll')} <Icon name="chevRight" size={14} />
              </Button>
            </div>
            <div className={styles.meetingList}>
              {meetings.slice(0, 5).map((m) => (
                <MeetingRow
                  key={m.id}
                  meeting={m}
                  onClick={() => navigate(Paths.MeetingDetail(m.id))}
                />
              ))}
            </div>
          </section>
          <aside className={styles.aside}>
            <CaptureStatusCard device={activeDevice} />
            <QuickStartCard
              templates={templates}
              onPick={(tplId) => navigate(`${Paths.PreRecord}?tpl=${tplId}`)}
            />
          </aside>
        </div>
      </div>
    </div>
  );
}
