'use strict';

// An output window renders one conversation as a transcript: the question
// as a quiet chip, the answer as a document with streamed markdown, inline
// error rows that offer the fix, and (trail.js) Find results and task
// trails. Each output window is its own floating window with a title bar it
// is moved by (REQUESTS.md 11); there is no input here, the bar is the only
// one (REQUESTS.md 9). The sheet grows in the page and Rust follows it; the
// transcript scrolls inside, so streaming never jitters the window edge.

const opanel = document.getElementById('opanel');
const chatEl = document.getElementById('chat');
const sheetBar = document.getElementById('sheet-bar');
const newChatBtn = document.getElementById('new-chat');
const historyBtn = document.getElementById('history-btn');
const copyAllBtn = document.getElementById('copy-all');
const closeOutBtn = document.getElementById('close-out');
const sheetTitle = document.getElementById('sheet-title');
const sheetKind = document.getElementById('sheet-kind');

let mode = 'idle';
let lastGen = 0;
let renderQueued = false;
let resizeQueued = false;
let lastSentHeight = 0;
let lastGlass = '';
let fixedHeight = false;
// The assistant message currently streaming: { el, body, raw, ... }.
let liveMsg = null;
// What Rust last said about this window (output:meta, K-06).
let meta = { grow: 'down', front: true, target: false };

// ------------------------------------------------------------------ bridge

// Channels another lane adds (K-06, K-19, K-23) are not in every build's
// allowlist yet: listening on one throws in the bridge and invoking one
// rejects. The page must load and work without them.
function listen(channel, fn) {
  try {
    return window.aeval.on(channel, fn);
  } catch (err) {
    return null;
  }
}

function send(channel, payload) {
  try {
    return Promise.resolve(window.aeval.invoke(channel, payload)).catch(() => null);
  } catch (err) {
    return Promise.resolve(null);
  }
}

// This window's label, for the channels that name a window.
const winLabel = (() => {
  try {
    return window.__TAURI__.window.getCurrentWindow().label;
  } catch (err) {
    return window.name || '';
  }
})();

// ------------------------------------------------------------ announcements

// One way to speak (AevalA11y.say, K-42): into this page's live region while
// it has focus, else through Rust to the window that has it. Polite for news,
// assertive for errors and a task's question. Before glass.js carries it,
// the same rule here.
function speakHere(text, priority) {
  const el = document.getElementById(priority === 'assertive' ? 'announce-alert' : 'announce');
  if (!el) return;
  el.textContent = '';
  setTimeout(() => {
    el.textContent = text;
  }, 30);
}

function say(text, priority) {
  const t = String(text || '').trim();
  if (!t) return;
  const p = priority === 'assertive' ? 'assertive' : 'polite';
  if (window.AevalA11y && typeof window.AevalA11y.say === 'function') {
    window.AevalA11y.say(t, p);
    return;
  }
  if (document.hasFocus()) {
    speakHere(t, p);
    return;
  }
  let relayed;
  try {
    relayed = window.aeval.invoke('a11y:announce', { text: t, priority: p });
  } catch (err) {
    relayed = Promise.reject(err);
  }
  Promise.resolve(relayed).catch(() => speakHere(t, p));
}
window.say = say;
window.announce = (text) => say(text, 'polite');

// ------------------------------------------------------------------ scroll

function scrollToBottom(force) {
  const nearBottom = chatEl.scrollHeight - chatEl.scrollTop - chatEl.clientHeight < 48;
  if (force || nearBottom) chatEl.scrollTop = chatEl.scrollHeight;
  markOverflow();
}

// More below the fold, and scrolled under the title bar: the edge fade and
// the title bar's hairline read these.
function markOverflow() {
  const more = chatEl.scrollHeight - chatEl.scrollTop - chatEl.clientHeight > 8;
  document.body.dataset.more = more ? '1' : '0';
  document.body.dataset.scrolled = chatEl.scrollTop > 2 ? '1' : '0';
}

// The thumb brightens while the transcript moves, for 800 ms after.
let scrollingTimer = null;
function markScrolling() {
  document.body.dataset.scrolling = '';
  clearTimeout(scrollingTimer);
  scrollingTimer = setTimeout(() => {
    delete document.body.dataset.scrolling;
  }, 800);
}

chatEl.addEventListener(
  'scroll',
  () => {
    markOverflow();
    markScrolling();
  },
  { passive: true }
);

// A code block or table that scrolls sideways fades at the edge while there
// is more to the right.
chatEl.addEventListener(
  'scroll',
  (e) => {
    const t = e.target;
    if (t !== chatEl && t.matches && t.matches('.md-code pre, .md-table') && window.AevalMarkdown) {
      window.AevalMarkdown.markScrollOf(t);
    }
  },
  true
);

let markQueued = false;
new ResizeObserver(() => {
  if (markQueued) return;
  markQueued = true;
  requestAnimationFrame(() => {
    markQueued = false;
    markOverflow();
    if (window.AevalMarkdown) window.AevalMarkdown.markScroll(chatEl);
  });
}).observe(chatEl);

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
  if (opanel.hidden) return;
  if (e.ctrlKey || e.altKey || e.metaKey || e.defaultPrevented) return;
  const step = SCROLL_KEYS[e.key];
  if (!step) return;
  const target = e.target;
  // Never steal a key from a control that wants it.
  if (target && target.closest && target.closest('input, textarea, button, [role="option"], .retry-menu')) return;
  const box = chatEl.hidden ? document.getElementById('find-view') : chatEl;
  if (!box) return;
  const by = step(box.clientHeight);
  if (by === Infinity) box.scrollTop = box.scrollHeight;
  else if (by === -Infinity) box.scrollTop = 0;
  else box.scrollTop += by;
  markOverflow();
  e.preventDefault();
});

// --------------------------------------------------------------- the title

// An icon from the one family (renderer/icons.svg, K-40): a span glass.css
// masks with the sprite, filled with currentColor.
function icon(name, cls) {
  const el = document.createElement('span');
  el.className = `ico i-${name}${cls ? ` ${cls}` : ''}`;
  el.setAttribute('aria-hidden', 'true');
  return el;
}

// The kind is the bar token's glyph, the word in aria-label: Ask, Find, Do.
const KIND_GLYPH = { answer: 'bubble', find: 'viewfinder', task: 'pointer' };

