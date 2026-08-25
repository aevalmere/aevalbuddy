'use strict';

// The answer window renders the conversation as a transcript: user bubbles,
// assistant bubbles with streamed markdown, inline error rows. The window
// only ever grows during a conversation (in 24px steps); the transcript
// scrolls inside it, so streaming never jitters the window edge.

const opanel = document.getElementById('opanel');
const chatEl = document.getElementById('chat');
const newChatBtn = document.getElementById('new-chat');
const historyBtn = document.getElementById('history-btn');
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
let fixedHeight = false;
// The assistant message currently streaming: { el, body, raw }.
let liveMsg = null;

// ------------------------------------------------------------------ bubbles

function scrollToBottom(force) {
  const nearBottom = chatEl.scrollHeight - chatEl.scrollTop - chatEl.clientHeight < 48;
  if (force || nearBottom) chatEl.scrollTop = chatEl.scrollHeight;
  markOverflow();
}

// Fades the bottom edge while there is more transcript below the fold. A
// transcript that scrolls but gives no sign that it does is indistinguishable
// from one that has been cut off.
function markOverflow() {
  const more = chatEl.scrollHeight - chatEl.scrollTop - chatEl.clientHeight > 8;
  document.body.dataset.more = more ? '1' : '0';
}

chatEl.addEventListener('scroll', markOverflow, { passive: true });

// Keys reach the transcript whether or not it holds focus, because this
// window is shown without taking focus from whatever the user was doing and
// they may never click into it.
const SCROLL_KEYS = {
  PageDown: (h) => h * 0.9,
  PageUp: (h) => -h * 0.9,
  ArrowDown: () => 60,
  ArrowUp: () => -60,
  End: () => Infinity,
  Home: () => -Infinity,
};

document.addEventListener('keydown', (e) => {
  if (mode === 'hud' || opanel.hidden) return;
  if (e.ctrlKey || e.altKey || e.metaKey) return;
  const step = SCROLL_KEYS[e.key];
  if (!step) return;
  const target = e.target;
  // Never steal a key from a control that wants it.
  if (target && target.closest && target.closest('input, textarea, button')) return;
  const by = step(chatEl.clientHeight);
  if (by === Infinity) chatEl.scrollTop = chatEl.scrollHeight;
  else if (by === -Infinity) chatEl.scrollTop = 0;
  else chatEl.scrollTop += by;
  markOverflow();
  e.preventDefault();
});

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

  // The default models are free, which means queued: a wait of half a minute
  // is ordinary and there was no way out of one except collapsing the panel
  // and losing the turn. Stop lives beside the answer it is stopping, and
  // stays there through the stream rather than only during the dots.
  const stop = document.createElement('button');
  stop.type = 'button';
  stop.className = 'stop-btn';
  stop.textContent = 'Stop';
  stop.hidden = true;
  stop.setAttribute('aria-label', 'Stop this answer');
  stop.addEventListener('click', () => {
    stop.disabled = true;
    stop.textContent = 'Stopping';
    window.aeval.invoke('query:cancel');
  });
  meta.appendChild(stop);

  const copy = document.createElement('button');
  copy.type = 'button';
  copy.className = 'copy-msg';
  copy.textContent = 'Copy';
  copy.setAttribute('aria-label', 'Copy this answer');
  meta.appendChild(copy);
  msg.appendChild(meta);

  chatEl.appendChild(msg);
  const entry = { el: msg, body, raw: '', stop };
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

// While this is on, the answer is still coming and the row shows Stop
// instead of Copy.
function setBusy(entry, on) {
  if (!entry) return;
  entry.el.classList.toggle('busy', on);
  entry.stop.hidden = !on;
  if (!on) {
    entry.stop.disabled = false;
    entry.stop.textContent = 'Stop';
  }
}

