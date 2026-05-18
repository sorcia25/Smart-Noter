import { Icon, type IconName } from '@/components/primitives/Icon/Icon';
import { useT } from '@/i18n/useT';
import type { MeetingDetail, Template } from '@/ipc/bindings';
import { pickL } from '@/utils/format';
import styles from './SummaryTab.module.css';

interface SectionConfig {
  titleKey:
    | 'secSummary'
    | 'secDecisions'
    | 'secMetrics'
    | 'secRisks'
    | 'secBlockers'
    | 'secArchitecture'
    | 'secTechDecisions'
    | 'secDeliverables';
  icon: IconName;
  render: () => JSX.Element | null;
}

export interface SummaryTabProps {
  meeting: MeetingDetail;
  template: Template | undefined;
}

const FAKE_METRICS = (lang: 'es' | 'en') => [
  { l: lang === 'es' ? 'Avance Sprint' : 'Sprint progress', v: '86%', d: '18/21' },
  { l: lang === 'es' ? 'Velocidad' : 'Velocity', v: '+12%', d: 'vs Sprint 3' },
  { l: lang === 'es' ? 'Cobertura tests' : 'Test coverage', v: '74%', d: 'target 80%' },
];

const FAKE_ARCH = {
  es: 'Arquitectura por capas con frontend React + Tailwind, backend NestJS, base de datos PostgreSQL y cola de mensajería con RabbitMQ. Integración con SAP vía API REST con reintento exponencial.',
  en: 'Layered architecture: React + Tailwind frontend, NestJS backend, PostgreSQL database, RabbitMQ messaging queue. SAP integration via REST API with exponential backoff.',
};

const FAKE_TECH = {
  es: [
    'Migrar de WebSockets a SSE para notificaciones (menor overhead).',
    'Adoptar Drizzle ORM en lugar de TypeORM para nuevos módulos.',
    'Mover jobs largos a worker pool con Redis.',
  ],
  en: [
    'Migrate WebSockets → SSE for notifications (lower overhead).',
    'Adopt Drizzle ORM instead of TypeORM for new modules.',
    'Move long jobs to worker pool backed by Redis.',
  ],
};

const FAKE_DELIVERABLES = {
  es: [
    'Módulo de pipeline desplegado en staging (RC1).',
    'Documento de arquitectura del módulo de reportería.',
    'Plan de rollback para Go-Live.',
  ],
  en: [
    'Pipeline module deployed to staging (RC1).',
    'Reporting module architecture document.',
    'Rollback plan for Go-Live.',
  ],
};

export function SummaryTab({ meeting, template }: SummaryTabProps) {
  const { t, lang } = useT();

  const sectionsForTemplate = template?.sections ?? [
    'summary',
    'decisions',
    'metrics',
    'actions',
    'risks',
  ];

  const sections: Record<string, SectionConfig> = {
    summary: {
      titleKey: 'secSummary',
      icon: 'sparkles',
      render: () => <p>{pickL(meeting.summary, lang)}</p>,
    },
    decisions: {
      titleKey: 'secDecisions',
      icon: 'check',
      render: () =>
        meeting.decisions.length === 0 ? null : (
          <ul>
            {meeting.decisions.map((d) => (
              <li key={d.es}>{pickL(d, lang)}</li>
            ))}
          </ul>
        ),
    },
    metrics: {
      titleKey: 'secMetrics',
      icon: 'zap',
      render: () => (
        <div className={styles.metrics}>
          {FAKE_METRICS(lang).map((m) => (
            <div key={m.l} className={styles.metric}>
              <div className={styles.metricLabel}>{m.l}</div>
              <div className={styles.metricValue}>{m.v}</div>
              <div className={styles.metricSub}>{m.d}</div>
            </div>
          ))}
        </div>
      ),
    },
    risks: {
      titleKey: 'secRisks',
      icon: 'flag',
      render: () =>
        meeting.blockers.length === 0 ? null : (
          <ul>
            {meeting.blockers.map((b) => (
              <li key={b.es}>{pickL(b, lang)}</li>
            ))}
          </ul>
        ),
    },
    blockers: {
      titleKey: 'secBlockers',
      icon: 'flag',
      render: () =>
        meeting.blockers.length === 0 ? null : (
          <ul>
            {meeting.blockers.map((b) => (
              <li key={b.es}>{pickL(b, lang)}</li>
            ))}
          </ul>
        ),
    },
    architecture: {
      titleKey: 'secArchitecture',
      icon: 'cpu',
      render: () => <p>{FAKE_ARCH[lang]}</p>,
    },
    'tech-decisions': {
      titleKey: 'secTechDecisions',
      icon: 'check',
      render: () => (
        <ul>
          {FAKE_TECH[lang].map((x) => (
            <li key={x}>{x}</li>
          ))}
        </ul>
      ),
    },
    deliverables: {
      titleKey: 'secDeliverables',
      icon: 'bookmark',
      render: () => (
        <ul>
          {FAKE_DELIVERABLES[lang].map((x) => (
            <li key={x}>{x}</li>
          ))}
        </ul>
      ),
    },
  };

  return (
    <div>
      {sectionsForTemplate.map((key) => {
        if (key === 'actions') return null;
        const conf = sections[key];
        if (!conf) return null;
        const body = conf.render();
        if (!body) return null;
        return (
          <div className={styles.block} key={key}>
            <h3>
              <Icon name={conf.icon} size={14} />
              <span>{t(conf.titleKey)}</span>
            </h3>
            {body}
          </div>
        );
      })}
    </div>
  );
}
