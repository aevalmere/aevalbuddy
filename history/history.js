'use strict';

// The history list. Rows are rebuilt from Rust on every change rather than
// mutated in place: the list is at most two hundred short rows, and one
// render path is worth more than the frames it costs.
//
// A row is two buttons side by side: the conversation, and Delete. Delete
// hides the row at once and says so with Undo for six seconds; only then,
// or when the window goes, is history:remove sent.

const $ = (id) => document.getElementById(id);
const listEl = $('list');
const emptyEl = $('empty');
const emptyText = $('empty-text');
const emptyTurnOn = $('empty-turn-on');
const offNote = $('off-note');
const searchEl = $('search');
const clearSearchBtn = $('clear-search');
const countEl = $('count');
const noteEl = $('note');
const undoBtn = $('undo');
const clearAllBtn = $('clear-all');
const confirmEl = $('confirm');
const confirmText = $('confirm-text');
const confirmYes = $('confirm-yes');
const confirmNo = $('confirm-no');
const menuEl = $('row-menu');

let query = '';
let rows = [];
let enabled = true;
let searchTimer = null;
let pending = null; // { id, title, timer } while Undo is offered
let noteTimer = null;
let menuFor = null; // the row id the context menu is open on

function call(channel, payload) {
  try {
    return Promise.resolve(window.aeval.invoke(channel, payload)).catch(() => null);
  } catch {
    return Promise.resolve(null);
  }
}

function listen(channel, cb) {
  try {
    window.aeval.on(channel, cb);
  } catch {
    // Not allowed by this build's bridge yet.
  }
}

// ------------------------------------------------------------------- dates

const MIN = 60 * 1000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

