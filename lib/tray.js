'use strict';

// The tray icon, and the reason it exists.
//
// Every window this app owns sets skipTaskbar and, on Windows, the 'toolbar'
// window type: no taskbar button, and nothing in Alt-Tab. That is right for an
// overlay and wrong for the only copy of an application. Hide the overlay with
// its shortcut, forget the shortcut, and there was no surface left to click --
// no window, no taskbar entry, no menu, and the quit handler unreachable. Task
// Manager was the exit. The tray is the way back in.
//
// The icon is the buddy's own arrow, rasterised from the same polygon the
// pointer window draws, and carried here as base64 so the repository stays
// text-only and the app keeps its no-build-step promise. Two sizes: Windows
// picks 16 for the notification area and 32 when the display is scaled.

const { Tray, Menu, nativeImage } = require('electron');

const ICON_16 =
  'iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAA40lEQVR42mNgQAInz98NYKAEbF5w4P/ujafmnzx/V4AsA5a1' +
  'rv/f5tz2f/PCA+dPnr9rQJLmk+fvKoAMaLJqAOMVnZvenzx/N4EUAxwWVq+EGwDCc4sW/z95/i5xXgIZMCV+OooBINwfNvH/' +
  'kSPXCHsJ5FxsBoAwKFx2bzyF30snz99twGUAsiEkGwASm5Y6u6DJqsGBkBfmwwyYnb/wfrdvD5gNok+ev9tPTCCuB2lssmpI' +
  'QI/SrUsO3Sc2KuHRBUqRaNFJesJa1bcFHI3bVx0DGaBActIGRdvJ83f3k5Qi6Q4AIKPkKV/EBQ0AAAAASUVORK5CYII=';

const ICON_32 =
  'iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAABsUlEQVR42u2Xv2rDMBDG/Qh+hD6CZ0+GzqWeujbP0LVQhKFZ' +
  'mjYUCpmK+wcK9VAPKRRKIBmagKAgOmUSfgQ/gsoXJBBBDrUsK0sFt0TD/XLf3XdyEBgOZZxQxnPKeBjs4wBg+rAQlHFGGY/2' +
  'AnB3OhHjk1vxtVzXlPGBb4A5ALKYiOHhUMym38KrJDqAgnibfPqTZBtAxeP5KyD6lwRJTAAI/L5crfuVhDK+aUATAAJ3si/6' +
  'kQQATcn1vpCjCklSl8mjvwCoKG7eAYEYuwJI2gAg7s+eVV9AkgPvAKovFrOf7pJgxGwAtkzLXhK5iKwAVEjTspMEAPgXXQBU' +
  'X1hVAgbzUaw6A1wdjRRA1NqG2wKgAUfH11UWk7kWifUeaAsguz935QOsCaDJntWScmbDJoCni6KE2WDUTOMn9U6dAMDjtQR1' +
  'FpOBvKvQ3TtGL3dWAXSxTB7pJgW9m+RxIgOMQ+36LCah4b5qeitIF0yCPg+MCuU2Abxclu624g6AEKU2NaN6pHh5tuMdoENA' +
  'FjkJxAfApgoYS4Boy6f2+XRPKeOlTIyonD7NWlbD/+fb/3F1fgFvqYMQkYvkrgAAAABJRU5ErkJggg==';

let tray = null;
let deps = null;
let hidden = false;
// The tray's own Check for updates item answers the click that asked it:
// 'checking', then 'current' (held for 10s), 'failed', or nothing.
let manual = { state: 'idle', until: 0, version: '' };
let manualTimer = null;

function image() {
  const img = nativeImage.createFromBuffer(Buffer.from(ICON_16, 'base64'), {
    scaleFactor: 1,
  });
  img.addRepresentation({ scaleFactor: 2, buffer: Buffer.from(ICON_32, 'base64') });
  return img;
}

/**
 * @param {object} d
 * @param {() => void} d.capture            ask about the whole screen
 * @param {() => void} d.captureRegion      ask about a dragged area
 * @param {() => void} d.find               open Find
 * @param {() => void} d.agent              open the bar in Agent mode
 * @param {() => void} d.stopAgent
 * @param {() => boolean} d.agentRunning
 * @param {() => void} d.openLast           left click: the bar in the last-used mode
 * @param {() => void} d.toggleVisibility
 * @param {() => void} d.newChat
 * @param {() => void} d.openHistory
 * @param {() => void} d.openSettings
 * @param {() => Promise<object>} d.checkUpdates
 * @param {() => void} d.quit
 * @param {() => object} d.shortcuts        current accelerators, for the labels
 * @param {() => object} [d.update]         lib/updater.js state
 * @param {() => void} [d.restartToUpdate]
 */
function init(d) {
  deps = d;
  if (tray) return;
  tray = new Tray(image());
  // Left click opens the bar where the person left it; right click is the
  // menu, which Electron wires to setContextMenu on Windows.
  tray.on('click', () => (deps.openLast ? deps.openLast() : deps.capture()));
  tray.on('double-click', () => (deps.openLast ? deps.openLast() : deps.capture()));
  refresh();
}

