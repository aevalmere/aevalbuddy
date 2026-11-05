#!/usr/bin/env node
'use strict';

// Exercises lib/desktop outside Electron: spawns the helper, lists windows,
// opens a Notepad of its own on a temp file, reads its tree, OCRs it, types
// into it and sets its value, reads it back, and closes it without saving.
// The checks are lib/desktop/selfcheck.js, the same ones AEVAL_SMOKE runs.
//
//   node tools/desktop-check.js            everything, Notepad included
//   node tools/desktop-check.js --no-notepad
//
// Prints `[DESKTOP] key=value` lines; exits 1 if any check failed. It only
// touches the Notepad window it launched, but that window does take focus
// for a few seconds, so on a shared desktop run it when you are not typing.

const desktop = require('../lib/desktop');
const selfcheck = require('../lib/desktop/selfcheck');

const failures = [];
const log = (k, v) => console.log(`[DESKTOP] ${k}=${v}`);
const must = (k, ok, detail) => {
  log(k, ok ? 'ok' : `FAIL${detail ? `:${detail}` : ''}`);
  if (!ok) failures.push(k);
};

(async () => {
  const t = Date.now();
  try {
    await selfcheck.run({ desktop, log, must, notepad: !process.argv.includes('--no-notepad') });
  } catch (err) {
    failures.push('exception');
    console.error(`[DESKTOP] error=${err.code || ''}:${err.stack || err.message}`);
    log('helperStderr', JSON.stringify(desktop.status().stderr));
  }
  desktop.stop();
  log('totalMs', Date.now() - t);
  if (failures.length) {
    console.error(`[DESKTOP] failed: ${failures.join(', ')}`);
    process.exit(1);
  }
  console.log('[DESKTOP] done');
  process.exit(0);
})();
