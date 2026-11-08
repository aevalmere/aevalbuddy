'use strict';

// The answer window's two v2 views, loaded after output.js and sharing its
// globals (chatEl, opanel, addAssistantMsg, requestResize, scrollToBottom).
//
//   Find view   the results list: each hit's snippet, and for related hits,
//               why. Clicking a row moves the ring to it.
//   Agent trail inside the transcript, one block per run: the plan, one row
//               per step, a confirm card when a step needs a yes, and the
//               final message. One conversation, three verbs.

const findView = document.getElementById('find-view');
// Copying the conversation means nothing over a list of matches.
const copyAll = document.getElementById('copy-all');

// ------------------------------------------------------------------- views

let view = 'chat';
let findHits = [];
let findCurrent = -1;
let findStatus = null;

function setView(next) {
  view = next === 'find' ? 'find' : 'chat';
  document.body.dataset.view = view;
  findView.hidden = view !== 'find';
  chatEl.hidden = view === 'find';
  copyAll.hidden = view === 'find';
  // Hits that arrived while the chat was showing were stored, not drawn.
  if (view === 'find') renderFind();
  if (view === 'find') window.setSheetTitle('Find', (findStatus && findStatus.query) || '', true);
  if (view === 'find' && opanel.hidden) {
    opanel.hidden = false;
    playEntrance();
  }
  if (view === 'chat' && !chatEl.childElementCount) opanel.hidden = true;
  requestResize();
}

window.aeval.on('view:set', ({ view: v }) => setView(v));

// -------------------------------------------------------------------- find

function kindMark(kind) {
  const m = document.createElement('span');
  m.className = `kind-mark ${kind}`;
  m.setAttribute('aria-hidden', 'true');
  return m;
}

function renderFind() {
  findView.innerHTML = '';
  if (!findHits.length) {
    const p = document.createElement('p');
    p.className = 'find-empty';
    const s = findStatus || {};
    // No instructions here: how to search is the bar's to say (REQUESTS.md 6).
    p.textContent = [s.status, ...(s.lines || [])].filter(Boolean).join('\n') || 'Looking…';
    findView.appendChild(p);
  }
  findHits.forEach((h) => {
    const row = document.createElement('button');
    row.type = 'button';
    row.className = 'find-row';
    row.setAttribute('role', 'option');
    row.setAttribute('aria-selected', String(h.i === findCurrent));
    row.dataset.i = String(h.i);
    if (h.i === findCurrent) row.classList.add('current');
    if (h.offscreen) row.classList.add('offscreen');
    row.appendChild(kindMark(h.kind));
    const body = document.createElement('span');
    body.className = 'find-body';
    const snip = document.createElement('span');
    snip.className = 'find-snippet';
    // The match itself in the snippet is set apart by weight, not colour.
    const at = h.match ? h.text.toLowerCase().indexOf(h.match.toLowerCase()) : -1;
    if (at >= 0) {
      snip.append(h.text.slice(0, at));
      const b = document.createElement('b');
      b.textContent = h.text.slice(at, at + h.match.length);
      snip.append(b, h.text.slice(at + h.match.length));
    } else {
      snip.textContent = h.text;
    }
    body.appendChild(snip);
    if (h.why) {
      const why = document.createElement('span');
      why.className = 'find-why';
      why.textContent = h.why;
      body.appendChild(why);
    }
    row.appendChild(body);
    row.addEventListener('click', () => window.aeval.invoke('find:step', { by: { index: h.i } }));
    row.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      window.aeval.invoke('ui:context-menu', { kind: 'hit', index: h.i, text: h.match || h.text });
    });
    findView.appendChild(row);
  });
  const cur = findView.querySelector('.find-row.current');
  if (cur) cur.scrollIntoView({ block: 'nearest' });
  requestResize();
}

window.aeval.on('find:hits', ({ hits, current, query }) => {
  if (view === 'find' && query) window.setSheetTitle('Find', query, true);
  findHits = hits || [];
  findCurrent = current;
  if (view === 'find') renderFind();
});

window.aeval.on('find:state', (s) => {
  findStatus = s;
  if (view === 'find' && !findHits.length) renderFind();
});

// ------------------------------------------------------------------- trail

let block = null; // the running (or last) trail: { el, rows: Map, list, meta, stop, task, ended }

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

