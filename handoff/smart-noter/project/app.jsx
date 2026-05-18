/* Smart Noter — main app */
const { createElement: hApp, useState: useStateA, useEffect: useEffectA } = React;

const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "theme": "light",
  "accent": "#10b981",
  "lang": "es",
  "aiChat": true,
  "avatarStyle": "circle"
}/*EDITMODE-END*/;

function applyAccent(hex) {
  // compute hover/press from hex
  function shade(h, pct) {
    const c = parseInt(h.slice(1), 16);
    let r = (c >> 16) & 0xff, g = (c >> 8) & 0xff, b = c & 0xff;
    r = Math.max(0, Math.min(255, Math.round(r + (pct < 0 ? r * pct : (255 - r) * pct))));
    g = Math.max(0, Math.min(255, Math.round(g + (pct < 0 ? g * pct : (255 - g) * pct))));
    b = Math.max(0, Math.min(255, Math.round(b + (pct < 0 ? b * pct : (255 - b) * pct))));
    return '#' + [r,g,b].map(x => x.toString(16).padStart(2,'0')).join('');
  }
  const root = document.documentElement;
  root.style.setProperty('--accent', hex);
  root.style.setProperty('--accent-hover', shade(hex, -0.10));
  root.style.setProperty('--accent-press', shade(hex, -0.20));
  // soft + ring as rgba
  const c = parseInt(hex.slice(1), 16);
  const r = (c >> 16) & 0xff, g = (c >> 8) & 0xff, b = c & 0xff;
  root.style.setProperty('--accent-soft', `rgba(${r},${g},${b},0.12)`);
  root.style.setProperty('--accent-ring', `rgba(${r},${g},${b},0.35)`);
}

function App() {
  const tw = window.useTweaks ? window.useTweaks(TWEAK_DEFAULTS) : [TWEAK_DEFAULTS, () => {}];
  const t0 = tw[0];
  const setTweak = tw[1];

  const [view, setView] = useStateA('dashboard');
  const [viewMeta, setViewMeta] = useStateA(null);
  const [showExport, setShowExport] = useStateA(false);

  const lang = t0.lang;
  const t = (k) => tr(lang, k);

  useEffectA(() => {
    document.documentElement.setAttribute('data-theme', t0.theme);
  }, [t0.theme]);

  useEffectA(() => {
    applyAccent(t0.accent || '#10b981');
  }, [t0.accent]);

  function navigate(v, meta = null) {
    setView(v);
    setViewMeta(meta);
  }

  // Determine page title for window chrome
  const titles = {
    dashboard: t('navDashboard'),
    meetings: t('navMeetings'),
    meeting: viewMeta && (() => { const m = MEETINGS.find(x => x.id === viewMeta.id); return m ? pickL(m.title, lang) : ''; })(),
    prerecord: t('navRecord'),
    live: t('liveStatus'),
    templates: t('navTemplates'),
    participants: t('participants'),
    settings: t('navSettings')
  };

  return hApp('div', { className: 'win-shell' },
    hApp('div', { className: 'win-window' },
      hApp(WindowChrome, { lang, t, title: titles[view] }),
      hApp('div', { className: 'win-body' },
        hApp(Sidebar, { view, navigate, lang, t }),
        // render the right screen
        view === 'dashboard' && hApp(Dashboard, { lang, t, navigate }),
        view === 'meetings' && hApp(MeetingsList, { lang, t, navigate }),
        view === 'prerecord' && hApp(PreRecord, { lang, t, navigate, initial: viewMeta }),
        view === 'live' && hApp(LiveRecording, { lang, t, navigate, sessionMeta: viewMeta }),
        view === 'meeting' && hApp(MeetingDetail, {
          lang, t, navigate,
          meetingId: viewMeta ? viewMeta.id : 'm-001',
          aiChatVisible: t0.aiChat !== false,
          openExport: () => setShowExport(true)
        }),
        view === 'templates' && hApp(TemplatesGallery, { lang, t }),
        view === 'participants' && hApp(ParticipantsManager, { lang, t }),
        view === 'settings' && hApp(Settings, { lang, t, tweaks: t0, setTweak })
      )
    ),
    showExport && hApp(ExportModal, {
      lang, t,
      meeting: viewMeta && MEETINGS.find(m => m.id === viewMeta.id) || MEETINGS[0],
      onClose: () => setShowExport(false)
    }),
    // Tweaks panel
    hApp(ApplyAvatarStyle, { style: t0.avatarStyle }),
    tweaks_render(t0, setTweak, lang)
  );
}

function tweaks_render(t0, setTweak, lang) {
  if (!window.TweaksPanel) return null;
  return hApp(window.TweaksPanel, { title: 'Tweaks' },
    hApp(window.TweakSection, { label: lang === 'es' ? 'Apariencia' : 'Appearance' }),
    hApp(window.TweakRadio, {
      label: lang === 'es' ? 'Tema' : 'Theme',
      value: t0.theme,
      options: [{ label: 'Light', value: 'light' }, { label: 'Dark', value: 'dark' }],
      onChange: v => setTweak('theme', v)
    }),
    hApp(window.TweakColor, {
      label: lang === 'es' ? 'Color de acento' : 'Accent',
      value: t0.accent,
      options: ['#10b981', '#3b82f6', '#8b5cf6', '#f97316', '#ec4899'],
      onChange: v => setTweak('accent', v)
    }),
    hApp(window.TweakRadio, {
      label: lang === 'es' ? 'Avatar' : 'Avatar',
      value: t0.avatarStyle,
      options: [{ label: lang === 'es' ? 'Círculo' : 'Circle', value: 'circle' }, { label: lang === 'es' ? 'Cuadrado' : 'Square', value: 'square' }],
      onChange: v => setTweak('avatarStyle', v)
    }),
    hApp(window.TweakSection, { label: lang === 'es' ? 'Idioma & UI' : 'Language & UI' }),
    hApp(window.TweakRadio, {
      label: 'Idioma',
      value: t0.lang,
      options: [{ label: 'ES', value: 'es' }, { label: 'EN', value: 'en' }],
      onChange: v => setTweak('lang', v)
    }),
    hApp(window.TweakToggle, {
      label: lang === 'es' ? 'Chat IA' : 'AI chat',
      value: t0.aiChat,
      onChange: v => setTweak('aiChat', v)
    })
  );
}

// apply avatar style globally
function ApplyAvatarStyle({ style }) {
  useEffectA(() => {
    const css = style === 'square'
      ? `.avatar { border-radius: 8px !important; }`
      : '';
    let s = document.getElementById('__avatar-style');
    if (!s) { s = document.createElement('style'); s.id = '__avatar-style'; document.head.appendChild(s); }
    s.textContent = css;
  }, [style]);
  return null;
}

// Mount
const root = ReactDOM.createRoot(document.getElementById('root'));
root.render(hApp(App));
