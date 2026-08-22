'use strict';

const pill = document.getElementById('pill');
const panel = document.getElementById('panel');
const input = document.getElementById('query-input');
const settingsBtn = document.getElementById('settings-btn');
const closeBtn = document.getElementById('close-btn');
const notice = document.getElementById('notice');
const noticeText = document.getElementById('notice-text');
const noticeSettings = document.getElementById('notice-settings');
const errorBox = document.getElementById('error');
const errorText = document.getElementById('error-text');
const response = document.getElementById('response');
const hintCapture = document.getElementById('hint-capture');

let state = { mode: 'pill', hasKey: false, captureOk: true, shortcuts: {} };
let busy = false;
let lastGen = 0;
let watchSelector = null;
let ignoreState = true;
let resizeQueued = false;

// ----------------------------------------------------------------- render

function render(prevMode) {
  document.body.dataset.mode = state.mode;
  pill.hidden = state.mode !== 'pill';
  panel.hidden = state.mode !== 'panel';
  if (state.shortcuts.captureQuery) {
    hintCapture.textContent = state.shortcuts.captureQuery;
  }
  if (state.mode === 'panel') {
    renderNotice();
    if (prevMode !== 'panel') {
      resetConversation();
      input.focus();
      input.select();
    }
    requestResize();
  }
}

function renderNotice() {
  if (busy) {
    notice.hidden = true;
    return;
  }
  if (!state.hasKey) {
    noticeText.textContent = 'No API key saved yet.';
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

function resetConversation() {
  response.hidden = true;
  response.textContent = '';
  response.classList.remove('thinking');
  errorBox.hidden = true;
  busy = false;
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
  errorBox.hidden = true;
  response.hidden = false;
  response.textContent = 'Thinking...';
  response.classList.add('thinking');
  busy = true;
  renderNotice();
  requestResize();
  window.aeval.invoke('query:submit', { question }).then((result) => {
    if (!result) return;
    if (result.status === 'needs-key' || result.status === 'walkthrough') {
      busy = false;
      response.hidden = true;
      response.textContent = '';
      response.classList.remove('thinking');
      if (result.status === 'walkthrough') input.value = '';
      renderNotice();
      requestResize();
    }
  });
}

window.aeval.on('query:delta', ({ gen, text }) => {
  if (gen < lastGen) return;
  if (gen > lastGen) {
    lastGen = gen;
    response.textContent = '';
    response.classList.remove('thinking');
    response.hidden = false;
  }
  if (text) {
    response.textContent += text;
    response.scrollTop = response.scrollHeight;
    requestResize();
  }
});

window.aeval.on('query:done', ({ gen }) => {
  if (gen < lastGen) return;
  busy = false;
  response.classList.remove('thinking');
  if (!response.textContent) response.hidden = true;
  renderNotice();
  requestResize();
});

window.aeval.on('query:error', ({ gen, message }) => {
  if (gen < lastGen) return;
  busy = false;
  response.classList.remove('thinking');
  if (response.textContent === 'Thinking...') {
    response.hidden = true;
    response.textContent = '';
  }
  errorText.textContent = message;
  errorBox.hidden = false;
  renderNotice();
  requestResize();
});

// ------------------------------------------------------------------ state

window.aeval.on('overlay:state', (next) => {
  const prevMode = state.mode;
  state = { ...state, ...next };
  render(prevMode);
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
