'use strict';

// The answer window renders the conversation as a transcript: user bubbles,
// assistant bubbles with streamed markdown, inline error rows. The window
// only ever grows during a conversation (in 24px steps); the transcript
// scrolls inside it, so streaming never jitters the window edge.

const opanel = document.getElementById('opanel');
const chatEl = document.getElementById('chat');
const newChatBtn = document.getElementById('new-chat');
const hud = document.getElementById('hud');
const hudDone = document.getElementById('hud-done');
const dockToggle = document.getElementById('dock-toggle');
const dockBadge = document.getElementById('dock-badge');

let mode = 'idle';
let prevMode = 'idle';
let lastGen = 0;
let docked = true;
let renderQueued = false;
let resizeQueued = false;
let lastSentHeight = 0;
// The assistant message currently streaming: { el, body, raw }.
let liveMsg = null;

// ------------------------------------------------------------------ bubbles

function scrollToBottom(force) {
  const nearBottom = chatEl.scrollHeight - chatEl.scrollTop - chatEl.clientHeight < 48;
  if (force || nearBottom) chatEl.scrollTop = chatEl.scrollHeight;
}

function addUserMsg(text) {
  const msg = document.createElement('div');
  msg.className = 'msg user';
  const bubble = document.createElement('div');
  bubble.className = 'bubble';
  bubble.textContent = text;
  msg.appendChild(bubble);
  chatEl.appendChild(msg);
  scrollToBottom(true);
}

function addAssistantMsg() {
  const msg = document.createElement('div');
  msg.className = 'msg assistant';
  const bubble = document.createElement('div');
  bubble.className = 'bubble';
  const body = document.createElement('div');
  body.className = 'md-body';
  bubble.appendChild(body);
  msg.appendChild(bubble);

  const meta = document.createElement('div');
  meta.className = 'msg-meta';
  const copy = document.createElement('button');
  copy.type = 'button';
  copy.className = 'copy-msg';
  copy.textContent = 'Copy';
  copy.setAttribute('aria-label', 'Copy this answer');
  meta.appendChild(copy);
  msg.appendChild(meta);

  chatEl.appendChild(msg);
  const entry = { el: msg, body, raw: '' };
  copy.addEventListener('click', () => {
    if (!entry.raw) return;
    window.aeval.invoke('clipboard:write', { text: entry.raw }).then(() => {
      copy.textContent = 'Copied';
      setTimeout(() => {
        copy.textContent = 'Copy';
      }, 1500);
    });
  });
  return entry;
}

function setThinking(entry, on) {
  if (on) {
    entry.body.innerHTML =
      '<span class="think-dots" aria-label="Thinking"><i></i><i></i><i></i></span>';
  }
  entry.el.classList.toggle('thinking', on);
}

function addErrorRow(message) {
  const row = document.createElement('div');
  row.className = 'msg error-row';
  row.setAttribute('role', 'alert');
  row.innerHTML =
    '<svg viewBox="0 0 24 24" width="14" height="14" aria-hidden="true" fill="none"' +
    ' stroke="currentColor" stroke-width="2" stroke-linecap="round">' +
    '<circle cx="12" cy="12" r="10"></circle>' +
    '<line x1="15" y1="9" x2="9" y2="15"></line>' +
    '<line x1="9" y1="9" x2="15" y2="15"></line></svg>';
  const span = document.createElement('span');
  span.textContent = message;
  row.appendChild(span);
  chatEl.appendChild(row);
  scrollToBottom(true);
}

// ------------------------------------------------------------------- render

function renderLive() {
  if (!liveMsg || renderQueued) return;
  renderQueued = true;
  requestAnimationFrame(() => {
    renderQueued = false;
    if (!liveMsg) return;
    window.renderMarkdownInto(liveMsg.body, liveMsg.raw);
    scrollToBottom(false);
    requestResize();
  });
}

// Grow-only, coarse steps: the window widens its viewport as the transcript
// needs it and stays put otherwise. Shrinking happens only on New chat.
function requestResize() {
  if (mode === 'hud' || opanel.hidden || resizeQueued) return;
  resizeQueued = true;
  requestAnimationFrame(() => {
    resizeQueued = false;
    const raw = Math.ceil(opanel.getBoundingClientRect().height) + 2;
    const stepped = Math.ceil(raw / 24) * 24;
    if (stepped > lastSentHeight) {
      lastSentHeight = stepped;
      window.aeval.invoke('output:resize', { height: stepped });
    }
  });
}

