/* Smart Noter — mock data + i18n */

// ===== Templates =====
const TEMPLATES = [
  {
    id: 'ejecutiva', colorClass: 't-color-ejecutiva',
    icon: 'briefcase',
    name: { es: 'Ejecutiva', en: 'Executive' },
    desc: {
      es: 'Resumen ejecutivo, decisiones clave, métricas y próximos pasos.',
      en: 'Executive summary, key decisions, metrics and next steps.'
    },
    sections: ['summary', 'decisions', 'metrics', 'actions', 'risks']
  },
  {
    id: 'discovery', colorClass: 't-color-discovery',
    icon: 'search',
    name: { es: 'Levantamiento de información', en: 'Discovery' },
    desc: {
      es: 'Pain points, requerimientos funcionales, stakeholders y supuestos.',
      en: 'Pain points, functional requirements, stakeholders and assumptions.'
    },
    sections: ['context', 'pain-points', 'requirements', 'stakeholders', 'assumptions', 'actions']
  },
  {
    id: 'tecnica', colorClass: 't-color-tecnica',
    icon: 'cpu',
    name: { es: 'Técnica — implementación', en: 'Technical — implementation' },
    desc: {
      es: 'Arquitectura, decisiones técnicas, bloqueos y entregables.',
      en: 'Architecture, tech decisions, blockers and deliverables.'
    },
    sections: ['architecture', 'tech-decisions', 'blockers', 'deliverables', 'actions']
  },
  {
    id: 'webinar', colorClass: 't-color-webinar',
    icon: 'megaphone',
    name: { es: 'Webinar', en: 'Webinar' },
    desc: {
      es: 'Agenda, mensajes clave, Q&A y métricas de audiencia.',
      en: 'Agenda, key messages, Q&A and audience metrics.'
    },
    sections: ['agenda', 'key-messages', 'qa', 'audience-metrics']
  },
  {
    id: 'daily', colorClass: 't-color-daily',
    icon: 'sun',
    name: { es: 'Daily / Standup', en: 'Daily / Standup' },
    desc: {
      es: 'Ayer / hoy / bloqueos por persona, foco del día.',
      en: 'Yesterday / today / blockers per person, daily focus.'
    },
    sections: ['yesterday', 'today', 'blockers', 'focus']
  },
  {
    id: 'retro', colorClass: 't-color-retro',
    icon: 'refresh',
    name: { es: 'Retrospectiva', en: 'Retrospective' },
    desc: {
      es: 'Qué funcionó, qué no, aprendizajes y experimentos.',
      en: 'What worked, what didn\'t, learnings and experiments.'
    },
    sections: ['went-well', 'went-wrong', 'learnings', 'experiments', 'actions']
  },
  {
    id: 'entrevista', colorClass: 't-color-entrevista',
    icon: 'user',
    name: { es: 'Entrevista', en: 'Interview' },
    desc: {
      es: 'Background, evaluación por competencia, señales y recomendación.',
      en: 'Background, competency assessment, signals and recommendation.'
    },
    sections: ['background', 'competencies', 'signals', 'recommendation']
  },
  {
    id: 'coaching', colorClass: 't-color-coaching',
    icon: 'compass',
    name: { es: '1:1 / Coaching', en: '1:1 / Coaching' },
    desc: {
      es: 'Estado, feedback, objetivos y acompañamiento.',
      en: 'Status, feedback, goals and follow-up.'
    },
    sections: ['status', 'feedback', 'goals', 'follow-up']
  },
  {
    id: 'conferencia', colorClass: 't-color-conferencia',
    icon: 'mic',
    name: { es: 'Conferencia', en: 'Conference' },
    desc: {
      es: 'Speakers, temas, citas destacadas y contactos clave.',
      en: 'Speakers, topics, key quotes and contacts.'
    },
    sections: ['speakers', 'topics', 'quotes', 'contacts']
  }
];

