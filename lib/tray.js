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
 * @param {() => void} d.toggleVisibility
 * @param {() => void} d.newChat
 * @param {() => void} d.openHistory
 * @param {() => void} d.openSettings
 * @param {() => void} d.quit
 * @param {() => object} d.shortcuts        current accelerators, for the labels
 * @param {() => string} [d.updateReady]    version downloaded and waiting, or ''
 * @param {() => void} [d.restartToUpdate]
 */
function init(d) {
  deps = d;
  if (tray) return;
  tray = new Tray(image());
  // Left click runs the thing the app is for; right click opens the menu,
  // which Electron wires to setContextMenu on Windows.
  tray.on('click', () => deps.capture());
  tray.on('double-click', () => deps.capture());
  refresh();
}

/** Called whenever a label in the menu would have changed. */
function refresh(state) {
  if (!tray || tray.isDestroyed()) return;
  if (state && typeof state.hidden === 'boolean') hidden = state.hidden;
  const keys = (deps.shortcuts && deps.shortcuts()) || {};
  // registerAccelerator:false shows the combo beside the label without
  // claiming it a second time; the global shortcut already owns it.
  const item = (label, accelerator, click, extra) => ({
    label,
    ...(accelerator ? { accelerator, registerAccelerator: false } : {}),
    click,
    ...extra,
  });
  // A downloaded update waits here rather than in a dialog: this is the one
  // surface that is always reachable, and the user restarts when they like.
  const ready = deps.updateReady ? deps.updateReady() : '';
  const updateItems = ready
    ? [item(`Restart to update to ${ready}`, null, () => deps.restartToUpdate()), { type: 'separator' }]
    : [];
  tray.setContextMenu(
    Menu.buildFromTemplate([
      ...updateItems,
      item('Ask about my screen', keys.captureQuery, () => deps.capture()),
      item('Ask about an area\u2026', keys.captureRegion, () => deps.captureRegion()),
      { type: 'separator' },
      item(hidden ? 'Show the overlay' : 'Hide the overlay', keys.toggleVisibility, () =>
        deps.toggleVisibility()
      ),
      item('New chat', null, () => deps.newChat()),
      item('History\u2026', null, () => deps.openHistory()),
      { type: 'separator' },
      item('Settings\u2026', null, () => deps.openSettings()),
      { type: 'separator' },
      item('Quit AevalBuddy', null, () => deps.quit()),
    ])
  );
  tray.setToolTip(
    keys.captureQuery ? `AevalBuddy \u2014 ${keys.captureQuery} to ask` : 'AevalBuddy'
  );
}

function destroy() {
  if (tray && !tray.isDestroyed()) tray.destroy();
  tray = null;
}

// The visual self-check needs to know the tray came up.
function isReady() {
  return !!(tray && !tray.isDestroyed());
}

module.exports = { init, refresh, destroy, isReady };