// The title bar names the conversation by its first question, the way a
// document is named by its first line; a run is named by its task.
let titled = false;
function setTitle(kind, text) {
  const k = String(kind || 'Answer');
  const glyph = KIND_GLYPH[k.toLowerCase()] || KIND_GLYPH.answer;
  for (const c of [...sheetKind.classList]) if (c.startsWith('i-')) sheetKind.classList.remove(c);
  sheetKind.classList.add(`i-${glyph}`);
  sheetKind.setAttribute('aria-label', k);
  const t = String(text || '').replace(/\s+/g, ' ').trim();
  sheetTitle.textContent = t || k;
  sheetTitle.title = t;
  document.title = t ? `${t} · AevalBuddy` : 'AevalBuddy answer';
}
window.setSheetTitle = (kind, text, force) => {
  if (titled && !force) return;
  titled = true;
  setTitle(kind, text);
};

// The title bar catches the pointer's light (F42).
sheetBar.addEventListener('pointermove', (e) => {
  const r = sheetBar.getBoundingClientRect();
  sheetBar.style.setProperty('--mx', `${Math.round(e.clientX - r.left)}px`);
  sheetBar.style.setProperty('--my', `${Math.round(e.clientY - r.top)}px`);
});
sheetBar.addEventListener('pointerleave', () => {
  sheetBar.style.removeProperty('--mx');
  sheetBar.style.removeProperty('--my');
});

// Copy in the title bar copies the whole conversation: with one answer it
// would say what the answer's own Copy says, so it waits for a second.
function answerCount() {
  return chatEl.querySelectorAll(':scope > .msg.assistant:not(.busy), :scope > .msg.trail').length;
}

function syncCopyAll() {
  copyAllBtn.hidden = document.body.dataset.view === 'find' || answerCount() <= 1;
}
window.syncCopyAll = syncCopyAll;

// ------------------------------------------------------------- model names

// "Gemma 3 27B via OpenRouter": names come from models:list (K-19 adds
// providers); an id with no name yet reads as its last segment.
const PROVIDER_NAMES = { openrouter: 'OpenRouter', anthropic: 'Anthropic', openai: 'OpenAI', gemini: 'Gemini', google: 'Gemini' };
const modelNames = new Map();
let showPaid = false;
let catalogue = null;
let namesAsked = false;

function collectModels(list, into) {
  for (const m of list || []) {
    if (m && m.id) into.set(m.id, m.name || m.id);
  }
}

function loadNames() {
  if (namesAsked) return Promise.resolve();
  namesAsked = true;
  const models = send('models:list').then((r) => {
    if (!r) return;
    catalogue = r;
    for (const g of r.groups || []) collectModels(g.models, modelNames);
    for (const p of r.providers || []) {
      collectModels(p.models, modelNames);
      for (const g of p.groups || []) collectModels(g.models, modelNames);
    }
  });
  const settings = send('settings:get').then((s) => {
    showPaid = !!(s && s.showPaid);
  });
  return Promise.all([models, settings]);
}

function modelName(id, given) {
  if (given) return String(given);
  if (!id) return '';
  if (modelNames.has(id)) return modelNames.get(id);
  const base = String(id)
    .split('/')
    .pop()
    .replace(/:[\w-]+$/, '')
    .replace(/[-_]+/g, ' ')
    .replace(/\b(it|instruct|chat|latest|preview)\b/gi, '')
    .replace(/\s+/g, ' ')
    .trim();
  return base.replace(/\b([a-z])/g, (c) => c.toUpperCase()).replace(/\b(\d+(?:\.\d+)?)b\b/gi, '$1B');
}

function providerName(id, given) {
  if (given) return String(given);
  if (!id) return '';
  return PROVIDER_NAMES[id] || `${String(id).charAt(0).toUpperCase()}${String(id).slice(1)}`;
}

// Which provider answered: K-19 says so; before it, an id with a vendor
// prefix came through OpenRouter, the only provider.
function answeredBy(u, sent) {
  const s = sent || {};
  const id = s.model || u.model || '';
  const pid = s.provider || u.provider || (id.includes('/') ? 'openrouter' : '');
  return {
    id,
    name: modelName(id, s.modelName || u.modelName),
    provider: pid,
    providerName: providerName(pid, s.providerName || u.providerName || u.via),
  };
}

const sec = (ms) => `${(ms / 1000).toFixed(ms < 10000 ? 1 : 0)} s`;

// The full telemetry: "0.9 s to first word · 3.2 s · 1,204 tokens · free ·
// google/gemma-3-27b-it:free". It lives in the usage line's tooltip and in
// the answer's context menu (Copy details), not on the sheet.
function usageLine(u) {
  if (!u) return '';
  const parts = [];
  if (u.ttftMs) parts.push(`${sec(u.ttftMs)} to first word`);
  if (u.ms) parts.push(sec(u.ms));
  const tok = (u.prompt || 0) + (u.completion || 0);
  if (tok) parts.push(`${tok.toLocaleString('en-US')} tokens`);
  if (u.cost === 0) parts.push('free');
  else if (Number.isFinite(u.cost) && u.cost > 0) parts.push(`$${u.cost < 0.01 ? u.cost.toFixed(4) : u.cost.toFixed(2)}`);
  if (u.model) parts.push(u.model);
  if (u.provider) parts.push(`via ${providerName(u.provider, u.via)}`);
  if (u.textOnly) parts.push('read as text');
  return parts.join(' · ');
}
window.usageLine = usageLine;

// The quiet line under an answer (D-29): the model, who served it and the
// time; after a hop, who was busy; "free" only when paid models are on.
function quietUsage(u, sent) {
  if (!u) return '';
  const who = answeredBy(u, sent);
  const parts = [];
  if (who.name && who.providerName) parts.push(`${who.name} via ${who.providerName}`);
  else if (who.name || who.providerName) parts.push(who.name || who.providerName);
  if (u.ms) parts.push(sec(u.ms));
  const hop = u.hopFrom || u.busyProvider || (sent && sent.hopFrom);
  if (hop) parts.push(`${providerName(hop)} was busy`);
  const free = u.cost === 0 || (sent && sent.free) || /:free$/.test(who.id);
  if (showPaid && free) parts.push('free');
  return parts.join(' · ');
}

// --------------------------------------------------------------- messages

