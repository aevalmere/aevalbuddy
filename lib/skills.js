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

// Which skill fits which kind of request. The reader can name one of these
// after looking at the screen, and the question is scored against the same
// vocabulary, so automatic selection has one set of names end to end.
const TASK_TO_SKILL = {
  debug: 'debug',
  explain: 'explain',
  rewrite: 'rewrite',
  assist: DEFAULT_ID,
};

// Scored against the question. Deliberately small and readable: a wrong
// guess costs a slightly worse answer, so a transparent rule beats a clever
// model call that adds a round trip.
const CUES = [
  {
    skill: 'debug',
    strong:
      /\b(errors?|exceptions?|traceback|stack ?trace|failing|failed|crash(ed|ing)?|bugs?|broken|stuck)\b/i,
    weak: /\b(fix|debug|red|warnings?|tests?|build|compile|lint|wont|cannot)\b/i,
  },
  {
    skill: 'explain',
    strong: /\b(what (is|are|does|do)|explain|how does|what.s (this|that)|meaning of)\b/i,
    weak: /\b(understand|difference|means?|purpose)\b/i,
  },
  {
    skill: 'rewrite',
    strong:
      /\b(rewrite|reword|rephrase|proofread|shorten|draft|make (this|it) (shorter|clearer|better|nicer|polite|professional))\b/i,
    weak: /\b(email|message|reply|wording|grammar|polish|tone)\b/i,
  },
];

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

/**
 * Picks the skill for one request.
 *
 * Two inputs, in order of trust. The reader has actually looked at the
 * screen, so a TASK line from it outranks anything guessed from wording. The
 * question itself is scored second, strong cues before weak ones. Anything
 * unrecognised lands on the general skill, which is the right answer for most
 * questions anyway.
 *
 * @param {object} opts
 * @param {string} [opts.question]
 * @param {string} [opts.task] a task word the reader supplied
 * @returns {{id: string, why: string}}
 */
function route(opts) {
  const o = opts || {};
  const task = String(o.task || '').trim().toLowerCase();
  if (TASK_TO_SKILL[task] && has(TASK_TO_SKILL[task])) {
    return { id: TASK_TO_SKILL[task], why: 'screen' };
  }
  const q = String(o.question || '');
  for (const cue of CUES) {
    if (has(cue.skill) && cue.strong.test(q)) return { id: cue.skill, why: 'asked' };
  }
  for (const cue of CUES) {
    if (has(cue.skill) && cue.weak.test(q)) return { id: cue.skill, why: 'asked' };
  }
  return { id: DEFAULT_ID, why: 'default' };
}

module.exports = { list, get, has, route, DEFAULT_ID, TASK_TO_SKILL };