// ===== Devices (audio capture) =====
const AUDIO_DEVICES = [
  {
    id: 'system-loopback',
    name: { es: 'Audio del sistema (Loopback)', en: 'System Audio (Loopback)' },
    desc: {
      es: 'Captura todo el audio que reproduce la PC — recomendado para Teams/Zoom.',
      en: 'Captures all audio playing on this PC — recommended for Teams/Zoom.'
    },
    icon: 'monitor',
    recommended: true,
    active: true
  },
  {
    id: 'realtek-mic',
    name: { es: 'Micrófono — Realtek HD Audio', en: 'Microphone — Realtek HD Audio' },
    desc: {
      es: 'Sólo capturará tu voz local, no la de los demás participantes.',
      en: 'Will only capture your local voice, not other participants.'
    },
    icon: 'mic',
    recommended: false,
    active: false
  },
  {
    id: 'jabra-evolve',
    name: { es: 'Jabra Evolve2 75 — Headset', en: 'Jabra Evolve2 75 — Headset' },
    desc: {
      es: 'Audio del headset USB. Captura el lado del usuario.',
      en: 'USB headset audio. Captures the user side.'
    },
    icon: 'headphones',
    recommended: false,
    active: false
  },
  {
    id: 'stereo-mix',
    name: { es: 'Mezcla estéreo (Stereo Mix)', en: 'Stereo Mix' },
    desc: {
      es: 'Combina entrada y salida del sistema. Alternativa al loopback.',
      en: 'Combines system input and output. Alternative to loopback.'
    },
    icon: 'sliders',
    recommended: false,
    active: false
  }
];

// ===== Subject palette =====
const SUBJECT_COLORS = [
  { id: 1, class: 's-color-1' },
  { id: 2, class: 's-color-2' },
  { id: 3, class: 's-color-3' },
  { id: 4, class: 's-color-4' },
  { id: 5, class: 's-color-5' },
  { id: 6, class: 's-color-6' },
  { id: 7, class: 's-color-7' },
  { id: 8, class: 's-color-8' }
];