new ResizeObserver(requestResize).observe(opanel);

function playEntrance() {
  opanel.classList.remove('in');
  void opanel.offsetWidth; // restart the animation
  opanel.classList.add('in');
}

// ------------------------------------------------------------------- state

window.aeval.on('overlay:state', (next) => {
  prevMode = mode;
  mode = next.mode;
  hud.hidden = mode !== 'hud';
  if (mode === 'hud') {
    opanel.hidden = true;
  }
  // Entrance origin: a docked answer grows away from the input bar's edge;
  // a free one grows from its own anchor.
  if (next.outputPos) {
    docked = next.outputPos.docked;
    const v = docked ? (next.inputAnchor && next.inputAnchor.v) || 'top' : next.outputPos.v;
    opanel.dataset.origin = v === 'bottom' ? 'bottom' : v === 'center' ? 'center' : 'top';
  }
  // Reopening the panel with an existing transcript replays the entrance.
  if (mode === 'panel' && prevMode === 'pill' && !opanel.hidden) {
    playEntrance();
  }
});

window.aeval.on('hud:docked', ({ docked: d }) => {
  dockBadge.hidden = !d;
  dockToggle.textContent = d ? 'Detach' : 'Dock to input';
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

dockToggle.addEventListener('click', () => {
  window.aeval.invoke('hud:set-dock', { docked: dockToggle.textContent !== 'Detach' });
});

// -------------------------------------------------------------------- chat

window.aeval.on('chat:user', ({ gen, text }) => {
  lastGen = gen;
  if (opanel.hidden) {
    opanel.hidden = false;
    playEntrance();
  }
  addUserMsg(text);
  requestResize();
});

window.aeval.on('query:start', ({ gen }) => {
  lastGen = gen;
  if (opanel.hidden) {
    opanel.hidden = false;
    playEntrance();
  }
  liveMsg = addAssistantMsg();
  setThinking(liveMsg, true);
  scrollToBottom(true);
  requestResize();
});

window.aeval.on('query:delta', ({ gen, text }) => {
  if (gen < lastGen || !liveMsg) return;
  lastGen = gen;
  if (text) {
    if (!liveMsg.raw) setThinking(liveMsg, false);
    liveMsg.raw += text;
    renderLive();
  }
});

window.aeval.on('query:done', ({ gen }) => {
  if (gen < lastGen) return;
  if (liveMsg) {
    if (!liveMsg.raw) {
      // Nothing streamed (for example an abort before the first token);
      // an empty bubble is noise.
      liveMsg.el.remove();
    } else {
      setThinking(liveMsg, false);
      window.renderMarkdownInto(liveMsg.body, liveMsg.raw);
    }
    liveMsg = null;
  }
  requestResize();
});

window.aeval.on('query:error', ({ gen, message }) => {
  if (gen < lastGen) return;
  if (liveMsg) {
    if (!liveMsg.raw) liveMsg.el.remove();
    else setThinking(liveMsg, false);
    liveMsg = null;
  }
  addErrorRow(message);
  requestResize();
});

window.aeval.on('chat:reset', () => {
  chatEl.innerHTML = '';
  liveMsg = null;
  lastSentHeight = 0;
  opanel.hidden = true;
});

// Rebuilds the transcript after a renderer reload (dev watcher).
window.aeval.on('chat:sync', ({ messages }) => {
  chatEl.innerHTML = '';
  liveMsg = null;
  for (const m of messages || []) {
    if (m.role === 'user') {
      addUserMsg(m.text);
    } else {
      const entry = addAssistantMsg();
      entry.raw = m.text;
      window.renderMarkdownInto(entry.body, entry.raw);
    }
  }
  if (messages && messages.length) {
    opanel.hidden = false;
    scrollToBottom(true);
    requestResize();
  }
});

newChatBtn.addEventListener('click', () => {
  window.aeval.invoke('chat:new');
});

// Links from model answers open in the default browser, main-process
// validated to http/https.
chatEl.addEventListener('click', (e) => {
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