// The question an answer was for, so "Do it" can hand both to the agent.
let lastUserText = '';

// An answer that reads as steps to take gets a Do it button: at least two
// list lines that start with an action (U39).
const STEP_LINE = /^\s*(?:\d+[.)]|[-*])\s+(?:\*\*|__)?(?:click|double-click|right-click|press|open|type|select|choose|go to|drag|enter)\b/gim;
function readsAsSteps(raw) {
  const m = String(raw || '').match(STEP_LINE);
  return !!m && m.length >= 2;
}

function clearPlaceholders() {
  for (const n of chatEl.querySelectorAll(':scope > .reset-row, :scope > .empty-sheet')) n.remove();
  clearTimeout(resetTimer);
}

function addUserMsg(text) {
  clearPlaceholders();
  lastUserText = text;
  window.setSheetTitle('Answer', text);
  const msg = document.createElement('div');
  msg.className = 'msg user';
  // The first question is the title; the transcript only reads it out.
  if (!chatEl.querySelector(':scope > .msg.user')) msg.classList.add('is-title');
  const bubble = document.createElement('div');
  bubble.className = 'bubble';
  bubble.textContent = text;
  msg.appendChild(bubble);
  chatEl.appendChild(msg);
  scrollToBottom(true);
}

function button(cls, text, label) {
  const b = document.createElement('button');
  b.type = 'button';
  b.className = cls;
  if (text) b.textContent = text;
  if (label) b.setAttribute('aria-label', label);
  return b;
}

function addAssistantMsg() {
  clearPlaceholders();
  const msg = document.createElement('div');
  msg.className = 'msg assistant';
  const bubble = document.createElement('div');
  bubble.className = 'bubble';
  const body = document.createElement('div');
  body.className = 'md-body';
  bubble.appendChild(body);
  msg.appendChild(bubble);

  const metaRow = document.createElement('div');
  metaRow.className = 'msg-meta';

  // The default models are free, which means queued: Stop lives beside the
  // answer it is stopping, through the wait and the stream.
  const stop = button('stop-btn', 'Stop', 'Stop this answer');
  stop.hidden = true;
  stop.title = 'Stop (Ctrl+.)';
  stop.setAttribute('aria-keyshortcuts', 'Control+.');
  stop.addEventListener('click', () => {
    stop.disabled = true;
    stop.textContent = 'Stopping';
    send('query:cancel');
  });
  metaRow.appendChild(stop);

  const copy = button('copy-msg', 'Copy', 'Copy this answer');
  metaRow.appendChild(copy);

  const doIt = button('copy-msg do-it', 'Do it', 'Do the steps in this answer on your computer');
  doIt.title = 'Do the steps in this answer on your computer';
  doIt.hidden = true;
  metaRow.appendChild(doIt);
  msg.appendChild(metaRow);

  chatEl.appendChild(msg);
  const entry = { el: msg, body, raw: '', stop, doIt, meta: metaRow, question: lastUserText, details: '' };
  msg.__entry = entry;
  doIt.addEventListener('click', () => {
    send('agent:do-it', { question: entry.question, answer: entry.raw });
  });
  msg.addEventListener('contextmenu', (e) => {
    if (!entry.raw) return;
    if (e.target.closest && e.target.closest('.retry-menu')) return;
    e.preventDefault();
    send('ui:context-menu', {
      kind: 'answer',
      text: entry.raw,
      question: entry.question,
      selection: String(window.getSelection() || ''),
      details: entry.details,
    });
  });
  copy.addEventListener('click', () => {
    if (!entry.raw) return;
    send('clipboard:write', { text: entry.raw }).then(() => {
      copy.textContent = 'Copied';
      say('Copied the answer');
      setTimeout(() => {
        copy.textContent = 'Copy';
      }, 1500);
    });
  });
  return entry;
}

// The sheet's rim carries the light while anything is still coming: an
// answer streaming, or a task running (trail.js calls this too).
function syncOrbit() {
  const working = !!chatEl.querySelector('.msg.assistant.busy, .msg.trail[data-state="running"], .msg.trail[data-state="planning"], .msg.trail[data-state="thinking"]');
  opanel.classList.toggle('orbit', working);
}

function setBusy(entry, on) {
  if (!entry) return;
  entry.el.classList.toggle('busy', on);
  syncOrbit();
  entry.stop.hidden = !on;
  if (entry.doIt) entry.doIt.hidden = on || !readsAsSteps(entry.raw);
  if (!on) {
    entry.stop.disabled = false;
    entry.stop.textContent = 'Stop';
  }
  syncCopyAll();
}

function setThinking(entry, on, label) {
  if (on) {
    entry.body.__md = null;
    entry.body.textContent = '';
    const dots = document.createElement('span');
    dots.className = 'think-dots';
    dots.setAttribute('aria-hidden', 'true');
    dots.innerHTML = '<i></i><i></i><i></i>';
    entry.body.appendChild(dots);
    // The word is the whole indicator; the moving part is the light on the
    // sheet's rim.
    const tag = document.createElement('span');
    tag.className = 'stage-label';
    tag.textContent = label || 'Thinking';
    entry.body.appendChild(tag);
  }
  entry.el.classList.toggle('thinking', on);
}

// Free models queue. After a few seconds with no word the label says so and
// counts (C-20), the reason in its tooltip, so a long wait reads as waiting.
const WAIT_SAY_MS = 6000;
let waitTimer = null;
let waitModel = '';
function startWaitCount(entry) {
  clearInterval(waitTimer);
  const t0 = performance.now();
  let said = false;
  waitTimer = setInterval(() => {
    if (!entry || entry.raw || !entry.el.isConnected) {
      clearInterval(waitTimer);
      return;
    }
    const ms = performance.now() - t0;
    if (ms < WAIT_SAY_MS) return;
    const who = waitModel || 'the model';
    const label = `Waiting for ${who} · ${Math.round(ms / 1000)} s`;
    let tag = entry.body.querySelector('.stage-label');
    if (!tag) {
      setThinking(entry, true, label);
      tag = entry.body.querySelector('.stage-label');
    }
    tag.textContent = label;
    tag.title = "Free models queue when they're busy.";
    if (!said) {
      said = true;
      say(`Still waiting for ${who}.`);
    }
  }, 1000);
}

// ------------------------------------------------------------- what was sent