// ===== Mock meetings =====
const MEETINGS = [
  {
    id: 'm-001',
    title: { es: 'Implementación CRM — Sprint 4 Review', en: 'CRM Implementation — Sprint 4 Review' },
    template: 'tecnica',
    date: '2025-11-12T15:30:00',
    durationSec: 3274, // 54:34
    participants: [
      { id: 's1', label: 'S1', name: 'Andrea Solís', colorClass: 's-color-1', wordCount: 2840, talkPct: 38 },
      { id: 's2', label: 'S2', name: 'Diego Mejía', colorClass: 's-color-2', wordCount: 1920, talkPct: 25 },
      { id: 's3', label: 'S3', name: null, colorClass: 's-color-3', wordCount: 1450, talkPct: 19 },
      { id: 's4', label: 'S4', name: 'Marta Cervantes', colorClass: 's-color-4', wordCount: 1320, talkPct: 18 }
    ],
    deviceUsed: 'system-loopback',
    wordCount: 7530,
    summary: {
      es: 'Sesión de revisión del Sprint 4. El equipo presentó el avance del módulo de pipeline (85% completo) y discutió la migración del legacy. Se confirmó la fecha de Go-Live para el 18 de diciembre. Diego reportó un bloqueo en la integración con SAP que requiere coordinación con el equipo de infraestructura. Se acordó priorizar el módulo de reportería en el siguiente sprint.',
      en: 'Sprint 4 review session. The team presented progress on the pipeline module (85% complete) and discussed legacy migration. Go-Live confirmed for December 18. Diego reported a blocker in the SAP integration requiring coordination with infra. Reporting module prioritized for next sprint.'
    },
    decisions: [
      { es: 'Go-Live confirmado para 18 de diciembre.', en: 'Go-Live confirmed for December 18.' },
      { es: 'Priorizar módulo de reportería en Sprint 5.', en: 'Prioritize reporting module in Sprint 5.' },
      { es: 'Contratar consultor externo SAP por 2 semanas.', en: 'Hire external SAP consultant for 2 weeks.' }
    ],
    blockers: [
      { es: 'Integración SAP: API de inventario lanza timeout en cargas > 5k registros.', en: 'SAP integration: inventory API times out on loads > 5k records.' },
      { es: 'Pendiente firma de cliente para acceso al ambiente productivo.', en: 'Client signature pending for production env access.' }
    ],
    actions: [
      { id: 'a1', text: { es: 'Agendar sesión con equipo SAP para resolver timeout', en: 'Schedule SAP team session to resolve timeout' }, owner: 's2', due: '2025-11-15', done: false },
      { id: 'a2', text: { es: 'Generar documento de arquitectura del módulo de reportería', en: 'Generate reporting module architecture doc' }, owner: 's1', due: '2025-11-18', done: false },
      { id: 'a3', text: { es: 'Solicitar firma del cliente para ambiente productivo', en: 'Request client signature for production env' }, owner: 's4', due: '2025-11-14', done: true },
      { id: 'a4', text: { es: 'Revisar pruebas de carga del pipeline antes del Go-Live', en: 'Review pipeline load tests before Go-Live' }, owner: 's3', due: '2025-12-10', done: false },
      { id: 'a5', text: { es: 'Preparar plan de rollback en caso de falla en producción', en: 'Prepare rollback plan in case of production failure' }, owner: 's1', due: '2025-12-15', done: false }
    ],
    transcript: [
      { t: '00:00:04', speakerId: 's1', text: { es: 'Buenas tardes a todos, gracias por conectarse. Vamos a iniciar la revisión del Sprint 4. Diego, ¿puedes compartir pantalla con el tablero?', en: 'Good afternoon everyone, thanks for joining. Let\'s start the Sprint 4 review. Diego, can you share the board?' } },
      { t: '00:00:18', speakerId: 's2', text: { es: 'Claro, dame un segundo. Listo. Como ven, terminamos 18 de 21 historias planificadas, el pipeline está al 85%.', en: 'Sure, give me a second. Done. As you can see, we finished 18 of 21 planned stories, pipeline is at 85%.' } },
      { t: '00:00:42', speakerId: 's3', text: { es: 'Excelente avance. ¿Cuál fue el principal blocker que enfrentaron?', en: 'Excellent progress. What was the main blocker you faced?' } },
      { t: '00:00:51', speakerId: 's2', text: { es: 'La integración con SAP. La API de inventario nos lanza timeout cuando cargamos más de 5 mil registros. Estamos revisando si es problema de red o del endpoint.', en: 'The SAP integration. The inventory API times out when we load more than 5k records. We\'re checking if it\'s a network or endpoint issue.' } },
      { t: '00:01:14', speakerId: 's1', text: { es: 'Necesitamos coordinar una sesión urgente con el equipo de SAP. Diego, ¿puedes agendarla para esta semana?', en: 'We need to coordinate an urgent session with the SAP team. Diego, can you schedule it this week?' } },
      { t: '00:01:24', speakerId: 's2', text: { es: 'Sí, la agendo para el viernes.', en: 'Yes, I\'ll schedule it for Friday.' } },
      { t: '00:01:30', speakerId: 's4', text: { es: 'Sobre el Go-Live, ¿mantenemos la fecha del 18 de diciembre? Necesito confirmar con el cliente.', en: 'About Go-Live, do we keep December 18? I need to confirm with the client.' } },
      { t: '00:01:42', speakerId: 's1', text: { es: 'Sí, mantenemos el 18 de diciembre. Marta, por favor pide la firma para acceso al ambiente productivo.', en: 'Yes, we keep December 18. Marta, please request the signature for production env access.' } },
      { t: '00:01:55', speakerId: 's4', text: { es: 'Anotado. Se la pido hoy mismo.', en: 'Noted. I\'ll request it today.' } },
      { t: '00:02:10', speakerId: 's3', text: { es: 'Una pregunta — ¿el módulo de reportería entra en Sprint 5 completo o lo dividimos?', en: 'A question — does the reporting module fit fully in Sprint 5 or do we split it?' } },
      { t: '00:02:22', speakerId: 's1', text: { es: 'Lo entramos completo. Es prioritario para el cliente. Necesitamos un documento de arquitectura antes del kickoff.', en: 'We\'ll fit it fully. It\'s a client priority. We need an architecture doc before kickoff.' } },
      { t: '00:02:38', speakerId: 's1', text: { es: 'Yo me encargo del documento, lo tengo listo el martes 18.', en: 'I\'ll take the doc, ready by Tuesday the 18th.' } },
      { t: '00:02:48', speakerId: 's2', text: { es: 'Perfecto. También quería plantear que necesitamos un consultor externo de SAP por al menos 2 semanas. La curva de aprendizaje es demasiado pronunciada para los junior.', en: 'Perfect. I also wanted to raise that we need an external SAP consultant for at least 2 weeks. The learning curve is too steep for juniors.' } },
      { t: '00:03:05', speakerId: 's1', text: { es: 'Aprobado. Voy a hablar con compras mañana para acelerar la contratación.', en: 'Approved. I\'ll talk to procurement tomorrow to fast-track the hire.' } }
    ]
  },
  {
    id: 'm-002',
    title: { es: 'Q4 Board — Resultados financieros', en: 'Q4 Board — Financial Results' },
    template: 'ejecutiva',
    date: '2025-11-10T10:00:00',
    durationSec: 2841,
    participants: [
      { id: 's1', label: 'S1', name: 'Carmen Velázquez', colorClass: 's-color-1', wordCount: 1820, talkPct: 32 },
      { id: 's2', label: 'S2', name: 'Roberto Aguilar', colorClass: 's-color-2', wordCount: 1640, talkPct: 28 },
      { id: 's3', label: 'S3', name: 'Pablo Trejo', colorClass: 's-color-3', wordCount: 1180, talkPct: 21 },
      { id: 's4', label: 'S4', name: null, colorClass: 's-color-4', wordCount: 980, talkPct: 19 }
    ],
    deviceUsed: 'system-loopback',
    wordCount: 5620,
    actions: [],
    summary: { es: 'Cierre Q4 con +18% YoY en ingresos. EBITDA al 24%.', en: 'Q4 close with +18% YoY revenue. EBITDA at 24%.' }
  },
  {
    id: 'm-003',
    title: { es: 'Discovery — Logística Norteña SA', en: 'Discovery — Logística Norteña SA' },
    template: 'discovery',
    date: '2025-11-08T13:00:00',
    durationSec: 4521,
    participants: [
      { id: 's1', label: 'S1', name: 'Helena Pacheco', colorClass: 's-color-1', talkPct: 42 },
      { id: 's2', label: 'S2', name: null, colorClass: 's-color-2', talkPct: 31 },
      { id: 's3', label: 'S3', name: null, colorClass: 's-color-3', talkPct: 27 }
    ],
    deviceUsed: 'jabra-evolve',
    wordCount: 8920,
    actions: []
  },
  {
    id: 'm-004',
    title: { es: 'Daily Standup — Equipo Mobile', en: 'Daily Standup — Mobile Team' },
    template: 'daily',
    date: '2025-11-12T09:00:00',
    durationSec: 924,
    participants: [
      { id: 's1', label: 'S1', name: 'Tania López', colorClass: 's-color-1', talkPct: 22 },
      { id: 's2', label: 'S2', name: 'Israel Núñez', colorClass: 's-color-2', talkPct: 19 },
      { id: 's3', label: 'S3', name: 'Andrés Vélez', colorClass: 's-color-3', talkPct: 18 },
      { id: 's4', label: 'S4', name: null, colorClass: 's-color-4', talkPct: 21 },
      { id: 's5', label: 'S5', name: null, colorClass: 's-color-5', talkPct: 20 }
    ],
    deviceUsed: 'system-loopback',
    wordCount: 2140,
    actions: []
  },
  {
    id: 'm-005',
    title: { es: 'Retro — Sprint 3 (Backend Pagos)', en: 'Retro — Sprint 3 (Payments Backend)' },
    template: 'retro',
    date: '2025-11-05T16:00:00',
    durationSec: 3680,
    participants: [
      { id: 's1', label: 'S1', name: 'Beatriz Sánchez', colorClass: 's-color-1', talkPct: 28 },
      { id: 's2', label: 'S2', name: 'Mauricio Ríos', colorClass: 's-color-2', talkPct: 24 },
      { id: 's3', label: 'S3', name: null, colorClass: 's-color-3', talkPct: 26 },
      { id: 's4', label: 'S4', name: null, colorClass: 's-color-4', talkPct: 22 }
    ],
    deviceUsed: 'system-loopback',
    wordCount: 6840,
    actions: []
  },
  {
    id: 'm-006',
    title: { es: 'Webinar — Tendencias en IA generativa 2026', en: 'Webinar — Generative AI Trends 2026' },
    template: 'webinar',
    date: '2025-11-04T11:00:00',
    durationSec: 3600,
    participants: [
      { id: 's1', label: 'S1', name: 'Dr. Ricardo Aceves', colorClass: 's-color-1', talkPct: 78 },
      { id: 's2', label: 'S2', name: null, colorClass: 's-color-2', talkPct: 12 },
      { id: 's3', label: 'S3', name: null, colorClass: 's-color-3', talkPct: 10 }
    ],
    deviceUsed: 'system-loopback',
    wordCount: 9420,
    actions: []
  }
];

