'use strict';

// Conversations that outlive the process.
//
// The app was deliberately amnesiac and that was defensible as a default: a
// screen assistant sees whatever happens to be in front of you, and quietly
// keeping all of it is not obviously the user's wish. What it cost was the
// thing that makes a tool worth keeping open -- the answer that fixed your
// build yesterday is unrecoverable, and you ask it again.
//
// So: text only, never a screenshot and never the reading of one, on by
// default with a switch in settings and a clear-all beside it. What is written
// is exactly what the user could already read in the transcript.
//
// The file is small and rewritten whole. A conversation is a few kilobytes of
// text and the cap is two hundred of them, so there is no case for a database,
// and a temp-then-rename keeps a crash mid-write from leaving half a file.

const fs = require('fs');
const path = require('path');
const { app } = require('electron');

const MAX_ENTRIES = 200;
// Roughly four megabytes of prose. Long past the point where the list is
// useful to scroll, and small enough to read and write in one go.
const MAX_BYTES = 4 * 1024 * 1024;
const TITLE_MAX = 90;

let file = null;
let cache = null;
let writeTimer = null;
let enabled = true;

function location() {
  if (!file) file = path.join(app.getPath('userData'), 'history.json');
  return file;
}

function load() {
  if (cache) return cache;
  try {
    const raw = fs.readFileSync(location(), 'utf8');
    const parsed = JSON.parse(raw);
    cache = Array.isArray(parsed && parsed.conversations) ? parsed.conversations : [];
  } catch (err) {
    // Missing on first run, and unreadable if it was hand-edited or truncated.
    // Either way an empty list is the right recovery: losing history is bad,
    // refusing to start because of it is worse.
    cache = [];
  }
  return cache;
}

// Batched, because an upsert runs after every answer and a conversation of ten
// turns would otherwise rewrite the file ten times.
function scheduleWrite() {
  if (writeTimer) return;
  writeTimer = setTimeout(() => {
    writeTimer = null;
    flush();
  }, 400);
}

function flush() {
  if (!cache) return;
  const target = location();
  const tmp = `${target}.tmp`;
  try {
    fs.writeFileSync(tmp, JSON.stringify({ version: 1, conversations: cache }), 'utf8');
    fs.renameSync(tmp, target);
  } catch (err) {
    // A full or read-only profile directory must not take the app down; the
    // conversation is still on screen either way.
  }
}

/** Called on quit, where the debounce would never fire. */
function flushNow() {
  if (writeTimer) {
    clearTimeout(writeTimer);
    writeTimer = null;
  }
  flush();
}

function setEnabled(on) {
  enabled = !!on;
}

function firstLine(text) {
  const line = String(text || '')
    .replace(/\s+/g, ' ')
    .trim();
  return line.length > TITLE_MAX ? `${line.slice(0, TITLE_MAX - 1)}…` : line;
}

// Roughly how many bytes an entry costs in the written file. Its own text
// plus the keys and quoting around it; exact enough to decide whether the
// budget is anywhere near, which is all it is asked to do.
function entryBytes(e) {
  let n = 120 + e.title.length + e.model.length;
  for (const m of e.messages) n += m.text.length + m.role.length + 24;
  return n;
}

// The comment here used to say "only measure when the count alone could
// plausibly exceed the budget". There was no such guard: JSON.stringify ran
// over the whole history after every single answer, on the process that draws
// the UI, and again on every pop -- at the 4MB cap, a multi-megabyte serialise
// per answer to discover that nothing needed dropping. A comment describing a
// guard nobody wrote is the arrow-tip defect over again.
//
// Now the sizes are summed, which is one pass over lengths rather than a
// serialise, and entries come off the end until the running total fits. The
// tail is the oldest, because the list is sorted newest-first above.
function trim() {
  cache.sort((a, b) => b.at - a.at);
  if (cache.length > MAX_ENTRIES) cache.length = MAX_ENTRIES;
  let total = 0;
  const sizes = cache.map(entryBytes);
  for (const n of sizes) total += n;
  while (cache.length > 1 && total > MAX_BYTES) {
    total -= sizes.pop();
    cache.pop();
  }
}

/**
 * Writes the current conversation, replacing the previous version of it.
 *
 * @param {object} conversation
 * @param {string} conversation.id stable for the life of one chat
 * @param {Array<{role: string, text: string}>} conversation.messages
 * @param {string} [conversation.model]
 */
function save(conversation) {
  if (!enabled) return;
  const c = conversation || {};
  const messages = (Array.isArray(c.messages) ? c.messages : [])
    .filter((m) => m && (m.role === 'user' || m.role === 'assistant') && m.text)
    .map((m) => ({ role: m.role, text: String(m.text) }));
  if (!messages.length) return;
  const list = load();
  const at = Date.now();
  const first = messages.find((m) => m.role === 'user');
  const entry = {
    id: String(c.id),
    at,
    title: firstLine(first ? first.text : messages[0].text),
    model: String(c.model || ''),
    messages,
  };
  const i = list.findIndex((e) => e.id === entry.id);
  if (i === -1) list.push(entry);
  else list[i] = entry;
  trim();
  scheduleWrite();
}

/**
 * The list for the history window: enough to render a row, without the
 * transcript. Newest first, optionally filtered.
 *
 * @param {string} [query] matched against the title and every message
 */
function list(query) {
  const q = String(query || '')
    .trim()
    .toLowerCase();
  const rows = load()
    .filter((e) => {
      if (!q) return true;
      if (e.title.toLowerCase().includes(q)) return true;
      return e.messages.some((m) => m.text.toLowerCase().includes(q));
    })
    .map((e) => ({
      id: e.id,
      at: e.at,
      title: e.title,
      model: e.model,
      count: e.messages.length,
      // What the user did, not how many objects it took to store it.
      asked: e.messages.filter((m) => m.role === 'user').length,
      // The first thing the assistant said, which is what the user is
      // scanning for when they remember an answer but not the question.
      reply: firstLine(
        (e.messages.find((m) => m.role === 'assistant') || {}).text || ''
      ),
    }));
  rows.sort((a, b) => b.at - a.at);
  return rows;
}

function get(id) {
  return load().find((e) => e.id === String(id)) || null;
}

function remove(id) {
  const l = load();
  const i = l.findIndex((e) => e.id === String(id));
  if (i === -1) return false;
  l.splice(i, 1);
  scheduleWrite();
  return true;
}

function clear() {
  cache = [];
  scheduleWrite();
  return true;
}

function count() {
  return load().length;
}

module.exports = {
  save,
  list,
  get,
  remove,
  clear,
  count,
  setEnabled,
  flushNow,
  MAX_ENTRIES,
  MAX_BYTES,
  // The estimator in trim() is an approximation, and an approximation that
  // reads low would let the file grow past its cap. The self-check measures
  // the real serialised size against the budget, which is the only way to
  // know the estimate is on the safe side.
  measuredBytes: () => JSON.stringify({ version: 1, conversations: load() }).length,
};