const IMAGE_URL = /^(data:image\/|blob:|aeval-frame:|https?:\/\/aeval-frame\.localhost\/)/;

function imageUrlOf(r) {
  const u = typeof r === 'string' ? r : r && (r.url || r.src || r.dataUrl);
  return typeof u === 'string' && IMAGE_URL.test(u) ? u : '';
}

function sentPanel(sent, msgId) {
  const panel = document.createElement('div');
  panel.className = 'sent-panel';
  panel.hidden = true;
  panel.id = `sent-${Math.random().toString(36).slice(2, 9)}`;
  const who = answeredBy({}, sent);
  const head = document.createElement('p');
  head.className = 'sent-head';
  head.textContent = who.name ? `Sent to ${who.providerName || 'the provider'} (${who.name}):` : `Sent to ${who.providerName || 'the provider'}:`;
  panel.appendChild(head);
  const ul = document.createElement('ul');
  const item = (text) => {
    const li = document.createElement('li');
    li.textContent = text;
    ul.appendChild(li);
    return li;
  };
  item('Your question');
  if (sent.image === 'screen' || sent.image === 'region') {
    const size = Array.isArray(sent.imageSize) && sent.imageSize.length === 2 ? `, ${sent.imageSize[0]} x ${sent.imageSize[1]}` : '';
    const li = item(sent.image === 'screen' ? `A screenshot of the whole screen${size}` : `A screenshot of the area you picked${size}`);
    const view = button('sent-view', 'View', 'View the screenshot that was sent');
    view.addEventListener('click', () => {
      view.disabled = true;
      send('output:sent-image', { msgId }).then((r) => {
        const url = imageUrlOf(r);
        if (!url) {
          view.textContent = 'Not kept';
          view.title = 'Only the last three screenshots are kept, in memory.';
          return;
        }
        const img = document.createElement('img');
        img.className = 'sent-img';
        img.alt = 'The screenshot that was sent';
        img.src = url;
        img.addEventListener('load', requestResize);
        view.remove();
        li.appendChild(img);
        requestResize();
      });
    });
    li.appendChild(view);
  }
  if (sent.windowText && sent.windowText.chars) {
    const app = sent.windowText.app ? `the ${sent.windowText.app} window` : 'the window';
    item(`The text of ${app}, ${Number(sent.windowText.chars).toLocaleString('en-US')} characters`);
  }
  if (sent.history > 0) item(`${sent.history} earlier message${sent.history === 1 ? '' : 's'} from this conversation`);
  if (sent.reader && sent.reader.modelName) item(`${sent.reader.modelName} read the screen first`);
  panel.appendChild(ul);
  if (sent.free) {
    const p = document.createElement('p');
    p.className = 'sent-free';
    p.textContent = "Free models' providers may keep and train on what they receive.";
    panel.appendChild(p);
  }
  return panel;
}

function addSentDisclosure(entry, sent, msgId) {
  const btn = button('sent-btn', 'What was sent');
  btn.setAttribute('aria-expanded', 'false');
  const panel = sentPanel(sent, msgId);
  btn.setAttribute('aria-controls', panel.id);
  btn.addEventListener('click', () => {
    const open = panel.hidden;
    panel.hidden = !open;
    btn.setAttribute('aria-expanded', String(open));
    entry.el.classList.toggle('sent-open', open);
    requestResize();
    if (open) scrollToBottom(false);
  });
  entry.meta.insertBefore(btn, entry.meta.querySelector('.usage'));
  entry.el.appendChild(panel);
}

// ------------------------------------------------------------- retry split

// Retry, and a chevron that lists the models of every ready provider (U20):
// a pick sends query:retry {label, provider, model}.
function retryHere(provider, model) {
  const payload = { label: winLabel };
  if (provider) payload.provider = provider;
  if (model) payload.model = model;
  return send('query:retry', payload);
}

function retrySplit(onRetry, iconOnly) {
  const wrap = document.createElement('span');
  wrap.className = 'retry-split';
  const main = button(`retry-btn${iconOnly ? ' icon-only' : ''}`, iconOnly ? '' : 'Retry', iconOnly ? 'Retry' : '');
  if (iconOnly) main.appendChild(icon('retry'));
  main.title = 'Retry (Ctrl+Shift+R)';
  main.setAttribute('aria-keyshortcuts', 'Control+Shift+R');
  main.addEventListener('click', () => onRetry(null, null));
  const more = button('retry-more', '', 'Retry with another model');
  more.appendChild(icon('chevron'));
  more.setAttribute('aria-haspopup', 'listbox');
  more.setAttribute('aria-expanded', 'false');
  more.addEventListener('click', () => openRetryMenu(more, onRetry));
  wrap.append(main, more);
  return wrap;
}

let menu = null;
function closeRetryMenu(refocus) {
  if (!menu) return;
  const { el, anchor } = menu;
  menu = null;
  el.remove();
  anchor.setAttribute('aria-expanded', 'false');
  if (refocus) anchor.focus();
  document.removeEventListener('pointerdown', outsideMenu, true);
}

function outsideMenu(e) {
  if (menu && !menu.el.contains(e.target) && e.target !== menu.anchor && !menu.anchor.contains(e.target)) closeRetryMenu(false);
}

// The providers and their models: K-19's providers, else the catalogue's
// groups (all OpenRouter).
function menuGroups() {
  const r = catalogue || {};
  const current = r.current || '';
  if (Array.isArray(r.providers) && r.providers.length) {
    return r.providers
      .filter((p) => p && p.ready !== false)
      .map((p) => ({
        label: p.name || providerName(p.id),
        provider: p.id,
        models: [...(p.models || []), ...(p.groups || []).flatMap((g) => g.models || [])],
        current: p.current || current,
      }))
      .filter((g) => g.models.length);
  }
  return (r.groups || []).map((g) => ({ label: g.label || 'OpenRouter', provider: 'openrouter', models: g.models || [], current }));
}