function newBlock(task) {
  const wrap = el('div', 'msg trail');
  const head = el('div', 'trail-head');
  head.appendChild(el('span', 'trail-title', 'Task'));
  const status = el('span', 'trail-status', 'Planning…');
  head.appendChild(status);
  // Once a run ends its steps fold away under the outcome; this opens them.
  const toggle = el('button', 'trail-toggle', '');
  toggle.type = 'button';
  toggle.hidden = true;
  toggle.setAttribute('aria-expanded', 'false');
  toggle.addEventListener('click', () => {
    const open = !wrap.classList.contains('open');
    wrap.classList.toggle('open', open);
    toggle.setAttribute('aria-expanded', String(open));
    requestResize();
  });
  head.appendChild(toggle);
  wrap.appendChild(head);
  const list = el('ol', 'trail-rows');
  wrap.appendChild(list);
  const meta = el('div', 'msg-meta trail-meta');
  const stop = el('button', 'stop-btn', 'Stop');
  stop.type = 'button';
  stop.title = 'Esc';
  stop.addEventListener('click', () => {
    stop.disabled = true;
    stop.textContent = 'Stopping';
    window.aeval.invoke('agent:stop');
  });
  meta.appendChild(stop);
  wrap.appendChild(meta);
  if (opanel.hidden) {
    opanel.hidden = false;
    playEntrance();
  }
  chatEl.appendChild(wrap);
  wrap.dataset.state = 'planning';
  block = { el: wrap, rows: new Map(), list, meta, stop, status, toggle, task, ended: false, card: null };
  syncOrbit();
  scrollToBottom(true);
  requestResize();
  return block;
}

const STATE_MARK = { running: '', done: '', skipped: 'Skipped', failed: 'Failed', paused: '' };

function rowEl(r) {
  const li = el('li', `trow ${r.kind} ${r.state || ''}`);
  li.dataset.id = String(r.id);
  const mark = el('span', 'trow-mark');
  mark.setAttribute('aria-hidden', 'true');
  li.appendChild(mark);
  const body = el('span', 'trow-body');
  if (r.kind === 'thinking') {
    const dots = el('span', 'think-dots');
    dots.innerHTML = '<i></i><i></i><i></i>';
    body.appendChild(dots);
    body.appendChild(el('span', 'stage-label', r.text));
  } else if (r.kind === 'plan') {
    body.appendChild(el('span', 'trow-text', 'Plan'));
    const ol = el('ol', 'plan-steps');
    for (const s of r.steps || []) ol.appendChild(el('li', '', s));
    body.appendChild(ol);
  } else {
    body.appendChild(el('span', 'trow-text', r.text));
    const note = r.error || (r.state === 'skipped' && r.why ? `Not done: ${r.why}` : '');
    if (note) body.appendChild(el('span', 'trow-note', note));
    if (r.kind === 'ask' && r.state === 'running') {
      // The answer is typed in the bar, the app's one input (REQUESTS.md 9);
      // the bar knows a question is waiting from overlay:state and sends
      // what is typed as agent:continue to this run.
      body.appendChild(el('span', 'trow-note waiting', 'Waiting for your answer in the bar'));
    } else if (r.kind === 'ask' && r.answer) {
      body.appendChild(el('span', 'trow-note', `You: ${r.answer}`));
    }
  }
  li.appendChild(body);
  const side = el('span', 'trow-side');
  if (r.state === 'running' && r.kind !== 'thinking') {
    const dots = el('span', 'think-dots small');
    dots.innerHTML = '<i></i><i></i><i></i>';
    side.appendChild(dots);
  } else if (STATE_MARK[r.state]) {
    side.textContent = STATE_MARK[r.state];
  } else if (r.ms >= 1500) {
    // Only a step slow enough to notice says how long it took.
    side.textContent = `${(r.ms / 1000).toFixed(1)}s`;
  }
  li.appendChild(side);
  li.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    window.aeval.invoke('ui:context-menu', {
      kind: 'row',
      text: r.kind === 'plan' ? (r.steps || []).join('; ') : r.text,
      ended: !!(block && block.ended),
      task: block ? block.task : '',
    });
  });
  return li;
}

function putRow(r) {
  if (!block) newBlock('');
  const old = block.rows.get(r.id);
  if (r.kind === 'remove') {
    if (old) old.remove();
    block.rows.delete(r.id);
    requestResize();
    return;
  }
  if (r.kind === 'message') return; // the final message is its own answer
  const li = rowEl(r);
  if (old) old.replaceWith(li);
  else block.list.appendChild(li);
  block.rows.set(r.id, li);
  scrollToBottom(false);
  requestResize();
}

const STATUS_TEXT = {
  planning: 'Planning…',
  thinking: 'Thinking…',
  running: 'Working',
  confirm: 'Waiting for you',
  asking: 'Waiting for your answer',
  paused: 'Paused',
  plan: 'Waiting for you',
};

