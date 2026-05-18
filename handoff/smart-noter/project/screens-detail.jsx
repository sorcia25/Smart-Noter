/* Smart Noter — Meeting Detail (transcript / summary / actions / audio) + AI chat */
const { createElement: hM, useState: useStateM, useMemo: useMemoM } = React;

function MeetingDetail({ lang, t, navigate, meetingId, aiChatVisible, openExport }) {
  const meeting = MEETINGS.find(m => m.id === meetingId) || MEETINGS[0];
  const tmpl = getTemplate(meeting.template);

  const [tab, setTab] = useStateM('summary');
  const [actions, setActions] = useStateM(meeting.actions || []);
  const [participants, setParticipants] = useStateM(meeting.participants);
  const [editingPart, setEditingPart] = useStateM(null);
  const [aiOpen, setAiOpen] = useStateM(true);

  const speakerById = useMemoM(() => {
    const map = {};
    participants.forEach(p => map[p.id] = p);
    return map;
  }, [participants]);

  function toggleAction(id) {
    setActions(prev => prev.map(a => a.id === id ? { ...a, done: !a.done } : a));
  }

  function renamePart(id, name) {
    setParticipants(prev => prev.map(p => p.id === id ? { ...p, name } : p));
  }

  return hM('div', { className: 'win-main', 'data-screen-label': '05 Meeting detail' },
    hM('div', { className: 'page-header', style: { paddingBottom: 12 } },
      hM('div', { style: { minWidth: 0 } },
        hM('button', { className: 'btn btn-ghost', onClick: () => navigate('meetings'), style: { padding: '4px 6px', marginBottom: 8, fontSize: 12 } },
          hM(Icon, { name: 'chevLeft', size: 14 }), t('backToMeetings')),
        hM('div', { className: 'flex items-center gap-3' },
          hM(TemplateIcon, { tmplId: meeting.template, size: 40 }),
          hM('div', { style: { minWidth: 0 } },
            hM('h1', { className: 'page-title', style: { fontSize: 22 } }, pickL(meeting.title, lang)),
            hM('div', { className: 'page-sub', style: { display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap' } },
              hM('span', null, pickL(tmpl.name, lang)),
              hM('span', { className: 'sep', style: { width: 3, height: 3, borderRadius: '50%', background: 'var(--text-subtle)' } }),
              hM('span', null, fmtDate(meeting.date, lang)),
              hM('span', { className: 'sep', style: { width: 3, height: 3, borderRadius: '50%', background: 'var(--text-subtle)' } }),
              hM('span', null, fmtDuration(meeting.durationSec), ' · ', meeting.wordCount, ' ' + (lang === 'es' ? 'palabras' : 'words')),
              hM('span', { className: 'chip chip-accent' }, '99.2% ' + t('fidelity'))
            )
          )
        )
      ),
      hM('div', { className: 'page-actions' },
        hM('button', { className: 'btn' }, hM(Icon, { name: 'share', size: 14 }), t('share')),
        hM('button', { className: 'btn btn-primary', onClick: openExport },
          hM(Icon, { name: 'download', size: 14 }), t('export'))
      )
    ),
    hM('div', { className: 'detail-wrap' },
      hM('div', { className: 'detail-main scroll' },
        // tabs
        hM('div', { className: 'segmented', style: { marginBottom: 14 } },
          [
            { value: 'summary', label: t('summary') },
            { value: 'transcript', label: t('transcript') },
            { value: 'actions', label: t('actions') + ` (${actions.length})` },
            { value: 'audio', label: t('audio') }
          ].map(o => hM('button', {
            key: o.value, className: tab === o.value ? 'active' : '',
            onClick: () => setTab(o.value)
          }, o.label))
        ),

        tab === 'summary' && hM(SummaryView, { meeting, tmpl, lang, t }),
        tab === 'transcript' && hM(TranscriptView, { meeting, speakerById, lang, t }),
        tab === 'actions' && hM(ActionsView, { actions, toggleAction, participants: speakerById, lang, t }),
        tab === 'audio' && hM(AudioView, { meeting, lang, t })
      ),
      hM(SidePanel, {
        meeting, participants, lang, t,
        editingPart, setEditingPart, renamePart,
        aiOpen, setAiOpen, aiChatVisible
      })
    )
  );
}

function SummaryView({ meeting, tmpl, lang, t }) {
  // Section content registry
  const SECTIONS = {
    summary: { key: 'secSummary', icon: 'sparkles', render: () => hM('p', null, pickL(meeting.summary, lang)) },
    decisions: {
      key: 'secDecisions', icon: 'check',
      render: () => hM('ul', null, (meeting.decisions || []).map((d, i) => hM('li', { key: i }, pickL(d, lang))))
    },
    metrics: {
      key: 'secMetrics', icon: 'zap',
      render: () => hM('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(3,1fr)', gap: 12 } },
        [
          { l: lang === 'es' ? 'Avance Sprint' : 'Sprint progress', v: '86%', d: '18/21' },
          { l: lang === 'es' ? 'Velocidad' : 'Velocity', v: '+12%', d: 'vs Sprint 3' },
          { l: lang === 'es' ? 'Cobertura tests' : 'Test coverage', v: '74%', d: 'target 80%' }
        ].map((m,i) => hM('div', { key: i, style: { padding: 12, background: 'var(--bg-inset)', borderRadius: 8 } },
          hM('div', { className: 'text-xs text-muted' }, m.l),
          hM('div', { style: { fontSize: 20, fontWeight: 600, marginTop: 4 } }, m.v),
          hM('div', { className: 'text-xs text-subtle', style: { marginTop: 2 } }, m.d)))
      )
    },
    risks: {
      key: 'secRisks', icon: 'flag',
      render: () => hM('ul', null, (meeting.blockers || []).map((b, i) => hM('li', { key: i }, pickL(b, lang))))
    },
    blockers: {
      key: 'secBlockers', icon: 'flag',
      render: () => hM('ul', null, (meeting.blockers || []).map((b, i) => hM('li', { key: i }, pickL(b, lang))))
    },
    actions: { key: 'secActions', icon: 'check', render: () => null }, // shown in dedicated tab
    // Synthetic content for other templates
    architecture: { key: 'secArchitecture', icon: 'cpu', render: () => hM('p', null,
      lang === 'es' ? 'Arquitectura por capas con frontend React + Tailwind, backend NestJS, base de datos PostgreSQL y cola de mensajería con RabbitMQ. Integración con SAP vía API REST con reintento exponencial.' : 'Layered architecture: React + Tailwind frontend, NestJS backend, PostgreSQL database, RabbitMQ messaging queue. SAP integration via REST API with exponential backoff.'
    )},
    'tech-decisions': { key: 'secTechDecisions', icon: 'check', render: () => hM('ul', null,
      [
        lang === 'es' ? 'Migrar de WebSockets a SSE para notificaciones (menor overhead).' : 'Migrate WebSockets → SSE for notifications (lower overhead).',
        lang === 'es' ? 'Adoptar Drizzle ORM en lugar de TypeORM para nuevos módulos.' : 'Adopt Drizzle ORM instead of TypeORM for new modules.',
        lang === 'es' ? 'Mover jobs largos a worker pool con Redis.' : 'Move long jobs to worker pool backed by Redis.'
      ].map((x, i) => hM('li', { key: i }, x))
    )},
    deliverables: { key: 'secDeliverables', icon: 'bookmark', render: () => hM('ul', null,
      [
        lang === 'es' ? 'Módulo de pipeline desplegado en staging (RC1).' : 'Pipeline module deployed to staging (RC1).',
        lang === 'es' ? 'Documento de arquitectura del módulo de reportería.' : 'Reporting module architecture document.',
        lang === 'es' ? 'Plan de rollback para Go-Live.' : 'Rollback plan for Go-Live.'
      ].map((x, i) => hM('li', { key: i }, x))
    )}
  };

  return hM('div', null,
    tmpl.sections.map(secKey => {
      const conf = SECTIONS[secKey];
      if (!conf || secKey === 'actions') return null;
      const body = conf.render();
      if (!body) return null;
      return hM('div', { className: 'summary-block', key: secKey },
        hM('h3', null,
          hM(Icon, { name: conf.icon, size: 14 }),
          hM('span', null, t(conf.key))),
        body
      );
    })
  );
}