function openRetryMenu(anchor, onRetry) {
  if (menu && menu.anchor === anchor) {
    closeRetryMenu(true);
    return;
  }
  closeRetryMenu(false);
  const el = document.createElement('div');
  el.className = 'retry-menu g-menu solid';
  el.setAttribute('role', 'listbox');
  el.setAttribute('aria-label', 'Retry with another model');
  menu = { el, anchor };
  anchor.setAttribute('aria-expanded', 'true');
  const fill = () => {
    el.textContent = '';
    const groups = menuGroups();
    if (!groups.length) {
      const p = document.createElement('div');
      p.className = 'menu-empty';
      p.textContent = catalogue ? 'No other models are ready.' : 'Loading models…';
      el.appendChild(p);
      return;
    }
    for (const g of groups) {
      const group = document.createElement('div');
      group.setAttribute('role', 'group');
      group.setAttribute('aria-label', g.label);
      const head = document.createElement('div');
      head.className = 'menu-group';
      head.setAttribute('aria-hidden', 'true');
      head.textContent = g.label;
      group.appendChild(head);
      for (const m of g.models) {
        const opt = document.createElement('div');
        opt.setAttribute('role', 'option');
        opt.tabIndex = -1;
        opt.setAttribute('aria-selected', String(m.id === g.current));
        const label = document.createElement('span');
        label.className = 'menu-label';
        label.textContent = m.name || modelName(m.id);
        opt.appendChild(label);
        if (m.free && showPaid) {
          const note = document.createElement('span');
          note.className = 'menu-note';
          note.textContent = 'free';
          opt.appendChild(note);
        }
        const pick = () => {
          closeRetryMenu(false);
          onRetry(g.provider, m.id);
        };
        opt.addEventListener('click', pick);
        opt.addEventListener('keydown', (e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            pick();
          }
        });
        group.appendChild(opt);
      }
      el.appendChild(group);
    }
  };
  fill();
  el.addEventListener('keydown', (e) => {
    const opts = [...el.querySelectorAll('[role="option"]')];
    const i = opts.indexOf(document.activeElement);
    let to = -1;
    if (e.key === 'ArrowDown') to = Math.min(opts.length - 1, i + 1);
    else if (e.key === 'ArrowUp') to = Math.max(0, i - 1);
    else if (e.key === 'Home') to = 0;
    else if (e.key === 'End') to = opts.length - 1;
    else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      closeRetryMenu(true);
      return;
    } else if (e.key === 'Tab') {
      closeRetryMenu(false);
      return;
    }
    if (to >= 0 && opts[to]) {
      e.preventDefault();
      opts[to].focus();
    }
  });
  opanel.appendChild(el);
  // Under the chevron, or above it when the sheet has no room below.
  const a = anchor.getBoundingClientRect();
  const s = opanel.getBoundingClientRect();
  const h = el.offsetHeight;
  const below = a.bottom + 4 + h <= s.bottom - 8;
  el.style.top = `${Math.round((below ? a.bottom + 4 : a.top - 4 - h) - s.top)}px`;
  el.style.left = `${Math.round(Math.max(16, Math.min(a.left - s.left, s.width - el.offsetWidth - 16)))}px`;
  document.addEventListener('pointerdown', outsideMenu, true);
  const focusFirst = () => {
    const sel = el.querySelector('[aria-selected="true"]') || el.querySelector('[role="option"]');
    if (sel) sel.focus();
  };
  if (!catalogue) {
    namesAsked = false;
    loadNames().then(() => {
      if (menu && menu.el === el) {
        fill();
        focusFirst();
      }
    });
  }
  focusFirst();
}

function placeAnswerRetry(entry) {
  for (const old of chatEl.querySelectorAll('.msg.assistant .retry-split')) old.remove();
  if (!entry || !entry.question) return;
  const split = retrySplit((provider, model) => retryHere(provider, model), true);
  entry.meta.insertBefore(split, entry.meta.querySelector('.sent-btn, .usage'));
}

// --------------------------------------------------------------- error rows

const CREDIT_URL = {
  openrouter: 'https://openrouter.ai/settings/credits',
  anthropic: 'https://console.anthropic.com/settings/billing',
  openai: 'https://platform.openai.com/settings/organization/billing',
  gemini: 'https://aistudio.google.com/',
};
const PRIVACY_URL = 'https://openrouter.ai/settings/privacy';

let countdown = null;
function stopCountdown() {
  if (countdown) clearInterval(countdown.timer);
  countdown = null;
}

// The seconds in a busy note count down in place (not announced; the start
// and the retry are).
function startCountdown(span, message, ms) {
  stopCountdown();
  const end = performance.now() + ms;
  const text = (n) => {
    const all = [...message.matchAll(/\d+(?=\s*s\b)/g)];
    if (!all.length) return `${message} Trying again in ${n} s.`;
    const last = all[all.length - 1];
    return `${message.slice(0, last.index)}${n}${message.slice(last.index + last[0].length)}`;
  };
  const tick = () => {
    const n = Math.max(0, Math.ceil((end - performance.now()) / 1000));
    span.textContent = text(n);
    if (n <= 0) stopCountdown();
  };
  countdown = { timer: setInterval(tick, 1000) };
  tick();
}

