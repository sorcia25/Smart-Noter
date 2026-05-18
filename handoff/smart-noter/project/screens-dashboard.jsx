/* Smart Noter — Dashboard + Meetings List */
const { createElement: hD, useState: useStateD } = React;

function MeetingRow({ meeting, lang, t, onClick }) {
  const tmpl = getTemplate(meeting.template);
  return hD('div', { className: 'meeting-row', onClick, style: { cursor: 'pointer' } },
    hD(TemplateIcon, { tmplId: meeting.template, size: 44 }),
    hD('div', { style: { minWidth: 0 } },
      hD('div', { className: 'title', style: { whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' } }, pickL(meeting.title, lang)),
      hD('div', { className: 'sub' },
        hD('span', null, pickL(tmpl.name, lang)),
        hD('span', { className: 'sep' }),
        hD('span', null, fmtDate(meeting.date, lang)),
        hD('span', { className: 'sep' }),
        hD('span', null, `${meeting.participants.length} ${lang === 'es' ? 'participantes' : 'participants'}`)
      )
    ),
    hD(AvatarStack, { participants: meeting.participants, size: 26, max: 4 }),
    hD('div', { className: 'duration' }, fmtDuration(meeting.durationSec))
  );
}

function Dashboard({ navigate, lang, t }) {
  const totalHours = (MEETINGS.reduce((s, m) => s + m.durationSec, 0) / 3600).toFixed(1);
  const totalActions = MEETINGS.reduce((s, m) => s + (m.actions ? m.actions.filter(a => !a.done).length : 0), 0) || 12;
  const totalWords = MEETINGS.reduce((s, m) => s + (m.wordCount || 0), 0);

  return hD('div', { className: 'win-main', 'data-screen-label': '01 Dashboard' },
    hD('div', { className: 'page-header' },
      hD('div', null,
        hD('h1', { className: 'page-title' }, t('welcome')),
        hD('div', { className: 'page-sub' }, t('welcomeSub'))
      ),
      hD('div', { className: 'page-actions' },
        hD('div', { className: 'search-box' },
          hD(Icon, { name: 'search', size: 14 }),
          hD('input', { placeholder: t('searchMeetings') })
        ),
        hD('button', { className: 'btn' }, hD(Icon, { name: 'filter', size: 14 }), lang === 'es' ? 'Filtros' : 'Filters'),
        hD('button', { className: 'btn btn-primary', onClick: () => navigate('prerecord') },
          hD(Icon, { name: 'record', size: 11 }), t('quickRecord'))
      )
    ),
    hD('div', { className: 'page-scroll scroll' },
      // stats
      hD('div', { className: 'stat-row' },
        hD('div', { className: 'stat' },
          hD('div', { className: 'stat-label' }, t('statTotal')),
          hD('div', { className: 'stat-value' }, MEETINGS.length),
          hD('div', { className: 'stat-delta' }, '+3 ' + t('thisWeek'))),
        hD('div', { className: 'stat' },
          hD('div', { className: 'stat-label' }, t('statHours')),
          hD('div', { className: 'stat-value' }, totalHours, hD('span', { style: { fontSize: 14, color: 'var(--text-muted)', marginLeft: 4 } }, 'h')),
          hD('div', { className: 'stat-delta' }, '+2.4 ' + t('thisWeek'))),
        hD('div', { className: 'stat' },
          hD('div', { className: 'stat-label' }, t('statActions')),
          hD('div', { className: 'stat-value' }, totalActions),
          hD('div', { className: 'stat-delta', style: { color: '#f59e0b' } }, lang === 'es' ? '4 vencidas' : '4 overdue')),
        hD('div', { className: 'stat' },
          hD('div', { className: 'stat-label' }, t('statTranscript')),
          hD('div', { className: 'stat-value' }, (totalWords / 1000).toFixed(1) + 'k'),
          hD('div', { className: 'stat-delta' }, '99.2% ' + t('fidelity')))
      ),
      // main grid
      hD('div', { className: 'dash-grid' },
        hD('div', null,
          hD('div', { className: 'flex items-center justify-between', style: { marginTop: 6 } },
            hD('h2', { style: { margin: 0, fontSize: 14, fontWeight: 600 } }, t('recentMeetings')),
            hD('button', { className: 'btn btn-ghost', style: { fontSize: 12 }, onClick: () => navigate('meetings') },
              t('seeAll'), hD(Icon, { name: 'chevRight', size: 14 }))
          ),
          hD('div', { className: 'meeting-list' },
            MEETINGS.slice(0, 5).map(m =>
              hD(MeetingRow, { key: m.id, meeting: m, lang, t, onClick: () => navigate('meeting', { id: m.id }) }))
          )
        ),
        // right column
        hD('div', { className: 'col gap-3', style: { marginTop: 6 } },
          // capture status
          hD('div', { className: 'quick-card' },
            hD('h3', null, t('captureStatus')),
            hD('div', { className: 'sub' }, t('captureDesc')),
            hD('div', { className: 'device-pill' },
              hD('div', { className: 'live-dot' }),
              hD('div', { style: { lineHeight: 1.3 } },
                hD('div', { style: { fontWeight: 600, fontSize: 12 } }, pickL(AUDIO_DEVICES[0].name, lang)),
                hD('div', { className: 'text-subtle text-xs' }, lang === 'es' ? 'WASAPI Loopback · 48 kHz' : 'WASAPI Loopback · 48 kHz')
              ),
              hD('div', { className: 'eq-bar' }, [1,2,3,4,5].map(i => hD('span', { key: i })))
            ),
            hD('div', { style: { marginTop: 14 } },
              hD('div', { className: 'flex items-center justify-between', style: { marginBottom: 6 } },
                hD('span', { className: 'text-xs text-muted' }, t('inputLevel')),
                hD('span', { className: 'text-xs font-mono text-muted' }, '−12 dB')),
              hD('div', { className: 'level-bar' }, hD('div', { className: 'fill', style: { width: '62%' } }))
            )
          ),
          // quick actions
          hD('div', { className: 'quick-card' },
            hD('h3', null, lang === 'es' ? 'Inicio rápido' : 'Quick start'),
            hD('div', { className: 'sub' }, lang === 'es' ? 'Empieza una sesión con plantilla preconfigurada.' : 'Start a session with a preset template.'),
            hD('div', { className: 'col gap-2' },
              hD('button', { className: 'btn', style: { justifyContent: 'flex-start' }, onClick: () => navigate('prerecord', { template: 'daily' }) },
                hD(TemplateIcon, { tmplId: 'daily', size: 24 }),
                hD('div', { style: { textAlign: 'left' } },
                  hD('div', { style: { fontWeight: 600, fontSize: 12 } }, pickL(getTemplate('daily').name, lang)),
                  hD('div', { className: 'text-xs text-subtle' }, '15 min'))
              ),
              hD('button', { className: 'btn', style: { justifyContent: 'flex-start' }, onClick: () => navigate('prerecord', { template: 'ejecutiva' }) },
                hD(TemplateIcon, { tmplId: 'ejecutiva', size: 24 }),
                hD('div', { style: { textAlign: 'left' } },
                  hD('div', { style: { fontWeight: 600, fontSize: 12 } }, pickL(getTemplate('ejecutiva').name, lang)),
                  hD('div', { className: 'text-xs text-subtle' }, '60 min'))
              ),
              hD('button', { className: 'btn', style: { justifyContent: 'flex-start' }, onClick: () => navigate('prerecord', { template: 'tecnica' }) },
                hD(TemplateIcon, { tmplId: 'tecnica', size: 24 }),
                hD('div', { style: { textAlign: 'left' } },
                  hD('div', { style: { fontWeight: 600, fontSize: 12 } }, pickL(getTemplate('tecnica').name, lang)),
                  hD('div', { className: 'text-xs text-subtle' }, '45 min'))
              )
            )
          )
        )
      )
    )
  );
}

function MeetingsList({ navigate, lang, t }) {
  const [q, setQ] = useStateD('');
  const [tmpl, setTmpl] = useStateD('all');
  const filtered = MEETINGS.filter(m => {
    if (tmpl !== 'all' && m.template !== tmpl) return false;
    if (!q) return true;
    return pickL(m.title, lang).toLowerCase().includes(q.toLowerCase());
  });
  return hD('div', { className: 'win-main', 'data-screen-label': '02 Meetings list' },
    hD('div', { className: 'page-header' },
      hD('div', null,
        hD('h1', { className: 'page-title' }, t('navMeetings')),
        hD('div', { className: 'page-sub' }, lang === 'es' ? 'Todas tus reuniones grabadas y transcritas.' : 'All your recorded and transcribed meetings.')
      ),
      hD('div', { className: 'page-actions' },
        hD('div', { className: 'search-box' },
          hD(Icon, { name: 'search', size: 14 }),
          hD('input', { placeholder: t('searchMeetings'), value: q, onChange: e => setQ(e.target.value) })),
        hD('button', { className: 'btn btn-primary', onClick: () => navigate('prerecord') },
          hD(Icon, { name: 'plus', size: 14 }), t('quickRecord'))
      )
    ),
    hD('div', { className: 'page-scroll scroll' },
      hD('div', { className: 'flex gap-2', style: { marginTop: 6, marginBottom: 14, flexWrap: 'wrap' } },
        hD('button', {
          className: 'chip' + (tmpl === 'all' ? ' chip-accent' : ''),
          onClick: () => setTmpl('all'),
          style: { cursor: 'pointer' }
        }, lang === 'es' ? 'Todas' : 'All', ' · ', MEETINGS.length),
        TEMPLATES.map(T => hD('button', {
          key: T.id,
          className: 'chip' + (tmpl === T.id ? ' chip-accent' : ''),
          onClick: () => setTmpl(T.id),
          style: { cursor: 'pointer' }
        }, pickL(T.name, lang)))
      ),
      hD('div', { className: 'meeting-list' },
        filtered.map(m => hD(MeetingRow, { key: m.id, meeting: m, lang, t, onClick: () => navigate('meeting', { id: m.id }) }))
      )
    )
  );
}

window.Dashboard = Dashboard;
window.MeetingsList = MeetingsList;
window.MeetingRow = MeetingRow;