function checkItem(u) {
  if (manual.state === 'checking') return { label: 'Checking…', enabled: false };
  if (u.status === 'downloading' && u.version) {
    return { label: `Downloading ${u.version}… ${Math.floor((u.progress || 0) * 100)}%`, enabled: false };
  }
  if (manual.state === 'current' && Date.now() < manual.until) {
    return { label: `Up to date (${manual.version})`, enabled: false };
  }
  if (manual.state === 'failed') return { label: 'Update failed, try again', click: () => runCheck() };
  return { label: 'Check for updates', click: () => runCheck() };
}

function runCheck() {
  if (!deps.checkUpdates) return;
  manual = { state: 'checking', until: 0, version: '' };
  refresh();
  Promise.resolve(deps.checkUpdates())
    .then((u) => {
      const s = (u && u.status) || '';
      if (s === 'current') manual = { state: 'current', until: Date.now() + 10000, version: u.current || '' };
      else if (s === 'downloading' || s === 'ready' || s === 'dev') manual = { state: 'idle', until: 0, version: '' };
      else manual = { state: 'failed', until: 0, version: '' };
    })
    .catch(() => {
      manual = { state: 'failed', until: 0, version: '' };
    })
    .then(() => {
      refresh();
      clearTimeout(manualTimer);
      if (manual.state === 'current') {
        manualTimer = setTimeout(() => {
          manual = { state: 'idle', until: 0, version: '' };
          refresh();
        }, 10000);
      }
    });
}

/**
 * The menu as data, in SPEC-v2 section 4.4's order, so the smoke run can
 * check the order in each state without opening a menu.
 */
function template(over) {
  const o = over || {};
  const keys = (deps.shortcuts && deps.shortcuts()) || {};
  // registerAccelerator:false shows the combo beside the label without
  // claiming it a second time; the global shortcut already owns it.
  const item = (label, accelerator, click, extra) => ({
    label,
    ...(accelerator ? { accelerator, registerAccelerator: false } : {}),
    click,
    ...extra,
  });
  const sep = { type: 'separator' };
  const u = o.update || (deps.update && deps.update()) || {};
  const out = [];
  // A downloaded update waits here rather than in a dialog: this is the one
  // surface that is always reachable, and the user restarts when they like.
  // A failed background check shows nothing here; it retries on its own.
  if (u.status === 'ready' && u.restartPending) {
    out.push({ label: 'Restarting after this answer…', enabled: false }, sep);
  } else if (u.status === 'ready') {
    out.push(item(`Restart to update to ${u.version}`, null, () => deps.restartToUpdate()), sep);
  }
  const running = o.agentRunning !== undefined ? o.agentRunning : deps.agentRunning && deps.agentRunning();
  if (running) {
    out.push(item('Stop agent', 'Esc', () => deps.stopAgent()), sep);
  }
  out.push(
    item('Ask, find, or do…', keys.captureQuery, () => deps.capture()),
    item('Ask about an area…', keys.captureRegion, () => deps.captureRegion()),
    item('Find on screen…', keys.findOnScreen, () => deps.find()),
    item('Do a task…', keys.agentTask, () => deps.agent()),
    sep,
    item(hidden ? 'Show the overlay' : 'Hide the overlay', keys.toggleVisibility, () => deps.toggleVisibility()),
    item('New chat', null, () => deps.newChat()),
    item('History…', null, () => deps.openHistory()),
    sep,
    item('Settings…', null, () => deps.openSettings()),
    item('Help and feedback…', null, () => deps.help()),
    { ...checkItem(u) },
    sep,
    item('Quit AevalBuddy', null, () => deps.quit())
  );
  return out;
}

/** Called whenever a label in the menu would have changed. */
function refresh(state) {
  if (!tray || tray.isDestroyed()) return;
  if (state && typeof state.hidden === 'boolean') hidden = state.hidden;
  const keys = (deps.shortcuts && deps.shortcuts()) || {};
  const u = (deps.update && deps.update()) || {};
  tray.setContextMenu(Menu.buildFromTemplate(template()));
  tray.setToolTip(
    u.status === 'ready'
      ? `AevalBuddy · ${u.version} is ready, restart to update`
      : deps.agentRunning && deps.agentRunning()
        ? 'AevalBuddy is using your computer · Esc to stop'
        : keys.captureQuery
          ? `AevalBuddy · ${keys.captureQuery}`
          : 'AevalBuddy'
  );
}

/** Labels only, separators as '-', for the order check. */
function labels(over) {
  return template(over).map((i) => (i.type === 'separator' ? '-' : i.label));
}

function destroy() {
  clearTimeout(manualTimer);
  if (tray && !tray.isDestroyed()) tray.destroy();
  tray = null;
}

// The visual self-check needs to know the tray came up.
function isReady() {
  return !!(tray && !tray.isDestroyed());
}

module.exports = { init, refresh, destroy, isReady, template, labels };