function TranscriptView({ meeting, speakerById, lang, t }) {
  const lines = meeting.transcript || [];
  // For meetings without full transcript, synthesize a sample
  const effective = lines.length ? lines : [
    { t: '00:00:04', speakerId: meeting.participants[0].id, text: { es: 'Bienvenidos. Vamos a comenzar la sesión hoy revisando los puntos pendientes.', en: 'Welcome. Let\'s start today\'s session reviewing pending items.' } },
    { t: '00:00:20', speakerId: meeting.participants[1].id, text: { es: 'Gracias. Tengo varios puntos importantes que compartir con el equipo.', en: 'Thanks. I have several important points to share with the team.' } }
  ];
  return hM('div', { className: 'card card-pad' },
    hM('div', { className: 'flex items-center justify-between', style: { marginBottom: 12 } },
      hM('div', { className: 'flex gap-2 items-center' },
        hM(Icon, { name: 'mic', size: 14, stroke: 'var(--accent)' }),
        hM('span', { style: { fontSize: 13, fontWeight: 600 } }, t('transcript')),
        hM('span', { className: 'chip chip-accent' }, '99.2% ' + t('fidelity'))),
      hM('div', { className: 'flex items-center gap-2' },
        hM('button', { className: 'btn btn-icon', title: t('search') }, hM(Icon, { name: 'search', size: 14 })),
        hM('div', { className: 'segmented' },
          hM('button', { className: 'active' }, t('timestampsOn')),
          hM('button', null, lang === 'es' ? 'Sólo texto' : 'Text only')))
    ),
    hM('div', null, effective.map((l, i) => {
      const sp = speakerById[l.speakerId] || { label: 'S?', colorClass: 's-color-1' };
      return hM('div', { className: 'transcript-line', key: i },
        hM('div', { className: 'who' },
          hM(SubjectAvatar, { p: sp, size: 32 }),
          hM('div', { className: 'time' }, l.t)),
        hM('div', null,
          hM('div', { className: 'flex items-center gap-2', style: { marginBottom: 4 } },
            hM('span', { className: 'speaker', style: { color: 'var(--text)' } }, sp.name || (lang === 'es' ? `Sujeto ${sp.label.slice(1)}` : `Subject ${sp.label.slice(1)}`)),
            hM('button', { className: 'play-here btn btn-ghost', style: { padding: 2, fontSize: 11 } },
              hM(Icon, { name: 'play', size: 11 }), t('play'))),
          hM('div', { className: 'text' }, pickL(l.text, lang)))
      );
    }))
  );
}

