'use strict';

const opanel = document.getElementById('opanel');
const response = document.getElementById('response');
const streamState = document.getElementById('stream-state');
const copyBtn = document.getElementById('copy-response');
const errorBox = document.getElementById('error');
const errorText = document.getElementById('error-text');
const hud = document.getElementById('hud');
const hudDone = document.getElementById('hud-done');
const dockBadge = document.getElementById('dock-badge');

let mode = 'idle';
let lastGen = 0;
let rawText = '';
let renderQueued = false;
let resizeQueued = false;
let lastResizeAt = 0;
let lastSentHeight = 0;

// ----------------------------------------------------------------- render

function renderResponse() {
  if (renderQueued) return;
  renderQueued = true;
  requestAnimationFrame(() => {
    renderQueued = false;
    const nearBottom =
      response.scrollHeight - response.scrollTop - response.clientHeight < 40;
    window.renderMarkdownInto(response, rawText);
    if (nearBottom) response.scrollTop = response.scrollHeight;
    requestResize();
  });
}

// Height updates are throttled and rounded so the window does not jitter on
// every streamed token.
function requestResize() {
  if (mode === 'hud' || opanel.hidden || resizeQueued) return;
  const now = Date.now();
  const wait = Math.max(0, 80 - (now - lastResizeAt));
  resizeQueued = true;
  setTimeout(() => {
    resizeQueued = false;
    lastResizeAt = Date.now();
    const raw = Math.ceil(opanel.getBoundingClientRect().height) + 2;
    const stepped = Math.ceil(raw / 6) * 6;
    if (stepped !== lastSentHeight) {
      lastSentHeight = stepped;
      window.aeval.invoke('output:resize', { height: stepped });
    }
  }, wait);
}

new ResizeObserver(requestResize).observe(opanel);

// ------------------------------------------------------------------ state

window.aeval.on('overlay:state', (next) => {
  mode = next.mode;
  hud.hidden = mode !== 'hud';
  if (mode === 'hud') {
    opanel.hidden = true;
  }
});

window.aeval.on('hud:docked', ({ docked }) => {
  dockBadge.hidden = !docked;
});

let snapFlashTimer = null;
window.aeval.on('hud:snapped', () => {
  hud.classList.add('snapped');
  clearTimeout(snapFlashTimer);
  snapFlashTimer = setTimeout(() => hud.classList.remove('snapped'), 350);
});

hudDone.addEventListener('click', () => {
  window.aeval.invoke('hud:exit');
});

// ------------------------------------------------------------------ query

window.aeval.on('query:start', ({ gen }) => {
  lastGen = gen;
  rawText = '';
  lastSentHeight = 0;
  opanel.hidden = false;
  errorBox.hidden = true;
  response.textContent = '';
  streamState.textContent = 'Thinking...';
  requestResize();
});

window.aeval.on('query:delta', ({ gen, text }) => {
  if (gen < lastGen) return;
  lastGen = gen;
  if (text) {
    if (!rawText) streamState.textContent = 'Streaming...';
    rawText += text;
    renderResponse();
  }
});

window.aeval.on('query:done', ({ gen }) => {
  if (gen < lastGen) return;
  streamState.textContent = '';
  if (!rawText) opanel.hidden = true;
  requestResize();
});

window.aeval.on('query:error', ({ gen, message }) => {
  if (gen < lastGen) return;
  streamState.textContent = '';
  errorText.textContent = message;
  errorBox.hidden = false;
  opanel.hidden = false;
  requestResize();
});

// ------------------------------------------------------------------- copy

let copyFlashTimer = null;
copyBtn.addEventListener('click', () => {
  if (!rawText) return;
  window.aeval.invoke('clipboard:write', { text: rawText }).then(() => {
    copyBtn.textContent = 'Copied.';
    clearTimeout(copyFlashTimer);
    copyFlashTimer = setTimeout(() => {
      copyBtn.textContent = 'Copy';
    }, 1600);
  });
});

// Links from model answers open in the default browser, main-process
// validated to http/https.
response.addEventListener('click', (e) => {
  const a = e.target.closest('a[data-href]');
  if (!a) return;
  e.preventDefault();
  window.aeval.invoke('open-external', { url: a.dataset.href });
});

// Escape fallback mirrors the input window; the global shortcut normally
// handles it first.
window.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    window.aeval.invoke('overlay:collapse');
  }
});
