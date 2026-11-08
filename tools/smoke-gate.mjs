// The rust workflow's smoke gate. Reads what `AEVAL_SMOKE=1 AevalBuddy.exe`
// printed and fails when:
//   - any `[SMOKE] <label>=FAIL...` line is there,
//   - the run never printed `[SMOKE] done` (or printed `failed:`/`error=`),
//   - a label listed in tools/smoke-labels/<lane>.txt never printed.
// One label per line in those files; blank lines and `#` comments are
// ignored. A label is present when a line reads `[SMOKE] <label>=...`, or
// `[SMOKE] <label>` alone (that is how `done` prints).
//
//   node tools/smoke-gate.mjs <smoke stdout> [--labels <dir>] [--prefix SMOKE]
//   node tools/smoke-gate.mjs --self-test
//
// Prints `[GATE] ...` lines and exits 0 (pass) or 1 (fail).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));

export function readText(file) {
  const buf = fs.readFileSync(file);
  const text = buf[0] === 0xff && buf[1] === 0xfe ? buf.toString('utf16le') : buf.toString('utf8');
  return text.replace(/^﻿/, '');
}

// { lane: [labels] } from every <lane>.txt in dir.
export function readLabels(dir) {
  const lanes = {};
  if (!fs.existsSync(dir)) return lanes;
  for (const f of fs.readdirSync(dir).filter((n) => n.endsWith('.txt')).sort()) {
    lanes[f.slice(0, -4)] = readText(path.join(dir, f))
      .split(/\r?\n/)
      .map((l) => l.replace(/#.*$/, '').trim())
      .filter(Boolean);
  }
  return lanes;
}

// The verdict for one smoke log against the listed labels.
export function judge(log, lanes, prefix = 'SMOKE') {
  const tag = `[${prefix}] `;
  const lines = log.split(/\r?\n/).filter((l) => l.startsWith(tag)).map((l) => l.slice(tag.length));
  const seen = new Map();
  for (const l of lines) {
    const eq = l.indexOf('=');
    const label = eq < 0 ? l.trim() : l.slice(0, eq);
    if (!seen.has(label)) seen.set(label, []);
    seen.get(label).push(eq < 0 ? '' : l.slice(eq + 1));
  }
  // Report::must prints `=FAIL` or `=FAIL:<why>`.
  const fails = lines.filter((l) => /^[^=\s]+=FAIL(:|$)/.test(l));
  const done = lines.some((l) => l.trim() === 'done');
  const failedLine = lines.find((l) => l.startsWith('failed:'));
  const errorLine = lines.find((l) => l.startsWith('error='));
  const missing = [];
  const owners = new Map();
  for (const [lane, labels] of Object.entries(lanes)) {
    for (const label of labels) {
      if (!owners.has(label)) owners.set(label, []);
      owners.get(label).push(lane);
      if (!seen.has(label)) missing.push(`${lane}:${label}`);
    }
  }
  const shared = [...owners].filter(([, o]) => o.length > 1).map(([l, o]) => `${l} (${o.join(', ')})`);
  const problems = [];
  if (fails.length) problems.push(`${fails.length} FAIL line(s)`);
  if (!done) problems.push('no "done" line');
  if (failedLine) problems.push(failedLine);
  if (errorLine) problems.push(errorLine);
  if (missing.length) problems.push(`${missing.length} listed label(s) never printed`);
  return { ok: problems.length === 0, lines: lines.length, fails, done, missing, shared, problems, listed: owners.size };
}

function report(v, prefix) {
  const out = [];
  out.push(`[GATE] ${prefix} lines=${v.lines} listed=${v.listed}`);
  for (const f of v.fails) out.push(`[GATE] fail ${f}`);
  for (const m of v.missing) out.push(`[GATE] missing ${m}`);
  for (const s of v.shared) out.push(`[GATE] listed twice ${s}`);
  out.push(`[GATE] verdict=${v.ok ? 'ok' : `FAIL:${v.problems.join('; ')}`}`);
  return out;
}

function selfTest() {
  const lanes = { harness: ['version', 'done'], windows: ['outputMulti'] };
  const good = ['noise', '[SMOKE] version=1.1.1', '[SMOKE] outputMulti=ok', '[SMOKE] done'].join('\r\n');
  const cases = [
    ['passes', good, true],
    ['a FAIL line', good.replace('outputMulti=ok', 'outputMulti=FAIL:2 outputs'), false],
    ['a bare FAIL', good.replace('outputMulti=ok', 'outputMulti=FAIL'), false],
    ['no done', good.replace('[SMOKE] done', ''), false],
    ['failed line', good.replace('[SMOKE] done', '[SMOKE] failed: outputMulti'), false],
    ['error line', `${good}\n[SMOKE] error=boom`, false],
    ['a missing label', good.replace('[SMOKE] outputMulti=ok', ''), false],
    ['a FAIL value is not a FAIL label', good.replace('version=1.1.1', 'version=FAILSAFE'), true],
  ];
  let bad = 0;
  for (const [name, log, want] of cases) {
    const got = judge(log, lanes).ok;
    if (got !== want) bad += 1;
    console.log(`[GATE] self-test ${name}=${got === want ? 'ok' : `FAIL:want ${want}, got ${got}`}`);
  }
  return bad === 0;
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  const argv = process.argv.slice(2);
  if (argv.includes('--self-test')) process.exit(selfTest() ? 0 : 1);
  const opt = (name, dflt) => {
    const i = argv.indexOf(name);
    return i >= 0 ? argv.splice(i, 2)[1] : dflt;
  };
  const dir = opt('--labels', path.join(HERE, 'smoke-labels'));
  const prefix = opt('--prefix', 'SMOKE');
  const file = argv[0];
  if (!file) {
    console.log('usage: node tools/smoke-gate.mjs <smoke stdout> [--labels <dir>] [--prefix SMOKE] | --self-test');
    process.exit(2);
  }
  const log = fs.existsSync(file) ? readText(file) : '';
  const v = judge(log, readLabels(dir), prefix);
  for (const l of report(v, prefix)) console.log(l);
  process.exit(v.ok ? 0 : 1);
}
