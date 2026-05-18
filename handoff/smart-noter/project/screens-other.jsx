/* Smart Noter — Templates / Participants / Settings / Export modal */
const { createElement: hS, useState: useStateS } = React;

function TemplatesGallery({ lang, t }) {
  const [defaultTmpl, setDefault] = useStateS('tecnica');
  return hS('div', { className: 'win-main', 'data-screen-label': '06 Templates' },
    hS('div', { className: 'page-header' },
      hS('div', null,
        hS('h1', { className: 'page-title' }, t('tmplTitle')),
        hS('div', { className: 'page-sub' }, t('tmplSub'))),
      hS('div', { className: 'page-actions' },
        hS('button', { className: 'btn' }, hS(Icon, { name: 'download', size: 14 }), lang === 'es' ? 'Importar' : 'Import'),
        hS('button', { className: 'btn btn-primary' }, hS(Icon, { name: 'plus', size: 14 }), lang === 'es' ? 'Crear plantilla' : 'Create template'))
    ),
    hS('div', { className: 'page-scroll scroll' },
      hS('div', { className: 'tmpl-gallery' },
        TEMPLATES.map(T => {
          const isDef = defaultTmpl === T.id;
          // Features per template
          const features = featuresFor(T.id, lang);
          return hS('div', { key: T.id, className: 'tmpl-gallery-card' },
            hS('div', { className: 'header' },
              hS('div', { className: 'icon-lg ' + T.colorClass },
                hS(Icon, { name: T.icon, size: 22, stroke: 'white' })),
              hS('div', null,
                hS('h4', null, pickL(T.name, lang)),
                hS('div', { className: 'h-sub' }, `${T.sections.length} ${lang === 'es' ? 'secciones' : 'sections'}`)),
              isDef ? hS('span', { className: 'chip chip-accent', style: { marginLeft: 'auto' } }, lang === 'es' ? 'Predeterminada' : 'Default') : null
            ),
            hS('div', { className: 'text-sm text-muted', style: { lineHeight: 1.5 } }, pickL(T.desc, lang)),
            hS('div', { className: 'features' },
              features.map((f, i) => hS('div', { key: i, className: 'feature' },
                hS(Icon, { name: 'check', size: 11 }),
                hS('span', null, f)))),
            hS('div', { className: 'flex gap-2', style: { marginTop: 14 } },
              hS('button', {
                className: 'btn',
                onClick: () => setDefault(T.id),
                style: { flex: 1, fontSize: 12 }
              }, isDef ? (lang === 'es' ? 'Activa' : 'Active') : t('tmplUseDefault')),
              hS('button', { className: 'btn btn-icon btn-ghost' }, hS(Icon, { name: 'edit', size: 14 })),
              hS('button', { className: 'btn btn-icon btn-ghost' }, hS(Icon, { name: 'copy', size: 14 })))
          );
        })
      )
    )
  );
}

function featuresFor(id, lang) {
  const F = {
    ejecutiva: { es: ['Resumen ejecutivo en máx. 5 viñetas', 'Decisiones con responsable y fecha', 'Tabla de KPIs y métricas', 'Riesgos y acciones críticas'], en: ['Exec summary, 5 bullets max', 'Decisions with owner & date', 'KPI table', 'Risks & critical actions'] },
    discovery: { es: ['Pain points priorizados', 'Requerimientos funcionales', 'Mapa de stakeholders', 'Supuestos a validar'], en: ['Prioritized pain points', 'Functional requirements', 'Stakeholder map', 'Assumptions to validate'] },
    tecnica: { es: ['Diagrama de arquitectura ASCII', 'ADRs (decisiones técnicas)', 'Bloqueos con severidad', 'Entregables con DoD'], en: ['ASCII architecture diagram', 'ADRs (tech decisions)', 'Blockers with severity', 'Deliverables with DoD'] },
    webinar: { es: ['Agenda y duración por bloque', 'Mensajes clave del speaker', 'Q&A más relevantes', 'Métricas de asistencia'], en: ['Agenda with block timing', 'Speaker key messages', 'Most relevant Q&A', 'Attendance metrics'] },
    daily: { es: ['Ayer/Hoy/Bloqueos por persona', 'Foco diario destacado', 'Acciones <24h', 'Tiempo total y participación'], en: ['Yesterday/Today/Blockers per person', 'Daily focus highlight', '<24h actions', 'Total time & participation'] },
    retro: { es: ['Funcionó / no funcionó / aprendizajes', 'Experimentos a probar', 'Compromisos del equipo', 'Voto cuantitativo de items'], en: ['Worked / didn\'t / learnings', 'Experiments to try', 'Team commitments', 'Quantitative item voting'] },
    entrevista: { es: ['Background estructurado', 'Evaluación por competencia 1-5', 'Señales fuertes/débiles', 'Recomendación final'], en: ['Structured background', '1-5 competency scoring', 'Strong/weak signals', 'Final recommendation'] },
    coaching: { es: ['Estado emocional y profesional', 'Feedback bidireccional', 'Objetivos SMART', 'Plan de acompañamiento'], en: ['Emotional & professional status', 'Two-way feedback', 'SMART goals', 'Follow-up plan'] },
    conferencia: { es: ['Ponentes y temas', 'Citas destacadas con timestamp', 'Contactos clave a seguir', 'Recursos mencionados'], en: ['Speakers and topics', 'Notable quotes with timestamp', 'Key contacts to follow', 'Resources mentioned'] }
  };
  return (F[id] && F[id][lang]) || [];
}

