'use strict';

// The history list. Rows are rebuilt from the main process on every change
// rather than mutated in place: the list is at most two hundred short rows,
// and one render path is worth more than the frames it costs.

const listEl = document.getElementById('list');
const emptyEl = document.getElementById('empty');
const searchEl = document.getElementById('search');
const clearSearchBtn = document.getElementById('clear-search');
const countEl = document.getElementById('count');
const clearAllBtn = document.getElementById('clear-all');
const confirmEl = document.getElementById('confirm');
const confirmYes = document.getElementById('confirm-yes');
const confirmNo = document.getElementById('confirm-no');
const closeBtn = document.getElementById('close-history');

let query = '';
let rows = [];
let searchTimer = null;

// ------------------------------------------------------------------- dates

const MIN = 60 * 1000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

// Relative up to a week, then the date. "Tuesday" stops meaning anything once
// there is more than one of them in the list.
function when(at) {
  const delta = Date.now() - at;
  if (delta < MIN) return 'just now';
  if (delta < HOUR) return `${Math.floor(delta / MIN)}m ago`;
  if (delta < DAY) return `${Math.floor(delta / HOUR)}h ago`;
  const d = new Date(at);
  const days = Math.floor(delta / DAY);
  if (days === 1) return 'yesterday';
  if (days < 7) return d.toLocaleDateString(undefined, { weekday: 'long' });
  return d.toLocaleDateString(undefined, { day: 'numeric', month: 'short' });
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

function row(entry) {
  const item = document.createElement('button');
  item.type = 'button';
  item.className = 'item';
  item.dataset.id = entry.id;

  const q = document.createElement('span');
  q.className = 'q';
  q.appendChild(withHighlight(entry.title, query));
  item.appendChild(q);

  if (entry.reply) {
    const a = document.createElement('span');
    a.className = 'a';
    a.appendChild(withHighlight(entry.reply, query));
    item.appendChild(a);
  }

  const meta = document.createElement('span');
  meta.className = 'meta';
  const asked = entry.asked || 1;
  meta.textContent = [
    when(entry.at),
    `${asked} ${asked === 1 ? 'question' : 'questions'}`,
  ].join('  ·  ');
  item.appendChild(meta);

  const del = document.createElement('span');
  del.className = 'del';
  del.setAttribute('role', 'button');
  del.setAttribute('tabindex', '0');
  del.setAttribute('aria-label', `Delete "${entry.title}"`);
  del.innerHTML =
    '<svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true" fill="none"' +
    ' stroke="currentColor" stroke-width="2" stroke-linecap="round">' +
    '<line x1="18" y1="6" x2="6" y2="18"></line>' +
    '<line x1="6" y1="6" x2="18" y2="18"></line></svg>';
  item.appendChild(del);

  return item;
}

function render() {
  listEl.textContent = '';
  for (const entry of rows) listEl.appendChild(row(entry));

  const none = rows.length === 0;
  emptyEl.hidden = !none;
  listEl.hidden = none;
  if (none) {
    emptyEl.textContent = query
      ? `Nothing matches “${query}”.`
      : 'Conversations you have will be listed here. Nothing is saved until you ask something.';
  }

  countEl.textContent = rows.length
    ? `${rows.length} ${rows.length === 1 ? 'conversation' : 'conversations'}`
    : '';
  clearAllBtn.hidden = !rows.length && !query;
  clearSearchBtn.hidden = !query;
}

function refresh() {
  window.aeval.invoke('history:list', { query }).then((result) => {
    rows = (result && result.rows) || [];
    render();
  });
}

// ------------------------------------------------------------ interactions

searchEl.addEventListener('input', () => {
  // The filter runs in the main process over every message body; typing fast
  // should not queue one pass per keystroke.
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    query = searchEl.value.trim();
    refresh();
  }, 120);
});

searchEl.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && searchEl.value) {
    // Escape clears the filter first and only closes the window once there is
    // nothing left to clear.
    e.stopPropagation();
    e.preventDefault();
    resetSearch();
  }
});

function resetSearch() {
  searchEl.value = '';
  query = '';
  refresh();
  searchEl.focus();
}

clearSearchBtn.addEventListener('click', resetSearch);

listEl.addEventListener('click', (e) => {
  const del = e.target.closest('.del');
  const item = e.target.closest('.item');
  if (!item) return;
  if (del) {
    e.stopPropagation();
    window.aeval.invoke('history:remove', { id: item.dataset.id }).then(refresh);
    return;
  }
  window.aeval.invoke('history:open', { id: item.dataset.id });
});

listEl.addEventListener('keydown', (e) => {
  const del = e.target.closest && e.target.closest('.del');
  if (!del || (e.key !== 'Enter' && e.key !== ' ')) return;
  const item = del.closest('.item');
  e.preventDefault();
  e.stopPropagation();
  window.aeval.invoke('history:remove', { id: item.dataset.id }).then(refresh);
});

clearAllBtn.addEventListener('click', () => {
  clearAllBtn.hidden = true;
  confirmEl.hidden = false;
  confirmYes.focus();
});

confirmNo.addEventListener('click', () => {
  confirmEl.hidden = true;
  clearAllBtn.hidden = false;
  clearAllBtn.focus();
});

confirmYes.addEventListener('click', () => {
  window.aeval.invoke('history:clear').then(() => {
    confirmEl.hidden = true;
    clearAllBtn.hidden = false;
    refresh();
  });
});

closeBtn.addEventListener('click', () => window.aeval.invoke('history:close'));

window.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') window.aeval.invoke('history:close');
});

// The main process pushes a refresh when a conversation is saved while this
// window is open, so the list is never stale behind a live chat.
window.aeval.on('history:changed', refresh);

refresh();
searchEl.focus();
