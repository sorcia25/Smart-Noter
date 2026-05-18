import type { IconName } from '@/components/primitives/Icon/Icon';
import type { Bilingual } from '@/ipc/bindings';

export type ProviderId = 'local' | 'openai' | 'azure' | 'custom';

export interface ProviderMeta {
  id: ProviderId;
  icon: IconName;
  color: string;
  name: Bilingual;
  short: string;
  desc: Bilingual;
  badge: Bilingual;
  badgeAccent?: boolean;
  models: readonly string[];
}

export const PROVIDERS: readonly ProviderMeta[] = [
  {
    id: 'local',
    icon: 'cpu',
    color: '#10b981',
    name: { es: 'Local (en este equipo)', en: 'Local (on this device)' },
    short: 'Local',
    desc: {
      es: 'Procesa el audio en tu PC con Whisper. Máxima privacidad, sin costos por minuto.',
      en: 'Processes audio on your PC with Whisper. Max privacy, no per-minute cost.',
    },
    badge: { es: 'Predeterminado · privado', en: 'Default · private' },
    badgeAccent: true,
    models: ['Whisper Large v3', 'Whisper Large v3 Turbo', 'Whisper Medium', 'Distil-Whisper'],
  },
  {
    id: 'openai',
    icon: 'sparkles',
    color: '#1aaf8b',
    name: { es: 'OpenAI API', en: 'OpenAI API' },
    short: 'OpenAI',
    desc: {
      es: 'Usa los modelos de OpenAI (gpt-4o-transcribe, whisper-1) vía API.',
      en: 'Use OpenAI models (gpt-4o-transcribe, whisper-1) via API.',
    },
    badge: { es: 'Requiere API key', en: 'API key required' },
    models: ['gpt-4o-transcribe', 'gpt-4o-mini-transcribe', 'whisper-1'],
  },
  {
    id: 'azure',
    icon: 'cpu',
    color: '#0078d4',
    name: { es: 'Azure OpenAI / Speech', en: 'Azure OpenAI / Speech' },
    short: 'Azure',
    desc: {
      es: 'Modelos desplegados en tu tenant de Azure. Cumplimiento empresarial y región configurable.',
      en: 'Models deployed in your Azure tenant. Enterprise compliance and configurable region.',
    },
    badge: { es: 'Enterprise · residencia de datos', en: 'Enterprise · data residency' },
    models: ['whisper (Azure)', 'gpt-4o-transcribe', 'Azure Speech-to-Text'],
  },
  {
    id: 'custom',
    icon: 'sliders',
    color: '#8b5cf6',
    name: { es: 'Endpoint personalizado', en: 'Custom endpoint' },
    short: 'Custom',
    desc: {
      es: 'Apunta a cualquier servicio compatible con la API de OpenAI (Groq, Together, on-prem).',
      en: 'Point to any OpenAI-compatible service (Groq, Together, on-prem).',
    },
    badge: { es: 'Avanzado', en: 'Advanced' },
    models: ['Detectar automáticamente'],
  },
] as const;
