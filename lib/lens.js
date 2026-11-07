'use strict';

// The lens: a sheet of glass over each display that draws Find's highlights
// and, while the agent runs, the control frame and its chip.
//
// It is the second surface with no background of its own (the area selector
// was the first), so every mark on it carries a dark rim for pale content.
// One window per display that has something to draw, sized to the display,
// click-through and never focusable, so the person keeps using the app
// underneath, and content-protected like every other window, so a presenter
// can Find without the audience seeing the highlights.
//
// Kept apart from cursor-buddy on purpose: the buddy can be pointing at a
// control while Find is highlighting, and an agent run uses both at once.
//
// Rects in are Electron DIP screen coordinates; main converts the desktop
// engine's physical pixels before they get here.

const { BrowserWindow, screen } = require('electron');
const path = require('path');
const backdrop = require('./backdrop');

const wins = new Map(); // display id -> BrowserWindow
let hits = []; // [{ i, kind, rect, why, displayId }]
let current = -1;
let frame = null; // { displayId, chip }
let hidden = false;
let loupeFor = ''; // the hit the magnifier was last cut for
const LOUPE_PAD = 3; // matches PAD in renderer/lens.js, the ring's inset

function displayOf(rect) {
  try {
    return screen.getDisplayMatching(rect);
  } catch (err) {
    return screen.getPrimaryDisplay();
  }
}

function create(display) {
  const b = display.bounds;
  const win = new BrowserWindow({
    x: b.x,
    y: b.y,
    width: b.width,
    height: b.height,
    frame: false,
    transparent: true,
    resizable: false,
    movable: false,
    focusable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    hasShadow: false,
    show: false,
    ...(process.platform === 'win32' ? { type: 'toolbar' } : {}),
    webPreferences: {
      preload: path.join(__dirname, '..', 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: false,
    },
  });
  win.setIgnoreMouseEvents(true);
  win.setAlwaysOnTop(true, 'screen-saver', 1);
  win.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
  win.setContentProtection(true);
  win.aevalLens = true;
  // Ready is tracked here, not read from webContents.isLoading(): that is
  // still true inside did-finish-load (Chromium stops loading after it), so
  // the redraw on load was skipped as well, and a lens whose hits all came
  // in while it loaded stayed empty until the next Find update (D16).
  win.aevalReady = false;
  win.webContents.on('did-start-loading', () => {
    win.aevalReady = false;
  });
  win.webContents.on('did-finish-load', () => {
    win.aevalReady = true;
    draw();
  });
  win.loadFile(path.join(__dirname, '..', 'renderer', 'lens.html'));
  win.on('closed', () => {
    for (const [id, w] of wins) if (w === win) wins.delete(id);
  });
  return win;
}

function draw() {
  const byDisplay = new Map();
  for (const h of hits) {
    if (!byDisplay.has(h.displayId)) byDisplay.set(h.displayId, []);
    byDisplay.get(h.displayId).push(h);
  }
  const wanted = new Set(byDisplay.keys());
  if (frame) wanted.add(frame.displayId);
  for (const display of screen.getAllDisplays()) {
    let win = wins.get(display.id);
    if (!wanted.has(display.id) || hidden) {
      if (win && !win.isDestroyed()) {
        win.webContents.send('lens:draw', { hits: [], current: -1, frame: null });
        win.hide();
      }
      continue;
    }
    if (!win || win.isDestroyed()) {
      win = create(display);
      wins.set(display.id, win);
    } else {
      win.setBounds(display.bounds);
    }
    const o = display.bounds;
    const list = (byDisplay.get(display.id) || []).map((h) => ({
      i: h.i,
      kind: h.kind,
      why: h.why,
      x: Math.round(h.rect.x - o.x),
      y: Math.round(h.rect.y - o.y),
      w: Math.max(4, Math.round(h.rect.width)),
      h: Math.max(4, Math.round(h.rect.height)),
    }));
    const cur = hits.find((h) => h.i === current);
    if (win.aevalReady) {
      win.webContents.send('lens:draw', {
        hits: list,
        current: cur && cur.displayId === display.id ? current : -1,
        frame: frame && frame.displayId === display.id ? { chip: frame.chip } : null,
      });
    }
    if (!win.isVisible()) win.showInactive();
    win.setAlwaysOnTop(true, 'screen-saver', 1);
    if (cur && cur.displayId === display.id && win.aevalReady) loupe(win, cur);
  }
}

// The current hit is shown through a magnifier: a full-resolution cut of
// what is under it, scaled up a little inside the ring. One capture each
// time the current hit changes, never on a timer. Our own windows are
// excluded from capture, so the cut never contains the lens's own marks.
function loupe(win, cur) {
  const r = cur.rect;
  const key = `${cur.i}:${Math.round(r.x)},${Math.round(r.y)},${Math.round(r.width)},${Math.round(r.height)}`;
  if (key === loupeFor) return;
  loupeFor = key;
  const rect = {
    x: Math.round(r.x - LOUPE_PAD),
    y: Math.round(r.y - LOUPE_PAD),
    width: Math.max(4, Math.round(r.width + LOUPE_PAD * 2)),
    height: Math.max(4, Math.round(r.height + LOUPE_PAD * 2)),
  };
  backdrop
    .crop(rect)
    .then((img) => {
      if (!img || loupeFor !== key || win.isDestroyed()) return;
      win.webContents.send('lens:loupe', { i: cur.i, img });
    })
    .catch(() => {});
}

/**
 * Find's hits. Offscreen hits are listed in the results but not drawn.
 *
 * @param {Array<{i, kind, rect, why, offscreen}>} list DIP rects
 * @param {number} cur index of the current hit
 */
function setHits(list, cur) {
  hits = (list || [])
    .filter((h) => h.rect && !h.offscreen)
    .map((h) => ({ ...h, displayId: displayOf(h.rect).id }));
  current = cur;
  draw();
}

/** The agent's control frame on one display, or null to take it down. */
function setFrame(display, chip) {
  frame = display ? { displayId: display.id, chip: chip || '' } : null;
  draw();
}

function clear() {
  hits = [];
  current = -1;
  loupeFor = '';
  draw();
}

function setHidden(on) {
  hidden = !!on;
  draw();
}

/** For the desktop engine: ours, but never in the way of a click. */
function handles() {
  return [...wins.values()].filter((w) => !w.isDestroyed()).map((w) => w.getNativeWindowHandle());
}

function windows() {
  return [...wins.values()].filter((w) => !w.isDestroyed());
}

function destroy() {
  for (const w of wins.values()) if (!w.isDestroyed()) w.destroy();
  wins.clear();
}

module.exports = { setHits, setFrame, clear, setHidden, handles, windows, destroy };