function ActionsView({ actions, toggleAction, participants, lang, t }) {
  return hM('div', { className: 'card card-pad' },
    hM('div', { className: 'flex items-center justify-between', style: { marginBottom: 12 } },
      hM('div', { className: 'flex gap-2 items-center' },
        hM(Icon, { name: 'check', size: 14, stroke: 'var(--accent)' }),
        hM('span', { style: { fontSize: 13, fontWeight: 600 } }, t('actions')),
        hM('span', { className: 'chip' }, actions.length, ' ', lang === 'es' ? 'total' : 'total')),
      hM('button', { className: 'btn' }, hM(Icon, { name: 'plus', size: 14 }), lang === 'es' ? 'Añadir' : 'Add')
    ),
    actions.map(a => {
      const owner = participants[a.owner];
      return hM('div', { className: 'action-item' + (a.done ? ' done' : ''), key: a.id },
        hM('button', {
          className: 'action-check', onClick: () => toggleAction(a.id),
          style: { cursor: 'pointer' }
        }, a.done ? hM(Icon, { name: 'check', size: 11, stroke: 'white' }) : null),
        hM('div', null,
          hM('div', { className: 'action-text' }, pickL(a.text, lang)),
          hM('div', { className: 'action-meta' },
            owner ? hM(SubjectAvatar, { p: owner, size: 18 }) : null,
            owner ? hM('span', null, owner.name || `Sujeto ${owner.label.slice(1)}`) : null,
            hM('span', null, '·'),
            hM(Icon, { name: 'clock', size: 11 }),
            hM('span', null, new Date(a.due).toLocaleDateString(lang === 'en' ? 'en-US' : 'es-MX', { day: '2-digit', month: 'short' }))
          )
        ),
        hM('button', { className: 'btn btn-icon btn-ghost' }, hM(Icon, { name: 'more', size: 16 }))
      );
    })
  );
}

