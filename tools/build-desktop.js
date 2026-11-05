#!/usr/bin/env node
'use strict';

// Precompiles the desktop helper (lib/desktop/host.cs) into
// build/desktop-<hash>.dll, which electron-builder ships beside app.asar.
// Compiling on first use takes 5 to 10 seconds on a cold machine; loading a
// prebuilt assembly takes well under one, so the first Find or agent run on
// a fresh install no longer waits for a compiler. The hash is of the source,
// so a stale file is never loaded: a mismatch just compiles as before.
//
// Runs in `npm run build` (prebuild), like tools/make-icon.js. Windows only;
// elsewhere it does nothing.

const path = require('path');
const fs = require('fs');
const { spawnSync } = require('child_process');

if (process.platform !== 'win32') {
  console.log('[build-desktop] not Windows; skipped');
  process.exit(0);
}

const out = path.join(__dirname, '..', 'build');
fs.mkdirSync(out, { recursive: true });
for (const f of fs.readdirSync(out)) if (/^desktop-.*\.dll$/.test(f)) fs.unlinkSync(path.join(out, f));

const desktop = require('../lib/desktop');
const line = desktop.bootstrapLine({ cache: out, compileOnly: true });
const ps = path.join(process.env.SystemRoot || 'C:\\Windows', 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe');
const t = Date.now();
const r = spawnSync(ps, ['-NoLogo', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-Command', '-'], {
  input: line,
  encoding: 'utf8',
  windowsHide: true,
  timeout: 120000,
});
const built = fs.readdirSync(out).filter((f) => /^desktop-[0-9a-f]+\.dll$/.test(f));
console.log(`[build-desktop] ${built.join(', ') || 'nothing'} in ${Date.now() - t}ms`);
if (!built.length) {
  console.error(r.stdout, r.stderr);
  process.exit(1);
}
