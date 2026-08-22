'use strict';

// Window-level motion. CSS can only move pixels inside a window; the window
// itself is an OS object, so growing the pill into the panel has to happen
// here. That distinction is the whole reason opening used to flash: Electron
// paints a window at its final bounds in one step, and any CSS scale played
// inside it afterwards reads as a snap followed by a wobble.
//
// One curve drives every transition, matching --ease-glide in the
// stylesheets, so window motion and content motion are the same gesture.

const { BrowserWindow } = require('electron');

// 60Hz, matching the display. This ran at 120 and paired every frame with a
// setOpacity call; on Windows that toggles the layered window style, and the
// churn showed up as repaint artifacts on the desktop. Fades now happen in
// CSS inside the window, where they cost the window manager nothing.
const FRAME_MS = 16;

// Solves cubic-bezier(x1, y1, x2, y2) for y at a given x, the same way a
// browser does: Newton-Raphson on the x polynomial, then evaluate y.
function cubicBezier(x1, y1, x2, y2) {
  const ax = 3 * x1 - 3 * x2 + 1;
  const bx = 3 * x2 - 6 * x1;
  const cx = 3 * x1;
  const ay = 3 * y1 - 3 * y2 + 1;
  const by = 3 * y2 - 6 * y1;
  const cy = 3 * y1;
  const sampleX = (t) => ((ax * t + bx) * t + cx) * t;
  const slopeX = (t) => (3 * ax * t + 2 * bx) * t + cx;
  return (x) => {
    if (x <= 0) return 0;
    if (x >= 1) return 1;
    let t = x;
    for (let i = 0; i < 6; i += 1) {
      const dx = sampleX(t) - x;
      if (Math.abs(dx) < 1e-4) break;
      const d = slopeX(t);
      if (Math.abs(d) < 1e-6) break;
      t -= dx / d;
    }
    return ((ay * t + by) * t + cy) * t;
  };
}

const GLIDE = cubicBezier(0.32, 0.72, 0, 1);

// Read once from a renderer at boot; when the user asks for less motion,
// every transition collapses to its end state.
let reduced = false;

function setReducedMotion(value) {
  reduced = !!value;
}

function isReduced() {
  return reduced;
}

// One animation per window at a time. A new one takes over from wherever the
// previous had reached, so rapid open/close never leaves a window stranded.
const running = new Map();

function stop(win) {
  const id = win.id;
  const timer = running.get(id);
  if (timer) {
    clearInterval(timer);
    running.delete(id);
  }
}

function isAnimating(win) {
  return !!(win && !win.isDestroyed() && running.has(win.id));
}

function lerpRect(a, b, p) {
  return {
    x: Math.round(a.x + (b.x - a.x) * p),
    y: Math.round(a.y + (b.y - a.y) * p),
    width: Math.max(1, Math.round(a.width + (b.width - a.width) * p)),
    height: Math.max(1, Math.round(a.height + (b.height - a.height) * p)),
  };
}

/**
 * Glides a window from one rectangle to another, optionally fading.
 *
 * @param {BrowserWindow} win
 * @param {object} opts
 * @param {object} [opts.from] starting rect; defaults to where the window is
 * @param {object} opts.to end rect
 * @param {number} [opts.duration] milliseconds
 * @param {() => void} [opts.onDone] runs on completion. A later animate()
 *        call takes over instead, and does not run it: the state it would
 *        settle is no longer the state being aimed at.
 */
function animate(win, opts) {
  if (!win || win.isDestroyed()) return;
  const to = opts.to;
  const duration = opts.duration || 240;
  const finish = () => {
    if (win.isDestroyed()) return;
    win.setBounds(to);
    if (opts.onDone) opts.onDone();
  };

  stop(win);
  if (reduced || duration <= 0) {
    finish();
    return;
  }

  const from = opts.from || win.getBounds();
  win.setBounds(lerpRect(from, to, 0));

  const started = Date.now();
  const timer = setInterval(() => {
    if (win.isDestroyed()) {
      stop(win);
      return;
    }
    const elapsed = Date.now() - started;
    const linear = Math.min(1, elapsed / duration);
    const p = GLIDE(linear);
    if (linear >= 1) {
      stop(win);
      finish();
      return;
    }
    win.setBounds(lerpRect(from, to, p));
  }, FRAME_MS);
  running.set(win.id, timer);
}

/**
 * Start rect for a surface arriving: slightly smaller, held against the edge
 * it grows away from, so it reads as unfolding from that edge rather than
 * inflating from its middle.
 *
 * The anchored edges match the ones the stylesheet pins the panel to, so the
 * contents stay still and the window uncovers them, rather than the contents
 * sliding around inside a moving frame.
 *
 * @param {object} rect the final rectangle
 * @param {'top'|'bottom'|'center'} anchorV which horizontal edge stays put
 * @param {'left'|'right'|'center'} anchorH which vertical edge stays put
 */
function shrunkFrom(rect, anchorV, anchorH) {
  const width = Math.round(rect.width * 0.92);
  const height = Math.round(rect.height * 0.86);
  let x;
  if (anchorH === 'right') x = rect.x + (rect.width - width);
  else if (anchorH === 'left') x = rect.x;
  else x = rect.x + Math.round((rect.width - width) / 2);
  let y;
  if (anchorV === 'bottom') y = rect.y + (rect.height - height);
  else if (anchorV === 'center') y = rect.y + Math.round((rect.height - height) / 2);
  else y = rect.y;
  return { x, y, width, height };
}

module.exports = { animate, stop, isAnimating, shrunkFrom, setReducedMotion, isReduced };
