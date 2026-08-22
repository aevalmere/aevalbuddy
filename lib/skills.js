'use strict';

// Skills are markdown procedures that ride along with every request. The
// model is told how to think about the screen before it is told what the
// user asked, which is what keeps a small fast model on the rails: the
// answer shape is enforced outside the model rather than hoped for.
//
// One file per skill in skills/, front matter for the label:
//
//   ---
//   name: Screen assistant
//   summary: one line for the settings picker
//   ---
//   the procedure, sent verbatim
//
// Files load once at boot. A packaged build ships the same directory, so
// adding a skill is dropping in a file.

const fs = require('fs');
const path = require('path');

const DIR = path.join(__dirname, '..', 'skills');
const DEFAULT_ID = 'screen-assist';

// Order in the settings picker; anything else follows alphabetically.
const ORDER = ['screen-assist', 'debug', 'explain', 'rewrite'];

let cache = null;

function parse(id, raw) {
  const text = String(raw).replace(/^﻿/, '');
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/);
  const head = m ? m[1] : '';
  const body = (m ? m[2] : text).trim();
  const field = (key) => {
    const hit = head.match(new RegExp(`^${key}\\s*:\\s*(.+)$`, 'im'));
    return hit ? hit[1].trim() : '';
  };
  return {
    id,
    name: field('name') || id,
    summary: field('summary') || '',
    body,
  };
}

function load() {
  if (cache) return cache;
  let files = [];
  try {
    files = fs.readdirSync(DIR).filter((f) => f.toLowerCase().endsWith('.md'));
  } catch (err) {
    // A build that somehow shipped without the directory still runs; the
    // system prompt just loses its procedure section.
    cache = [];
    return cache;
  }
  const parsed = files
    .map((f) => {
      try {
        return parse(path.basename(f, path.extname(f)), fs.readFileSync(path.join(DIR, f), 'utf8'));
      } catch (err) {
        return null;
      }
    })
    .filter((s) => s && s.body);
  parsed.sort((a, b) => {
    const ia = ORDER.indexOf(a.id);
    const ib = ORDER.indexOf(b.id);
    if (ia !== -1 || ib !== -1) return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib);
    return a.name.localeCompare(b.name);
  });
  cache = parsed;
  return cache;
}

/** Every skill, without its body: enough for the settings picker. */
function list() {
  return load().map(({ id, name, summary }) => ({ id, name, summary }));
}

/** The skill to apply, falling back to the default and then to nothing. */
function get(id) {
  const all = load();
  return all.find((s) => s.id === id) || all.find((s) => s.id === DEFAULT_ID) || all[0] || null;
}

function has(id) {
  return load().some((s) => s.id === id);
}

module.exports = { list, get, has, DEFAULT_ID };
