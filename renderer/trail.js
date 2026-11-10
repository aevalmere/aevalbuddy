'use strict';

// The answer window's two v2 views, loaded after output.js and sharing its
// globals (chatEl, opanel, listen, send, say, showSheet, requestResize,
// scrollToBottom, syncOrbit).
//
//   Find view   the results list: each hit's snippet, and for related hits,
//               why. A listbox: the arrows move through it and the ring
//               follows. Nothing found offers the next step.
//   Agent trail inside the transcript, one block per run: the plan as the
//               checklist with each act under the item it serves, a confirm
//               card when a step needs a yes, and the outcome. One
//               conversation, three verbs.

const findView = document.getElementById('find-view');

// ------------------------------------------------------------------- views

let view = 'chat';
let findHits = [];
let findCurrent = -1;
let findStatus = null;
let findQuery = '';

function setView(next) {
  view = next === 'find' ? 'find' : 'chat';
  document.body.dataset.view = view;
  findView.hidden = view !== 'find';
  chatEl.hidden = view === 'find';
  // Copying the conversation means nothing over a list of matches.
  window.syncCopyAll();
  // Hits that arrived while the chat was showing were stored, not drawn.
  if (view === 'find') {
    renderFind();
    window.setSheetTitle('Find', findQuery, true);
    showSheet();
  }
  if (view === 'chat' && !chatEl.childElementCount) opanel.hidden = true;
  requestResize();
}

listen('view:set', ({ view: v }) => setView(v));

// -------------------------------------------------------------------- find

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

// One press, one request: the button is off until Rust has answered, so a
// second press cannot look as if it did something.
function once(btn, channel, payload) {
  if (btn.disabled) return;
  btn.disabled = true;
  send(channel, payload).then(() => {
    btn.disabled = false;
  });
}

function kindMark(kind) {
  const m = el('span', `kind-mark ${kind}`);
  m.setAttribute('aria-hidden', 'true');
  return m;
}

// The search ran and found nothing, exact or related.
function findCameUpEmpty() {
  const s = findStatus || {};
  if (findHits.length || !findQuery || s.reading || s.open === false) return false;
  const c = s.counts || {};
  return !c.exact && !c.related;
}

// Nothing found: what was said, then what to do next (U24). Actions only,
// no keys and no how-to: how to search is the bar's to say.
function renderEmpty() {
  const s = findStatus || {};
  const wrap = el('div', 'find-empty-wrap');
  const p = el('p', 'find-empty', [s.status, ...(s.lines || [])].filter(Boolean).join('\n') || 'Looking…');
  wrap.appendChild(p);
  if (findCameUpEmpty()) {
    const next = el('div', 'find-next');
    // Offline there is no one to ask.
    if (s.stage !== 'offline') {
      const ask = el('button', 'retry-btn', `Ask about "${findQuery}"`);
      ask.type = 'button';
      ask.title = `Ask about "${findQuery}"`;
      ask.addEventListener('click', askAboutQuery);
      next.appendChild(ask);
    }
    if (s.scope !== 'screen') {
      const wide = el('button', 'retry-btn', 'Look on the whole screen');
      wide.type = 'button';
      wide.addEventListener('click', () => send('find:scope', { scope: 'screen' }));
      next.appendChild(wide);
    }
    wrap.appendChild(next);
  }
  findView.appendChild(wrap);
}

// With no hit to explain, the words themselves become the question.
function askAboutQuery() {
  const q = findQuery;
  let asked;
  try {
    asked = window.aeval.invoke('find:to-ask', {});
  } catch (err) {
    asked = Promise.reject(err);
  }
  Promise.resolve(asked)
    .then((r) => {
      if (r && r.status === 'no-hit') {
        send('find:close');
        send('query:submit', { question: q });
      }
    })
    .catch(() => send('query:submit', { question: q }));
}