function setThinking(entry, on, label) {
  if (on) {
    entry.body.textContent = '';
    const dots = document.createElement('span');
    dots.className = 'think-dots';
    dots.setAttribute('aria-label', label || 'Thinking');
    dots.innerHTML = '<i></i><i></i><i></i>';
    entry.body.appendChild(dots);
    if (label) {
      const tag = document.createElement('span');
      tag.className = 'stage-label';
      tag.textContent = label;
      entry.body.appendChild(tag);
    }
  }
  entry.el.classList.toggle('thinking', on);
}

function addErrorRow(message, question) {
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
  span.className = 'error-text';
  span.textContent = message;
  row.appendChild(span);
  if (question) {
    const retry = document.createElement('button');
    retry.type = 'button';
    retry.className = 'retry-btn';
    retry.textContent = 'Retry';
    retry.title = `Ask "${question}" again`;
    retry.addEventListener('click', () => {
      row.remove();
      requestResize();
      window.aeval.invoke('query:retry');
    });
    row.appendChild(retry);
  }
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

// The window is sized to the transcript exactly. It used to round up to 24px
// steps and never shrink, which left a band of empty panel under short
// answers. An 8px deadband keeps it from twitching, and it still refuses to
// shrink while an answer is streaming, so the bottom edge does not chase the
// text as it arrives.
function requestResize() {
  if (mode === 'hud' || opanel.hidden || resizeQueued || fixedHeight) return;
  resizeQueued = true;
  requestAnimationFrame(() => {
    resizeQueued = false;
    markOverflow();
    const height = Math.ceil(opanel.getBoundingClientRect().height);
    if (height < lastSentHeight && liveMsg) return;
    if (Math.abs(height - lastSentHeight) < 8) return;
    lastSentHeight = height;
    window.aeval.invoke('output:resize', { height });
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
  } else if (chatEl.childElementCount) {
    // Leaving the layout editor (or any later state change) brings an
    // existing transcript back; without this the window would show blank.
    opanel.hidden = false;
  }
  // A docked answer grows away from the input bar's edge; a free one grows
  // from its own anchor. The stylesheet pins the panel to those edges so the
  // window's own glide uncovers it.
  if (next.outputWidth) {
    document.documentElement.style.setProperty('--panel-w', next.outputWidth + 'px');
  }
  if (next.chatMax) {
    document.documentElement.style.setProperty('--chat-max', next.chatMax + 'px');
  }
  // A dragged height means the window no longer follows the transcript, so
  // the renderer stops asking it to.
  fixedHeight = !!next.outputFixedHeight;
  document.body.dataset.fixedHeight = fixedHeight ? '1' : '0';
  if (next.outputPos) {
    docked = next.outputPos.docked;
    const anchor = docked ? next.inputAnchor || next.outputPos : next.outputPos;
    document.body.dataset.anchorH = anchor.h || 'right';
    document.body.dataset.anchorV = anchor.v || 'top';
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
  setBusy(liveMsg, true);
  scrollToBottom(true);
  requestResize();
});

// Brain mode reads the screen before it thinks; each stage says so.
window.aeval.on('query:stage', ({ gen, label }) => {
  if (gen < lastGen || !liveMsg || liveMsg.raw) return;
  setThinking(liveMsg, true, label);
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
    setBusy(liveMsg, false);
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

window.aeval.on('query:error', ({ gen, message, question }) => {
  if (gen < lastGen) return;
  if (liveMsg) {
    setBusy(liveMsg, false);
    if (!liveMsg.raw) liveMsg.el.remove();
    else setThinking(liveMsg, false);
    liveMsg = null;
  }
  addErrorRow(message, question);
  requestResize();
});

window.aeval.on('chat:reset', () => {
  chatEl.innerHTML = '';
  liveMsg = null;
  lastSentHeight = 0;
  opanel.hidden = true;
  markOverflow();
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

historyBtn.addEventListener('click', () => {
  window.aeval.invoke('overlay:open-history');
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
