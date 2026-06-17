#!/usr/bin/env node
// Forbid hardcoded user-facing JSX text nodes — everything should flow through useT().
// JSX expressions ({...}) and JSX text inside .stories.tsx / .test.tsx are allowed.
import { readFileSync, readdirSync } from 'node:fs';

const ALLOWLIST_PATTERNS = [
  /src[\\/]i18n[\\/]keys\.ts$/,
  /src[\\/]i18n[\\/]locales[\\/]/,
  /src[\\/]components[\\/]primitives[\\/]Icon[\\/]icons\.ts$/,
  /\.stories\.tsx$/,
  /\.test\.tsx?$/,
  /src[\\/]features[\\/]settings[\\/]providers\.ts$/,
  /src[\\/]features[\\/]templates[\\/]featuresFor\.ts$/,
];

// JSX text nodes: `>TEXT<` (anything between a closing-bracket and the next opening tag/brace).
// Constrained to single lines, must start with a letter, and must not contain JS-expression
// boundaries `{` `}` or stray `<` `>` (avoids false-positives across multi-line attribute
// expressions and across arrow functions `=>`).
const JSX_TEXT_REGEX = />\s*([A-Za-zÁÉÍÓÚÑáéíóúñ][^<>{}\n]*)</g;

// What we DON'T flag (numbers, separators, units, ASCII art, mono-fonted code, etc.).
const IGNORE_VALUES = /^[\s\d.,:;%·×–—\-+=()\\[\]{}|/\\&]+$/;

const args = process.argv.slice(2);
// `fs.globSync` needs Node 22+, but CI and engines target Node 20 — walk src/ instead.
const files =
  args.length > 0
    ? args.filter((f) => f.endsWith('.tsx'))
    : readdirSync('src', { recursive: true })
        .map((entry) => `src/${entry}`)
        .filter((f) => f.endsWith('.tsx'));

const problems = [];

for (const file of files) {
  const normalized = file.replace(/\\/g, '/');
  if (ALLOWLIST_PATTERNS.some((re) => re.test(normalized))) continue;

  let source;
  try {
    source = readFileSync(file, 'utf-8');
  } catch {
    continue;
  }

  JSX_TEXT_REGEX.lastIndex = 0;
  let match = JSX_TEXT_REGEX.exec(source);
  while (match !== null) {
    const raw = match[1] ?? '';
    const text = raw.trim();

    if (text.length < 3) {
      match = JSX_TEXT_REGEX.exec(source);
      continue;
    }
    if (IGNORE_VALUES.test(text)) {
      match = JSX_TEXT_REGEX.exec(source);
      continue;
    }
    // Skip if the matched chunk doesn't actually start with a letter (could be punctuation, etc.).
    if (!/[A-Za-zÁÉÍÓÚÑáéíóúñ]/.test(text[0])) {
      match = JSX_TEXT_REGEX.exec(source);
      continue;
    }

    const lineNumber = source.slice(0, match.index).split('\n').length;
    problems.push(`${file}:${lineNumber}: "${text}"`);
    match = JSX_TEXT_REGEX.exec(source);
  }
}

if (problems.length > 0) {
  console.error(
    `Hardcoded user-facing strings found (use useT() or move to an allowlisted constants module):\n${problems
      .map((p) => `  ${p}`)
      .join('\n')}`
  );
  process.exit(1);
}
console.log(`No hardcoded strings found (${files.length} file(s) scanned).`);
