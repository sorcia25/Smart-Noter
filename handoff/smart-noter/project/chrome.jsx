/* Smart Noter — window chrome + sidebar */
const { createElement: h2 } = React;

function WindowChrome({ lang, t, onMin, onMax, onClose, title, deviceName }) {
  return h2('div', { className: 'win-titlebar', 'data-screen-label': 'Window chrome' },
    h2('div', { className: 'win-title' },
      h2('div', { className: 'brand-mark', style: { width: 16, height: 16, borderRadius: 4 } },
        h2(Icon, { name: 'mic', size: 10, stroke: 'white' })),
      h2('span', null, `${t('appName')} — ${title || ''}`)
    ),
    h2('div', { className: 'win-controls' },
      h2('button', { className: 'win-ctrl', onClick: onMin, title: 'Minimize' },
        h2('svg', { viewBox: '0 0 10 10' }, h2('rect', { x: 1, y: 5, width: 8, height: 1, fill: 'currentColor' }))
      ),
      h2('button', { className: 'win-ctrl', onClick: onMax, title: 'Maximize' },
        h2('svg', { viewBox: '0 0 10 10', fill: 'none' }, h2('rect', { x: 1, y: 1, width: 8, height: 8, stroke: 'currentColor' }))
      ),
      h2('button', { className: 'win-ctrl close', onClick: onClose, title: 'Close' },
        h2('svg', { viewBox: '0 0 10 10', stroke: 'currentColor', strokeWidth: 1 },
          h2('line', { x1: 1, y1: 1, x2: 9, y2: 9 }),
          h2('line', { x1: 9, y1: 1, x2: 1, y2: 9 })
        )
      )
    )
  );
}

function Sidebar({ view, navigate, lang, t }) {
  const items = [
    { id: 'dashboard', icon: 'home', label: t('navDashboard') },
    { id: 'meetings', icon: 'list', label: t('navMeetings'), count: MEETINGS.length },
    { id: 'templates', icon: 'templates', label: t('navTemplates') },
  ];
  const tools = [
    { id: 'participants', icon: 'user', label: t('participants') },
    { id: 'settings', icon: 'settings', label: t('navSettings') }
  ];
  return h2('aside', { className: 'win-sidebar' },
    h2('div', { className: 'brand' },
      h2('div', { className: 'brand-mark' }, h2(Icon, { name: 'mic', size: 16, stroke: 'white' })),
      h2('div', null,
        h2('div', { className: 'brand-name' }, t('appName')),
        h2('div', { className: 'brand-sub' }, t('appTag'))
      )
    ),
    h2('button', { className: 'cta-record', onClick: () => navigate('prerecord') },
      h2('div', { className: 'dot' }), t('navRecord')
    ),
    h2('div', { className: 'nav-section' },
      h2('div', { className: 'nav-section-label' }, t('navWorkspace')),
      items.map(it => h2('button', {
        key: it.id,
        className: 'nav-item' + (view === it.id || (view === 'meeting' && it.id === 'meetings') ? ' active' : ''),
        onClick: () => navigate(it.id)
      }, h2(Icon, { name: it.icon, size: 18 }), h2('span', null, it.label), it.count ? h2('span', { className: 'count' }, it.count) : null))
    ),
    h2('div', { className: 'nav-section' },
      h2('div', { className: 'nav-section-label' }, t('navTools')),
      tools.map(it => h2('button', {
        key: it.id,
        className: 'nav-item' + (view === it.id ? ' active' : ''),
        onClick: () => navigate(it.id)
      }, h2(Icon, { name: it.icon, size: 18 }), h2('span', null, it.label)))
    ),
    h2('div', { className: 'sidebar-footer' },
      h2('div', { className: 'avatar', style: { width: 30, height: 30, fontSize: 11 } }, 'CR'),
      h2('div', { style: { lineHeight: 1.2 } },
        h2('div', { className: 'name' }, 'Carlos Rivera'),
        h2('div', { className: 'role' }, 'Pro · v3.1.4')
      )
    )
  );
}

window.WindowChrome = WindowChrome;
window.Sidebar = Sidebar;
