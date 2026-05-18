#!/usr/bin/env node
// Enforce: every primitive component under src/components/primitives/<Name>/ has a
// matching <Name>.stories.tsx file so Storybook stays in sync with the codebase.
import { existsSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';

const primitivesDir = 'src/components/primitives';
const entries = readdirSync(primitivesDir);
const missing = [];

for (const name of entries) {
  const dir = path.join(primitivesDir, name);
  if (!statSync(dir).isDirectory()) continue;
  const story = path.join(dir, `${name}.stories.tsx`);
  if (!existsSync(story)) missing.push(name);
}

if (missing.length > 0) {
  console.error(`Primitives missing .stories.tsx:\n  ${missing.join('\n  ')}`);
  process.exit(1);
}

const count = entries.filter((n) => statSync(path.join(primitivesDir, n)).isDirectory()).length;
console.log(`All primitives have stories (${count} components).`);
