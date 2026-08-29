'use strict';

// The static gate. `npm run check`.
//
// This project already learned what an observation nobody reads is worth:
// `npm run shots` photographed a picker labelling the app's own default
// "Custom model id" on every run, and the smoke suite printed
// `modelDefaultOffered=false` on every launch, and both went past unread for
// a release. The answer was to give the smoke run a `must()` that takes the
// exit code with it. This file is the same idea for the things that are true
// of the source without booting anything: it exits non-zero, so it can stop
// something.
//
// Three checks, each one a class of defect this repository has actually
// shipped rather than a style opinion:
//
//   1. Syntax. Every .js file parses. The app has no build step and no
//      typechecker, so a stray bracket in a rarely-taken branch reaches a
//      user before it reaches anyone else.
//
//   2. Palette. DESIGN.md allows six colour values plus a named exception
//      "named as a token where it is defined, carries its reason there, and
//      is listed here", and says outright that an unnamed hex inline in a
//      stylesheet "is still a defect, because it is a decision nobody can
//      count". renderer/region.css once carried seven of them under a header
//      comment claiming it used the palette. A comment cannot enforce that;
//      this can.
//
//   3. IPC coherence. The preload is an allowlist, and an allowlist drifts in
//      both directions. A renderer calling a channel that is not on it fails
//      only at runtime, in a frameless window with no devtools open. A
//      channel on it that no renderer ever calls is the `query:cancel` defect
//      from v0.7 exactly: "allowlisted in the preload and handled in main. No
//      renderer ever called it" -- the app had no Stop button while every
//      layer claimed it did.

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const SKIP_DIRS = new Set(['node_modules', '.git', 'dist', 'build', 'shots', '.claude']);

const failures = [];
const notes = [];

function fail(check, detail) {
  failures.push(`${check}: ${detail}`);
}

function walk(dir, ext, out) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (SKIP_DIRS.has(entry.name)) continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, ext, out);
    else if (entry.name.toLowerCase().endsWith(ext)) out.push(full);
  }
  return out;
}

const rel = (p) => path.relative(ROOT, p).replace(/\\/g, '/');

// ------------------------------------------------------------------ 1. syntax

function checkSyntax() {
  const files = walk(ROOT, '.js', []);
  for (const file of files) {
    try {
      execFileSync(process.execPath, ['--check', file], { stdio: 'pipe' });
    } catch (err) {
      const out = String((err.stderr || '') + (err.stdout || ''));
      const line = out.split(/\r?\n/).find((l) => /Error/.test(l)) || 'parse error';
      fail('syntax', `${rel(file)}: ${line.trim()}`);
    }
  }
  return files.length;
}

// ----------------------------------------------------------------- 2. palette

// DESIGN.md, "Palette (fixed, six values, nothing else)".
const PALETTE = new Set(['#000000', '#10254f', '#2b2c33', '#823a80', '#c9cfdd', '#303d6a']);

// A hex is allowed when it is a palette value, or when the line it sits on
// declares a custom property: that is what "named as a token where it is
// defined" means in practice, and a named token is a decision somebody can
// count.
function checkPalette() {
  const files = walk(ROOT, '.css', []);
  let hexes = 0;
  for (const file of files) {
    const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);
    lines.forEach((line, i) => {
      // Not a colour: an id selector such as `#panel`, which shares the
      // syntax and none of the meaning.
      const found = line.match(/#[0-9a-fA-F]{3,8}\b/g) || [];
      for (const hex of found) {
        if (!/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/.test(hex)) continue;
        hexes += 1;
        if (PALETTE.has(hex.toLowerCase())) continue;
        if (/^\s*--[a-z0-9-]+\s*:/i.test(line)) continue;
        fail(
          'palette',
          `${rel(file)}:${i + 1} unnamed hex ${hex} outside the six. Name it as a token ` +
            'where it is defined and list it in DESIGN.md, or use a palette value.'
        );
      }
    });
  }
  return hexes;
}

// ------------------------------------------------------------------- 3. ipc

function readSet(source, name) {
  const m = source.match(new RegExp(`const ${name} = new Set\\(\\[([\\s\\S]*?)\\]\\)`));
  if (!m) return null;
  return new Set(m[1].match(/'([^']+)'/g).map((s) => s.slice(1, -1)));
}

function collect(files, re) {
  const hits = new Map();
  for (const file of files) {
    const text = fs.readFileSync(file, 'utf8');
    let m;
    const rx = new RegExp(re.source, 'g');
    while ((m = rx.exec(text))) {
      if (!hits.has(m[1])) hits.set(m[1], []);
      hits.get(m[1]).push(rel(file));
    }
  }
  return hits;
}

