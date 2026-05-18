/* Smart Noter — Pre-record + Live screens */
const { createElement: hP, useState: useStateP, useEffect: useEffectP, useRef: useRefP } = React;

function PreRecord({ lang, t, navigate, initial }) {
  const [device, setDevice] = useStateP(AUDIO_DEVICES[0].id);
  const [template, setTemplate] = useStateP((initial && initial.template) || 'tecnica');
  const [name, setName] = useStateP('');
  const [autoId, setAutoId] = useStateP(true);
  const [detectLang, setDetectLang] = useStateP(true);
  const [saveAudio, setSaveAudio] = useStateP(true);

  return hP('div', { className: 'win-main', 'data-screen-label': '03 Pre-record' },
    hP('div', { className: 'page-header', style: { paddingBottom: 0 } },
      hP('div', null,
        hP('button', { className: 'btn btn-ghost', onClick: () => navigate('dashboard'), style: { padding: 4, marginBottom: 8 } },
          hP(Icon, { name: 'chevLeft', size: 14 }), lang === 'es' ? 'Volver' : 'Back'),
        hP('h1', { className: 'page-title' }, t('preTitle')),
        hP('div', { className: 'page-sub' }, t('preSub'))
      )
    ),
    hP('div', { className: 'page-scroll scroll' },
      hP('div', { className: 'prerec', style: { padding: '8px 0 40px' } },
        // Name
        hP('div', null,
          hP('label', { className: 'field-label' }, t('meetingNameLabel')),
          hP('input', {
            className: 'input', placeholder: t('meetingNamePh'),
            value: name, onChange: e => setName(e.target.value),
            style: { fontSize: 14, padding: '11px 14px' }
          })
        ),
        // Device
        hP('h2', null, t('deviceSection')),
        hP('div', { className: 'text-sm text-muted', style: { marginTop: -6, marginBottom: 8 } }, t('deviceHint')),
        hP('div', { className: 'grid-2' },
          AUDIO_DEVICES.map(d => hP('button', {
            key: d.id,
            className: 'opt-card' + (device === d.id ? ' selected' : ''),
            onClick: () => setDevice(d.id)
          },
            hP('div', { className: 'icon-box' }, hP(Icon, { name: d.icon, size: 18 })),
            hP('div', { className: 'meta' },
              hP('div', { className: 'name' },
                pickL(d.name, lang),
                d.recommended ? hP('span', { className: 'chip chip-accent', style: { marginLeft: 8 } }, lang === 'es' ? 'Recomendado' : 'Recommended') : null
              ),
              hP('div', { className: 'desc' }, pickL(d.desc, lang))
            ),
            hP('div', { className: 'radio' })
          ))
        ),
        // Audio level meter live
        hP('div', { className: 'card card-pad', style: { marginTop: 12 } },
          hP('div', { className: 'flex items-center justify-between', style: { marginBottom: 8 } },
            hP('div', null,
              hP('div', { style: { fontSize: 13, fontWeight: 600 } }, lang === 'es' ? 'Vista previa del audio' : 'Audio preview'),
              hP('div', { className: 'text-xs text-subtle', style: { marginTop: 2 } }, lang === 'es' ? 'Reproduce algo en tu PC para verificar la señal' : 'Play something on your PC to check the signal')),
            hP('div', { className: 'eq-bar', style: { height: 22 } }, [1,2,3,4,5,6,7,8].map(i => hP('span', { key: i })))
          ),
          hP('div', { className: 'level-bar' }, hP('div', { className: 'fill', style: { width: '54%' } }))
        ),

        // Template
        hP('h2', null, t('templateSection')),
        hP('div', { className: 'text-sm text-muted', style: { marginTop: -6, marginBottom: 8 } }, t('templateHint')),
        hP('div', { className: 'tmpl-grid' },
          TEMPLATES.map(T => hP('button', {
            key: T.id,
            className: 'tmpl-card' + (template === T.id ? ' selected' : ''),
            onClick: () => setTemplate(T.id)
          },
            template === T.id ? hP('div', { className: 'check' }, hP(Icon, { name: 'check', size: 12, stroke: 'white' })) : null,
            hP('div', { className: 'icon ' + T.colorClass }, hP(Icon, { name: T.icon, size: 18, stroke: 'white' })),
            hP('div', { className: 'name' }, pickL(T.name, lang)),
            hP('div', { className: 'desc' }, pickL(T.desc, lang))
          ))
        ),

        // Advanced
        hP('h2', null, t('advancedSection')),
        hP('div', { className: 'card' },
          [
            { id: 'autoId', label: t('autoIdSpeakers'), desc: t('autoIdSpeakersDesc'), value: autoId, set: setAutoId },
            { id: 'detect', label: t('detectLang'), desc: t('detectLangDesc'), value: detectLang, set: setDetectLang },
            { id: 'save', label: t('saveAudio'), desc: t('saveAudioDesc'), value: saveAudio, set: setSaveAudio }
          ].map(opt => hP('div', { key: opt.id, className: 'setting-row' },
            hP('div', null,
              hP('div', { className: 'label' }, opt.label),
              hP('div', { className: 'desc' }, opt.desc)),
            hP(Toggle, { on: opt.value, onChange: opt.set })
          ))
        ),

        hP('div', { className: 'prerec-footer' },
          hP('div', { className: 'flex items-center gap-2' },
            hP(Icon, { name: 'shield', size: 16, stroke: 'var(--text-muted)' }),
            hP('div', { className: 'text-xs text-muted' }, lang === 'es' ? 'Procesamiento 100% local. Tu audio nunca sale de tu PC.' : '100% local processing. Your audio never leaves your PC.')
          ),
          hP('div', { className: 'flex gap-2' },
            hP('button', { className: 'btn', onClick: () => navigate('dashboard') }, t('cancel')),
            hP('button', { className: 'btn btn-primary', onClick: () => navigate('live', { template, name: name || (lang === 'es' ? 'Reunión sin título' : 'Untitled meeting') }) },
              hP(Icon, { name: 'record', size: 12 }), t('startRecording'))
          )
        )
      )
    )
  );
}