function AudioView({ meeting, lang, t }) {
  // synthetic waveform path
  const bars = useMemoM(() => Array.from({ length: 120 }, (_, i) => 0.2 + Math.sin(i / 4) * 0.3 + Math.random() * 0.5), [meeting.id]);
  const progress = 0.32;
  return hM('div', null,
    hM('div', { className: 'card card-pad', style: { marginBottom: 14 } },
      hM('div', { className: 'flex items-center justify-between', style: { marginBottom: 12 } },
        hM('div', { className: 'flex gap-2 items-center' },
          hM(Icon, { name: 'mic', size: 14, stroke: 'var(--accent)' }),
          hM('span', { style: { fontSize: 13, fontWeight: 600 } }, t('audio'))),
        hM('div', { className: 'flex items-center gap-2' },
          hM('span', { className: 'chip' }, 'WAV · 48 kHz'),
          hM('span', { className: 'chip' }, '47.2 MB'))
      ),
      // waveform display
      hM('div', {
        style: {
          background: 'var(--bg-inset)',
          borderRadius: 8,
          padding: '20px 16px',
          display: 'flex',
          alignItems: 'center',
          gap: 2,
          height: 120,
          position: 'relative'
        }
      },
        bars.map((b, i) => {
          const isPlayed = i / bars.length < progress;
          return hM('div', {
            key: i,
            style: {
              flex: 1,
              height: `${Math.max(8, Math.min(95, b * 100))}%`,
              minHeight: 4,
              background: isPlayed ? 'var(--accent)' : 'var(--stroke-strong)',
              borderRadius: 2
            }
          });
        })
      ),
      // controls
      hM('div', { className: 'flex items-center gap-3', style: { marginTop: 14 } },
        hM('button', { className: 'btn btn-icon' }, hM(Icon, { name: 'back', size: 14 })),
        hM('button', { className: 'player-play' }, hM(Icon, { name: 'play', size: 18, stroke: 'currentColor' })),
        hM('button', { className: 'btn btn-icon' }, hM(Icon, { name: 'forward', size: 14 })),
        hM('span', { className: 'player-time' }, fmtDuration(Math.floor(meeting.durationSec * progress)), ' / ', fmtDuration(meeting.durationSec)),
        hM('div', { style: { flex: 1 } }),
        hM('div', { className: 'segmented' },
          hM('button', null, '0.5×'),
          hM('button', { className: 'active' }, '1×'),
          hM('button', null, '1.5×'),
          hM('button', null, '2×')
        ),
        hM('button', { className: 'btn btn-icon' }, hM(Icon, { name: 'download', size: 14 }))
      )
    ),
    hM('div', { className: 'card card-pad' },
      hM('h3', { style: { margin: 0, fontSize: 13, fontWeight: 600 } }, lang === 'es' ? 'Marcadores' : 'Markers'),
      hM('div', { className: 'text-sm text-muted', style: { marginTop: 4, marginBottom: 12 } }, lang === 'es' ? 'Puntos importantes detectados automáticamente.' : 'Important points detected automatically.'),
      [
        { t: '00:01:24', txt: lang === 'es' ? 'Decisión: agendar sesión con SAP' : 'Decision: schedule SAP session', icon: 'check' },
        { t: '00:01:42', txt: lang === 'es' ? 'Confirmación de Go-Live para 18 dic' : 'Go-Live confirmed for Dec 18', icon: 'flag' },
        { t: '00:03:05', txt: lang === 'es' ? 'Acción: contratar consultor SAP' : 'Action: hire SAP consultant', icon: 'zap' }
      ].map((m, i) => hM('div', { key: i, className: 'flex items-center gap-3', style: { padding: '8px 0', borderBottom: i < 2 ? '1px solid var(--stroke)' : 'none' } },
        hM('span', { className: 'duration', style: { width: 70 } }, m.t),
        hM(Icon, { name: m.icon, size: 14, stroke: 'var(--accent)' }),
        hM('span', { style: { fontSize: 13 } }, m.txt),
        hM('div', { style: { flex: 1 } }),
        hM('button', { className: 'btn btn-icon btn-ghost' }, hM(Icon, { name: 'play', size: 12 }))
      ))
    )
  );
}

