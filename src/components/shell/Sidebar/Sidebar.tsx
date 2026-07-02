import { Icon, type IconName } from '@/components/primitives/Icon/Icon';
import { useT } from '@/i18n/useT';
import { Paths } from '@/router/paths';
import { useListMeetingsQuery } from '@/store/api/meetings.api';
import { NavLink, useLocation, useNavigate } from 'react-router-dom';
import styles from './Sidebar.module.css';

// Placeholder user. This footer gets wired to a real profile when cloud sync
// (audios/transcripts/summaries) lands — see the cloud-integration sub-project.
const MOCK_USER = {
  initials: 'TM',
  name: 'Toño Maldonado',
  role: 'Pro · v1.0.1',
} as const;

interface NavEntry {
  to: string;
  icon: IconName;
  label: string;
  count?: number;
}

export function Sidebar() {
  const navigate = useNavigate();
  const location = useLocation();
  const { t } = useT();
  const { data: meetings } = useListMeetingsQuery();

  const workspace: NavEntry[] = [
    { to: Paths.Dashboard, icon: 'home', label: t('navDashboard') },
    { to: Paths.Meetings, icon: 'list', label: t('navMeetings'), count: meetings?.length },
    { to: Paths.Templates, icon: 'templates', label: t('navTemplates') },
    { to: Paths.Trash, icon: 'trash', label: t('navTrash') },
  ];
  const tools: NavEntry[] = [
    { to: Paths.Participants, icon: 'user', label: t('participants') },
    { to: Paths.Settings, icon: 'settings', label: t('navSettings') },
  ];

  const isMeetingDetail = location.pathname.startsWith('/meetings/');

  return (
    <aside className={styles.sidebar}>
      <div className={styles.brand}>
        <div className={styles.brandMark}>
          <Icon name="mic" size={16} stroke="white" />
        </div>
        <div>
          <div className={styles.brandName}>{t('appName')}</div>
          <div className={styles.brandSub}>{t('appTag')}</div>
        </div>
      </div>
      <button type="button" className={styles.ctaRecord} onClick={() => navigate(Paths.PreRecord)}>
        <div className={styles.dot} />
        {t('navRecord')}
      </button>
      <div className={styles.navSection}>
        <div className={styles.navSectionLabel}>{t('navWorkspace')}</div>
        {workspace.map((it) => (
          <NavLink
            key={it.to}
            to={it.to}
            className={({ isActive }) =>
              `${styles.navItem} ${
                isActive || (it.to === Paths.Meetings && isMeetingDetail) ? styles.active : ''
              }`
            }
          >
            <Icon name={it.icon} size={18} />
            <span>{it.label}</span>
            {it.count != null && <span className={styles.count}>{it.count}</span>}
          </NavLink>
        ))}
      </div>
      <div className={styles.navSection}>
        <div className={styles.navSectionLabel}>{t('navTools')}</div>
        {tools.map((it) => (
          <NavLink
            key={it.to}
            to={it.to}
            className={({ isActive }) => `${styles.navItem} ${isActive ? styles.active : ''}`}
          >
            <Icon name={it.icon} size={18} />
            <span>{it.label}</span>
          </NavLink>
        ))}
      </div>
      <div className={styles.sidebarFooter}>
        <div className={styles.avatar}>{MOCK_USER.initials}</div>
        <div>
          <div className={styles.name}>{MOCK_USER.name}</div>
          <div className={styles.role}>{MOCK_USER.role}</div>
        </div>
      </div>
    </aside>
  );
}
