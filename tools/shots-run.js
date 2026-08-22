'use strict';

// npm run shots
//
// Boots the app into its visual self-check (lib/shots.js) and writes a PNG of
// every surface to shots/. Runs against a throwaway settings profile, so it
// never touches a real install, and it does not need the app to be closed
// first for the same reason.

const { spawn } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

const root = path.join(__dirname, '..');
const outDir = path.join(root, 'shots');
const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'aevalbuddy-shots-'));

fs.rmSync(outDir, { recursive: true, force: true });
fs.mkdirSync(outDir, { recursive: true });

const electron = path.join(
  root,
  'node_modules',
  '.bin',
  process.platform === 'win32' ? 'electron.cmd' : 'electron'
);

const child = spawn(electron, ['.'], {
  cwd: root,
  stdio: 'inherit',
  shell: process.platform === 'win32',
  env: { ...process.env, AEVAL_SHOT: outDir, AEVAL_USERDATA: profile },
});

child.on('exit', (code) => {
  fs.rmSync(profile, { recursive: true, force: true });
  console.log(`\nShots written to ${outDir}`);
  process.exit(code || 0);
});