function findRow(h) {
  const row = el('button', 'find-row');
  row.type = 'button';
  row.setAttribute('role', 'option');
  const on = h.i === findCurrent;
  row.setAttribute('aria-selected', String(on));
  // Roving focus: one row in the tab order, the current one.
  row.tabIndex = on ? 0 : -1;
  row.dataset.i = String(h.i);
  if (on) row.classList.add('current');
  if (h.offscreen) row.classList.add('offscreen');
  row.appendChild(kindMark(h.kind));
  const body = el('span', 'find-body');
  const snip = el('span', 'find-snippet');
  // The match itself in the snippet is set apart by weight, not colour.
  const text = String(h.text || '');
  const at = h.match ? text.toLowerCase().indexOf(String(h.match).toLowerCase()) : -1;
  if (at >= 0) {
    snip.append(text.slice(0, at));
    snip.appendChild(el('b', '', text.slice(at, at + h.match.length)));
    snip.append(text.slice(at + h.match.length));
  } else {
    snip.textContent = text;
  }
  body.appendChild(snip);
  if (h.why) body.appendChild(el('span', 'find-why', h.why));
  row.appendChild(body);
  row.addEventListener('click', () => goTo(h.i, false));
  row.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    send('ui:context-menu', { kind: 'hit', index: h.i, text: h.match || h.text });
  });
  return row;
}

function renderFind() {
  const hadFocus = findView.contains(document.activeElement);
  findView.innerHTML = '';
  findView.setAttribute('aria-label', findQuery ? `Matches for ${findQuery}` : 'Matches');
  if (!findHits.length) renderEmpty();
  for (const h of findHits) findView.appendChild(findRow(h));
  const cur = findView.querySelector('.find-row.current') || findView.querySelector('.find-row');
  if (cur) {
    cur.tabIndex = 0;
    cur.scrollIntoView({ block: 'nearest' });
    if (hadFocus) cur.focus({ preventScroll: true });
  }
  requestResize();
}

// The ring follows the row: focus moves at once, Rust moves the ring.
function goTo(i, focus) {
  findCurrent = i;
  for (const r of findView.querySelectorAll('.find-row')) {
    const on = Number(r.dataset.i) === i;
    r.classList.toggle('current', on);
    r.setAttribute('aria-selected', String(on));
    r.tabIndex = on ? 0 : -1;
    if (on && focus) {
      r.focus();
      r.scrollIntoView({ block: 'nearest' });
    }
  }
  send('find:step', { by: { index: i } });
}

findView.addEventListener('keydown', (e) => {
  if (!e.target.classList || !e.target.classList.contains('find-row') || e.ctrlKey || e.altKey || e.metaKey) return;
  const rows = [...findView.querySelectorAll('.find-row')];
  const at = rows.indexOf(e.target);
  let to = -1;
  if (e.key === 'ArrowDown') to = Math.min(rows.length - 1, at + 1);
  else if (e.key === 'ArrowUp') to = Math.max(0, at - 1);
  else if (e.key === 'Home') to = 0;
  else if (e.key === 'End') to = rows.length - 1;
  else return;
  e.preventDefault();
  e.stopPropagation();
  if (to >= 0 && to !== at) goTo(Number(rows[to].dataset.i), true);
});

listen('find:hits', ({ hits, current, query }) => {
  findQuery = String(query || findQuery || '');
  if (view === 'find' && query) window.setSheetTitle('Find', query, true);
  findHits = hits || [];
  findCurrent = typeof current === 'number' ? current : -1;
  if (view === 'find') renderFind();
});

listen('find:state', (s) => {
  findStatus = s;
  if (view === 'find' && !findHits.length) renderFind();
});

// ------------------------------------------------------------------- trail

// The running (or last) trail: { el, list, plan, rows, meta, stop, status,
// toggle, task, ended, card, steps, said }.
let block = null;

// What a screen reader hears before each row's words (U27).
const SR_PREFIX = { pending: 'Planned: ', running: 'Now: ', done: 'Done: ', skipped: 'Skipped: ', failed: 'Failed: ', paused: 'Now: ' };