function SidePanel({ meeting, participants, lang, t, editingPart, setEditingPart, renamePart, aiOpen, setAiOpen, aiChatVisible }) {
  return hM('aside', { className: 'detail-side' },
    // Participants block
    hM('div', { style: { padding: '14px 16px', borderBottom: '1px solid var(--stroke)' } },
      hM('div', { className: 'flex items-center justify-between', style: { marginBottom: 4 } },
        hM('div', { style: { fontSize: 13, fontWeight: 600 } }, t('participants'), ' (', participants.length, ')'),
        hM('button', { className: 'btn btn-ghost', style: { fontSize: 11, padding: '3px 6px' } }, hM(Icon, { name: 'edit', size: 12 }), t('rename')))
    ),
    hM('div', { style: { padding: '6px', maxHeight: aiChatVisible ? 280 : '100%', overflow: 'auto' }, className: 'scroll' },
      participants.map(p => hM('div', {
        key: p.id, className: 'participant-row',
        onMouseEnter: () => {}, // hover feedback handled by CSS
      },
        hM(SubjectAvatar, { p, size: 36 }),
        hM('div', { style: { minWidth: 0, flex: 1 } },
          editingPart === p.id
            ? hM('input', {
                className: 'input',
                style: { padding: '4px 6px', fontSize: 12 },
                autoFocus: true,
                defaultValue: p.name || '',
                placeholder: lang === 'es' ? `Sujeto ${p.label.slice(1)}` : `Subject ${p.label.slice(1)}`,
                onBlur: e => { renamePart(p.id, e.target.value || null); setEditingPart(null); },
                onKeyDown: e => { if (e.key === 'Enter') e.target.blur(); if (e.key === 'Escape') setEditingPart(null); }
              })
            : hM('div', { onClick: () => setEditingPart(p.id), style: { cursor: 'pointer' } },
                hM('div', { className: 'participant-name' }, p.name || (lang === 'es' ? `Sujeto ${p.label.slice(1)}` : `Subject ${p.label.slice(1)}`)),
                hM('div', { className: 'participant-orig' }, p.name ? p.label : (lang === 'es' ? 'click para nombrar' : 'click to name')))
        ),
        hM('div', { className: 'participant-stats text-xs' }, (p.talkPct || 0) + '%')
      ))
    ),
    // AI chat
    aiChatVisible && hM('div', { className: 'ai-panel' },
      hM('div', { className: 'ai-header' },
        hM('div', { className: 'ai-icon' }, hM(Icon, { name: 'sparkles', size: 12, stroke: 'white' })),
        hM('span', null, t('aiAsk')),
        hM('button', {
          className: 'btn btn-icon btn-ghost ai-toggle',
          onClick: () => setAiOpen(v => !v)
        }, hM(Icon, { name: aiOpen ? 'chevDown' : 'chevRight', size: 14 }))
      ),
      aiOpen && hM('div', { className: 'ai-body scroll' },
        hM('div', { className: 'ai-msg ai-msg-bot' },
          lang === 'es'
            ? '¡Hola! Tengo cargada esta reunión. Puedes preguntar cualquier cosa sobre lo que se dijo.'
            : 'Hi! I have this meeting loaded. Ask anything about what was said.'),
        hM('div', { className: 'ai-msg ai-msg-user' },
          lang === 'es' ? '¿Cuáles fueron los principales bloqueos discutidos?' : 'What were the main blockers discussed?'),
        hM('div', { className: 'ai-msg ai-msg-bot' },
          lang === 'es'
            ? 'Dos bloqueos principales: (1) timeout en la API de SAP al cargar > 5k registros (mencionado por Diego @ 00:51), y (2) firma pendiente del cliente para acceso al ambiente productivo (Marta @ 01:42). Ambos tienen acciones asignadas.'
            : 'Two main blockers: (1) SAP API timeout on > 5k records (Diego @ 00:51), and (2) pending client signature for production env (Marta @ 01:42). Both have assigned actions.')
      ),
      aiOpen && hM('div', { className: 'ai-suggested' },
        [t('suggestedQ1'), t('suggestedQ2'), t('suggestedQ3')].map((q, i) => hM('button', { key: i, className: 'ai-chip' }, q))),
      aiOpen && hM('div', { className: 'ai-footer' },
        hM('input', { className: 'input', placeholder: t('askPlaceholder') }),
        hM('button', { className: 'btn btn-primary btn-icon' }, hM(Icon, { name: 'send', size: 14, stroke: 'currentColor' })))
    )
  );
}

window.MeetingDetail = MeetingDetail;
