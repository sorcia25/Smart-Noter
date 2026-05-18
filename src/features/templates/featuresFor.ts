// Per-template feature bullets shown on the gallery cards.
// Ported verbatim from handoff/.../screens-other.jsx:50-63.

type Lang = 'es' | 'en';

const FEATURES: Record<string, Record<Lang, readonly string[]>> = {
  ejecutiva: {
    es: [
      'Resumen ejecutivo en máx. 5 viñetas',
      'Decisiones con responsable y fecha',
      'Tabla de KPIs y métricas',
      'Riesgos y acciones críticas',
    ],
    en: [
      'Exec summary, 5 bullets max',
      'Decisions with owner & date',
      'KPI table',
      'Risks & critical actions',
    ],
  },
  discovery: {
    es: [
      'Pain points priorizados',
      'Requerimientos funcionales',
      'Mapa de stakeholders',
      'Supuestos a validar',
    ],
    en: [
      'Prioritized pain points',
      'Functional requirements',
      'Stakeholder map',
      'Assumptions to validate',
    ],
  },
  tecnica: {
    es: [
      'Diagrama de arquitectura ASCII',
      'ADRs (decisiones técnicas)',
      'Bloqueos con severidad',
      'Entregables con DoD',
    ],
    en: [
      'ASCII architecture diagram',
      'ADRs (tech decisions)',
      'Blockers with severity',
      'Deliverables with DoD',
    ],
  },
  webinar: {
    es: [
      'Agenda y duración por bloque',
      'Mensajes clave del speaker',
      'Q&A más relevantes',
      'Métricas de asistencia',
    ],
    en: [
      'Agenda with block timing',
      'Speaker key messages',
      'Most relevant Q&A',
      'Attendance metrics',
    ],
  },
  daily: {
    es: [
      'Ayer/Hoy/Bloqueos por persona',
      'Foco diario destacado',
      'Acciones <24h',
      'Tiempo total y participación',
    ],
    en: [
      'Yesterday/Today/Blockers per person',
      'Daily focus highlight',
      '<24h actions',
      'Total time & participation',
    ],
  },
  retro: {
    es: [
      'Funcionó / no funcionó / aprendizajes',
      'Experimentos a probar',
      'Compromisos del equipo',
      'Voto cuantitativo de items',
    ],
    en: [
      "Worked / didn't / learnings",
      'Experiments to try',
      'Team commitments',
      'Quantitative item voting',
    ],
  },
  entrevista: {
    es: [
      'Background estructurado',
      'Evaluación por competencia 1-5',
      'Señales fuertes/débiles',
      'Recomendación final',
    ],
    en: [
      'Structured background',
      '1-5 competency scoring',
      'Strong/weak signals',
      'Final recommendation',
    ],
  },
  coaching: {
    es: [
      'Estado emocional y profesional',
      'Feedback bidireccional',
      'Objetivos SMART',
      'Plan de acompañamiento',
    ],
    en: ['Emotional & professional status', 'Two-way feedback', 'SMART goals', 'Follow-up plan'],
  },
  conferencia: {
    es: [
      'Ponentes y temas',
      'Citas destacadas con timestamp',
      'Contactos clave a seguir',
      'Recursos mencionados',
    ],
    en: [
      'Speakers and topics',
      'Notable quotes with timestamp',
      'Key contacts to follow',
      'Resources mentioned',
    ],
  },
};

export function featuresFor(templateId: string, lang: Lang): readonly string[] {
  return FEATURES[templateId]?.[lang] ?? [];
}
