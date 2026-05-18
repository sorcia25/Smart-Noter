/* Smart Noter — shared UI primitives (avatars, badges, audio bits) */
const { createElement: hh } = React;

function SubjectAvatar({ p, size = 32 }) {
  const initial = p.name
    ? p.name.split(' ').map(s => s[0]).slice(0,2).join('').toUpperCase()
    : (p.label || 'S?');
  return hh('div', {
    className: 'avatar ' + (p.colorClass || 's-color-1'),
    style: { width: size, height: size, fontSize: Math.max(10, Math.round(size * 0.38)) }
  }, initial);
}

function AvatarStack({ participants, size = 26, max = 4 }) {
  const shown = (participants || []).slice(0, max);
  const extra = (participants || []).length - max;
  return hh('div', { className: 'avatar-stack' },
    shown.map(p => hh(SubjectAvatar, { key: p.id, p, size })),
    extra > 0 ? hh('div', {
      className: 'avatar',
      style: {
        width: size, height: size,
        background: 'var(--bg-surface-active)', color: 'var(--text-muted)',
        boxShadow: '0 0 0 2px var(--bg-surface)',
        fontSize: Math.max(10, Math.round(size * 0.38))
      }
    }, `+${extra}`) : null
  );
}

function TemplateIcon({ tmplId, size = 44 }) {
  const tmpl = getTemplate(tmplId);
  return hh('div', {
    className: 'tmpl-icon ' + tmpl.colorClass,
    style: { width: size, height: size, borderRadius: Math.round(size / 4.4) }
  }, hh(Icon, { name: tmpl.icon, size: Math.round(size * 0.48), stroke: 'white' }));
}

function Toggle({ on, onChange }) {
  return hh('button', {
    className: 'toggle' + (on ? ' on' : ''),
    onClick: () => onChange && onChange(!on),
    'aria-pressed': on
  });
}

function Segmented({ value, options, onChange }) {
  return hh('div', { className: 'segmented' },
    options.map(o => hh('button', {
      key: o.value,
      className: value === o.value ? 'active' : '',
      onClick: () => onChange(o.value)
    }, o.label))
  );
}

window.SubjectAvatar = SubjectAvatar;
window.AvatarStack = AvatarStack;
window.TemplateIcon = TemplateIcon;
window.Toggle = Toggle;
window.Segmented = Segmented;