function startOfToday() {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

// Rows are grouped by calendar day: Today, Yesterday, the weekday for the
// rest of the week, then Earlier. The label on a row never contradicts its
// group: relative today, the time under a day, the date under Earlier.
function dayGroup(at) {
  const t0 = startOfToday();
  if (at >= t0) return 'Today';
  if (at >= t0 - DAY) return 'Yesterday';
  if (at >= t0 - 6 * DAY) return new Date(at).toLocaleDateString(undefined, { weekday: 'long' });
  return 'Earlier';
}

function when(at) {
  const t0 = startOfToday();
  if (at >= t0) {
    const delta = Date.now() - at;
    if (delta < MIN) return 'Just now';
    if (delta < HOUR) return `${Math.floor(delta / MIN)} min ago`;
    return `${Math.floor(delta / HOUR)} h ago`;
  }
  if (at >= t0 - 6 * DAY) return new Date(at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  return new Date(at).toLocaleDateString(undefined, { day: 'numeric', month: 'short' });
}

// ------------------------------------------------------------------ render

// Marks the search term inside a row. Built from text nodes rather than
// innerHTML so a conversation about angle brackets cannot inject markup.
function withHighlight(text, term) {
  const frag = document.createDocumentFragment();
  const value = String(text || '');
  if (!term) {
    frag.appendChild(document.createTextNode(value));
    return frag;
  }
  const lower = value.toLowerCase();
  const needle = term.toLowerCase();
  let from = 0;
  for (;;) {
    const at = lower.indexOf(needle, from);
    if (at === -1) break;
    if (at > from) frag.appendChild(document.createTextNode(value.slice(from, at)));
    const mark = document.createElement('mark');
    mark.textContent = value.slice(at, at + needle.length);
    frag.appendChild(mark);
    from = at + needle.length;
  }
  frag.appendChild(document.createTextNode(value.slice(from)));
  return frag;
}

const TRASH =
  '<svg viewBox="0 0 20 20" width="16" height="16" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">' +
  '<path d="M4.5 6h11M8 6V4.5h4V6M6 6l.7 9.5h6.6L14 6M8.75 8.5v4.5M11.25 8.5v4.5" vector-effect="non-scaling-stroke"/></svg>';

function row(entry, n) {
  const item = document.createElement('div');
  item.className = 'item';
  item.setAttribute('role', 'listitem');
  item.dataset.id = entry.id;

  const open = document.createElement('button');
  open.type = 'button';
  open.className = 'item-open';
  open.id = `r${n}`;
  open.setAttribute('aria-labelledby', `r${n}-q`);
  open.setAttribute('aria-describedby', `r${n}-m r${n}-a`);

  const q = document.createElement('span');
  q.className = 'q';
  q.id = `r${n}-q`;
  q.appendChild(withHighlight(entry.title, query));
  open.appendChild(q);

  const a = document.createElement('span');
  a.className = 'a';
  a.id = `r${n}-a`;
  a.appendChild(withHighlight(entry.reply || '', query));
  open.appendChild(a);

  const meta = document.createElement('span');
  meta.className = 'meta';
  meta.id = `r${n}-m`;
  const asked = entry.asked || 1;
  meta.textContent = asked > 1 ? `${when(entry.at)} · ${asked} questions` : when(entry.at);
  open.appendChild(meta);
  item.appendChild(open);

  const del = document.createElement('button');
  del.type = 'button';
  del.className = 'del';
  del.setAttribute('aria-label', `Delete "${entry.title}"`);
  del.innerHTML = TRASH;
  item.appendChild(del);

  return item;
}

function visibleRows() {
  return rows.filter((r) => !pending || r.id !== pending.id);
}

function render() {
  const focusedId = document.activeElement && document.activeElement.closest && document.activeElement.closest('.item');
  const keepId = focusedId ? focusedId.dataset.id : null;
  listEl.textContent = '';
  const shown = visibleRows();
  let list = null;
  let name = '';
  let g = 0;
  shown.forEach((entry, i) => {
    const group = dayGroup(entry.at);
    if (!list || group !== name) {
      name = group;
      g += 1;
      const head = document.createElement('h2');
      head.className = 'group-head';
      head.id = `g${g}`;
      head.textContent = group;
      listEl.appendChild(head);
      list = document.createElement('div');
      list.className = 'rows';
      list.setAttribute('role', 'list');
      list.setAttribute('aria-labelledby', `g${g}`);
      listEl.appendChild(list);
    }
    list.appendChild(row(entry, i));
  });

  const none = shown.length === 0;
  emptyEl.hidden = !none;
  listEl.hidden = none;
  emptyTurnOn.hidden = true;
  if (none) {
    if (query) {
      emptyText.textContent = `Nothing matches "${query}".`;
    } else if (!enabled) {
      emptyText.textContent = 'History is off, so nothing is being saved. Turn it on in Settings, Privacy.';
      emptyTurnOn.hidden = false;
    } else {
      emptyText.textContent = 'Conversations you have will be listed here. Nothing is saved until you ask something.';
    }
  }
  offNote.hidden = enabled || none;

  renderFoot();

  const again = keepId && listEl.querySelector(`.item[data-id="${CSS.escape(keepId)}"] .item-open`);
  rove(again || listEl.querySelector('.item-open'));
  if (again) again.focus({ preventScroll: true });
}

// One row is in the tab order, with its Delete; arrows move between rows.
// Tab from the search reaches the list once, not two hundred times.
function rove(open) {
  for (const b of listEl.querySelectorAll('.item-open, .del')) b.tabIndex = -1;
  if (!open) return;
  open.tabIndex = 0;
  open.parentElement.querySelector('.del').tabIndex = 0;
}

listEl.addEventListener('focusin', (e) => {
  const item = e.target.closest('.item');
  if (item) rove(item.querySelector('.item-open'));
});

function renderFoot() {
  const n = visibleRows().length;
  const confirming = !confirmEl.hidden;
  const noting = !!noteEl.textContent;
  countEl.textContent = n && !query ? `${n} ${n === 1 ? 'conversation' : 'conversations'}` : '';
  countEl.hidden = confirming || noting || !countEl.textContent;
  noteEl.hidden = confirming || !noting;
  undoBtn.hidden = confirming || !pending;
  clearAllBtn.hidden = confirming || !n || !!query;
}

function refresh() {
  return call('history:list', { query }).then((result) => {
    rows = (result && result.rows) || [];
    if (result && typeof result.enabled === 'boolean') {
      enabled = result.enabled;
      render();
      return;
    }
    // An older build says whether history is on in settings:get only.
    call('settings:get').then((d) => {
      enabled = !(d && d.saveHistory === false);
      render();
    });
  });
}

// ------------------------------------------------------------- the note

function say(text, sticky) {
  clearTimeout(noteTimer);
  noteEl.textContent = text || '';
  renderFoot();
  if (text && !sticky) {
    noteTimer = setTimeout(() => {
      noteEl.textContent = '';
      renderFoot();
    }, 4000);
  }
}

// ---------------------------------------------------------------- delete

function commitPending() {
  if (!pending) return;
  const { id, timer } = pending;
  clearTimeout(timer);
  pending = null;
  call('history:remove', { id });
}

function remove(id) {
  const entry = rows.find((r) => r.id === id);
  if (!entry) return;
  // The row that takes focus next: the one after, else the one before.
  const order = visibleRows().map((r) => r.id);
  const at = order.indexOf(id);
  const nextId = order[at + 1] || order[at - 1] || null;
  commitPending();
  closeConfirm(false);
  pending = {
    id,
    title: entry.title,
    timer: setTimeout(() => {
      commitPending();
      rows = rows.filter((r) => r.id !== id);
      noteEl.textContent = '';
      render();
    }, 6000),
  };
  clearTimeout(noteTimer);
  noteEl.textContent = `Deleted "${entry.title}".`;
  render();
  const next = nextId && listEl.querySelector(`.item[data-id="${CSS.escape(nextId)}"] .item-open`);
  if (next) next.focus();
  else searchEl.focus();
}

function undo() {
  if (!pending) return;
  const { id, timer } = pending;
  clearTimeout(timer);
  pending = null;
  noteEl.textContent = '';
  render();
  const back = listEl.querySelector(`.item[data-id="${CSS.escape(id)}"] .item-open`);
  if (back) back.focus();
  say('Restored.');
}

undoBtn.addEventListener('click', undo);

// The window can go before the six seconds are up; the delete still holds.
window.addEventListener('pagehide', commitPending);

// --------------------------------------------------------------- confirm

function openConfirm() {
  const n = visibleRows().length;
  confirmText.textContent = n === 1 ? 'Delete the 1 conversation?' : `Delete all ${n} conversations?`;
  confirmEl.hidden = false;
  renderFoot();
  confirmNo.focus();
}

function closeConfirm(focus) {
  if (confirmEl.hidden) return;
  confirmEl.hidden = true;
  renderFoot();
  if (focus) clearAllBtn.focus();
}

clearAllBtn.addEventListener('click', openConfirm);
confirmNo.addEventListener('click', () => closeConfirm(true));

confirmYes.addEventListener('click', () => {
  const n = rows.length;
  if (pending) {
    clearTimeout(pending.timer);
    pending = null;
  }
  call('history:clear').then((r) => {
    confirmEl.hidden = true;
    if (!r || r.ok === false) {
      say("Couldn't delete the history.");
      clearAllBtn.focus();
      return;
    }
    rows = [];
    render();
    say(n === 1 ? 'Deleted 1 conversation.' : `Deleted ${n} conversations.`);
    searchEl.focus();
    refresh();
  });
});

// ---------------------------------------------------------------- search

searchEl.addEventListener('input', () => {
  clearSearchBtn.hidden = !searchEl.value;
  // The filter runs in Rust over every message body; typing fast should not
  // queue one pass per keystroke.
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    query = searchEl.value.trim();
    refresh();
  }, 120);
});

