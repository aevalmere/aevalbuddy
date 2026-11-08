// Unit checks for src-tauri/bridge/aeval-bridge.js under plain node, with a
// fake window.__TAURI__: the Electron-shaped API, the allowlists, listeners
// registered before an invoke goes out, and the browser keys it takes back.
//   node tools/ui-shots/bridge.test.mjs
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const SRC = fs.readFileSync(path.join(HERE, '..', '..', 'src-tauri', 'bridge', 'aeval-bridge.js'), 'utf8');

let failed = 0;
function check(name, ok, why) {
  if (!ok) failed += 1;
  console.log(`[BRIDGE] ${name}=${ok ? 'ok' : `FAIL:${why || ''}`}`);
}

function makeWindow() {
  const log = [];
  const handlers = {};
  const listenResolvers = [];
  const win = {
    addEventListener: (type, fn) => {
      (handlers[type] = handlers[type] || []).push(fn);
    },
    __TAURI__: {
      core: {
        invoke: (cmd, args) => {
          log.push(['invoke', args.channel]);
          return args.channel === 'fail' ? Promise.reject('boom') : Promise.resolve({ ok: args.channel });
        },
      },
      webviewWindow: {
        getCurrentWebviewWindow: () => ({
          listen: (channel, cb) => {
            log.push(['listen', channel]);
            return new Promise((res) => listenResolvers.push(() => res(() => log.push(['unlisten', channel]))));
          },
        }),
      },
    },
  };
  win.window = win;
  return { win, log, handlers, listenResolvers };
}

const env = makeWindow();
vm.runInNewContext(`${SRC}\naevalBridge(['settings:get','glass:hello','fail'], ['overlay:state']);`, { window: env.win, Promise, Set, Error, String, Object, setTimeout });
const aeval = env.win.aeval;

check('shape', typeof aeval.invoke === 'function' && typeof aeval.on === 'function' && Object.isFrozen(aeval));
let blockedInvoke = null;
await aeval.invoke('fs:read').catch((e) => {
  blockedInvoke = e.message;
});
check('blockedInvoke', blockedInvoke === 'Blocked IPC channel: fs:read', blockedInvoke);
let blockedOn = null;
try {
  aeval.on('fs:watch', () => {});
} catch (e) {
  blockedOn = e.message;
}
check('blockedOn', blockedOn === 'Blocked IPC channel: fs:watch', blockedOn);

// A listen in flight holds the next invoke back until it is registered.
const off = aeval.on('overlay:state', () => {});
check('onReturnsUnsubscribe', typeof off === 'function');
const hello = aeval.invoke('glass:hello');
await new Promise((r) => setTimeout(r, 10));
check('invokeWaitsForListen', !env.log.some((l) => l[0] === 'invoke'), JSON.stringify(env.log));
env.listenResolvers.forEach((r) => r());
const got = await hello;
check('invokeAfterListen', got && got.ok === 'glass:hello' && env.log.findIndex((l) => l[0] === 'invoke') > env.log.findIndex((l) => l[0] === 'listen'), JSON.stringify(env.log));
check('invokeUngatedLater', (await aeval.invoke('settings:get')).ok === 'settings:get');
let err = null;
await aeval.invoke('fail').catch((e) => {
  err = e;
});
check('rejectionIsError', err instanceof Error && err.message === 'boom', String(err));
off();
check('unsubscribe', env.log.some((l) => l[0] === 'unlisten'), JSON.stringify(env.log));