function ParticipantsManager({ lang, t }) {
  // pull all unique participants across meetings
  const all = useStateS(() => {
    const map = new Map();
    MEETINGS.forEach(m => m.participants.forEach(p => {
      const k = p.name || `${m.id}-${p.id}`;
      if (!map.has(k)) map.set(k, { ...p, meetings: [m.id] });
      else map.get(k).meetings.push(m.id);
    }));
    return Array.from(map.values());
  })[0];

  return hS('div', { className: 'win-main', 'data-screen-label': '07 Participants' },
    hS('div', { className: 'page-header' },
      hS('div', null,
        hS('h1', { className: 'page-title' }, t('partTitle')),
        hS('div', { className: 'page-sub' }, t('partSub'))),
      hS('div', { className: 'page-actions' },
        hS('div', { className: 'search-box' },
          hS(Icon, { name: 'search', size: 14 }),
          hS('input', { placeholder: lang === 'es' ? 'Buscar participante…' : 'Search participant…' })),
        hS('button', { className: 'btn btn-primary' }, hS(Icon, { name: 'plus', size: 14 }), lang === 'es' ? 'Añadir' : 'Add'))
    ),
    hS('div', { className: 'page-scroll scroll' },
      hS('div', { className: 'card', style: { overflow: 'hidden' } },
        hS('div', { style: { padding: '10px 14px', background: 'var(--bg-surface-2)', borderBottom: '1px solid var(--stroke)', fontSize: 11, fontWeight: 600, color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.05em', display: 'grid', gridTemplateColumns: '40px 1.5fr 1fr 1fr auto', gap: 12, alignItems: 'center' } },
          hS('span', null), hS('span', null, lang === 'es' ? 'Nombre' : 'Name'),
          hS('span', null, lang === 'es' ? 'Etiqueta original' : 'Original label'),
          hS('span', null, lang === 'es' ? 'Reuniones' : 'Meetings'),
          hS('span', null)),
        all.map((p, i) => hS('div', { key: i, style: { padding: '12px 14px', borderBottom: i < all.length - 1 ? '1px solid var(--stroke)' : 'none', display: 'grid', gridTemplateColumns: '40px 1.5fr 1fr 1fr auto', gap: 12, alignItems: 'center' } },
          hS(SubjectAvatar, { p, size: 36 }),
          hS('div', null,
            hS('div', { style: { fontSize: 13, fontWeight: 500 } }, p.name || hS('span', { className: 'text-subtle' }, t('unnamed'))),
            hS('div', { className: 'text-xs text-subtle', style: { marginTop: 2 } },
              lang === 'es' ? `Sujeto ${p.label.slice(1)}` : `Subject ${p.label.slice(1)}`)),
          hS('span', { className: 'font-mono text-xs text-muted' }, p.label),
          hS('span', { className: 'text-sm text-muted' }, p.meetings.length, ' ', lang === 'es' ? 'reuniones' : 'meetings'),
          hS('div', { className: 'flex gap-1' },
            hS('button', { className: 'btn btn-icon btn-ghost' }, hS(Icon, { name: 'edit', size: 14 })),
            hS('button', { className: 'btn btn-icon btn-ghost' }, hS(Icon, { name: 'more', size: 16 })))
        ))
      )
    )
  );
}

function Settings({ lang, t, tweaks, setTweak }) {
  const [captureMode, setCaptureMode] = useStateS('system');
  const [defaultDevice, setDefaultDevice] = useStateS('system-loopback');
  const [runLocal, setRunLocal] = useStateS(true);
  const [autoDelete, setAutoDelete] = useStateS(false);
  return hS('div', { className: 'win-main', 'data-screen-label': '08 Settings' },
    hS('div', { className: 'page-header' },
      hS('div', null,
        hS('h1', { className: 'page-title' }, t('settingsTitle')),
        hS('div', { className: 'page-sub' }, t('settingsSub')))
    ),
    hS('div', { className: 'page-scroll scroll' },
      // Audio capture
      hS('div', { className: 'setting-group' },
        hS('div', { className: 'setting-group-header' }, t('audioCapture')),
        hS('div', { className: 'setting-row' },
          hS('div', { style: { maxWidth: 380 } },
            hS('div', { className: 'label' }, t('captureMode')),
            hS('div', { className: 'desc' }, t('captureModeDesc'))),
          hS('div', { className: 'segmented' },
            [
              { value: 'system', label: lang === 'es' ? 'Sistema' : 'System' },
              { value: 'mic', label: lang === 'es' ? 'Mic' : 'Mic' },
              { value: 'mix', label: lang === 'es' ? 'Mezcla' : 'Mix' }
            ].map(o => hS('button', { key: o.value, className: captureMode === o.value ? 'active' : '', onClick: () => setCaptureMode(o.value) }, o.label)))
        ),
        hS('div', { className: 'setting-row' },
          hS('div', { style: { maxWidth: 380 } },
            hS('div', { className: 'label' }, t('defaultDevice')),
            hS('div', { className: 'desc' }, lang === 'es' ? 'Se usará automáticamente al iniciar una nueva grabación.' : 'Used automatically when starting a new recording.')),
          hS('div', { className: 'select-trigger' },
            hS('span', null, pickL((AUDIO_DEVICES.find(d => d.id === defaultDevice) || AUDIO_DEVICES[0]).name, lang)),
            hS(Icon, { name: 'chevDown', size: 14 }))
        ),
        hS('div', { className: 'setting-row' },
          hS('div', { style: { maxWidth: 380 } },
            hS('div', { className: 'label' }, lang === 'es' ? 'Calidad de grabación' : 'Recording quality'),
            hS('div', { className: 'desc' }, lang === 'es' ? 'Mayor calidad ocupa más espacio en disco.' : 'Higher quality uses more disk space.')),
          hS('div', { className: 'segmented' },
            ['MP3 192k', 'MP3 320k', 'WAV 48k', 'FLAC'].map((o, i) => hS('button', { key: o, className: i === 2 ? 'active' : '' }, o))))
      ),
      // Transcription engine — providers
      hS(TranscriptionEngineSection, { lang, t }),
      // Privacy
      hS('div', { className: 'setting-group' },
        hS('div', { className: 'setting-group-header' }, t('privacy') + ' & ' + t('storage')),
        hS('div', { className: 'setting-row' },
          hS('div', { style: { maxWidth: 380 } },
            hS('div', { className: 'label' }, t('autoDeleteAudio')),
            hS('div', { className: 'desc' }, t('autoDeleteAudioDesc'))),
          hS(Toggle, { on: autoDelete, onChange: setAutoDelete })),
        hS('div', { className: 'setting-row' },
          hS('div', { style: { maxWidth: 380 } },
            hS('div', { className: 'label' }, lang === 'es' ? 'Ubicación de archivos' : 'File location'),
            hS('div', { className: 'desc font-mono', style: { fontFamily: 'var(--font-mono)' } }, 'C:\\Users\\carlos\\Documents\\SmartNoter')),
          hS('button', { className: 'btn' }, lang === 'es' ? 'Cambiar' : 'Change'))
      ),
      hS('div', { className: 'text-sm text-muted', style: { textAlign: 'center', marginTop: 18, paddingBottom: 20 } },
        'Smart Noter v3.1.4 · ',
        hS('a', { href: '#', style: { color: 'var(--accent)' } }, lang === 'es' ? 'Buscar actualizaciones' : 'Check for updates'))
    )
  );
}

function ExportModal({ lang, t, meeting, onClose }) {
  const [formats, setFormats] = useStateS({ mp3: true, md: true, pdf: false });
  const [timestamps, setTimestamps] = useStateS(true);
  const [bilingual, setBilingual] = useStateS(false);
  const [filename, setFilename] = useStateS('reunion-' + new Date().toISOString().slice(0,10));

  function toggle(k) { setFormats(f => ({ ...f, [k]: !f[k] })); }

  return hS('div', { className: 'modal-backdrop', onClick: onClose },
    hS('div', { className: 'modal', onClick: e => e.stopPropagation() },
      hS('div', { className: 'modal-head' },
        hS('h2', { className: 'modal-title' }, t('exportTitle')),
        hS('p', { className: 'modal-sub' }, t('exportSub'))),
      hS('div', { className: 'modal-body' },
        // file name
        hS('div', { style: { marginBottom: 14 } },
          hS('label', { className: 'field-label' }, lang === 'es' ? 'Nombre base del archivo' : 'Base filename'),
          hS('input', {
            className: 'input', value: filename,
            onChange: e => setFilename(e.target.value),
            placeholder: t('fileNamePh')
          })),
        // formats
        hS('div', null,
          hS('div', { className: 'field-label' }, lang === 'es' ? 'Formatos' : 'Formats'),
          [
            { id: 'mp3', icon: 'mp3', name: t('exportAudio'), desc: t('exportAudioDesc'), badge: 'MP3' },
            { id: 'md',  icon: 'md', name: t('exportMd'),    desc: t('exportMdDesc'),    badge: 'MD' },
            { id: 'pdf', icon: 'pdf', name: t('exportPdf'),  desc: t('exportPdfDesc'),   badge: 'PDF' }
          ].map(f => hS('div', {
            key: f.id, className: 'export-row' + (formats[f.id] ? ' selected' : ''),
            onClick: () => toggle(f.id)
          },
            hS('div', { className: 'export-icon fmt-' + f.id }, f.badge),
            hS('div', null,
              hS('div', { style: { fontSize: 13, fontWeight: 600 } }, f.name),
              hS('div', { className: 'text-xs text-muted', style: { marginTop: 2 } }, f.desc)),
            hS('div', null,
              hS('div', {
                style: {
                  width: 18, height: 18, borderRadius: 4,
                  border: '2px solid ' + (formats[f.id] ? 'var(--accent)' : 'var(--stroke-strong)'),
                  background: formats[f.id] ? 'var(--accent)' : 'transparent',
                  display: 'grid', placeItems: 'center'
                }
              }, formats[f.id] ? hS(Icon, { name: 'check', size: 12, stroke: 'white' }) : null)
            )
          ))
        ),
        // options
        hS('div', { className: 'flex items-center justify-between', style: { paddingTop: 12, marginTop: 4, borderTop: '1px solid var(--stroke)' } },
          hS('div', null,
            hS('div', { style: { fontSize: 12, fontWeight: 500 } }, t('timestampsOn')),
            hS('div', { className: 'text-xs text-subtle', style: { marginTop: 2 } }, lang === 'es' ? 'Incluir marcas de tiempo en transcripción' : 'Include timestamps in transcript')),
          hS(Toggle, { on: timestamps, onChange: setTimestamps })),
        hS('div', { className: 'flex items-center justify-between', style: { paddingTop: 12, marginTop: 12, borderTop: '1px solid var(--stroke)' } },
          hS('div', null,
            hS('div', { style: { fontSize: 12, fontWeight: 500 } }, t('bilingual')),
            hS('div', { className: 'text-xs text-subtle', style: { marginTop: 2 } }, lang === 'es' ? 'Generar también versión en inglés' : 'Also generate English version')),
          hS(Toggle, { on: bilingual, onChange: setBilingual }))
      ),
      hS('div', { className: 'modal-foot' },
        hS('button', { className: 'btn', onClick: onClose }, t('cancel')),
        hS('button', { className: 'btn btn-primary', onClick: onClose },
          hS(Icon, { name: 'download', size: 14 }),
          t('exportNow'), ' (', Object.values(formats).filter(Boolean).length, ')'))
    )
  );
}

// ============================================================
// Transcription Engine section — local / OpenAI / Azure / custom
// ============================================================

const PROVIDERS = [
  {
    id: 'local',
    icon: 'cpu',
    color: '#10b981',
    name: { es: 'Local (en este equipo)', en: 'Local (on this device)' },
    short: 'Local',
    desc: {
      es: 'Procesa el audio en tu PC con Whisper. Máxima privacidad, sin costos por minuto.',
      en: 'Processes audio on your PC with Whisper. Max privacy, no per-minute cost.'
    },
    badge: { es: 'Predeterminado · privado', en: 'Default · private' },
    badgeAccent: true,
    models: ['Whisper Large v3', 'Whisper Large v3 Turbo', 'Whisper Medium', 'Distil-Whisper'],
    needs: 'none',
    metrics: [
      { label: { es: 'Fidelidad', en: 'Fidelity' }, value: '99.2%' },
      { label: { es: 'Latencia', en: 'Latency' }, value: '1.8s' },
      { label: { es: 'Costo', en: 'Cost' }, value: { es: 'Gratis', en: 'Free' } },
      { label: { es: 'Privacidad', en: 'Privacy' }, value: { es: 'Máxima', en: 'Maximum' } }
    ]
  },
  {
    id: 'openai',
    icon: 'sparkles',
    color: '#1aaf8b',
    name: { es: 'OpenAI API', en: 'OpenAI API' },
    short: 'OpenAI',
    desc: {
      es: 'Usa los modelos de OpenAI (gpt-4o-transcribe, whisper-1) vía API.',
      en: 'Use OpenAI models (gpt-4o-transcribe, whisper-1) via API.'
    },
    badge: { es: 'Requiere API key', en: 'API key required' },
    models: ['gpt-4o-transcribe', 'gpt-4o-mini-transcribe', 'whisper-1'],
    needs: 'openai',
    metrics: [
      { label: { es: 'Fidelidad', en: 'Fidelity' }, value: '99.5%' },
      { label: { es: 'Latencia', en: 'Latency' }, value: '0.6s' },
      { label: { es: 'Costo', en: 'Cost' }, value: '~$0.006/min' },
      { label: { es: 'Privacidad', en: 'Privacy' }, value: { es: 'Estándar', en: 'Standard' } }
    ]
  },
  {
    id: 'azure',
    icon: 'cpu',
    color: '#0078d4',
    name: { es: 'Azure OpenAI / Speech', en: 'Azure OpenAI / Speech' },
    short: 'Azure',
    desc: {
      es: 'Modelos desplegados en tu tenant de Azure. Cumplimiento empresarial y región configurable.',
      en: 'Models deployed in your Azure tenant. Enterprise compliance and configurable region.'
    },
    badge: { es: 'Enterprise · residencia de datos', en: 'Enterprise · data residency' },
    models: ['whisper (Azure)', 'gpt-4o-transcribe', 'Azure Speech-to-Text'],
    needs: 'azure',
    metrics: [
      { label: { es: 'Fidelidad', en: 'Fidelity' }, value: '99.4%' },
      { label: { es: 'Latencia', en: 'Latency' }, value: '0.8s' },
      { label: { es: 'Costo', en: 'Cost' }, value: { es: 'Consumo Azure', en: 'Azure consumption' } },
      { label: { es: 'Privacidad', en: 'Privacy' }, value: { es: 'Tu tenant', en: 'Your tenant' } }
    ]
  },
  {
    id: 'custom',
    icon: 'sliders',
    color: '#8b5cf6',
    name: { es: 'Endpoint personalizado', en: 'Custom endpoint' },
    short: 'Custom',
    desc: {
      es: 'Apunta a cualquier servicio compatible con la API de OpenAI (Groq, Together, on-prem).',
      en: 'Point to any OpenAI-compatible service (Groq, Together, on-prem).'
    },
    badge: { es: 'Avanzado', en: 'Advanced' },
    models: ['Detectar automáticamente'],
    needs: 'custom'
  }
];

function TranscriptionEngineSection({ lang, t }) {
  const [provider, setProvider] = useStateS('local');
  const [model, setModel] = useStateS('Whisper Large v3');
  const [keyOpenAI, setKeyOpenAI] = useStateS('sk-proj-••••••••••••MTk2');
  const [showKey, setShowKey] = useStateS(false);
  const [keyStatus, setKeyStatus] = useStateS('saved'); // saved | invalid | testing
  const [azureEndpoint, setAzureEndpoint] = useStateS('https://acme-noter.openai.azure.com');
  const [azureDeployment, setAzureDeployment] = useStateS('whisper-prod');
  const [azureApiVersion, setAzureApiVersion] = useStateS('2024-10-01-preview');
  const [azureRegion, setAzureRegion] = useStateS('eastus');
  const [azureKey, setAzureKey] = useStateS('');
  const [customUrl, setCustomUrl] = useStateS('https://api.groq.com/openai/v1');
  const [customKey, setCustomKey] = useStateS('');
  const [fallback, setFallback] = useStateS('local');
  const [redactPII, setRedactPII] = useStateS(true);
  const [keepLocalCopy, setKeepLocalCopy] = useStateS(true);

  const cur = PROVIDERS.find(p => p.id === provider) || PROVIDERS[0];

  // Reset model when switching providers
  React.useEffect(() => {
    if (cur.models && cur.models.length) setModel(cur.models[0]);
  }, [provider]);

  return hS('div', { className: 'setting-group' },
    hS('div', { className: 'setting-group-header', style: { display: 'flex', alignItems: 'center', justifyContent: 'space-between' } },
      hS('span', null, t('transcriptionEngineLabel')),
      hS('span', { className: 'chip', style: { textTransform: 'none', letterSpacing: 0, fontWeight: 500 } },
        hS(Icon, { name: PROVIDERS.find(p => p.id === provider).icon, size: 11 }),
        cur.short)
    ),
    // Provider selector — radio cards
    hS('div', { style: { padding: 18, borderBottom: '1px solid var(--stroke)' } },
      hS('div', { className: 'label', style: { fontSize: 13, fontWeight: 500, marginBottom: 4 } },
        lang === 'es' ? 'Proveedor del motor' : 'Engine provider'),
      hS('div', { className: 'desc', style: { fontSize: 12, color: 'var(--text-muted)', marginBottom: 14 } },
        lang === 'es'
          ? 'Elige dónde se procesa la transcripción. Puedes cambiarlo en cualquier momento.'
          : 'Choose where transcription runs. You can switch any time.'),
      hS('div', { className: 'engine-grid' },
        PROVIDERS.map(p => hS('button', {
          key: p.id,
          className: 'engine-card' + (provider === p.id ? ' selected' : ''),
          onClick: () => setProvider(p.id)
        },
          hS('div', { className: 'engine-card-head' },
            hS('div', { className: 'engine-icon', style: { background: p.color + '22', color: p.color } },
              hS(Icon, { name: p.icon, size: 16, stroke: p.color })),
            hS('div', { className: 'engine-radio' },
              provider === p.id ? hS('div', { className: 'engine-radio-dot' }) : null)
          ),
          hS('div', { className: 'engine-name' }, pickL(p.name, lang)),
          hS('div', { className: 'engine-desc' }, pickL(p.desc, lang)),
          hS('div', { className: 'engine-badge' + (p.badgeAccent ? ' accent' : '') }, pickL(p.badge, lang))
        ))
      )
    ),

    // Provider-specific config
    provider === 'local' && hS(LocalProviderConfig, { lang, model, setModel, cur }),
    provider === 'openai' && hS(OpenAIProviderConfig, {
      lang, model, setModel, cur,
      keyValue: keyOpenAI, setKey: setKeyOpenAI,
      showKey, setShowKey,
      keyStatus, setKeyStatus
    }),
    provider === 'azure' && hS(AzureProviderConfig, {
      lang, model, setModel, cur,
      endpoint: azureEndpoint, setEndpoint: setAzureEndpoint,
      deployment: azureDeployment, setDeployment: setAzureDeployment,
      apiVersion: azureApiVersion, setApiVersion: setAzureApiVersion,
      region: azureRegion, setRegion: setAzureRegion,
      apiKey: azureKey, setApiKey: setAzureKey
    }),
    provider === 'custom' && hS(CustomProviderConfig, {
      lang, url: customUrl, setUrl: setCustomUrl, apiKey: customKey, setApiKey: setCustomKey,
      model, setModel
    }),

    // Provider metrics — always shown
    cur.metrics && hS('div', { style: { padding: '0 18px 16px' } },
      hS('div', { style: { display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 10 } },
        cur.metrics.map((m, i) => hS('div', {
          key: i,
          style: {
            padding: '10px 12px',
            background: 'var(--bg-inset)',
            borderRadius: 8,
            border: '1px solid var(--stroke)'
          }
        },
          hS('div', { className: 'text-xs text-subtle', style: { textTransform: 'uppercase', letterSpacing: '0.04em', fontWeight: 600, fontSize: 10 } }, pickL(m.label, lang)),
          hS('div', { style: { fontSize: 14, fontWeight: 600, marginTop: 4 } }, typeof m.value === 'object' ? pickL(m.value, lang) : m.value)))
      )
    ),

    // Fallback option (only when not local)
    provider !== 'local' && hS('div', { className: 'setting-row' },
      hS('div', { style: { maxWidth: 420 } },
        hS('div', { className: 'label' }, lang === 'es' ? 'Motor de respaldo' : 'Fallback engine'),
        hS('div', { className: 'desc' }, lang === 'es' ? 'Si la API falla o tu conexión cae, usar este motor como respaldo.' : 'If the API fails or your connection drops, use this as backup.')),
      hS(Segmented, {
        value: fallback,
        options: [
          { value: 'local', label: lang === 'es' ? 'Local' : 'Local' },
          { value: 'queue', label: lang === 'es' ? 'Encolar' : 'Queue' },
          { value: 'none',  label: lang === 'es' ? 'Ninguno' : 'None' }
        ],
        onChange: setFallback
      })
    ),

    // Privacy
    hS('div', { className: 'setting-row' },
      hS('div', { style: { maxWidth: 420 } },
        hS('div', { className: 'label' }, lang === 'es' ? 'Redactar PII antes de enviar' : 'Redact PII before sending'),
        hS('div', { className: 'desc' }, lang === 'es' ? 'Detecta nombres, teléfonos, correos y montos. Sólo aplica para proveedores en la nube.' : 'Detect names, phones, emails, amounts. Only applies to cloud providers.')),
      hS(Toggle, { on: redactPII, onChange: setRedactPII })
    ),
    hS('div', { className: 'setting-row' },
      hS('div', { style: { maxWidth: 420 } },
        hS('div', { className: 'label' }, lang === 'es' ? 'Conservar copia local de la transcripción' : 'Keep local copy of transcript'),
        hS('div', { className: 'desc' }, lang === 'es' ? 'La transcripción siempre se guarda en tu equipo, además de cualquier proveedor.' : 'Transcripts are always stored on this device, in addition to any provider.')),
      hS(Toggle, { on: keepLocalCopy, onChange: setKeepLocalCopy })
    )
  );
}

// Local model state — simulates downloaded models on disk
const LOCAL_MODELS = [
  { id: 'Whisper Large v3', size: 2.9, sizeUnit: 'GB', installed: true, default: true,
    desc: { es: 'Multilingüe · máxima precisión', en: 'Multilingual · top accuracy' } },
  { id: 'Whisper Large v3 Turbo', size: 1.6, sizeUnit: 'GB', installed: true,
    desc: { es: '4× más rápido · misma precisión', en: '4× faster · same accuracy' } },
  { id: 'Whisper Medium', size: 1.4, sizeUnit: 'GB', installed: false,
    desc: { es: 'Balanceado · multilingüe', en: 'Balanced · multilingual' } },
  { id: 'Distil-Whisper', size: 380, sizeUnit: 'MB', installed: false,
    desc: { es: 'Compacto · sólo inglés', en: 'Lightweight · English only' } }
];

function LocalProviderConfig({ lang, model, setModel, cur }) {
  const [models, setModels] = useStateS(LOCAL_MODELS);
  const [downloading, setDownloading] = useStateS(null); // model id currently downloading
  const [progress, setProgress] = useStateS(0);
  const [installPath, setInstallPath] = useStateS('C:\\Users\\carlos\\AppData\\Local\\SmartNoter\\models');

  // Simulate download progress
  React.useEffect(() => {
    if (!downloading) return;
    const id = setInterval(() => {
      setProgress(p => {
        if (p >= 100) {
          clearInterval(id);
          setModels(ms => ms.map(m => m.id === downloading ? { ...m, installed: true } : m));
          setDownloading(null);
          return 0;
        }
        return Math.min(100, p + 2 + Math.random() * 3);
      });
    }, 80);
    return () => clearInterval(id);
  }, [downloading]);

  function startDownload(id) {
    setDownloading(id);
    setProgress(0);
  }
  function removeModel(id) {
    setModels(ms => ms.map(m => m.id === id ? { ...m, installed: false } : m));
    if (model === id) {
      const fallback = models.find(m => m.installed && m.id !== id);
      if (fallback) setModel(fallback.id);
    }
  }

  const installedCount = models.filter(m => m.installed).length;
  const installedSize = models.filter(m => m.installed).reduce((s, m) => s + (m.sizeUnit === 'GB' ? m.size : m.size / 1024), 0);
  // Mocked detected hardware
  const hardware = {
    gpu: 'NVIDIA GeForce RTX 3060 · 6 GB VRAM',
    cuda: 'CUDA 12.4 · cuDNN 9.1',
    cpu: 'Intel Core i7-12700H · 14 cores',
    ram: '32 GB DDR5'
  };

  return hS('div', { style: { padding: 18, display: 'flex', flexDirection: 'column', gap: 16 } },
    // Engine status banner
    hS('div', { className: 'local-status-banner' },
      hS('div', { className: 'local-status-icon' },
        hS(Icon, { name: 'cpu', size: 22, stroke: '#10b981' })),
      hS('div', { style: { flex: 1, minWidth: 0 } },
        hS('div', { className: 'local-status-title' },
          hS('span', null, lang === 'es' ? 'Motor local instalado y listo' : 'Local engine installed and ready'),
          hS('span', { className: 'chip chip-accent' },
            hS('span', { className: 'live-dot', style: { width: 6, height: 6 } }),
            lang === 'es' ? 'Activo' : 'Active')
        ),
        hS('div', { className: 'local-status-grid' },
          hS('div', { className: 'local-status-cell' },
            hS('span', { className: 'lsc-label' }, lang === 'es' ? 'Modelos en disco' : 'Models on disk'),
            hS('span', { className: 'lsc-value' }, installedCount, ' · ', installedSize.toFixed(1), ' GB')),
          hS('div', { className: 'local-status-cell' },
            hS('span', { className: 'lsc-label' }, 'GPU'),
            hS('span', { className: 'lsc-value' }, hardware.gpu.split(' · ')[0])),
          hS('div', { className: 'local-status-cell' },
            hS('span', { className: 'lsc-label' }, 'CPU'),
            hS('span', { className: 'lsc-value' }, hardware.cpu.split(' · ')[0])),
          hS('div', { className: 'local-status-cell' },
            hS('span', { className: 'lsc-label' }, lang === 'es' ? 'Aceleración' : 'Acceleration'),
            hS('span', { className: 'lsc-value', style: { color: 'var(--accent)' } },
              hS(Icon, { name: 'zap', size: 11, stroke: 'var(--accent)' }), ' CUDA 12.4'))
        )
      )
    ),

    // Models on disk
    hS('div', null,
      hS('div', { style: { display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 } },
        hS('label', { className: 'field-label', style: { margin: 0 } }, lang === 'es' ? 'Modelos disponibles' : 'Available models'),
        hS('button', { className: 'btn btn-ghost', style: { fontSize: 11, padding: '4px 8px' } },
          hS(Icon, { name: 'refresh', size: 12 }),
          lang === 'es' ? 'Buscar actualizaciones' : 'Check for updates')
      ),
      hS('div', { className: 'model-list' },
        models.map(m => {
          const isSelected = model === m.id;
          const isDownloading = downloading === m.id;
          return hS('div', {
            key: m.id,
            className: 'model-row' + (isSelected ? ' selected' : '') + (!m.installed ? ' not-installed' : ''),
            onClick: () => m.installed && setModel(m.id)
          },
            hS('div', { className: 'model-radio' + (isSelected ? ' on' : '') },
              isSelected ? hS('div', { className: 'model-radio-dot' }) : null),
            hS('div', { className: 'model-meta' },
              hS('div', { className: 'model-name' },
                m.id,
                m.default && m.installed ? hS('span', { className: 'chip chip-accent', style: { fontSize: 10, padding: '2px 6px' } }, lang === 'es' ? 'Por defecto' : 'Default') : null,
                !m.installed && !isDownloading ? hS('span', { className: 'chip', style: { fontSize: 10, padding: '2px 6px', background: 'var(--bg-inset)' } },
                  hS(Icon, { name: 'download', size: 10 }), lang === 'es' ? 'No descargado' : 'Not downloaded') : null
              ),
              hS('div', { className: 'model-desc' },
                hS('span', null, pickL(m.desc, lang)),
                hS('span', { className: 'model-dot' }),
                hS('span', { className: 'font-mono' }, m.size, ' ', m.sizeUnit)
              ),
              isDownloading ? hS('div', { className: 'model-progress' },
                hS('div', { className: 'model-progress-bar' },
                  hS('div', { className: 'model-progress-fill', style: { width: progress + '%' } })),
                hS('div', { className: 'model-progress-meta' },
                  hS('span', null, lang === 'es' ? 'Descargando…' : 'Downloading…'),
                  hS('span', { className: 'font-mono' }, Math.round(progress), '% · ', (m.size * progress / 100).toFixed(1), '/', m.size, ' ', m.sizeUnit),
                  hS('span', { className: 'font-mono text-subtle' }, '14 MB/s · ETA ', Math.max(1, Math.round((100 - progress) / 8)), 's'))
              ) : null
            ),
            hS('div', { className: 'model-actions' },
              !m.installed && !isDownloading ? hS('button', {
                className: 'btn btn-primary', style: { fontSize: 11, padding: '5px 10px' },
                onClick: e => { e.stopPropagation(); startDownload(m.id); }
              }, hS(Icon, { name: 'download', size: 12 }), lang === 'es' ? 'Descargar' : 'Download') : null,
              isDownloading ? hS('button', {
                className: 'btn', style: { fontSize: 11, padding: '5px 10px' },
                onClick: e => { e.stopPropagation(); setDownloading(null); setProgress(0); }
              }, lang === 'es' ? 'Cancelar' : 'Cancel') : null,
              m.installed ? hS('button', {
                className: 'btn btn-icon btn-ghost', title: lang === 'es' ? 'Eliminar' : 'Remove',
                onClick: e => { e.stopPropagation(); removeModel(m.id); }
              }, hS(Icon, { name: 'trash', size: 13 })) : null
            )
          );
        })
      )
    ),

    // Hardware acceleration block
    hS('div', null,
      hS('label', { className: 'field-label' }, lang === 'es' ? 'Aceleración por hardware' : 'Hardware acceleration'),
      hS('div', { className: 'hardware-grid' },
        hS('div', { className: 'hardware-cell' },
          hS('div', { className: 'hardware-cell-head' },
            hS(Icon, { name: 'zap', size: 14, stroke: 'var(--accent)' }),
            hS('span', { style: { fontWeight: 600, fontSize: 12 } }, 'GPU'),
            hS('span', { className: 'chip chip-accent', style: { marginLeft: 'auto', fontSize: 10 } }, lang === 'es' ? 'En uso' : 'Active')
          ),
          hS('div', { className: 'hardware-cell-value' }, hardware.gpu),
          hS('div', { className: 'hardware-cell-meta' }, hardware.cuda)
        ),
        hS('div', { className: 'hardware-cell' },
          hS('div', { className: 'hardware-cell-head' },
            hS(Icon, { name: 'cpu', size: 14 }),
            hS('span', { style: { fontWeight: 600, fontSize: 12 } }, 'CPU'),
            hS('span', { className: 'chip', style: { marginLeft: 'auto', fontSize: 10 } }, lang === 'es' ? 'Respaldo' : 'Fallback')
          ),
          hS('div', { className: 'hardware-cell-value' }, hardware.cpu),
          hS('div', { className: 'hardware-cell-meta' }, hardware.ram)
        )
      )
    ),

    // Install location
    hS('div', { className: 'setting-row', style: { padding: '12px 0', borderTop: '1px solid var(--stroke)', borderBottom: 'none' } },
      hS('div', null,
        hS('div', { className: 'label' }, lang === 'es' ? 'Ubicación de los modelos' : 'Models location'),
        hS('div', { className: 'desc font-mono', style: { fontSize: 11, fontFamily: 'var(--font-mono)' } }, installPath)
      ),
      hS('div', { className: 'flex gap-2' },
        hS('button', { className: 'btn', style: { fontSize: 12 } },
          hS(Icon, { name: 'external', size: 12 }), lang === 'es' ? 'Abrir' : 'Open'),
        hS('button', { className: 'btn', style: { fontSize: 12 } },
          lang === 'es' ? 'Cambiar' : 'Change')
      )
    ),

    // Privacy footer
    hS('div', { className: 'engine-info' },
      hS(Icon, { name: 'shield', size: 14, stroke: 'var(--accent)' }),
      hS('span', null, lang === 'es'
        ? 'El motor local está embebido en Smart Noter. Los pesos del modelo y el audio nunca salen de tu equipo — todo el procesamiento ocurre offline en tu GPU/CPU.'
        : 'The local engine is embedded in Smart Noter. Model weights and audio never leave your device — all processing happens offline on your GPU/CPU.')
    )
  );
}

function OpenAIProviderConfig({ lang, model, setModel, cur, keyValue, setKey, showKey, setShowKey, keyStatus, setKeyStatus }) {
  function onTest() {
    setKeyStatus('testing');
    setTimeout(() => setKeyStatus(keyValue && keyValue.startsWith('sk-') ? 'saved' : 'invalid'), 900);
  }
  return hS('div', { style: { padding: 18, display: 'flex', flexDirection: 'column', gap: 14 } },
    hS('div', null,
      hS('label', { className: 'field-label' }, 'OpenAI API Key'),
      hS('div', { className: 'key-input-wrap' },
        hS('input', {
          className: 'input key-input',
          type: showKey ? 'text' : 'password',
          value: keyValue,
          onChange: e => { setKey(e.target.value); setKeyStatus('saved'); },
          placeholder: 'sk-proj-...'
        }),
        hS('button', {
          className: 'btn btn-ghost btn-icon key-input-eye',
          onClick: () => setShowKey(v => !v),
          title: showKey ? (lang === 'es' ? 'Ocultar' : 'Hide') : (lang === 'es' ? 'Mostrar' : 'Show')
        }, hS('svg', { width: 14, height: 14, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', strokeWidth: 1.7, strokeLinecap: 'round', strokeLinejoin: 'round' },
          showKey
            ? hS('path', { d: 'M3 3l18 18M10.6 6.1A10 10 0 0 1 12 6c5 0 9 4 10 6a13.2 13.2 0 0 1-3.4 4M6 8.5C4.1 10.2 3 12 2 12c1 2 5 6 10 6a10 10 0 0 0 3.5-.6M9.9 9.9a3 3 0 0 0 4.2 4.2' })
            : hS(React.Fragment, null,
                hS('path', { d: 'M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z' }),
                hS('circle', { cx: 12, cy: 12, r: 3 })
              )
        ))
      ),
      hS('div', { className: 'key-status-row' },
        keyStatus === 'saved' && hS('span', { className: 'chip chip-accent' },
          hS(Icon, { name: 'check', size: 11 }),
          lang === 'es' ? 'Clave válida · guardada cifrada' : 'Key valid · stored encrypted'),
        keyStatus === 'invalid' && hS('span', { className: 'chip', style: { background: 'rgba(239,68,68,0.1)', color: '#ef4444', borderColor: 'rgba(239,68,68,0.3)' } },
          hS(Icon, { name: 'close', size: 11 }),
          lang === 'es' ? 'No se pudo autenticar' : 'Authentication failed'),
        keyStatus === 'testing' && hS('span', { className: 'chip' },
          lang === 'es' ? 'Probando…' : 'Testing…'),
        hS('div', { style: { flex: 1 } }),
        hS('button', { className: 'btn', onClick: onTest, style: { fontSize: 12, padding: '5px 10px' } },
          hS(Icon, { name: 'zap', size: 12 }),
          lang === 'es' ? 'Probar conexión' : 'Test connection'),
        hS('a', { href: '#', style: { fontSize: 11, color: 'var(--accent)' } },
          hS(Icon, { name: 'external', size: 11, style: { marginRight: 4, verticalAlign: '-1px' } }),
          lang === 'es' ? 'Obtener una clave' : 'Get an API key')
      )
    ),
    hS('div', null,
      hS('label', { className: 'field-label' }, lang === 'es' ? 'Modelo' : 'Model'),
      hS('div', { className: 'field-grid' },
        cur.models.map(m => hS('button', {
          key: m, className: 'pill-radio' + (model === m ? ' selected' : ''),
          onClick: () => setModel(m)
        },
          hS('div', { className: 'pill-radio-name' }, m),
          hS('div', { className: 'pill-radio-meta' },
            m === 'gpt-4o-transcribe' ? (lang === 'es' ? 'Máxima fidelidad · streaming' : 'Max fidelity · streaming') :
            m === 'gpt-4o-mini-transcribe' ? (lang === 'es' ? 'Más barato · 3× rápido' : 'Cheaper · 3× faster') :
            'Whisper · ' + (lang === 'es' ? 'compatible amplio' : 'broad compat')
          )
        ))
      )
    ),
    hS('div', { className: 'field-row' },
      hS('div', { className: 'field-col' },
        hS('label', { className: 'field-label' }, lang === 'es' ? 'Organización (opcional)' : 'Organization (optional)'),
        hS('input', { className: 'input', placeholder: 'org-xxxxxxxxxxxx', defaultValue: 'org-9z8VfL' })
      ),
      hS('div', { className: 'field-col' },
        hS('label', { className: 'field-label' }, lang === 'es' ? 'Proyecto (opcional)' : 'Project (optional)'),
        hS('input', { className: 'input', placeholder: 'proj_xxxxxxxxxxxx', defaultValue: 'proj_KdH82L' })
      )
    ),
    hS('div', { className: 'engine-info' },
      hS(Icon, { name: 'shield', size: 14, stroke: 'var(--text-muted)' }),
      hS('span', null, lang === 'es'
        ? 'Las claves se guardan cifradas con DPAPI en tu perfil de Windows. Nunca se sincronizan a la nube.'
        : 'Keys are stored encrypted with DPAPI under your Windows profile. Never synced to the cloud.')
    )
  );
}

function AzureProviderConfig({ lang, model, setModel, cur, endpoint, setEndpoint, deployment, setDeployment, apiVersion, setApiVersion, region, setRegion, apiKey, setApiKey }) {
  return hS('div', { style: { padding: 18, display: 'flex', flexDirection: 'column', gap: 14 } },
    hS('div', { className: 'field-row' },
      hS('div', { className: 'field-col', style: { flex: 2 } },
        hS('label', { className: 'field-label' }, lang === 'es' ? 'Endpoint de Azure' : 'Azure endpoint'),
        hS('input', {
          className: 'input', value: endpoint, onChange: e => setEndpoint(e.target.value),
          placeholder: 'https://<resource>.openai.azure.com'
        })
      ),
      hS('div', { className: 'field-col' },
        hS('label', { className: 'field-label' }, lang === 'es' ? 'Región' : 'Region'),
        hS('div', { className: 'select-trigger', style: { width: '100%' } },
          hS('span', null, region),
          hS(Icon, { name: 'chevDown', size: 14 }))
      )
    ),
    hS('div', { className: 'field-row' },
      hS('div', { className: 'field-col' },
        hS('label', { className: 'field-label' }, lang === 'es' ? 'Nombre del deployment' : 'Deployment name'),
        hS('input', {
          className: 'input', value: deployment, onChange: e => setDeployment(e.target.value),
          placeholder: 'whisper-prod'
        })
      ),
      hS('div', { className: 'field-col' },
        hS('label', { className: 'field-label' }, 'API version'),
        hS('input', {
          className: 'input font-mono',
          value: apiVersion, onChange: e => setApiVersion(e.target.value),
          style: { fontFamily: 'var(--font-mono)', fontSize: 12 }
        })
      )
    ),
    hS('div', null,
      hS('label', { className: 'field-label' }, lang === 'es' ? 'Tipo de autenticación' : 'Authentication'),
      hS(Segmented, {
        value: 'key',
        options: [
          { value: 'key', label: 'API key' },
          { value: 'aad', label: 'Microsoft Entra ID' },
          { value: 'mi',  label: lang === 'es' ? 'Identidad administrada' : 'Managed identity' }
        ],
        onChange: () => {}
      })
    ),
    hS('div', null,
      hS('label', { className: 'field-label' }, 'API key'),
      hS('div', { className: 'key-input-wrap' },
        hS('input', {
          className: 'input key-input', type: 'password',
          value: apiKey, onChange: e => setApiKey(e.target.value),
          placeholder: '••••••••••••••••••••••••••••••••'
        })
      ),
      hS('div', { className: 'key-status-row' },
        hS('span', { className: 'chip' }, lang === 'es' ? 'Sin probar' : 'Not tested'),
        hS('div', { style: { flex: 1 } }),
        hS('button', { className: 'btn', style: { fontSize: 12, padding: '5px 10px' } },
          hS(Icon, { name: 'zap', size: 12 }),
          lang === 'es' ? 'Probar conexión' : 'Test connection')
      )
    ),
    hS('div', null,
      hS('label', { className: 'field-label' }, lang === 'es' ? 'Modelo desplegado' : 'Deployed model'),
      hS('div', { className: 'field-grid' },
        cur.models.map(m => hS('button', {
          key: m, className: 'pill-radio' + (model === m ? ' selected' : ''),
          onClick: () => setModel(m)
        },
          hS('div', { className: 'pill-radio-name' }, m),
          hS('div', { className: 'pill-radio-meta' },
            m.includes('whisper') ? (lang === 'es' ? 'STT clásico' : 'Classic STT') :
            m.includes('gpt-4o') ? (lang === 'es' ? 'Avanzado · diarización' : 'Advanced · diarization') :
            (lang === 'es' ? 'Servicio nativo' : 'Native service')
          )
        ))
      )
    ),
    hS('div', { className: 'engine-info' },
      hS(Icon, { name: 'shield', size: 14, stroke: '#0078d4' }),
      hS('span', null, lang === 'es'
        ? 'Tus datos permanecen dentro del perímetro de tu tenant de Azure. Compatible con HIPAA, ISO 27001 y SOC 2.'
        : 'Your data stays inside your Azure tenant perimeter. HIPAA, ISO 27001 and SOC 2 compliant.')
    )
  );
}

function CustomProviderConfig({ lang, url, setUrl, apiKey, setApiKey, model, setModel }) {
  return hS('div', { style: { padding: 18, display: 'flex', flexDirection: 'column', gap: 14 } },
    hS('div', null,
      hS('label', { className: 'field-label' }, lang === 'es' ? 'URL del endpoint (compatible con OpenAI)' : 'Endpoint URL (OpenAI-compatible)'),
      hS('input', {
        className: 'input font-mono',
        value: url, onChange: e => setUrl(e.target.value),
        placeholder: 'https://...',
        style: { fontFamily: 'var(--font-mono)', fontSize: 12 }
      })
    ),
    hS('div', { className: 'field-row' },
      hS('div', { className: 'field-col' },
        hS('label', { className: 'field-label' }, 'API key'),
        hS('input', {
          className: 'input', type: 'password',
          value: apiKey, onChange: e => setApiKey(e.target.value),
          placeholder: '••••••••••••'
        })
      ),
      hS('div', { className: 'field-col' },
        hS('label', { className: 'field-label' }, lang === 'es' ? 'Nombre del modelo' : 'Model name'),
        hS('input', {
          className: 'input', value: model || '', onChange: e => setModel(e.target.value),
          placeholder: 'whisper-large-v3'
        })
      )
    ),
    hS('div', { className: 'engine-info' },
      hS(Icon, { name: 'help', size: 14, stroke: 'var(--text-muted)' }),
      hS('span', null, lang === 'es'
        ? 'Funciona con cualquier servicio que exponga /v1/audio/transcriptions estilo OpenAI (Groq, Together, vLLM, on-prem).'
        : 'Works with any service exposing OpenAI-style /v1/audio/transcriptions (Groq, Together, vLLM, on-prem).')
    )
  );
}

window.TemplatesGallery = TemplatesGallery;
window.ParticipantsManager = ParticipantsManager;
window.Settings = Settings;
window.ExportModal = ExportModal;
window.TranscriptionEngineSection = TranscriptionEngineSection;