// addErrorRow(message, question, actions, tone, extra): the actions are
// query:error's (K-19), in order, each doing its own fix. Without actions a
// question gets Retry. tone 'note' is busy, not broken: neutral, a clock and
// a countdown from retryInMs.
function addErrorRow(message, question, actions, tone, extra) {
  const x = extra || {};
  const note = tone === 'note';
  const row = document.createElement('div');
  row.className = `msg error-row${note ? ' note' : ''}`;
  row.appendChild(icon(note ? 'clock' : 'info', 'row-glyph'));
  const span = document.createElement('span');
  span.className = 'error-text';
  span.textContent = message;
  row.appendChild(span);
  const acts = document.createElement('div');
  acts.className = 'row-actions';
  const provider = x.provider || '';
  const done = () => {
    stopCountdown();
    row.remove();
    requestResize();
  };
  let neverAsk = null;
  const list = Array.isArray(actions) ? actions : [];
  for (const a of list) {
    const id = String((a && (a.id || a.action)) || '').toLowerCase();
    const label = (a && a.label) || '';
    if (id === 'never-ask') {
      // A checkbox drawn as a button: an output holds no input element, the
      // bar is the app's one input (NEW-06).
      const box = button('row-check', label || "Don't ask again");
      box.setAttribute('role', 'checkbox');
      box.setAttribute('aria-checked', 'false');
      box.addEventListener('click', () => {
        box.setAttribute('aria-checked', String(box.getAttribute('aria-checked') !== 'true'));
      });
      neverAsk = box;
      acts.appendChild(box);
      continue;
    }
    if (id === 'retry' && question) {
      acts.appendChild(
        retrySplit((p, m) => {
          done();
          retryHere(p, m);
        }, false)
      );
      continue;
    }
    const prominent = id === 'hop' || id === 'fix-key';
    const b = button(prominent ? 'accent-btn' : 'retry-btn', label || id);
    b.addEventListener('click', () => {
      if (id === 'fix-key') send('overlay:open-settings', { section: `prov-${a.provider || provider || 'openrouter'}` });
      else if (id === 'choose-model') send('overlay:open-settings', { section: a.section || 'pg-models' });
      else if (id === 'add-provider') send('overlay:open-settings', { section: 'pg-account' });
      else if (id === 'add-credit') send('open-external', { url: a.url || CREDIT_URL[a.provider || provider] || CREDIT_URL.openrouter });
      else if (id === 'privacy' || id === 'open-privacy') send('open-external', { url: a.url || PRIVACY_URL });
      else if (id === 'retry' || id === 'try-now' || id === 'test-again') {
        done();
        retryHere();
      } else if (id === 'hop') {
        const go = () => {
          done();
          retryHere(a.provider, a.model);
        };
        if (neverAsk && neverAsk.getAttribute('aria-checked') === 'true') send('settings:set-failover', { askBeforePaid: false }).then(go);
        else go();
      } else if (id === 'wait' || id === 'keep-waiting') {
        if (a.channel) send(a.channel, a.payload);
        done();
      } else if (id === 'copy-details') {
        send('clipboard:write', { text: x.details || [message, x.code, provider, x.model].filter(Boolean).join('\n') }).then(() => {
          b.textContent = 'Copied';
          setTimeout(() => {
            b.textContent = label || 'Copy details';
          }, 1500);
        });
      } else if (a.channel) send(a.channel, a.payload);
    });
    acts.appendChild(b);
  }
  if (!list.length && question) {
    acts.appendChild(
      retrySplit((p, m) => {
        done();
        retryHere(p, m);
      }, false)
    );
  }
  if (acts.childElementCount) row.appendChild(acts);
  chatEl.appendChild(row);
  if (note && x.retryInMs > 0) startCountdown(span, message, x.retryInMs);
  // Past two lines of text the fixes go under it, left-aligned.
  requestAnimationFrame(() => {
    const lh = parseFloat(getComputedStyle(span).lineHeight) || 20;
    row.classList.toggle('long', span.getBoundingClientRect().height > lh * 2 + 2);
  });
  say(message, note ? 'polite' : 'assertive');
  scrollToBottom(true);
  return row;
}

// ------------------------------------------------------------------- render

function renderLive() {
  if (!liveMsg || renderQueued) return;
  renderQueued = true;
  requestAnimationFrame(() => {
    renderQueued = false;
    if (!liveMsg) return;
    window.streamMarkdownInto(liveMsg.body, liveMsg.raw, { busy: true, animate: true });
    scrollToBottom(false);
    requestResize();
  });
}

// Rust is told the sheet's glass rect once per frame (output:glass, K-23),
// so its resize bands follow the visible sheet; and, until the window stops
// following content on its own (B-027), the height it should be. An 8 px
// deadband keeps that from twitching, and it never shrinks while an answer
// streams, so the bottom edge does not chase the text.
function requestResize() {
  if (opanel.hidden || resizeQueued) return;
  resizeQueued = true;
  requestAnimationFrame(() => {
    resizeQueued = false;
    markOverflow();
    // The layout box, not the painted one: while the sheet scales in, its
    // transform would report it 6% short and no later resize corrects that.
    const top = opanel.offsetTop;
    const height = opanel.offsetHeight;
    const glass = `${top},${height}`;
    if (glass !== lastGlass) {
      lastGlass = glass;
      send('output:glass', { top, height });
    }
    if (fixedHeight) return;
    if (height < lastSentHeight && liveMsg) return;
    if (Math.abs(height - lastSentHeight) < 8) return;
    lastSentHeight = height;
    send('output:resize', { height });
  });
}
window.requestResize = requestResize;

new ResizeObserver(requestResize).observe(opanel);

// The sheet globs out of the bar (glob.js, REQUESTS.md 8); under reduced
// motion it fades.
function playEntrance() {
  opanel.classList.remove('away', 'fading-out', 'closing');
  if (window.AevalGlob) window.AevalGlob.open(opanel);
}

// A hidden window keeps its last frame and shows it for a moment when it
// comes back. The sheet goes blank as the window hides, and grows in again
// when it is shown.
document.addEventListener('visibilitychange', () => {
  if (document.hidden) {
    opanel.classList.add('away');
    if (window.AevalGlass) window.AevalGlass.resetPassthrough();
  } else if (!opanel.hidden) {
    playEntrance();
  } else {
    opanel.classList.remove('away');
  }
});

function showSheet() {
  if (!opanel.hidden) return;
  opanel.hidden = false;
  playEntrance();
}

// ------------------------------------------------------------------- state

function syncMeta() {
  document.body.dataset.grow = meta.grow === 'up' ? 'up' : 'down';
  document.body.dataset.front = meta.front === false ? '0' : '1';
  document.body.dataset.target = meta.target && mode === 'panel' ? '1' : '0';
}

// The layout editor places the bar only now (REQUESTS.md 11): an output
// window keeps its place and its transcript through every mode.
listen('overlay:state', (next) => {
  mode = next.mode;
  if (!document.hidden) opanel.classList.remove('away');
  if (chatEl.childElementCount && opanel.hidden) opanel.hidden = false;
  if (next.chatMax) {
    document.documentElement.style.setProperty('--chat-max', next.chatMax + 'px');
  }
  // A dragged height means the window no longer follows the transcript, so
  // the renderer stops asking it to.
  fixedHeight = !!next.outputFixedHeight;
  document.body.dataset.fixedHeight = fixedHeight ? '1' : '0';
  syncMeta();
});

// Which way the sheet grows, whether it is in front, and whether the next
// question goes here (K-06).
listen('output:meta', (m) => {
  if (!m) return;
  meta = { ...meta, ...m };
  syncMeta();
  requestResize();
});

// -------------------------------------------------------------------- chat

listen('chat:user', ({ gen, text }) => {
  lastGen = gen;
  showSheet();
  addUserMsg(text);
  requestResize();
});

