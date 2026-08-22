'use strict';

const pill = document.getElementById('pill');
const panel = document.getElementById('panel');
const input = document.getElementById('query-input');
const settingsBtn = document.getElementById('settings-btn');
const closeBtn = document.getElementById('close-btn');
const notice = document.getElementById('notice');
const noticeText = document.getElementById('notice-text');
const noticeSettings = document.getElementById('notice-settings');
const hud = document.getElementById('hud');
const hudDone = document.getElementById('hud-done');

let state = { mode: 'pill', hasKey: false, captureOk: true };
let watchSelector = null;
let ignoreState = true;
let resizeQueued = false;

// ----------------------------------------------------------------- render

// The panel expands out of the pill: both share the same screen anchor, so
// scaling from that corner reads as the pill growing into the panel.
function anchorOrigin() {
  const a = state.inputAnchor || { h: 'right', v: 'top' };
  const h = a.h === 'center' ? 'center' : a.h;
  const v = a.v === 'center' ? 'center' : a.v;
  return `${h} ${v}`;
}

function replayAnim(el, cls) {
  el.classList.remove(cls);
  void el.offsetWidth;
  el.classList.add(cls);
}

function render(prevMode) {
  document.body.dataset.mode = state.mode;
  pill.hidden = state.mode !== 'pill';
  panel.hidden = state.mode !== 'panel';
  hud.hidden = state.mode !== 'hud';
  if (state.mode === 'panel') {
    panel.style.transformOrigin = anchorOrigin();
    if (prevMode !== 'panel') replayAnim(panel, 'in');
    renderNotice();
    if (prevMode !== 'panel') {
      input.focus();
      input.select();
    }
    requestResize();
  }
  if (state.mode === 'pill' && prevMode !== 'pill') {
    pill.style.transformOrigin = anchorOrigin();
    replayAnim(pill, 'in');
  }
}

function renderNotice() {
  if (!state.hasKey) {
    noticeText.textContent = 'No OpenRouter API key saved yet.';
    noticeSettings.hidden = false;
    notice.hidden = false;
  } else if (!state.captureOk) {
    noticeText.textContent =
      'Screen capture failed. Your question will be sent without a screenshot.';
    noticeSettings.hidden = true;
    notice.hidden = false;
  } else {
    notice.hidden = true;
  }
}

function requestResize() {
  if (state.mode !== 'panel' || resizeQueued) return;
  resizeQueued = true;
  requestAnimationFrame(() => {
    resizeQueued = false;
    const height = Math.ceil(panel.getBoundingClientRect().height) + 2;
    window.aeval.invoke('overlay:resize', { height });
  });
}

new ResizeObserver(requestResize).observe(panel);

// ------------------------------------------------------------------ query

function submit() {
  const question = input.value;
  // Clear immediately, like a messaging app; the transcript echoes the
  // message. Without a key the text stays put so nothing typed is lost.
  if (state.hasKey) input.value = '';
  window.aeval.invoke('query:submit', { question }).then((result) => {
    if (result && result.status === 'walkthrough') {
      input.value = '';
    }
  });
}

// ------------------------------------------------------------------ state

window.aeval.on('overlay:state', (next) => {
  const prevMode = state.mode;
  state = { ...state, ...next };
  render(prevMode);
});

let snapFlashTimer = null;
window.aeval.on('hud:snapped', () => {
  hud.classList.add('snapped');
  clearTimeout(snapFlashTimer);
  snapFlashTimer = setTimeout(() => hud.classList.remove('snapped'), 350);
});

// ------------------------------------------------------------ interactions

pill.addEventListener('click', () => {
  window.aeval.invoke('overlay:trigger');
});
pill.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault();
    window.aeval.invoke('overlay:trigger');
  }
});

settingsBtn.addEventListener('click', () => {
  window.aeval.invoke('overlay:open-settings');
});
noticeSettings.addEventListener('click', () => {
  window.aeval.invoke('overlay:open-settings');
});
closeBtn.addEventListener('click', () => {
  window.aeval.invoke('overlay:collapse');
});
hudDone.addEventListener('click', () => {
  window.aeval.invoke('hud:exit');
});

input.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    e.preventDefault();
    submit();
  }
});

// Window-level fallback for Escape. While the panel is open Escape is a
// global shortcut handled in the main process; this covers the rare case
// where that registration failed.
window.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    window.aeval.invoke('overlay:collapse');
  }
});

// Click-through management while idle: the window ignores mouse events with
// forwarding on, so mousemove still arrives here. Hit-testing turns back on
// only while the cursor is over the pill.
function setIgnore(ignore) {
  if (ignore === ignoreState) return;
  ignoreState = ignore;
  window.aeval.invoke('overlay:set-ignore', { ignore });
}

document.addEventListener('mousemove', (e) => {
  if (state.mode !== 'pill') return;
  const el = document.elementFromPoint(e.clientX, e.clientY);
  setIgnore(!(el && el.closest('#pill')));
});

document.addEventListener('mouseleave', () => {
  if (state.mode === 'pill') setIgnore(true);
});

// ---------------------------------------------------------- cursor buddy

window.aeval.on('buddy:watch', ({ selector }) => {
  watchSelector = selector;
});

window.aeval.on('buddy:resolve', ({ selector }) => {
  const el = document.querySelector(selector);
  if (!el || el.hidden || el.closest('[hidden]')) {
    window.aeval.invoke('buddy:resolve-result', { found: false });
    return;
  }
  const r = el.getBoundingClientRect();
  window.aeval.invoke('buddy:resolve-result', {
    found: true,
    cx: window.screenX + r.left + r.width / 2,
    cy: window.screenY + r.top + r.height / 2,
    w: r.width,
    h: r.height,
  });
});

// Clicking the highlighted target lets the walkthrough advance naturally;
// clicking anywhere else dismisses it so it never traps the user.
document.addEventListener('click', (e) => {
  if (!watchSelector) return;
  if (!e.target.closest(watchSelector)) {
    window.aeval.invoke('buddy:dismiss');
  }
});
