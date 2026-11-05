'use strict';

// SPEC-v2 acceptance checks for the AEVAL_SMOKE run (section 6, B1 to B30).
// main.js hands in its internals as `c`; nothing here runs unless the smoke
// run calls it. The half that drives real windows (Notepad, a small form)
// runs only where selfcheck's Notepad half runs: on CI, or when asked.

const { spawn } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');
const selfcheck = require('./desktop/selfcheck');

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function until(fn, ms, step = 100) {
  const end = Date.now() + ms;
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() > end) return null;
    await sleep(step);
  }
}

// ------------------------------------------------------------------ always

/** Checks that need no other app: windows, menus, shortcuts, tour, bar. */
async function basics(c) {
  const { log, must } = c;

  // B29: the helper must not have started at boot.
  must('helperNotAtBoot', !c.helperStartedBeforeSmoke, 'the desktop helper was running before anything used it');
  log('bootMs', c.bootMs);
  must('bootMs', c.bootMs > 0 && c.bootMs < 15000, `pill showed after ${c.bootMs}ms`);

  // B21: the tray, in order, in each state.
  const k = c.store.getShortcuts();
  void k;
  const base = [
    'Ask about my screen',
    'Ask about an area…',
    'Find on screen…',
    'Do a task…',
    '-',
    'Hide the overlay',
    'New chat',
    'History…',
    '-',
    'Settings…',
    'Check for updates',
    '-',
    'Quit AevalBuddy',
  ];
  const idle = c.tray.labels({ agentRunning: false, update: { status: 'current' } });
  const running = c.tray.labels({ agentRunning: true, update: { status: 'current' } });
  const ready = c.tray.labels({ agentRunning: false, update: { status: 'ready', version: '9.9.9' } });
  const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  must(
    'trayOrder',
    same(idle, base) &&
      same(running, ['Stop agent', '-', ...base]) &&
      same(ready, ['Restart to update to 9.9.9', '-', ...base]),
    `${idle.join('|')} // ${running.slice(0, 3).join('|')} // ${ready.slice(0, 3).join('|')}`
  );

  // B22: all five globals registered, or reported as failed to Settings.
  const reg = c.lastRegisterResults;
  const five = ['captureQuery', 'captureRegion', 'findOnScreen', 'agentTask', 'toggleVisibility'];
  log('shortcuts', five.map((a) => `${a}:${reg[a]}`).join(','));
  must('shortcutsFive', five.every((a) => typeof reg[a] === 'boolean'), JSON.stringify(reg));

  // B20: Tab on an empty bar cycles the modes; text survives a switch.
  await c.openBar('ask');
  await sleep(400);
  const ov = c.overlayWin();
  const js = (code) => ov.webContents.executeJavaScript(code);
  const press = (key, extra) =>
    js(
      `(() => { const i = document.getElementById('query-input');` +
        ` i.dispatchEvent(new KeyboardEvent('keydown', Object.assign({ key: ${JSON.stringify(key)}, bubbles: true, cancelable: true }, ${JSON.stringify(extra || {})})));` +
        ` return document.body.dataset.barMode; })()`
    );
  await js(`document.getElementById('query-input').value = ''; true`);
  const cycle = [await press('Tab'), await press('Tab'), await press('Tab')];
  await js(`document.getElementById('query-input').value = 'invoice'; true`);
  const jumped = await press('2', { ctrlKey: true });
  const kept = await js(`document.getElementById('query-input').value`);
  const back = await press('1', { ctrlKey: true });
  await js(`document.getElementById('query-input').value = ''; true`);
  log('modeCycle', `${cycle.join('>')}:${jumped}:${kept}:${back}`);
  must(
    'modeCycle',
    cycle.join(',') === 'find,agent,ask' && jumped === 'find' && kept === 'invoice' && back === 'ask',
    `${cycle.join(',')} ctrl+2=${jumped} text=${kept}`
  );
  await sleep(300);

  // B23: a first run shows step 1; a saved key moves it on.
  c.cursorBuddy.cancel();
  c.startWalkthrough();
  await sleep(600);
  const first = c.cursorBuddy.stepIndex();
  c.cursorBuddy.notify('key-saved');
  const second = c.cursorBuddy.stepIndex();
  c.cursorBuddy.notify('first-answer');
  const third = c.cursorBuddy.stepIndex();
  must('onboardingSteps', first === 0 && second === 1 && third === 2, `${first}>${second}>${third}`);
  c.cursorBuddy.cancel();
  c.closeSettings();
  await sleep(300);
}

/** B1, last, once every surface has been opened at least once. */
function contentProtection(c) {
  const wins = c.BrowserWindow.getAllWindows();
  const bad = wins.filter((w) =>
    typeof w.isContentProtected === 'function' ? !w.isContentProtected() : !c.protectedWindows.has(w)
  );
  c.log('windowsChecked', wins.length);
  c.must(
    'contentProtectedAll',
    !c.canHide || bad.length === 0,
    bad.map((w) => w.webContents.getURL().split('/').pop()).join(',')
  );
}

module.exports = { basics, contentProtection, until, sleep, spawn, fs, os, path, selfcheck };