window.aeval.on('agent:begin', ({ task }) => {
  window.setSheetTitle('Task', task);
  newBlock(task);
});

window.aeval.on('agent:step', (r) => putRow(r));

window.aeval.on('agent:state', ({ state, message, step }) => {
  if (!block) return;
  block.status.textContent = STATUS_TEXT[state] ? `${STATUS_TEXT[state]}${step ? ` · step ${step}` : ''}` : '';
  if (block.el.dataset.state !== state && STATUS_TEXT[state]) window.announce(`Task: ${STATUS_TEXT[state]}`);
  block.el.dataset.state = state;
  syncOrbit();
  if (state !== 'confirm' && block.card) {
    block.card.remove();
    block.card = null;
  }
  let banner = block.el.querySelector('.trail-banner');
  if (state === 'paused' || state === 'plan') {
    if (!banner) {
      banner = el('div', 'trail-banner');
      block.el.insertBefore(banner, block.meta);
    }
    banner.innerHTML = '';
    banner.appendChild(el('span', 'banner-text', message || ''));
    const go = el('button', 'retry-btn', state === 'plan' ? 'Start' : 'Continue');
    go.type = 'button';
    go.addEventListener('click', () => window.aeval.invoke('agent:continue', {}));
    banner.appendChild(go);
    // Enter on this button is the only Enter that continues a run; nothing
    // is taken from other apps for it.
    setTimeout(() => go.focus(), 30);
  } else if (banner) {
    banner.remove();
  }
  requestResize();
});

const CONFIRM_SETTLE_MS = 700;

window.aeval.on('agent:confirm-request', ({ id, text, reason, why, url }) => {
  if (!block) newBlock('');
  if (block.card) block.card.remove();
  const card = el('div', 'confirm-card');
  card.setAttribute('role', 'alertdialog');
  card.setAttribute('aria-label', text);
  card.tabIndex = -1;
  card.appendChild(el('p', 'confirm-text', text));
  if (url && url.full) {
    // The host is what decides where it goes; the rest is shown whole so a
    // look-alike path cannot hide behind an ellipsis.
    const u = el('p', 'confirm-url');
    u.appendChild(el('strong', '', url.host || ''));
    u.appendChild(el('span', '', url.rest || ''));
    u.title = url.full;
    card.appendChild(u);
  }
  // One line: what it does, and why it stopped to ask.
  const says = [reason ? `${reason.charAt(0).toUpperCase()}${reason.slice(1)}.` : '', why ? `Asked because ${why}.` : '']
    .filter(Boolean)
    .join(' ');
  if (says) card.appendChild(el('p', 'confirm-reason', says));
  const actions = el('div', 'confirm-actions');
  actions.appendChild(el('span', 'confirm-hint', 'Tab to a button, then Enter'));
  const skip = el('button', 'retry-btn', 'Skip');
  skip.type = 'button';
  const allow = el('button', 'accent-btn', 'Allow');
  allow.type = 'button';
  const shownAt = Date.now();
  const answer = (yes) => {
    if (Date.now() - shownAt < CONFIRM_SETTLE_MS) return;
    allow.disabled = true;
    skip.disabled = true;
    window.aeval.invoke('agent:confirm', { id, allow: yes });
  };
  skip.addEventListener('click', () => answer(false));
  allow.addEventListener('click', () => answer(true));
  actions.append(skip, allow);
  card.appendChild(actions);
  card.dataset.id = String(id);
  block.card = card;
  block.el.insertBefore(card, block.meta);
  setTimeout(() => card.focus(), 30);
  window.announce(`Asking first: ${text}${why ? `. Asking because ${why}` : ''}. Tab to Skip or Allow.`);
  scrollToBottom(true);
  requestResize();
});

const END_TONE = { done: 'done', stopped: 'quiet', error: 'error', limit: 'error', offline: 'error' };

