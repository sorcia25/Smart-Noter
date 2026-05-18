export function shade(hex: string, pct: number): string {
  const c = Number.parseInt(hex.slice(1), 16);
  let r = (c >> 16) & 0xff;
  let g = (c >> 8) & 0xff;
  let b = c & 0xff;
  const adj = (v: number) =>
    Math.max(0, Math.min(255, Math.round(v + (pct < 0 ? v * pct : (255 - v) * pct))));
  r = adj(r);
  g = adj(g);
  b = adj(b);
  return `#${[r, g, b].map((x) => x.toString(16).padStart(2, '0')).join('')}`;
}

export function applyAccent(hex: string, root: HTMLElement = document.documentElement) {
  root.style.setProperty('--accent', hex);
  root.style.setProperty('--accent-hover', shade(hex, -0.1));
  root.style.setProperty('--accent-press', shade(hex, -0.2));
  const c = Number.parseInt(hex.slice(1), 16);
  const r = (c >> 16) & 0xff;
  const g = (c >> 8) & 0xff;
  const b = c & 0xff;
  root.style.setProperty('--accent-soft', `rgba(${r},${g},${b},0.12)`);
  root.style.setProperty('--accent-ring', `rgba(${r},${g},${b},0.35)`);
}