function srPrefix(state) {
  return el('span', 'sr-only', SR_PREFIX[state] || SR_PREFIX.done);
}

function newBlock(task) {
  const wrap = el('div', 'msg trail');
  const head = el('div', 'trail-head');
  // No "Task" word here: the title bar's glyph and title already name it.
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
  stop.title = 'Stop (Esc)';
  stop.setAttribute('aria-keyshortcuts', 'Escape');
  stop.addEventListener('click', () => {
    stop.disabled = true;
    stop.textContent = 'Stopping';
    send('agent:stop');
  });
  meta.appendChild(stop);
  wrap.appendChild(meta);
  showSheet();
  chatEl.appendChild(wrap);
  wrap.dataset.state = 'planning';
  block = { el: wrap, list, plan: null, rows: new Map(), meta, stop, status, toggle, task, ended: false, card: null, steps: 0, said: new Set() };
  syncOrbit();
  window.syncCopyAll();
  scrollToBottom(true);
  requestResize();
  return block;
}

const STATE_MARK = { skipped: 'Skipped', failed: 'Failed' };

function isAct(r) {
  return r.kind === 'action' || r.kind === 'act';
}

function sideOf(r) {
  const side = el('span', 'trow-side');
  if (r.state === 'running' && r.kind !== 'thinking') {
    const dots = el('span', 'think-dots small');
    dots.setAttribute('aria-hidden', 'true');
    dots.innerHTML = '<i></i><i></i><i></i>';
    side.appendChild(dots);
  } else if (STATE_MARK[r.state]) {
    side.textContent = STATE_MARK[r.state];
  } else if (r.ms >= 1500) {
    // Only a step slow enough to notice says how long it took.
    side.textContent = `${(r.ms / 1000).toFixed(1)}s`;
  }
  return side;
}

function noteOf(r) {
  return r.error || (r.state === 'skipped' && r.why ? `Not done: ${r.why}` : '');
}

function rowMenu(target, text) {
  target.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    send('ui:context-menu', {
      kind: 'row',
      text,
      ended: !!(block && block.ended),
      task: block ? block.task : '',
    });
  });
}

// A row of its own in the trail: thinking, a note, a question, or an act
// with no plan item to sit under.
function rowEl(r) {
  const li = el('li', `trow ${r.kind} ${r.state || ''}`);
  li.dataset.id = String(r.id);
  const mark = el('span', 'trow-mark');
  mark.setAttribute('aria-hidden', 'true');
  li.appendChild(mark);
  const body = el('span', 'trow-body');
  if (r.kind === 'thinking') {
    const dots = el('span', 'think-dots');
    dots.setAttribute('aria-hidden', 'true');
    dots.innerHTML = '<i></i><i></i><i></i>';
    body.appendChild(dots);
    body.appendChild(el('span', 'stage-label', r.text));
  } else {
    const text = el('span', 'trow-text');
    text.append(srPrefix(r.state), String(r.text || ''));
    body.appendChild(text);
    const note = noteOf(r);
    if (note) body.appendChild(el('span', 'trow-note', note));
    if (r.kind === 'ask' && r.state === 'running') {
      // The answer is typed in the bar, the app's one input (REQUESTS.md 9);
      // the bar knows a question is waiting from overlay:state and sends
      // what is typed as agent:continue to this run.
      body.appendChild(el('span', 'trow-note waiting', 'Answer in the bar'));
    } else if (r.kind === 'ask' && r.answer) {
      body.appendChild(el('span', 'trow-note', `You: ${r.answer}`));
    }
  }
  li.appendChild(body);
  li.appendChild(sideOf(r));
  rowMenu(li, r.text);
  return li;
}

// An act under the plan item it serves: a 13 px detail line.
function actEl(r) {
  const li = el('li', `plan-act ${r.state || ''}`);
  li.dataset.id = String(r.id);
  const text = el('span', 'plan-act-text');
  text.append(srPrefix(r.state), String(r.text || ''));
  const note = noteOf(r);
  if (note) text.append(` · ${note}`);
  li.appendChild(text);
  li.appendChild(sideOf(r));
  rowMenu(li, r.text);
  return li;
}

