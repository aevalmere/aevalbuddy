// The bar's mic (entries/voice.md). Click to start and click again to stop,
// or hold it and let go; Esc stops without keeping anything. The recording
// and the transcription happen in Rust (src-tauri/src/voice); the words land
// in the field at the caret, and the field's own input handling then guesses
// Ask, Find or Do as if they had been typed.
(() => {
  const btn = document.getElementById('mic-btn');
  const field = document.getElementById('query-input');
  if (!btn || !field || !window.aeval) return;

  // A press longer than this is push-to-talk: letting go stops.
  const HOLD_MS = 350;
  const LABELS = {
    idle: 'Speak instead of typing',
    listening: 'Stop listening and use what I said',
    working: 'Writing down what you said',
    preparing: 'Getting voice input ready',
    error: 'Speak instead of typing',
  };
  const placeholder = field.getAttribute('placeholder') || '';
  let state = 'idle';
  let pressedAt = 0;
  let noteTimer = 0;

  const say = (text, priority) => {
    if (window.AevalA11y) window.AevalA11y.say(text, priority === 'assertive' ? 'assertive' : 'polite');
  };

  function set(next, label) {
    state = next;
    btn.dataset.state = next;
    btn.setAttribute('aria-pressed', String(next === 'listening'));
    const name = label || LABELS[next] || LABELS.idle;
    btn.setAttribute('aria-label', name);
    btn.title = name;
    if (next !== 'listening') btn.style.setProperty('--l', '0');
  }

  // An error shows where the words would have gone, for a few seconds.
  function note(text) {
    clearTimeout(noteTimer);
    if (!field.value) field.setAttribute('placeholder', text);
    noteTimer = setTimeout(() => field.setAttribute('placeholder', placeholder), 4000);
  }

  function start() {
    if (state === 'listening' || state === 'working' || state === 'preparing') return;
    clearTimeout(noteTimer);
    field.setAttribute('placeholder', placeholder);
    set('listening');
    window.aeval.invoke('voice:start').then((r) => {
      if (r && r.ok === false && state === 'listening') {
        set('error');
        note(r.error);
        say(r.error, 'assertive');
      }
    }, () => set('idle'));
  }

  function stop() {
    if (state !== 'listening') return;
    set('working');
    window.aeval.invoke('voice:stop').catch(() => set('idle'));
  }

  function cancel(announce) {
    if (state === 'idle' || state === 'error') return;
    set('idle');
    window.aeval.invoke('voice:cancel').catch(() => {});
    if (announce) say('Stopped listening.');
  }

  // The words go in at the caret, with a space between them and any text
  // already there, then the field hears an input event as for typing.
  function put(text) {
    const s = field.selectionStart ?? field.value.length;
    const e = field.selectionEnd ?? s;
    const before = field.value.slice(0, s);
    const lead = before && !/\s$/.test(before) ? ' ' : '';
    field.focus();
    field.setRangeText(lead + text, s, e, 'end');
    field.dispatchEvent(new Event('input', { bubbles: true }));
  }

  btn.addEventListener('pointerdown', (e) => {
    if (e.button !== 0) return;
    e.preventDefault(); // keep the caret in the field
    pressedAt = state === 'listening' ? 0 : Date.now();
    if (state === 'listening') stop();
    else start();
  });
  btn.addEventListener('pointerup', () => {
    if (pressedAt && Date.now() - pressedAt >= HOLD_MS) stop();
    pressedAt = 0;
  });
  btn.addEventListener('pointercancel', () => {
    pressedAt = 0;
  });
  // Keyboard, switch access and screen readers click without a pointer.
  btn.addEventListener('click', (e) => {
    if (e.detail !== 0) return;
    if (state === 'listening') stop();
    else start();
  });

  // Before the field's own Esc, which would clear it or close the bar.
  window.addEventListener('keydown', (e) => {
    if (e.key !== 'Escape' || state === 'idle' || state === 'error') return;
    e.preventDefault();
    e.stopImmediatePropagation();
    cancel(true);
  }, true);

  window.aeval.on('voice:level', (p) => {
    if (p && p.origin === 'bar' && state === 'listening') {
      btn.style.setProperty('--l', String(Math.max(0, Math.min(1, Number(p.level) || 0)).toFixed(2)));
    }
  });

  window.aeval.on('voice:progress', (p) => {
    if (p && p.origin === 'bar' && state === 'preparing') {
      const label = `Getting voice input ready, ${p.progress}%`;
      btn.setAttribute('aria-label', label);
      btn.title = label;
    }
  });

  window.aeval.on('voice:state', (p) => {
    if (!p || p.origin !== 'bar') return;
    switch (p.state) {
      case 'listening':
        set('listening');
        say('Listening. Press the mic or Esc to stop.');
        break;
      case 'working':
      case 'cleaning':
        if (state !== 'idle') set('working');
        break;
      case 'preparing':
        if (state === 'idle') break;
        set('preparing');
        say('Downloading the voice model. This happens once.');
        break;
      case 'done':
        if (state === 'idle') break;
        set('idle');
        if (p.text) {
          put(p.text);
          say(p.text);
        }
        break;
      case 'error':
        set('error');
        note(p.message);
        say(p.message, 'assertive');
        break;
      default:
        set('idle');
    }
  });

  // The bar closing ends an utterance it started.
  window.aeval.on('overlay:state', (s) => {
    if (s && s.mode && s.mode !== 'panel') cancel(false);
  });

  set('idle');
})();