// Browser keys: F5, Ctrl+F, Ctrl+P, Ctrl+R are taken; Ctrl+1 and Ctrl+C are not.
const key = (k, mods) => {
  let prevented = false;
  const e = { key: k, ctrlKey: false, altKey: false, shiftKey: false, ...mods, target: null, preventDefault: () => (prevented = true) };
  env.handlers.keydown.forEach((h) => h(e));
  return prevented;
};
check('blocksF5', key('F5'));
check('blocksCtrlF', key('f', { ctrlKey: true }));
check('blocksCtrlP', key('p', { ctrlKey: true }));
check('blocksCtrlR', key('r', { ctrlKey: true }));
check('blocksAltLeft', key('ArrowLeft', { altKey: true }));
check('keepsCtrl1', !key('1', { ctrlKey: true }));
check('keepsCtrlC', !key('c', { ctrlKey: true }));
check('keepsF6', !key('F6'));
const menu = (target) => {
  let prevented = false;
  env.handlers.contextmenu.forEach((h) => h({ defaultPrevented: false, target, preventDefault: () => (prevented = true) }));
  return prevented;
};
check('noBrowserMenu', menu({ closest: () => null }));
// B-077 (WIN-14): a field no page menu claimed gets the app's own edit menu
// (Cut, Copy, Paste, Select all) instead of WebView2's, so the browser menu
// is prevented there too and the bridge asks for kind 'edit'.
const menuEnv = makeWindow();
vm.runInNewContext(`${SRC}\naevalBridge(['ui:context-menu'], []);`, { window: menuEnv.win, Promise, Set, Error, String, Object, setTimeout });
let fieldPrevented = false;
menuEnv.handlers.contextmenu.forEach((h) => h({ defaultPrevented: false, target: { closest: () => ({}) }, preventDefault: () => (fieldPrevented = true) }));
await new Promise((r) => setTimeout(r, 0));
const asked = menuEnv.log.some((l) => l[0] === 'invoke' && l[1] === 'ui:context-menu');
check('editMenuInFields', fieldPrevented && asked, JSON.stringify({ fieldPrevented, log: menuEnv.log }));

// The ui-shots lane files (K-31): all nine import, and each is in the shape
// shots.mjs documents, with every cssLintEnforce file present.
const { LANES, shapeProblems } = await import('./lanes.mjs');
const shape = shapeProblems(LANES);
check('laneFiles', Object.keys(LANES).length === 10 && shape.length === 0, shape.join('; ')); // voice lane: 10
const ROOT = path.join(HERE, '..', '..');
const listed = Object.values(LANES).flatMap((m) => m.cssLintEnforce || []);
const absent = listed.filter((f) => !fs.existsSync(path.join(ROOT, f)));
check('cssLintFilesExist', absent.length === 0, absent.join(', '));

// The CSS gate (B-145, tools/ui-shots/css-lint.mjs): every file a lane lists
// in cssLintEnforce must hold all three rules; the rest are counted, not
// enforced, until the final pass (B-148).
const { lintCss, selfTest } = await import('./css-lint.mjs');
check('cssLintSelfTest', selfTest() === 0, 'css-lint.mjs failed its own cases');
const enforced = [...new Set(listed)].filter((f) => fs.existsSync(path.join(ROOT, f)));
let violations = 0;
for (const f of enforced) {
  for (const v of lintCss(fs.readFileSync(path.join(ROOT, f), 'utf8'))) {
    violations += 1;
    console.log(`[BRIDGE] css-lint ${f}:${v.line} ${v.rule}: ${v.text} (${v.why})`);
  }
}
console.log(`[BRIDGE] css-lint: ${enforced.length} files enforced, ${violations} violations`);
check('cssLint', violations === 0, `${violations} violations in ${enforced.join(', ')}`);
const allCss = [];
const walk = (dir) => {
  for (const e of fs.readdirSync(path.join(ROOT, dir), { withFileTypes: true })) {
    const rel = path.posix.join(dir, e.name);
    if (e.isDirectory() && !['node_modules', 'target', '.git', 'dist', 'out'].includes(e.name)) walk(rel);
    else if (e.isFile() && e.name.endsWith('.css')) allCss.push(rel);
  }
};
for (const dir of ['renderer', 'settings', 'history', 'src-tauri/src', 'src-tauri/ui']) {
  if (fs.existsSync(path.join(ROOT, dir))) walk(dir);
}
const counts = allCss.filter((f) => !enforced.includes(f)).map((f) => [f, lintCss(fs.readFileSync(path.join(ROOT, f), 'utf8')).length]);
for (const [f, n] of counts) console.log(`[BRIDGE] css-lint report ${f}: ${n} violations (not enforced yet)`);

console.log(`[BRIDGE] done failed=${failed}`);
process.exit(failed ? 1 : 0);