// The plan: one item per step, each holding the acts it took.
function planEl(r) {
  const li = el('li', 'trow plan');
  li.dataset.id = String(r.id);
  const ol = el('ol', 'plan-steps');
  ol.setAttribute('aria-label', 'Plan');
  const items = (r.steps || []).map((s) => {
    const item = el('li', 'plan-item pending');
    const mark = el('span', 'plan-mark');
    mark.setAttribute('aria-hidden', 'true');
    const pre = srPrefix('pending');
    const text = el('span', 'plan-text');
    text.append(pre, String(s));
    const acts = el('ol', 'plan-acts');
    item.append(mark, text, acts);
    ol.appendChild(item);
    return { li: item, pre, acts, state: 'pending' };
  });
  li.appendChild(ol);
  rowMenu(li, (r.steps || []).join('; '));
  return { li, items, id: r.id };
}

// Each plan item's state from the acts so far. With planIndex an item is
// done once a later one has started; without it, item i is done when i acts
// have finished (F22).
function syncPlan() {
  const plan = block && block.plan;
  if (!plan || !plan.items.length) return;
  const acts = [...block.rows.values()].filter((x) => isAct(x.r));
  const indexed = acts.some((x) => typeof x.r.planIndex === 'number');
  const n = plan.items.length;
  let cur;
  if (indexed) {
    cur = acts.reduce((m, x) => (typeof x.r.planIndex === 'number' ? Math.max(m, x.r.planIndex) : m), -1);
    if (cur < 0) cur = 0;
  } else {
    cur = acts.filter((x) => x.r.state === 'done' || x.r.state === 'skipped').length;
  }
  const ended = block.ended ? block.el.dataset.state : '';
  plan.items.forEach((item, i) => {
    let s;
    if (ended === 'done') s = 'done';
    else if (i < cur) s = 'done';
    else if (i === cur && i < n) {
      const mine = indexed ? acts.filter((x) => x.r.planIndex === i) : [];
      const last = mine[mine.length - 1];
      if (last && last.r.state === 'failed') s = 'failed';
      else if (ended) s = ended === 'stopped' ? 'pending' : 'failed';
      else s = 'running';
    } else s = 'pending';
    if (item.state === s) return;
    item.state = s;
    item.li.className = `plan-item ${s}`;
    item.pre.textContent = SR_PREFIX[s];
  });
  // Acts that sit under no item, once the plan is the checklist, wait behind
  // the steps toggle; a failed one stays in sight.
  for (const x of block.rows.values()) {
    if (isAct(x.r) && x.li.classList.contains('trow')) x.li.classList.toggle('behind', x.r.state !== 'running');
  }
}

function underPlan(r) {
  const plan = block.plan;
  if (!plan || typeof r.planIndex !== 'number') return null;
  const item = plan.items[r.planIndex];
  return item ? item.acts : null;
}

function putRow(r) {
  if (!block) newBlock('');
  const old = block.rows.get(r.id);
  if (r.kind === 'remove') {
    if (old) old.li.remove();
    block.rows.delete(r.id);
    requestResize();
    return;
  }
  if (r.kind === 'message') return; // the final message is its own answer
  if (r.kind === 'plan') {
    const plan = planEl(r);
    if (block.plan && block.plan.li.isConnected) block.plan.li.replaceWith(plan.li);
    else block.list.insertBefore(plan.li, block.list.firstChild);
    block.plan = plan;
    // A new plan takes back the acts that name one of its items.
    for (const x of block.rows.values()) {
      if (!isAct(x.r)) continue;
      const into = underPlan(x.r);
      const li = into ? actEl(x.r) : rowEl(x.r);
      x.li.replaceWith(li);
      if (into) into.appendChild(li);
      x.li = li;
    }
  } else {
    const into = isAct(r) ? underPlan(r) : null;
    const li = into ? actEl(r) : rowEl(r);
    if (old && old.li.parentElement === (into || block.list)) old.li.replaceWith(li);
    else {
      if (old) old.li.remove();
      (into || block.list).appendChild(li);
    }
    block.rows.set(r.id, { li, r });
    hear(r, old && old.r);
  }
  syncPlan();
  scrollToBottom(false);
  requestResize();
}

