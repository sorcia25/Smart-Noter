/* Smart Noter — Fluent-style icon set (inline SVG) */
const { createElement: h, useState, useEffect, useRef, useMemo, useCallback } = React;

const ICONS = {
  // nav
  home: 'M3 11.5 12 4l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z',
  list: 'M4 6h16M4 12h16M4 18h10',
  templates: 'M4 5h7v6H4zM13 5h7v10h-7zM4 13h7v6H4zM13 17h7v2h-7z',
  settings: 'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8zm8.94 4a8.74 8.74 0 0 0-.13-1.46l2.1-1.65-2-3.46-2.5.85a8.91 8.91 0 0 0-2.53-1.46L15.5 2h-4l-.38 2.82a8.91 8.91 0 0 0-2.53 1.46l-2.5-.85-2 3.46 2.1 1.65A8.74 8.74 0 0 0 6.06 12c0 .5.05.99.13 1.46l-2.1 1.65 2 3.46 2.5-.85c.77.6 1.62 1.1 2.53 1.46L11.5 22h4l.38-2.82c.91-.36 1.76-.86 2.53-1.46l2.5.85 2-3.46-2.1-1.65c.08-.47.13-.97.13-1.46z',
  mic: 'M12 14a3 3 0 0 0 3-3V6a3 3 0 1 0-6 0v5a3 3 0 0 0 3 3zM19 11a7 7 0 0 1-14 0M12 19v3',
  record: 'M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16zm0 3a5 5 0 1 1 0 10 5 5 0 0 1 0-10z',
  stop: 'M6 6h12v12H6z',
  pause: 'M7 5h3v14H7zM14 5h3v14h-3z',
  play: 'M8 5v14l11-7z',
  back: 'M19 12H5M12 19l-7-7 7-7',
  forward: 'M5 12h14M12 5l7 7-7 7',
  search: 'M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zm5 12 5 5',
  plus: 'M12 5v14M5 12h14',
  download: 'M12 4v12m0 0-4-4m4 4 4-4M5 20h14',
  share: 'M18 8a3 3 0 1 0-3-3 3 3 0 0 0 .04.5l-6.1 3.55a3 3 0 1 0 0 4.9l6.1 3.55A3.06 3.06 0 0 0 15 18a3 3 0 1 0 3-3 3.06 3.06 0 0 0-2.04.81l-6.1-3.55a3.07 3.07 0 0 0 0-1.52l6.1-3.55A3 3 0 0 0 18 8z',
  edit: 'M4 20h4l11-11-4-4L4 16zM14 6l4 4',
  trash: 'M5 7h14M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2M6 7l1 13a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1l1-13',
  copy: 'M9 8V5a1 1 0 0 1 1-1h9a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-3M5 9h9a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1z',
  check: 'M5 12l5 5L20 7',
  close: 'M6 6l12 12M18 6 6 18',
  chevDown: 'M6 9l6 6 6-6',
  chevRight: 'M9 6l6 6-6 6',
  chevLeft: 'M15 6l-6 6 6 6',
  // template icons
  briefcase: 'M3 9h18v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM8 9V6a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v3',
  cpu: 'M9 3v3M15 3v3M9 18v3M15 18v3M3 9h3M3 15h3M18 9h3M18 15h3M6 6h12v12H6zM10 10h4v4h-4z',
  megaphone: 'M3 11v2a1 1 0 0 0 1 1h2l5 4V6L6 10H4a1 1 0 0 0-1 1zM15 9a3 3 0 0 1 0 6M19 6a7 7 0 0 1 0 12',
  sun: 'M12 5V3M12 21v-2M5 12H3M21 12h-2M6.3 6.3 4.9 4.9M19.1 19.1l-1.4-1.4M6.3 17.7 4.9 19.1M19.1 4.9l-1.4 1.4M12 7a5 5 0 1 0 0 10 5 5 0 0 0 0-10z',
  refresh: 'M21 12a9 9 0 0 1-15.5 6.3L3 21M3 12a9 9 0 0 1 15.5-6.3L21 3M3 21v-5h5M21 3v5h-5',
  user: 'M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4 21a8 8 0 0 1 16 0',
  compass: 'M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16zM10 14l5-5-2 7z',
  // devices
  monitor: 'M4 5h16a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1zM8 21h8M12 17v4',
  headphones: 'M4 14v3a2 2 0 0 0 2 2h1v-7H6a2 2 0 0 0-2 2zM20 14v3a2 2 0 0 1-2 2h-1v-7h1a2 2 0 0 1 2 2zM4 14a8 8 0 0 1 16 0',
  sliders: 'M4 6h7M14 6h6M4 12h3M10 12h10M4 18h11M18 18h2M11 4v4M7 10v4M15 16v4',
  // misc
  sparkles: 'M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1',
  bot: 'M12 2v4M9 6h6a3 3 0 0 1 3 3v8a3 3 0 0 1-3 3H9a3 3 0 0 1-3-3V9a3 3 0 0 1 3-3zM9 12h.01M15 12h.01M9 16h6',
  send: 'M5 12l16-8-6 18-4-8z',
  flag: 'M4 21V4h14l-2 5 2 5H4',
  clock: 'M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16zM12 8v4l3 2',
  globe: 'M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16zM4 12h16M12 4a12 12 0 0 1 0 16M12 4a12 12 0 0 0 0 16',
  filter: 'M4 6h16l-6 8v6l-4-2v-4z',
  pin: 'M12 2v6m0 0a4 4 0 0 0-4 4h8a4 4 0 0 0-4-4zm0 10v8M9 22h6',
  shield: 'M12 3 5 6v6c0 4 3 7.5 7 9 4-1.5 7-5 7-9V6z',
  bell: 'M6 11a6 6 0 0 1 12 0v4l2 3H4l2-3zM9 19a3 3 0 0 0 6 0',
  bookmark: 'M6 3h12v18l-6-4-6 4z',
  zap: 'M13 2 4 14h7l-1 8 9-12h-7z',
  arrow: 'M5 12h14M14 6l6 6-6 6',
  more: 'M5 12h.01M12 12h.01M19 12h.01',
  // help
  help: 'M12 4a8 8 0 1 0 0 16 8 8 0 0 0 0-16zM9 9a3 3 0 0 1 6 0c0 2-3 2-3 4M12 17h.01',
  external: 'M14 4h6v6M20 4l-9 9M9 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3'
};

function Icon({ name, size = 18, stroke = 'currentColor', fill = 'none', className = '', strokeWidth = 1.7, ...rest }) {
  const d = ICONS[name];
  if (!d) return null;
  // filled-style icons (record)
  const filled = ['record', 'play', 'stop', 'pause', 'send', 'briefcase'].includes(name);
  return h('svg', {
    width: size, height: size,
    viewBox: '0 0 24 24',
    fill: filled ? stroke : fill,
    stroke: filled ? 'none' : stroke,
    strokeWidth, strokeLinecap: 'round', strokeLinejoin: 'round',
    className, ...rest
  }, h('path', { d }));
}

window.Icon = Icon;
window.ICONS = ICONS;