// ===== i18n =====
const I18N = {
  es: {
    appName: 'Smart Noter',
    appTag: 'Notas de reunión con IA',
    // Nav
    navWorkspace: 'Espacio de trabajo',
    navDashboard: 'Inicio',
    navMeetings: 'Reuniones',
    navTemplates: 'Plantillas',
    navTools: 'Herramientas',
    navSettings: 'Configuración',
    navRecord: 'Nueva grabación',
    navHelp: 'Ayuda y soporte',
    // Dashboard
    welcome: 'Buenas tardes, Carlos',
    welcomeSub: 'Aquí está el resumen de tu semana.',
    statTotal: 'Reuniones',
    statHours: 'Horas grabadas',
    statActions: 'Acciones pendientes',
    statTranscript: 'Palabras transcritas',
    thisWeek: 'esta semana',
    recentMeetings: 'Reuniones recientes',
    seeAll: 'Ver todas',
    searchMeetings: 'Buscar reunión, participante, palabra clave…',
    captureStatus: 'Estado de captura',
    captureDesc: 'Audio del sistema listo. Las apps de videoconferencia no se ven afectadas.',
    activeDevice: 'Dispositivo activo',
    inputLevel: 'Nivel de entrada',
    quickRecord: 'Iniciar grabación',
    quickImport: 'Importar audio',
    // Pre-record
    preTitle: 'Nueva grabación',
    preSub: 'Configura antes de iniciar para que la transcripción y el resumen salgan perfectos.',
    meetingNameLabel: 'Nombre de la reunión',
    meetingNamePh: 'Ej: Comité directivo — Q4 review',
    deviceSection: 'Dispositivo de grabación',
    deviceHint: 'Elige el dispositivo de audio. La app no se conecta a Teams o Zoom — sólo escucha lo que reproduce tu PC.',
    templateSection: 'Plantilla de reunión',
    templateHint: 'Define qué tipo de resumen y acciones se generarán.',
    advancedSection: 'Opciones avanzadas',
    autoIdSpeakers: 'Identificar hablantes automáticamente',
    autoIdSpeakersDesc: 'Asigna Sujeto 1, 2, 3… con voz biométrica local. Puedes renombrar después.',
    detectLang: 'Detectar idioma del audio',
    detectLangDesc: 'Soporta es, en, pt — alterna en tiempo real si la reunión es bilingüe.',
    saveAudio: 'Guardar archivo de audio',
    saveAudioDesc: 'Conserva WAV/MP3 además de la transcripción. Útil para auditoría.',
    cancel: 'Cancelar',
    startRecording: 'Iniciar grabación',
    // Live
    liveStatus: 'GRABANDO',
    livePauseHint: 'Pausa', liveStopHint: 'Detener',
    transcriptionEngine: 'Transcripción local — fiabilidad 99.2%',
    speakersDetected: 'Hablantes detectados',
    sourceLabel: 'Fuente',
    // Detail
    transcript: 'Transcripción',
    summary: 'Resumen',
    actions: 'Acciones',
    participants: 'Participantes',
    audio: 'Audio',
    keyDecisions: 'Decisiones clave',
    blockersTitle: 'Bloqueos y riesgos',
    nextSteps: 'Próximos pasos',
    aiAsk: 'Pregúntale a la reunión',
    askPlaceholder: 'Pregunta sobre la reunión…',
    suggestedQ1: '¿Cuáles fueron los acuerdos?',
    suggestedQ2: 'Resume en 3 puntos',
    suggestedQ3: 'Acciones de Diego',
    metrics: 'Métricas',
    talkTime: 'Tiempo de palabra',
    rename: 'Renombrar',
    assignName: 'Asignar nombre',
    export: 'Exportar',
    share: 'Compartir',
    backToMeetings: 'Reuniones',
    // Export modal
    exportTitle: 'Exportar reunión',
    exportSub: 'Elige uno o varios formatos. Todos respetan la plantilla seleccionada.',
    exportAudio: 'Archivo de audio',
    exportAudioDesc: 'Grabación completa en alta calidad (MP3 / WAV)',
    exportMd: 'Markdown',
    exportMdDesc: 'Transcripción + resumen + acciones, listo para Notion / Obsidian',
    exportPdf: 'PDF',
    exportPdfDesc: 'Documento profesional con encabezado, participantes y firma',
    exportNow: 'Exportar',
    // Templates
    tmplTitle: 'Galería de plantillas',
    tmplSub: 'Cada plantilla genera un tipo distinto de resumen, acciones y métricas.',
    tmplUseDefault: 'Usar como predeterminada',
    tmplDuplicate: 'Duplicar',
    tmplEdit: 'Editar',
    // Settings
    settingsTitle: 'Configuración',
    settingsSub: 'Audio del sistema, idioma de transcripción y privacidad.',
    audioCapture: 'Captura de audio',
    captureMode: 'Modo de captura',
    captureModeDesc: 'Cómo se captura el audio sin interferir con Teams / Zoom.',
    captureSystem: 'Audio del sistema (Loopback)',
    captureMic: 'Sólo micrófono',
    captureMix: 'Mezcla (sistema + micrófono)',
    defaultDevice: 'Dispositivo predeterminado',
    transcriptionEngineLabel: 'Motor de transcripción',
    runLocal: 'Ejecutar localmente',
    runLocalDesc: 'Procesa el audio en tu equipo. Máxima privacidad, requiere ~4GB.',
    autoDeleteAudio: 'Eliminar audio después de 30 días',
    autoDeleteAudioDesc: 'La transcripción se conserva. Aplica sólo a grabaciones no exportadas.',
    privacy: 'Privacidad',
    storage: 'Almacenamiento',
    // Participants
    partTitle: 'Participantes',
    partSub: 'Renombra a los sujetos detectados. Los cambios se aplican a toda la transcripción.',
    unnamed: 'Sin nombre',
    // Sections (template)
    secSummary: 'Resumen ejecutivo',
    secDecisions: 'Decisiones clave',
    secMetrics: 'Métricas y KPIs',
    secActions: 'Acciones pendientes',
    secRisks: 'Riesgos y bloqueos',
    secContext: 'Contexto del negocio',
    secPainPoints: 'Pain points identificados',
    secRequirements: 'Requerimientos funcionales',
    secStakeholders: 'Stakeholders',
    secAssumptions: 'Supuestos',
    secArchitecture: 'Arquitectura propuesta',
    secTechDecisions: 'Decisiones técnicas',
    secBlockers: 'Bloqueos técnicos',
    secDeliverables: 'Entregables',
    secAgenda: 'Agenda',
    secKeyMessages: 'Mensajes clave',
    secQA: 'Q&A destacado',
    secAudienceMetrics: 'Métricas de audiencia',
    secYesterday: 'Avances de ayer',
    secToday: 'Foco de hoy',
    secFocus: 'Foco del día',
    secWentWell: '✅ Qué funcionó',
    secWentWrong: '⚠️ Qué no funcionó',
    secLearnings: '💡 Aprendizajes',
    secExperiments: '🧪 Experimentos',
    secBackground: 'Background del candidato',
    secCompetencies: 'Evaluación por competencia',
    secSignals: 'Señales observadas',
    secRecommendation: 'Recomendación',
    secStatus: 'Estado actual',
    secFeedback: 'Feedback',
    secGoals: 'Objetivos',
    secFollowUp: 'Acompañamiento',
    secSpeakers: 'Ponentes',
    secTopics: 'Temas',
    secQuotes: 'Citas destacadas',
    secContacts: 'Contactos clave',
    // Misc
    untitled: 'Sin título',
    today: 'Hoy', yesterday: 'Ayer',
    minAgo: 'min', hoursAgo: 'h',
    speaking: 'Hablando',
    silence: 'En silencio',
    duration: 'Duración',
    fidelity: 'Fidelidad',
    new: 'Nueva',
    play: 'Reproducir',
    paste: 'Pegar al markdown',
    timestampsOn: 'Timestamps',
    bilingual: 'Bilingüe',
    fileNamePh: 'p. ej. reunion-2025-11-12'
  },
  en: {
    appName: 'Smart Noter',
    appTag: 'AI meeting notes',
    navWorkspace: 'Workspace',
    navDashboard: 'Home',
    navMeetings: 'Meetings',
    navTemplates: 'Templates',
    navTools: 'Tools',
    navSettings: 'Settings',
    navRecord: 'New recording',
    navHelp: 'Help & support',
    welcome: 'Good afternoon, Carlos',
    welcomeSub: 'Here\'s your week at a glance.',
    statTotal: 'Meetings',
    statHours: 'Hours recorded',
    statActions: 'Pending actions',
    statTranscript: 'Words transcribed',
    thisWeek: 'this week',
    recentMeetings: 'Recent meetings',
    seeAll: 'See all',
    searchMeetings: 'Search meeting, participant, keyword…',
    captureStatus: 'Capture status',
    captureDesc: 'System audio ready. Your video conferencing apps are not affected.',
    activeDevice: 'Active device',
    inputLevel: 'Input level',
    quickRecord: 'Start recording',
    quickImport: 'Import audio',
    preTitle: 'New recording',
    preSub: 'Set things up before starting for the best transcript and summary.',
    meetingNameLabel: 'Meeting name',
    meetingNamePh: 'e.g. Steering committee — Q4 review',
    deviceSection: 'Recording device',
    deviceHint: 'Pick the audio device. The app never connects to Teams or Zoom — it only listens to what your PC plays.',
    templateSection: 'Meeting template',
    templateHint: 'Defines what kind of summary and actions are generated.',
    advancedSection: 'Advanced options',
    autoIdSpeakers: 'Auto-identify speakers',
    autoIdSpeakersDesc: 'Assigns Subject 1, 2, 3… via local voice biometrics. Rename anytime.',
    detectLang: 'Detect audio language',
    detectLangDesc: 'Supports es, en, pt — switches in real time for bilingual meetings.',
    saveAudio: 'Keep audio file',
    saveAudioDesc: 'Store WAV/MP3 alongside the transcript. Useful for auditing.',
    cancel: 'Cancel',
    startRecording: 'Start recording',
    liveStatus: 'RECORDING',
    livePauseHint: 'Pause', liveStopHint: 'Stop',
    transcriptionEngine: 'Local transcription — 99.2% fidelity',
    speakersDetected: 'Speakers detected',
    sourceLabel: 'Source',
    transcript: 'Transcript',
    summary: 'Summary',
    actions: 'Actions',
    participants: 'Participants',
    audio: 'Audio',
    keyDecisions: 'Key decisions',
    blockersTitle: 'Blockers & risks',
    nextSteps: 'Next steps',
    aiAsk: 'Ask this meeting',
    askPlaceholder: 'Ask anything about this meeting…',
    suggestedQ1: 'What were the agreements?',
    suggestedQ2: 'Summarize in 3 bullets',
    suggestedQ3: 'Diego\'s action items',
    metrics: 'Metrics',
    talkTime: 'Talk time',
    rename: 'Rename',
    assignName: 'Assign name',
    export: 'Export',
    share: 'Share',
    backToMeetings: 'Meetings',
    exportTitle: 'Export meeting',
    exportSub: 'Pick one or more formats. All respect the selected template.',
    exportAudio: 'Audio file',
    exportAudioDesc: 'Full high-quality recording (MP3 / WAV)',
    exportMd: 'Markdown',
    exportMdDesc: 'Transcript + summary + actions, ready for Notion / Obsidian',
    exportPdf: 'PDF',
    exportPdfDesc: 'Professional doc with header, participants and signature',
    exportNow: 'Export',
    tmplTitle: 'Template gallery',
    tmplSub: 'Each template produces a different summary, actions and metrics.',
    tmplUseDefault: 'Set as default',
    tmplDuplicate: 'Duplicate',
    tmplEdit: 'Edit',
    settingsTitle: 'Settings',
    settingsSub: 'System audio, transcription language and privacy.',
    audioCapture: 'Audio capture',
    captureMode: 'Capture mode',
    captureModeDesc: 'How audio is captured without interfering with Teams / Zoom.',
    captureSystem: 'System audio (Loopback)',
    captureMic: 'Microphone only',
    captureMix: 'Mix (system + mic)',
    defaultDevice: 'Default device',
    transcriptionEngineLabel: 'Transcription engine',
    runLocal: 'Run locally',
    runLocalDesc: 'Audio is processed on this device. Max privacy, ~4GB.',
    autoDeleteAudio: 'Auto-delete audio after 30 days',
    autoDeleteAudioDesc: 'Transcript is kept. Only applies to non-exported recordings.',
    privacy: 'Privacy',
    storage: 'Storage',
    partTitle: 'Participants',
    partSub: 'Rename detected subjects. Changes apply to the whole transcript.',
    unnamed: 'Unnamed',
    secSummary: 'Executive summary',
    secDecisions: 'Key decisions',
    secMetrics: 'Metrics & KPIs',
    secActions: 'Action items',
    secRisks: 'Risks & blockers',
    secContext: 'Business context',
    secPainPoints: 'Pain points',
    secRequirements: 'Functional requirements',
    secStakeholders: 'Stakeholders',
    secAssumptions: 'Assumptions',
    secArchitecture: 'Proposed architecture',
    secTechDecisions: 'Technical decisions',
    secBlockers: 'Technical blockers',
    secDeliverables: 'Deliverables',
    secAgenda: 'Agenda',
    secKeyMessages: 'Key messages',
    secQA: 'Notable Q&A',
    secAudienceMetrics: 'Audience metrics',
    secYesterday: 'Yesterday',
    secToday: 'Today',
    secFocus: 'Daily focus',
    secWentWell: '✅ What went well',
    secWentWrong: '⚠️ What didn\'t',
    secLearnings: '💡 Learnings',
    secExperiments: '🧪 Experiments',
    secBackground: 'Background',
    secCompetencies: 'Competency assessment',
    secSignals: 'Observed signals',
    secRecommendation: 'Recommendation',
    secStatus: 'Current status',
    secFeedback: 'Feedback',
    secGoals: 'Goals',
    secFollowUp: 'Follow-up',
    secSpeakers: 'Speakers',
    secTopics: 'Topics',
    secQuotes: 'Notable quotes',
    secContacts: 'Key contacts',
    untitled: 'Untitled',
    today: 'Today', yesterday: 'Yesterday',
    minAgo: 'min', hoursAgo: 'h',
    speaking: 'Speaking',
    silence: 'Silent',
    duration: 'Duration',
    fidelity: 'Fidelity',
    new: 'New',
    play: 'Play',
    paste: 'Paste to markdown',
    timestampsOn: 'Timestamps',
    bilingual: 'Bilingual',
    fileNamePh: 'e.g. meeting-2025-11-12'
  }
};

function tr(lang, key) {
  return (I18N[lang] && I18N[lang][key]) || key;
}

function pickL(obj, lang) {
  if (!obj) return '';
  if (typeof obj === 'string') return obj;
  return obj[lang] ?? obj.es ?? obj.en ?? '';
}

function fmtDuration(sec) {
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = sec % 60;
  if (h > 0) return `${h}:${String(m).padStart(2,'0')}:${String(s).padStart(2,'0')}`;
  return `${m}:${String(s).padStart(2,'0')}`;
}

function fmtDate(iso, lang) {
  const d = new Date(iso);
  const opts = { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' };
  return d.toLocaleString(lang === 'en' ? 'en-US' : 'es-MX', opts);
}

function getTemplate(id) { return TEMPLATES.find(t => t.id === id) || TEMPLATES[0]; }

Object.assign(window, {
  TEMPLATES, AUDIO_DEVICES, SUBJECT_COLORS, MEETINGS, I18N,
  tr, pickL, fmtDuration, fmtDate, getTemplate
});