// --------------------------------------------------------------- heard

// Each new act is announced as it starts, at most one every 2 s (the latest
// waits for the gap); a failure is said at once (U27).
const STEP_SAY_GAP = 2000;
let lastStepSaid = -Infinity;
let pendingStep = null;
let stepTimer = null;

function sayStep(text) {
  const now = performance.now();
  const wait = lastStepSaid + STEP_SAY_GAP - now;
  if (wait <= 0) {
    lastStepSaid = now;
    say(text, 'polite');
    return;
  }
  pendingStep = text;
  if (stepTimer) return;
  stepTimer = setTimeout(() => {
    stepTimer = null;
    const t = pendingStep;
    pendingStep = null;
    if (t) sayStep(t);
  }, wait);
}

function hear(r, before) {
  if (!block) return;
  if (r.kind === 'ask' && r.state === 'running' && !block.said.has(`ask${r.id}`)) {
    block.said.add(`ask${r.id}`);
    say(`The task has a question: ${r.text}${/[.?!]$/.test(r.text) ? '' : '.'} Type your answer in the bar.`, 'assertive');
    return;
  }
  if (!isAct(r)) return;
  if (!before) {
    block.steps += 1;
    r.__n = block.steps;
  } else {
    r.__n = before.__n;
  }
  if (r.state === 'failed' && !block.said.has(`fail${r.id}`)) {
    block.said.add(`fail${r.id}`);
    say(`Step ${r.__n} failed: ${r.text}${r.error ? `. ${r.error}` : ''}`, 'assertive');
  } else if (!before && r.state !== 'failed') {
    sayStep(`Step ${r.__n}: ${r.text}`);
  }
}

// ---------------------------------------------------------------- state

const STATUS_TEXT = {
  planning: 'Planning…',
  thinking: 'Thinking…',
  running: 'Working',
  confirm: 'Waiting for you',
  paused: 'Paused',
  plan: 'Waiting for you',
};

function statusText(state, step) {
  // The row says the question is waiting in the bar; the head only counts.
  if (state === 'asking') return step ? `Step ${step}` : '';
  const s = STATUS_TEXT[state];
  if (!s) return '';
  return step ? `${s} · step ${step}` : s;
}

listen('agent:begin', ({ task }) => {
  window.setSheetTitle('Task', task);
  newBlock(task);
});

listen('agent:step', (r) => putRow(r));

listen('agent:state', ({ state, message, step }) => {
  if (!block) return;
  block.status.textContent = statusText(state, step);
  if (block.el.dataset.state !== state && state === 'paused') say(`Task paused. ${message || ''}`, 'polite');
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
    go.addEventListener('click', () => once(go, 'agent:continue', {}));
    banner.appendChild(go);
    // Enter on this button is the only Enter that continues a run; nothing
    // is taken from other apps for it.
    setTimeout(() => go.focus(), 30);
  } else if (banner) {
    banner.remove();
  }
  requestResize();
});

// --------------------------------------------------------------- confirm

// The buttons wait 700 ms, a fill sweeping across them, so a key already on
// its way cannot answer a card it never saw (U45). Allow also waits until
// the whole text that will be typed has been seen (B-007).
const CONFIRM_SETTLE_MS = 700;
let reasonSeq = 0;

function settle(btn, then) {
  btn.setAttribute('aria-disabled', 'true');
  btn.classList.remove('settling');
  void btn.offsetWidth;
  btn.classList.add('settling');
  clearTimeout(btn.__settle);
  btn.__settle = setTimeout(() => {
    btn.classList.remove('settling');
    if (!then || then()) btn.removeAttribute('aria-disabled');
  }, CONFIRM_SETTLE_MS);
}

