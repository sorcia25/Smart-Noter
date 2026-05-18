#!/usr/bin/env node
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import vm from 'node:vm';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '..');
const dataJsPath = resolve(root, 'handoff/smart-noter/project/data.js');
const outPath = resolve(root, 'src-tauri/crates/db/seed_data.json');

const source = readFileSync(dataJsPath, 'utf-8');

// The prototype's data.js attaches symbols to `window`. Run it in a sandbox with a fake window.
const sandbox = { window: {} };
vm.createContext(sandbox);
vm.runInContext(source, sandbox);

const win = sandbox.window;
const out = {
  templates: win.TEMPLATES,
  meetings: win.MEETINGS,
  audioDevices: win.AUDIO_DEVICES,
};

mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, JSON.stringify(out, null, 2) + '\n');
console.log(`Wrote ${out.meetings.length} meetings, ${out.templates.length} templates → ${outPath}`);
