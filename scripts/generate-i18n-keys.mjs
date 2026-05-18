#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');
const esPath = resolve(root, 'src/i18n/locales/es.json');
const outPath = resolve(root, 'src/i18n/keys.ts');

const dict = JSON.parse(readFileSync(esPath, 'utf-8'));
const keys = Object.keys(dict)
  .sort()
  .map((k) => `  | '${k}'`)
  .join('\n');

writeFileSync(
  outPath,
  `// AUTO-GENERATED — do not edit. Run pnpm generate:i18n-keys to update.\nexport type TKey =\n${keys};\n`
);
console.log(`Wrote ${Object.keys(dict).length} keys → ${outPath}`);