listen('agent:confirm-request', ({ id, text, reason, why, url, value, truncated }) => {
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
  let box = null;
  if (typeof value === 'string' && value) {
    box = el('pre', 'confirm-value', value);
    box.setAttribute('aria-label', 'Text that will be typed');
    box.tabIndex = 0;
    card.appendChild(box);
  }
  // One line: what it does, and why it stopped to ask.
  const says = [reason ? `${reason.charAt(0).toUpperCase()}${reason.slice(1)}.` : '', why ? `Asked because ${why}.` : '']
    .filter(Boolean)
    .join(' ');
  if (says) {
    const p = el('p', 'confirm-reason', says);
    reasonSeq += 1;
    p.id = `confirm-reason-${reasonSeq}`;
    card.appendChild(p);
    card.setAttribute('aria-describedby', p.id);
  }
  const actions = el('div', 'confirm-actions');
  const skip = el('button', 'retry-btn', 'Skip');
  skip.type = 'button';
  const allow = el('button', 'accent-btn', 'Allow');
  allow.type = 'button';
  let answered = false;
  const answer = (btn, yes) => {
    if (answered || btn.getAttribute('aria-disabled') === 'true') return;
    answered = true;
    allow.disabled = true;
    skip.disabled = true;
    send('agent:confirm', { id, allow: yes });
  };
  skip.addEventListener('click', () => answer(skip, false));
  allow.addEventListener('click', () => answer(allow, true));
  actions.append(skip, allow);
  card.appendChild(actions);
  card.dataset.id = String(id);
  block.card = card;
  block.el.insertBefore(card, block.meta);

  // A long value that was cut short in the card's sentence has to be seen to
  // its end, scrolled or shown whole, before Allow works.
  let seen = true;
  if (box && truncated && box.scrollHeight > box.clientHeight + 1) {
    seen = false;
    const more = el('button', 'confirm-more', 'Show all');
    more.type = 'button';
    card.insertBefore(more, box.nextSibling);
    const reveal = () => {
      if (seen) return;
      seen = true;
      if (!allow.classList.contains('settling')) settle(allow);
    };
    box.addEventListener(
      'scroll',
      () => {
        if (box.scrollTop + box.clientHeight >= box.scrollHeight - 2) reveal();
      },
      { passive: true }
    );
    more.addEventListener('click', () => {
      box.classList.add('all');
      more.remove();
      reveal();
      requestResize();
    });
  }
  settle(skip);
  settle(allow, () => seen);

  setTimeout(() => card.focus(), 30);
  // The how-to is heard, not shown.
  const heard = `Asking first: ${text}${why ? `. Asking because ${why}` : ''}.${box ? ' The text that will be typed is in the card.' : ''} Tab to Skip or Allow.`;
  say(heard, 'assertive');
  scrollToBottom(true);
  requestResize();
});

// ------------------------------------------------------------------ done

const END_TONE = { done: 'done', stopped: 'quiet', error: 'error', limit: 'error', offline: 'error' };