function LiveRecording({ lang, t, navigate, sessionMeta }) {
  const [elapsed, setElapsed] = useStateP(143); // start at 2:23
  const [paused, setPaused] = useStateP(false);
  useEffectP(() => {
    if (paused) return;
    const id = setInterval(() => setElapsed(e => e + 1), 1000);
    return () => clearInterval(id);
  }, [paused]);

  // Generate stable waveform heights
  const bars = useRefP(null);
  if (!bars.current) {
    bars.current = Array.from({ length: 36 }, () => 0.25 + Math.random() * 0.75);
  }

  const device = AUDIO_DEVICES.find(d => d.id === 'system-loopback') || AUDIO_DEVICES[0];
  const tmpl = getTemplate((sessionMeta && sessionMeta.template) || 'tecnica');

  return hP('div', { className: 'win-main', 'data-screen-label': '04 Live recording' },
    hP('div', { className: 'live-wrap' },
      hP('div', { className: 'live-header' },
        hP('div', { className: 'flex items-center gap-3' },
          hP('div', { className: 'live-pill' },
            hP('div', { className: 'rec-dot' }), t('liveStatus')),
          hP('div', null,
            hP('div', { style: { fontSize: 15, fontWeight: 600 } }, (sessionMeta && sessionMeta.name) || (lang === 'es' ? 'Reunión sin título' : 'Untitled meeting')),
            hP('div', { className: 'text-xs text-muted', style: { marginTop: 2, display: 'flex', alignItems: 'center', gap: 6 } },
              hP(TemplateIcon, { tmplId: tmpl.id, size: 14 }),
              hP('span', null, pickL(tmpl.name, lang))))
        ),
        hP('div', { className: 'flex items-center gap-3' },
          hP('div', { className: 'chip' },
            hP('div', { style: { width: 6, height: 6, borderRadius: '50%', background: 'var(--accent)' } }),
            t('transcriptionEngine')),
          hP('button', { className: 'btn btn-icon' }, hP(Icon, { name: 'settings', size: 16 }))
        )
      ),
      hP('div', { className: 'live-stage' },
        hP('div', { className: 'live-center' },
          hP('div', { className: 'live-timer' }, fmtDuration(elapsed)),
          hP('div', { className: 'live-status' },
            paused ? (lang === 'es' ? 'Pausado' : 'Paused') : t('speaking') + ' — ' + (lang === 'es' ? 'Sujeto 2' : 'Subject 2')),
          hP('div', { className: 'waveform' },
            bars.current.map((b, i) => hP('span', {
              key: i,
              style: {
                height: `${Math.round((paused ? 0.2 : b) * 100)}%`,
                animationDelay: `${(i * 60) % 1200}ms`,
                opacity: paused ? 0.3 : 1
              }
            }))
          ),
          hP('div', { className: 'live-controls' },
            hP('button', { className: 'live-btn', onClick: () => setPaused(p => !p), title: paused ? 'Resume' : t('livePauseHint') },
              hP(Icon, { name: paused ? 'play' : 'pause', size: 22 })),
            hP('button', {
              className: 'live-btn live-btn-stop',
              onClick: () => navigate('meeting', { id: 'm-001' }),
              title: t('liveStopHint')
            }, hP(Icon, { name: 'stop', size: 22, stroke: 'white' })),
            hP('button', { className: 'live-btn', title: 'Flag' },
              hP(Icon, { name: 'flag', size: 18 }))
          )
        )
      ),
      hP('div', { className: 'live-meta' },
        hP('div', { className: 'live-meta-block' },
          hP(Icon, { name: device.icon, size: 14 }),
          hP('span', null, t('sourceLabel') + ': '),
          hP('span', { style: { fontWeight: 600, color: 'var(--text)' } }, pickL(device.name, lang))),
        hP('div', { className: 'live-meta-block' },
          hP(Icon, { name: 'user', size: 14 }),
          hP('span', null, t('speakersDetected') + ': '),
          hP(AvatarStack, { participants: [
            { id: 's1', label: 'S1', colorClass: 's-color-1' },
            { id: 's2', label: 'S2', colorClass: 's-color-2' },
            { id: 's3', label: 'S3', colorClass: 's-color-3' }
          ], size: 22 })),
        hP('div', { className: 'live-meta-block' },
          hP(Icon, { name: 'globe', size: 14 }),
          hP('span', null, 'ES · '), hP('span', { className: 'text-subtle' }, 'auto')),
        hP('div', { className: 'live-meta-block' },
          hP(Icon, { name: 'shield', size: 14 }),
          hP('span', null, lang === 'es' ? 'Local · cifrado' : 'Local · encrypted'))
      )
    )
  );
}

window.PreRecord = PreRecord;
window.LiveRecording = LiveRecording;