listen('query:start', (p) => {
  const { gen } = p || {};
  lastGen = gen;
  showSheet();
  loadNames();
  stopCountdown();
  // A busy note is replaced by the attempt it was counting down to.
  for (const n of chatEl.querySelectorAll(':scope > .error-row.note')) n.remove();
  for (const old of chatEl.querySelectorAll('.msg.assistant .retry-split')) old.remove();
  waitModel = (p && (p.modelName || (p.model && modelName(p.model)))) || '';
  liveMsg = addAssistantMsg();
  setThinking(liveMsg, true);
  setBusy(liveMsg, true);
  chatEl.setAttribute('aria-busy', 'true');
  startWaitCount(liveMsg);
  scrollToBottom(true);
  requestResize();
});

// Brain mode reads the screen before it thinks; each stage says so.
listen('query:stage', ({ gen, label }) => {
  if (gen < lastGen || !liveMsg || liveMsg.raw) return;
  setThinking(liveMsg, true, label);
});

listen('query:delta', ({ gen, text }) => {
  if (gen < lastGen || !liveMsg) return;
  lastGen = gen;
  if (text) {
    if (!liveMsg.raw) setThinking(liveMsg, false);
    liveMsg.raw += text;
    renderLive();
  }
});

// "Answer ready. <first sentence>. Press F6 to read it." (C-37): short, so
// it never talks over the person; the answer is one key away.
function firstSentence(raw) {
  const plain = String(raw || '')
    .replace(/```[\s\S]*?(```|$)/g, ' ')
    .replace(/`([^`]*)`/g, '$1')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/^\s*(#{1,6}|>|\d+[.)]|[-*+])\s+/gm, '')
    .replace(/[*_~|#]/g, '')
    .replace(/\s+/g, ' ')
    .trim();
  const m = /^(.+?[.!?])(\s|$)/.exec(plain);
  let s = m ? m[1] : plain;
  if (s.length > 160) s = `${s.slice(0, 159).replace(/\s+\S*$/, '')}…`;
  return s;
}

function announceDone(raw) {
  const s = firstSentence(raw);
  if (!s) {
    say('Answer ready. Press F6 to read it.');
    return;
  }
  say(`Answer ready. ${s}${/[.!?…]$/.test(s) ? '' : '.'} Press F6 to read it.`);
}

listen('query:done', (p) => {
  const { gen, usage, inferred, question, sent } = p || {};
  if (gen < lastGen) return;
  clearInterval(waitTimer);
  stopCountdown();
  chatEl.removeAttribute('aria-busy');
  if (liveMsg) {
    const done = liveMsg;
    liveMsg = null;
    setBusy(done, false);
    if (!done.raw) {
      // Nothing streamed (for example an abort before the first token);
      // an empty bubble is noise.
      done.el.remove();
    } else {
      setThinking(done, false);
      window.streamMarkdownInto(done.body, done.raw, { busy: false });
      if (usage) {
        done.details = usageLine(usage);
        const u = document.createElement('span');
        u.className = 'usage';
        u.title = done.details;
        u.textContent = quietUsage(usage, sent);
        done.meta.appendChild(u);
        // Names and the paid setting may arrive after the line is drawn.
        loadNames().then(() => {
          u.textContent = quietUsage(usage, sent);
        });
      }
      if (sent && typeof sent === 'object') addSentDisclosure(done, sent, (p && p.msgId) || sent.msgId || gen);
      placeAnswerRetry(done);
      // The guess sent this as a question. A short one may have been a word
      // to find; one click says so, without retyping.
      if (inferred && question && question.trim().split(/\s+/).length <= 6) {
        const f = button('copy-msg', 'Find this on screen instead');
        f.addEventListener('click', () => send('find:open', { prefill: question }));
        done.meta.insertBefore(f, done.meta.querySelector('.sent-btn, .usage'));
      }
      announceDone(done.raw);
    }
  }
  syncCopyAll();
  requestResize();
});

listen('query:error', (p) => {
  const { gen, message, question, actions, tone, retryInMs } = p || {};
  if (gen < lastGen) return;
  clearInterval(waitTimer);
  chatEl.removeAttribute('aria-busy');
  if (liveMsg) {
    setBusy(liveMsg, false);
    if (!liveMsg.raw) liveMsg.el.remove();
    else setThinking(liveMsg, false);
    liveMsg = null;
  }
  addErrorRow(message, question, actions, tone, {
    retryInMs,
    code: p && p.code,
    provider: p && p.provider,
    model: p && p.model,
    modelName: p && p.modelName,
    details: p && p.details,
  });
  syncCopyAll();
  requestResize();
});

// --------------------------------------------------------------- start over

// Start over keeps the window (U22): one neutral row says where the last
// conversation went, with Undo for ten seconds; then the empty sheet says
// what comes next.
let resetTimer = null;
const UNDO_MS = 10000;

function emptySheet() {
  const p = document.createElement('p');
  p.className = 'empty-sheet';
  p.textContent = 'Your next question goes here.';
  return p;
}

listen('chat:reset', (p) => {
  const undoMs = p && Number.isFinite(p.undoMs) ? p.undoMs : UNDO_MS;
  closeRetryMenu(false);
  stopCountdown();
  clearInterval(waitTimer);
  chatEl.innerHTML = '';
  chatEl.removeAttribute('aria-busy');
  liveMsg = null;
  lastSentHeight = 0;
  titled = false;
  setTitle('Answer', '');
  syncOrbit();
  clearTimeout(resetTimer);
  opanel.hidden = false;
  // Nothing to undo (the window was empty): the empty sheet at once.
  if (undoMs <= 0) {
    chatEl.appendChild(emptySheet());
    syncCopyAll();
    markOverflow();
    requestResize();
    return;
  }
  const row = document.createElement('div');
  row.className = 'msg note-row reset-row';
  const text = document.createElement('span');
  text.className = 'error-text';
  text.textContent = 'Started over. The last conversation is in History.';
  const undo = button('retry-btn', 'Undo', 'Undo start over');
  undo.addEventListener('click', () => {
    undo.disabled = true;
    let p;
    try {
      p = window.aeval.invoke('output:undo-reset', { label: winLabel });
    } catch (err) {
      p = Promise.reject(err);
    }
    // Rust restores the conversation and replays it (chat:sync); {ok: false}
    // when something new was asked since, or the time ran out.
    const failed = () => {
      undo.remove();
      text.textContent = "Couldn't bring it back here. It is in History.";
    };
    Promise.resolve(p)
      .then((r) => {
        if (r && r.ok === false) failed();
      })
      .catch(failed);
  });
  row.append(text, undo);
  chatEl.appendChild(row);
  say('Started over. The last conversation is in History.');
  resetTimer = setTimeout(() => {
    if (row.isConnected) row.replaceWith(emptySheet());
    requestResize();
  }, undoMs);
  syncCopyAll();
  markOverflow();
  requestResize();
});

// Rebuilds the transcript: after a renderer reload, a reopened history
// entry, or an undone Start over.
listen('chat:sync', ({ messages }) => {
  clearPlaceholders();
  closeRetryMenu(false);
  chatEl.innerHTML = '';
  liveMsg = null;
  titled = false;
  let last = null;
  for (const m of messages || []) {
    if (m.role === 'user') {
      addUserMsg(m.text);
    } else if (m.kind === 'trail') {
      window.addStoredTrail(m.text);
    } else {
      const entry = addAssistantMsg();
      entry.raw = m.text;
      window.renderMarkdownInto(entry.body, entry.raw);
      setBusy(entry, false);
      last = entry;
    }
  }
  if (last) placeAnswerRetry(last);
  syncCopyAll();
  if (messages && messages.length) {
    opanel.hidden = false;
    scrollToBottom(true);
    requestResize();
  }
});

// ------------------------------------------------------------- the toolbar

newChatBtn.addEventListener('click', () => {
  send('chat:new');
});

historyBtn.addEventListener('click', () => {
  send('overlay:open-history');
});

// The whole conversation as plain text: questions, answers, task outcomes.
function transcriptText() {
  const out = [];
  for (const m of chatEl.children) {
    if (m.classList.contains('user')) out.push(`> ${m.textContent.trim()}`);
    else if (m.__entry && m.__entry.raw) out.push(m.__entry.raw.trim());
    else if (m.classList.contains('trail')) {
      const o = m.querySelector('.trail-outcome');
      if (o) out.push(o.innerText.trim());
    } else if (m.classList.contains('error-row')) out.push(m.querySelector('.error-text').textContent.trim());
  }
  return out.filter(Boolean).join('\n\n');
}

function copyConversation() {
  const text = transcriptText();
  if (!text) return;
  send('clipboard:write', { text }).then(() => {
    copyAllBtn.dataset.done = '1';
    say('Copied the conversation');
    setTimeout(() => delete copyAllBtn.dataset.done, 1500);
  });
}

copyAllBtn.addEventListener('click', copyConversation);

// Close pulls the sheet back into the bar, then the window goes. Without
// the Rust side of output:close (a build that predates floating windows)
// the old behaviour stands: the bar collapses and takes the answer with it.
let closing = false;
async function closeWindow() {
  if (closing) return;
  closing = true;
  closeRetryMenu(false);
  if (window.AevalGlob) await window.AevalGlob.close(opanel);
  let p;
  try {
    p = window.aeval.invoke('output:close', { label: winLabel });
  } catch (err) {
    p = Promise.reject(err);
  }
  Promise.resolve(p)
    .catch(() => send('overlay:collapse'))
    .finally(() => {
      closing = false;
    });
}

closeOutBtn.addEventListener('click', closeWindow);

// A code block copies in one click; the icon is a check for 1.5 s.
chatEl.addEventListener('click', (e) => {
  const btn = e.target.closest && e.target.closest('.md-copy');
  if (!btn) return;
  const code = btn.closest('.md-code') && btn.closest('.md-code').querySelector('pre code');
  if (!code) return;
  send('clipboard:write', { text: code.textContent }).then(() => {
    btn.dataset.done = '';
    say('Copied the code');
    clearTimeout(btn.__done);
    btn.__done = setTimeout(() => delete btn.dataset.done, 1500);
  });
});

// Links from model answers open in the default browser, Rust-validated to
// http and https. A middle click is caught too: nothing here navigates.
function openLink(e) {
  const a = e.target.closest && e.target.closest('a[data-href]');
  if (!a) return;
  e.preventDefault();
  if (e.type === 'auxclick' && e.button !== 1) return;
  send('open-external', { url: a.dataset.href });
}
chatEl.addEventListener('click', openLink);
chatEl.addEventListener('auxclick', openLink);

// ---------------------------------------------------------------- the keys

// Stop whatever is still coming here: the answer streaming, or the task.
function stopHere() {
  if (liveMsg) {
    liveMsg.stop.click();
    return;
  }
  if (chatEl.querySelector('.msg.trail:not(.ended)')) send('agent:stop');
}

// Output-window keys (U19). Esc moves focus and never closes or stops
// anything (U3): to the bar while it is open, else back to the app below.
window.addEventListener('keydown', (e) => {
  if (e.defaultPrevented) return;
  const k = e.key;
  const ctrl = e.ctrlKey && !e.altKey && !e.metaKey;
  if (k === 'Escape' && !e.ctrlKey && !e.altKey) {
    if (menu) return;
    e.preventDefault();
    if (mode === 'panel') send('ui:focus-bar');
    else send('output:blur', { label: winLabel });
    return;
  }
  if (k === 'F6') {
    e.preventDefault();
    send('ui:focus-bar');
    return;
  }
  if (!ctrl) return;
  const lower = k.length === 1 ? k.toLowerCase() : k;
  let handled = true;
  if (lower === 'n' && !e.shiftKey) send('ui:focus-bar', { newConversation: true });
  else if (k === 'Tab') send('output:cycle-target', { dir: e.shiftKey ? -1 : 1 });
  else if (lower === 'w' && e.shiftKey) send('output:close-all');
  else if (lower === 'w') closeWindow();
  else if (lower === 'c' && e.shiftKey) copyConversation();
  else if (k === '.' && !e.shiftKey) stopHere();
  else if (lower === 'r' && e.shiftKey) retryHere();
  else if (k === ',' && !e.shiftKey) send('overlay:open-settings', {});
  else if (lower === 'h' && !e.shiftKey) send('overlay:open-history');
  else handled = false;
  if (handled) e.preventDefault();
});

syncMeta();
syncCopyAll();