function resetSearch() {
  searchEl.value = '';
  clearSearchBtn.hidden = true;
  query = '';
  refresh();
  searchEl.focus();
}

clearSearchBtn.addEventListener('click', resetSearch);

// The field opens focused, but its ring waits for the first key or Tab: at
// rest it would be the loudest thing in the window.
document.documentElement.classList.add('quiet-focus');
window.addEventListener(
  'keydown',
  () => {
    document.documentElement.classList.remove('quiet-focus');
  },
  { once: true, capture: true }
);
window.addEventListener(
  'pointerdown',
  () => {
    document.documentElement.classList.remove('quiet-focus');
  },
  { once: true, capture: true }
);

// ------------------------------------------------------------- the rows

function rowButtons() {
  return [...listEl.querySelectorAll('.item-open')];
}

function openRow(id) {
  commitPending();
  call('history:open', { id });
}

listEl.addEventListener('click', (e) => {
  const item = e.target.closest('.item');
  if (!item) return;
  if (e.target.closest('.del')) {
    remove(item.dataset.id);
    return;
  }
  if (e.target.closest('.item-open')) openRow(item.dataset.id);
});

listEl.addEventListener('contextmenu', (e) => {
  const item = e.target.closest('.item');
  if (!item) return;
  e.preventDefault();
  openMenu(item, e.clientX, e.clientY);
});

// Up and Down move between rows (and back up to the search), Delete
// deletes with Undo, a printable key goes to the search.
listEl.addEventListener('keydown', (e) => {
  const open = e.target.closest('.item-open');
  if (!open) return;
  const all = rowButtons();
  const i = all.indexOf(open);
  const go = (b) => {
    if (!b) return;
    e.preventDefault();
    b.focus();
    b.scrollIntoView({ block: 'nearest' });
  };
  if (e.key === 'ArrowDown') go(all[i + 1]);
  else if (e.key === 'ArrowUp') {
    if (i === 0) {
      e.preventDefault();
      searchEl.focus();
    } else go(all[i - 1]);
  } else if (e.key === 'Home') go(all[0]);
  else if (e.key === 'End') go(all[all.length - 1]);
  else if (e.key === 'Delete') {
    e.preventDefault();
    remove(open.closest('.item').dataset.id);
  } else if (e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)) {
    e.preventDefault();
    const r = open.getBoundingClientRect();
    openMenu(open.closest('.item'), r.left + 24, r.top + r.height / 2);
  } else if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey && e.key !== ' ') {
    e.preventDefault();
    searchEl.focus();
    searchEl.value += e.key;
    searchEl.dispatchEvent(new Event('input'));
  }
});

