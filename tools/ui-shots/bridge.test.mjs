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
check('editMenuInFields', !menu({ closest: () => ({}) }));

// The ui-shots lane files (K-31): all nine import, and each is in the shape
// shots.mjs documents, with every cssLintEnforce file present.
const { LANES, shapeProblems } = await import('./lanes.mjs');
const shape = shapeProblems(LANES);
check('laneFiles', Object.keys(LANES).length === 9 && shape.length === 0, shape.join('; '));
const ROOT = path.join(HERE, '..', '..');
const listed = Object.values(LANES).flatMap((m) => m.cssLintEnforce || []);
const absent = listed.filter((f) => !fs.existsSync(path.join(ROOT, f)));
check('cssLintFilesExist', absent.length === 0, absent.join(', '));

console.log(`[BRIDGE] done failed=${failed}`);
process.exit(failed ? 1 : 0);
