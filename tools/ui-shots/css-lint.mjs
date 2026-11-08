// The CSS gate for off-system values (B-145, design-visual F28 and F36).
// Three rules, checked per declaration:
//
//   radius   border-radius (any corner) written as 4, 5, 8, 10, 12, 14, 18
//            or 20 px instead of a radius token
//   grid     padding (any side) and gap (gap, row-gap, column-gap) values
//            off the 4 px grid; 2 px passes when the same or the line above
//            carries a /* hairline */ or /* optical */ comment
//   colour   a raw hex or rgb()/rgba() colour outside a :root token block
//            (a rule whose every selector starts with :root)
//
// var(...) is skipped, so a token, with or without a fallback, always passes;
// so are url(...) and quoted strings. bridge.test.mjs enforces every file a
// lane lists in its cssLintEnforce and reports the rest until B-148.
//
//   node tools/ui-shots/css-lint.mjs <file.css>...    lint, exit 1 on any
//   node tools/ui-shots/css-lint.mjs --self-test

import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const RADII = new Set([4, 5, 8, 10, 12, 14, 18, 20]);
const RADIUS = /^border(-(top|bottom)-(left|right)|-(start|end)-(start|end))?-radius$/;
const SPACING = /^(padding(-(top|right|bottom|left|inline|block)(-(start|end))?)?|gap|row-gap|column-gap|grid-gap|grid-row-gap|grid-column-gap)$/;

// Blanks comments (keeping newlines, so line numbers hold) and remembers
// which lines carry which comment text.
function splitComments(text) {
  const notes = new Map();
  let out = '';
  let line = 1;
  let i = 0;
  while (i < text.length) {
    if (text.startsWith('/*', i)) {
      const end = text.indexOf('*/', i + 2);
      const stop = end < 0 ? text.length : end + 2;
      const body = text.slice(i, stop);
      notes.set(line, `${notes.get(line) || ''}${body}`);
      for (const ch of body) {
        out += ch === '\n' ? '\n' : ' ';
        if (ch === '\n') line += 1;
      }
      i = stop;
      continue;
    }
    if (text[i] === '\n') line += 1;
    out += text[i];
    i += 1;
  }
  return { code: out, notes };
}

// The value with var(...), url(...) and strings taken out.
function bare(value) {
  let v = value.replace(/"[^"]*"|'[^']*'/g, ' ');
  for (const fn of ['var', 'url']) {
    let at = v.search(new RegExp(`\\b${fn}\\(`));
    while (at >= 0) {
      let depth = 0;
      let j = v.indexOf('(', at);
      for (; j < v.length; j += 1) {
        if (v[j] === '(') depth += 1;
        else if (v[j] === ')' && --depth === 0) break;
      }
      v = `${v.slice(0, at)} ${v.slice(j + 1)}`;
      at = v.search(new RegExp(`\\b${fn}\\(`));
    }
  }
  return v;
}

const pxValues = (v) => [...v.matchAll(/(-?\d*\.?\d+)px\b/g)].map((m) => Math.abs(Number(m[1])));