listen('agent:done', ({ state, text, usage }) => {
  if (!block) newBlock('');
  const line = window.usageLine && window.usageLine(usage && { ...usage, cost: usage.costKnown ? usage.cost : NaN });
  if (line) {
    const u = el('span', 'usage', `${usage.calls || 0} model calls`);
    u.title = `${usage.calls || 0} model calls · ${line}`;
    block.meta.appendChild(u);
  }
  say(state === 'done' ? 'Task done.' : state === 'stopped' ? 'Task stopped.' : 'The task did not finish.', state === 'done' || state === 'stopped' ? 'polite' : 'assertive');
  block.ended = true;
  block.el.dataset.state = state;
  block.el.classList.add('ended');
  syncOrbit();
  syncPlan();
  block.status.textContent = state === 'done' ? 'Done' : state === 'stopped' ? 'Stopped' : 'Did not finish';
  block.stop.hidden = true;
  clearTimeout(stepTimer);
  stepTimer = null;
  pendingStep = null;
  // The steps fold under the outcome; a run that failed keeps them open,
  // since which one failed is the point.
  const steps = [...block.rows.values()].filter((x) => isAct(x.r)).length;
  if (steps) {
    block.toggle.hidden = false;
    block.toggle.textContent = `${steps} step${steps === 1 ? '' : 's'}`;
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
      send('clipboard:write', { text }).then((r) => {
        if (!copied(r)) return;
        copy.textContent = 'Copied';
        setTimeout(() => {
          copy.textContent = 'Copy';
        }, 1500);
      });
    });
    block.meta.insertBefore(copy, block.meta.firstChild);
  } else if (text) {
    // A failed row that already says why leaves the banner only the remedy
    // (Rust words it, B-143); here it is shown once, under the trail.
    const row = el('div', `msg ${tone === 'error' ? 'error-row' : 'note-row'}`);
    if (tone === 'error') row.appendChild(icon('info', 'row-glyph'));
    row.appendChild(el('span', 'error-text', text));
    if (tone === 'error' && block.task) {
      const retry = el('button', 'retry-btn', 'Retry');
      retry.type = 'button';
      const task = block.task;
      retry.addEventListener('click', () => once(retry, 'agent:start', { task }));
      row.appendChild(retry);
    }
    chatEl.appendChild(row);
  }
  window.syncCopyAll();
  scrollToBottom(true);
  requestResize();
});

// --------------------------------------------------------------- consent

// Who receives the screen, by name (C-30): "OpenRouter (Gemma 3 27B)".
function recipientsOf(p) {
  const list = Array.isArray(p.recipients) && p.recipients.length ? p.recipients : [p];
  const seen = new Set();
  const out = [];
  for (const r of list) {
    const who = String(r.providerName || r.name || r.provider || r.id || '').trim();
    const what = String(r.modelName || r.model || '').trim();
    const s = who && what ? `${who} (${what})` : who || what;
    if (s && !seen.has(s)) {
      seen.add(s);
      out.push(s);
    }
  }
  return out;
}

function joinNames(names) {
  if (names.length <= 1) return names[0] || 'the model set for tasks';
  return `${names.slice(0, -1).join(', ')} and ${names[names.length - 1]}`;
}

// The first run asks once, in the answer window, before anything moves.
listen('agent:consent-request', (p) => {
  const task = p && p.task;
  showSheet();
  const sheet = el('div', 'msg consent');
  sheet.appendChild(el('p', 'consent-title', 'Let AevalBuddy use your computer?'));
  const ul = el('ul', 'consent-list');
  ul.appendChild(el('li', '', 'It can see, click and type in any app, the way you would.'));
  ul.appendChild(el('li', '', 'It asks first before anything that sends, deletes, buys, signs or opens a link.'));
  ul.appendChild(el('li', '', 'Move the mouse to pause it. Press Esc to stop it.'));
  const sends = `Each step sends a screenshot and the window's text to ${joinNames(recipientsOf(p || {}))}.`;
  const free = p && (p.free === true || (Array.isArray(p.recipients) && p.recipients.some((r) => r && r.free === true)));
  ul.appendChild(el('li', '', free ? `${sends} Free models' providers may keep and train on what they receive.` : sends));
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
    send('agent:consent', { allow: yes, task });
  };
  later.addEventListener('click', () => done(false));
  allow.addEventListener('click', () => done(true));
  actions.append(later, allow);
  sheet.appendChild(actions);
  chatEl.appendChild(sheet);
  // Focus on the sheet, not on Allow: Enter alone must not grant this.
  setTimeout(() => sheet.focus(), 30);
  say('Let AevalBuddy use your computer? Tab to Not now or Allow.', 'assertive');
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

listen('chat:reset', () => {
  block = null;
});

listen('chat:sync', () => {
  block = null;
});