window.aeval.on('agent:done', ({ state, text, usage }) => {
  if (!block) newBlock('');
  const line = window.usageLine && window.usageLine(usage && { ...usage, cost: usage.costKnown ? usage.cost : NaN });
  if (line) block.meta.appendChild(el('span', 'usage', `${usage.calls || 0} model calls · ${line}`));
  window.announce(state === 'done' ? 'Task done.' : state === 'stopped' ? 'Task stopped.' : 'The task did not finish.');
  block.ended = true;
  block.el.dataset.state = state;
  block.el.classList.add('ended');
  syncOrbit();
  block.status.textContent = state === 'done' ? 'Done' : state === 'stopped' ? 'Stopped' : 'Did not finish';
  block.stop.hidden = true;
  // The steps fold under the outcome; a run that failed keeps them open,
  // since which one failed is the point.
  const shown = [...block.list.children].filter((li) => getComputedStyle(li).display !== 'none').length;
  if (shown) {
    block.toggle.hidden = false;
    block.toggle.textContent = `${shown} step${shown === 1 ? '' : 's'}`;
    const open = state !== 'done';
    block.el.classList.toggle('open', open);
    block.toggle.setAttribute('aria-expanded', String(open));
  }
  if (block.card) {
    block.card.remove();
    block.card = null;
  }
  const banner = block.el.querySelector('.trail-banner');
  if (banner) banner.remove();
  for (const li of block.list.querySelectorAll('.trow.thinking')) li.remove();
  const tone = END_TONE[state] || 'quiet';
  if (tone === 'done' && text) {
    // Outcome first: the result is the headline of the block, above the
    // steps that produced it.
    const out = el('div', 'trail-outcome md-body');
    window.renderMarkdownInto(out, text);
    block.el.insertBefore(out, block.list);
    const copy = el('button', 'copy-msg', 'Copy');
    copy.type = 'button';
    copy.setAttribute('aria-label', 'Copy the result');
    copy.addEventListener('click', () => {
      window.aeval.invoke('clipboard:write', { text }).then(() => {
        copy.textContent = 'Copied';
        setTimeout(() => {
          copy.textContent = 'Copy';
        }, 1500);
      });
    });
    block.meta.insertBefore(copy, block.meta.firstChild);
  } else if (text) {
    const row = el('div', `msg ${tone === 'error' ? 'error-row' : 'note-row'}`);
    row.appendChild(el('span', 'error-text', text));
    if (tone === 'error' && block.task) {
      const retry = el('button', 'retry-btn', 'Retry');
      retry.type = 'button';
      const task = block.task;
      retry.addEventListener('click', () => window.aeval.invoke('agent:start', { task }));
      row.appendChild(retry);
    }
    chatEl.appendChild(row);
  }
  scrollToBottom(true);
  requestResize();
});

// The first run asks once, in the answer window, before anything moves.
window.aeval.on('agent:consent-request', ({ task }) => {
  if (opanel.hidden) {
    opanel.hidden = false;
    playEntrance();
  }
  const sheet = el('div', 'msg consent');
  sheet.appendChild(el('p', 'consent-title', 'Let AevalBuddy use your computer?'));
  const ul = el('ul', 'consent-list');
  ul.appendChild(el('li', '', 'It can see, click and type in any app, the way you would.'));
  ul.appendChild(el('li', '', 'It asks first before anything that sends, deletes, buys, signs or opens a link.'));
  ul.appendChild(el('li', '', 'Press Esc, or move the mouse, and it stops.'));
  ul.appendChild(
    el(
      'li',
      '',
      'Each step sends a screenshot and the window\'s text to OpenRouter and the model set for tasks. A free model\'s provider may keep and train on what it receives.'
    )
  );
  sheet.appendChild(ul);
  sheet.tabIndex = -1;
  sheet.setAttribute('role', 'dialog');
  sheet.setAttribute('aria-label', 'Let AevalBuddy use your computer?');
  const actions = el('div', 'confirm-actions');
  const later = el('button', 'link-btn', 'Not now');
  later.type = 'button';
  const allow = el('button', 'accent-btn', 'Allow');
  allow.type = 'button';
  const done = (yes) => {
    sheet.remove();
    requestResize();
    window.aeval.invoke('agent:consent', { allow: yes, task });
  };
  later.addEventListener('click', () => done(false));
  allow.addEventListener('click', () => done(true));
  actions.append(later, allow);
  sheet.appendChild(actions);
  chatEl.appendChild(sheet);
  // Focus on the sheet, not on Allow: Enter alone must not grant this.
  setTimeout(() => sheet.focus(), 30);
  window.announce('Let AevalBuddy use your computer? Tab to Not now or Allow.');
  scrollToBottom(true);
  requestResize();
});

// A run reopened from history: its steps, collapsed.
window.addStoredTrail = function addStoredTrail(text) {
  const wrap = el('div', 'msg trail stored');
  const det = el('details', 'trail-stored');
  const sum = el('summary', '', `Task steps (${String(text).split('\n').filter(Boolean).length})`);
  det.appendChild(sum);
  det.appendChild(el('pre', 'trail-text', text));
  det.addEventListener('toggle', requestResize);
  wrap.appendChild(det);
  chatEl.appendChild(wrap);
};

window.aeval.on('chat:reset', () => {
  block = null;
});

window.aeval.on('chat:sync', () => {
  block = null;
});