function judge(prop, value, line, inRoot, notes) {
  const out = [];
  const v = bare(value);
  const say = (rule, why) => out.push({ line, rule, text: `${prop}: ${value.trim()}`, why });
  if (RADIUS.test(prop)) {
    const off = pxValues(v).filter((n) => RADII.has(n));
    if (off.length) say('radius', `${off.join('px, ')}px is a radius token's value; use the token`);
  }
  if (SPACING.test(prop)) {
    const excused = /hairline|optical/i.test(`${notes.get(line) || ''} ${notes.get(line - 1) || ''}`);
    const off = pxValues(v).filter((n) => n % 4 !== 0 && !(n === 2 && excused));
    if (off.length) say('grid', `${off.join('px, ')}px is off the 4 px grid${off.includes(2) ? ' (2 px needs a /* hairline */ or /* optical */ comment)' : ''}`);
  }
  if (!inRoot) {
    const hex = v.match(/#(?:[0-9a-f]{8}|[0-9a-f]{6}|[0-9a-f]{3,4})\b/i);
    const rgb = v.match(/\brgba?\(/i);
    if (hex || rgb) say('colour', `${(hex || rgb)[0]}${rgb && !hex ? '...)' : ''} outside a :root token block; use a colour token`);
  }
  return out;
}

const isRoot = (selector) => selector.split(',').every((s) => s.trim().startsWith(':root'));

// Every violation in one stylesheet: [{ line, rule, text, why }].
export function lintCss(text) {
  const { code, notes } = splitComments(text);
  const out = [];
  const stack = [];
  let buf = '';
  let start = 1;
  let line = 1;
  let quote = '';
  let parens = 0;
  const flush = () => {
    const decl = buf.trim();
    buf = '';
    const top = stack[stack.length - 1];
    if (!decl || top === undefined || top.startsWith('@')) return;
    const m = /^(--[\w-]+|[a-zA-Z-]+)\s*:([\s\S]*)$/.exec(decl);
    if (!m) return;
    const value = m[2].replace(/!important\s*$/i, '');
    out.push(...judge(m[1].toLowerCase(), value, start, stack.some(isRoot), notes));
  };
  for (const ch of code) {
    if (quote) {
      if (ch === quote) quote = '';
    } else if (ch === '"' || ch === "'") {
      quote = ch;
    } else if (ch === '(') {
      parens += 1;
    } else if (ch === ')') {
      parens = Math.max(0, parens - 1);
    } else if (parens === 0 && ch === '{') {
      stack.push(buf.trim());
      buf = '';
      continue;
    } else if (parens === 0 && (ch === ';' || ch === '}')) {
      flush();
      if (ch === '}') stack.pop();
      if (ch === '\n') line += 1;
      continue;
    }
    if (!buf.trim() && ch.trim()) start = line;
    buf += ch;
    if (ch === '\n') line += 1;
  }
  return out;
}

function selfTest() {
  const cases = [
    ['a radius token passes', '.a { border-radius: var(--r-md, 12px); }', []],
    ['a raw radius fails', '.a {\n  border-radius: 12px;\n}', [['radius', 2]]],
    ['a corner radius fails', '.a { border-top-left-radius: 8px; }', [['radius', 1]]],
    ['an odd radius is not a token value', '.a { border-radius: 999px; }', []],
    ['the grid passes', '.a { padding: 4px 8px 12px 0; gap: 16px; }', []],
    ['off the grid fails', '.a { padding: 6px 8px; }', [['grid', 1]]],
    ['2 px needs a comment', '.a { gap: 2px; }', [['grid', 1]]],
    ['2 px with a hairline comment passes', '.a { padding-top: 2px; /* hairline */ }', []],
    ['the comment may sit on the line above', '.a {\n  /* optical */\n  column-gap: 2px;\n}', []],
    ['3 px is never excused', '.a { gap: 3px; /* optical */ }', [['grid', 1]]],
    ['colours in :root pass', ':root { --ink: #111; --glass: rgba(255, 255, 255, 0.4); }', []],
    ['colours in :root[data-theme] pass', ':root[data-theme="dark"], :root.x { --ink: #eee; }', []],
    ['colours in :root inside @media pass', '@media (prefers-color-scheme: dark) { :root { --ink: #eee; } }', []],
    ['a raw hex fails', '.a { color: #fff; }', [['colour', 1]]],
    ['a raw rgba fails', '.a {\n  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);\n}', [['colour', 2]]],
    ['a token colour passes', '.a { color: var(--ink, #111); background: url("data:image/svg+xml;utf8,<svg fill=\'%23fff\'/>"); }', []],
    ['comments are not code', '/* .a { color: #fff; } */ .b { margin: 3px; }', []],
    ['a selector with an id is not a colour', '#main { padding: 8px; }', []],
    ['keyframes colours fail', '@keyframes k { from { color: #000; } }', [['colour', 1]]],
    ['a local custom property colour fails', '.a { --tint: #abcdef; }', [['colour', 1]]],
  ];
  let bad = 0;
  for (const [name, css, want] of cases) {
    const got = lintCss(css).map((v) => [v.rule, v.line]);
    const ok = JSON.stringify(got) === JSON.stringify(want);
    if (!ok) bad += 1;
    console.log(`[CSSLINT] self-test ${name}=${ok ? 'ok' : `FAIL:got ${JSON.stringify(got)} want ${JSON.stringify(want)}`}`);
  }
  return bad;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const args = process.argv.slice(2);
  if (args[0] === '--self-test') process.exit(selfTest() ? 1 : 0);
  let n = 0;
  for (const f of args) {
    for (const v of lintCss(fs.readFileSync(f, 'utf8'))) {
      n += 1;
      console.log(`${f}:${v.line} ${v.rule}: ${v.text} (${v.why})`);
    }
  }
  console.log(`css-lint: ${args.length} files, ${n} violations`);
  process.exit(n ? 1 : 0);
}

export { selfTest };
