'use strict';

// A transparent margin around the bar and the answer window, so the glass
// can cast its shadow. Before this the window was exactly the size of the
// panel and everything past its edge was discarded: the "deep soft" shadow
// DESIGN.md described had never once been drawn.
//
// The rest of main.js thinks in the rectangle the person sees (the glass),
// and only the window itself is bigger. Rather than teach forty call sites
// about the margin, the window's own bounds methods are wrapped once here:
// getBounds returns the glass rectangle, setBounds takes one, and the size
// limits are offset by the same amount. lib/motion.js, the layout editor and
// the docking maths keep working in the units they always used.
//
// The margin is click-through: each page turns hit-testing off while the
// pointer is over anything that is not glass (see renderer/glass.js).

const MARGIN = 20;

function inflate(r, m) {
  return { x: r.x - m, y: r.y - m, width: r.width + 2 * m, height: r.height + 2 * m };
}

function deflate(r, m) {
  return { x: r.x + m, y: r.y + m, width: Math.max(1, r.width - 2 * m), height: Math.max(1, r.height - 2 * m) };
}

/** Bounds for a new window whose glass should land on `rect`. */
function outer(rect, m = MARGIN) {
  return inflate(rect, m);
}

/**
 * Wraps a window so its bounds API speaks in glass rectangles.
 * @param {Electron.BrowserWindow} win
 * @param {number} [m]
 */
function wrap(win, m = MARGIN) {
  if (!win || win.stageMargin !== undefined) return win;
  const raw = {
    setBounds: win.setBounds.bind(win),
    getBounds: win.getBounds.bind(win),
    setMinimumSize: win.setMinimumSize.bind(win),
    setMaximumSize: win.setMaximumSize.bind(win),
    getMinimumSize: win.getMinimumSize.bind(win),
    getMaximumSize: win.getMaximumSize.bind(win),
  };
  win.stageMargin = m;
  win.rawBounds = raw.getBounds;
  win.getBounds = () => deflate(raw.getBounds(), m);
  win.setBounds = (r, animate) => {
    const cur = deflate(raw.getBounds(), m);
    const full = {
      x: r.x !== undefined ? r.x : cur.x,
      y: r.y !== undefined ? r.y : cur.y,
      width: r.width !== undefined ? r.width : cur.width,
      height: r.height !== undefined ? r.height : cur.height,
    };
    raw.setBounds(inflate(full, m), animate);
  };
  win.setMinimumSize = (w, h) => raw.setMinimumSize(w + 2 * m, h + 2 * m);
  // 0 means "no limit" to Electron, and stays that way.
  win.setMaximumSize = (w, h) => raw.setMaximumSize(w ? w + 2 * m : 0, h ? h + 2 * m : 0);
  win.getMinimumSize = () => raw.getMinimumSize().map((v) => Math.max(0, v - 2 * m));
  win.getMaximumSize = () => raw.getMaximumSize().map((v) => (v ? v - 2 * m : 0));
  return win;
}

module.exports = { MARGIN, wrap, outer, inflate, deflate };