searchEl.addEventListener('keydown', (e) => {
  if (e.key === 'ArrowDown') {
    const first = rowButtons()[0];
    if (first) {
      e.preventDefault();
      first.focus();
    }
  } else if (e.key === 'Enter') {
    const first = rowButtons()[0];
    if (first) {
      e.preventDefault();
      openRow(first.closest('.item').dataset.id);
    }
  }
});

// --------------------------------------------------------- context menu

function menuItems() {
  return [...menuEl.querySelectorAll('[role="menuitem"]')];
}

function openMenu(item, x, y) {
  menuFor = item.dataset.id;
  menuEl.hidden = false;
  const app = $('app').getBoundingClientRect();
  const w = menuEl.offsetWidth;
  const h = menuEl.offsetHeight;
  const left = Math.max(8, Math.min(x - app.left, app.width - w - 8));
  const top = Math.max(8, Math.min(y - app.top, app.height - h - 8));
  menuEl.style.left = `${left}px`;
  menuEl.style.top = `${top}px`;
  menuItems()[0].focus();
}

function closeMenu(focusRow) {
  if (menuEl.hidden) return;
  menuEl.hidden = true;
  const id = menuFor;
  menuFor = null;
  if (focusRow && id) {
    const b = listEl.querySelector(`.item[data-id="${CSS.escape(id)}"] .item-open`);
    if (b) b.focus();
  }
}

function questionOf(id) {
  const entry = rows.find((r) => r.id === id);
  if (!entry) return '';
  return entry.question || entry.title || '';
}

menuEl.addEventListener('click', (e) => {
  const b = e.target.closest('[data-cmd]');
  if (!b) return;
  const id = menuFor;
  const cmd = b.dataset.cmd;
  closeMenu(cmd !== 'delete' && cmd !== 'ask');
  if (!id) return;
  if (cmd === 'ask') {
    call('bar:prefill-open', { text: questionOf(id) }).then((r) => {
      if (r === null || (r && r.ok === false)) say("Couldn't open the bar.");
    });
  } else if (cmd === 'copy') {
    call('clipboard:write', { text: questionOf(id) }).then((r) => say(r === null ? "Couldn't copy the question." : 'Question copied.'));
  } else if (cmd === 'delete') {
    remove(id);
  }
});

menuEl.addEventListener('keydown', (e) => {
  const items = menuItems();
  const i = items.indexOf(document.activeElement);
  if (e.key === 'ArrowDown') {
    e.preventDefault();
    items[(i + 1) % items.length].focus();
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    items[(i - 1 + items.length) % items.length].focus();
  } else if (e.key === 'Home') {
    e.preventDefault();
    items[0].focus();
  } else if (e.key === 'End') {
    e.preventDefault();
    items[items.length - 1].focus();
  } else if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    closeMenu(true);
  } else if (e.key === 'Tab') {
    e.preventDefault();
    closeMenu(true);
  }
});

document.addEventListener('pointerdown', (e) => {
  if (!menuEl.hidden && !e.target.closest('#row-menu')) closeMenu(false);
});

// ---------------------------------------------------------------- the rest

async function turnOn() {
  const r = await call('settings:set-save-history', { on: true });
  if (r && r.saveHistory === false) return;
  enabled = true;
  await refresh();
  say('History is on.');
}

emptyTurnOn.addEventListener('click', turnOn);
$('off-turn-on').addEventListener('click', turnOn);

$('close-history').addEventListener('click', () => {
  commitPending();
  call('history:close');
});

// Esc: the menu, then the confirm, then the search text, then the window.
window.addEventListener('keydown', (e) => {
  if (e.key !== 'Escape') return;
  if (!menuEl.hidden) {
    closeMenu(true);
    return;
  }
  if (!confirmEl.hidden) {
    closeConfirm(true);
    return;
  }
  if (searchEl.value) {
    e.preventDefault();
    resetSearch();
    return;
  }
  commitPending();
  call('history:close');
});

// Rust pushes a refresh when a conversation is saved while this window is
// open, so the list is never stale behind a live chat.
listen('history:changed', refresh);

refresh();
searchEl.focus();