function checkIpc() {
  const preloadPath = path.join(ROOT, 'preload.js');
  const preload = fs.readFileSync(preloadPath, 'utf8');
  const invokeAllow = readSet(preload, 'INVOKE_CHANNELS');
  const listenAllow = readSet(preload, 'LISTEN_CHANNELS');
  if (!invokeAllow || !listenAllow) {
    fail('ipc', 'could not read the preload allowlists; the shape of preload.js changed');
    return { invoked: 0, listened: 0 };
  }

  // Renderers: every window's script, wherever it lives.
  const rendererFiles = ['renderer', 'settings', 'history']
    .map((d) => path.join(ROOT, d))
    .filter((d) => fs.existsSync(d))
    .reduce((acc, d) => walk(d, '.js', acc), []);
  // Main side: main.js plus the lib modules that own their own windows.
  const mainFiles = [path.join(ROOT, 'main.js')].concat(walk(path.join(ROOT, 'lib'), '.js', []));

  const invoked = collect(rendererFiles, /invoke\(\s*'([^']+)'/);
  const listened = collect(rendererFiles, /\.on\(\s*'([^']+)'\s*,/);
  const handled = collect(mainFiles, /ipcMain\.handle\(\s*'([^']+)'/);
  // Any `send('channel'`, not only `webContents.send`: main.js routes the
  // streaming channels through a local `send` helper that closes over the
  // window and the generation counter, and matching only the qualified form
  // reported every one of them as dead.
  const sent = collect(mainFiles, /\bsend\(\s*'([^']+)'/);

  for (const [channel, where] of invoked) {
    if (!invokeAllow.has(channel)) {
      fail('ipc', `${where[0]} invokes '${channel}', which the preload blocks`);
    } else if (!handled.has(channel)) {
      fail('ipc', `${where[0]} invokes '${channel}', which no ipcMain.handle answers`);
    }
  }
  // A renderer listening off the allowlist throws at load, taking the window
  // with it.
  for (const [channel, where] of listened) {
    // Only the bridge is allowlisted; DOM and emitter .on calls are not.
    if (!listenAllow.has(channel) && /^[a-z-]+:[a-z-]+$/.test(channel)) {
      fail('ipc', `${where[0]} listens for '${channel}', which the preload blocks`);
    }
  }
  // The v0.7 defect: every layer present except the one that calls it.
  for (const channel of invokeAllow) {
    if (!invoked.has(channel)) {
      fail(
        'ipc',
        `'${channel}' is allowlisted and handled, but no renderer ever invokes it. ` +
          'Either wire it up or take it off the allowlist; a control nothing calls is a ' +
          'control that is lying.'
      );
    }
  }
  // The mirror of the same defect, and how buddy:move and settings:data were
  // found: allowlisted to be received, sent by nobody, listened for by nobody,
  // present in the repository only here. Dead widening of the one surface that
  // exists to be narrow.
  for (const channel of listenAllow) {
    if (!sent.has(channel)) {
      fail(
        'ipc',
        `'${channel}' is on the listen allowlist and nothing in main sends it. ` +
          'Take it off, or wire it up.'
      );
    }
  }
  return { invoked: invoked.size, listened: listened.size };
}

// ------------------------------------------------------------ 4. line endings

// The repository is CRLF throughout. That is not a taste question: a file
// written LF into a CRLF tree turns its next edit into a whole-file diff, and
// the real change disappears inside it. Cheap to check, tedious to notice.
const TEXT_EXT = new Set(['.js', '.css', '.html', '.json', '.md', '.yml']);

function checkLineEndings() {
  const files = [];
  const walkAll = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      if (SKIP_DIRS.has(entry.name)) continue;
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walkAll(full);
      else if (TEXT_EXT.has(path.extname(entry.name))) files.push(full);
    }
  };
  walkAll(ROOT);
  for (const file of files) {
    const text = fs.readFileSync(file, 'utf8');
    const lone = (text.match(/(^|[^\r])\n/g) || []).length;
    if (lone) fail('line-endings', `${rel(file)} has ${lone} LF line ending(s); this tree is CRLF`);
  }
  return files.length;
}

// ------------------------------------------------------------------- report

const jsCount = checkSyntax();
const hexCount = checkPalette();
const ipc = checkIpc();
const textCount = checkLineEndings();

console.log(`[CHECK] syntax=${jsCount} files`);
console.log(`[CHECK] palette=${hexCount} hex values`);
console.log(`[CHECK] ipc=${ipc.invoked} invoked, ${ipc.listened} listened`);
console.log(`[CHECK] lineEndings=${textCount} files`);
for (const note of notes) console.log(`[CHECK] note: ${note}`);

if (failures.length) {
  console.error(`\n[CHECK] ${failures.length} failure(s):`);
  for (const f of failures) console.error(`  - ${f}`);
  process.exit(1);
}
console.log('[CHECK] ok');
