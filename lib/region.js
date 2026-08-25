'use strict';

// Drag-to-choose-an-area, and the reason it is worth a whole module.
//
// The whole screen is the wrong thing to send. A 2560x1440 desktop squeezed
// into a 1600px long edge is a 0.62x downscale, so 12px interface text arrives
// at about seven pixels and the model is guessing at glyphs. DESIGN.md already
// names capture fidelity as the root of bad answers; raising the long edge from
// 1280 to 1600 helped and could not fix it, because the budget is spent on the
// whole desktop when the question is about one dialog.
//
// Choosing an area spends the same budget on a fraction of the pixels. A
// 600x300 selection goes over the wire at 1:1 instead of 0.62x, and small text
// is simply legible. It is the largest single lever on answer quality in the
// app.
//
// The scrim is live rather than a frozen screenshot: nothing has to be
// transferred to the renderer, and the capture that follows sees the screen as
// it is at that moment. This window sets content protection like every other
// one, so it never appears in the shot it is about to trigger.

const { BrowserWindow, ipcMain, screen } = require('electron');
const path = require('path');

// A drag shorter than this on both axes is a click that missed, not a
// selection. Cancelling beats sending the model nine pixels of wallpaper.
const MIN_EDGE = 12;

let win = null;
let pending = null;

function isActive() {
  return !!(win && !win.isDestroyed());
}

/**
 * Opens the selector on one display and resolves with the chosen rectangle in
 * screen coordinates, or null if the user cancelled.
 *
 * @param {Electron.Display} display
 * @returns {Promise<{x: number, y: number, width: number, height: number}|null>}
 */
function select(display) {
  if (isActive()) {
    // A second request while one is open replaces it rather than stacking two
    // full-screen windows the user has to dismiss twice.
    cancel();
  }
  const target = display || screen.getPrimaryDisplay();
  const b = target.bounds;
  return new Promise((resolve) => {
    pending = { resolve, origin: { x: b.x, y: b.y } };
    win = new BrowserWindow({
      x: b.x,
      y: b.y,
      width: b.width,
      height: b.height,
      frame: false,
      transparent: true,
      resizable: false,
      movable: false,
      minimizable: false,
      maximizable: false,
      fullscreenable: false,
      skipTaskbar: true,
      alwaysOnTop: true,
      hasShadow: false,
      // Unlike every other window here this one wants the keyboard and the
      // mouse: it is a modal gesture, not an overlay.
      focusable: true,
      show: false,
      ...(process.platform === 'win32' ? { type: 'toolbar' } : {}),
      webPreferences: {
        preload: path.join(__dirname, '..', 'preload.js'),
        contextIsolation: true,
        nodeIntegration: false,
        backgroundThrottling: false,
      },
    });
    win.setAlwaysOnTop(true, 'screen-saver', 1);
    win.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
    win.setContentProtection(true);
    win.loadFile(path.join(__dirname, '..', 'renderer', 'region.html'));
    win.once('ready-to-show', () => {
      if (!win || win.isDestroyed()) return;
      win.show();
      win.focus();
    });
    // Losing focus means the user clicked into something else; a full-screen
    // scrim they did not ask to keep must not survive that.
    win.on('blur', () => cancel());
    win.on('closed', () => {
      win = null;
      settle(null);
    });
  });
}

function settle(rect) {
  if (!pending) return;
  const { resolve } = pending;
  pending = null;
  resolve(rect);
}

function close() {
  if (win && !win.isDestroyed()) {
    const w = win;
    win = null;
    w.destroy();
  }
}

function cancel() {
  close();
  settle(null);
}

/** The renderer reports in window coordinates; the display origin is here. */
function finish(rect) {
  const origin = pending ? pending.origin : { x: 0, y: 0 };
  const r = rect || {};
  const width = Math.round(Number(r.width) || 0);
  const height = Math.round(Number(r.height) || 0);
  close();
  if (width < MIN_EDGE || height < MIN_EDGE) {
    settle(null);
    return;
  }
  settle({
    x: Math.round(origin.x + (Number(r.x) || 0)),
    y: Math.round(origin.y + (Number(r.y) || 0)),
    width,
    height,
  });
}

// Self-contained IPC: the selector owns its own two channels rather than
// threading a callback through main.
function registerIpc() {
  ipcMain.handle('region:done', (_e, rect) => finish(rect));
  ipcMain.handle('region:cancel', () => cancel());
}

// The visual self-check needs the selector window to photograph it.
function debugWindow() {
  return win && !win.isDestroyed() ? win : null;
}

module.exports = { select, cancel, isActive, registerIpc, debugWindow, MIN_EDGE };
