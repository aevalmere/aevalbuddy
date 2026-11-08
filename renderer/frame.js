'use strict';

// Window chrome for frameless pages: what Electron's -webkit-app-region and
// the thick frame used to give, done through Tauri's window calls so it works
// whatever the Rust hit-test does (REQUESTS.md 10 and 11, DESIGN.md "Windows
// you can hold").
//
//   [data-drag]         press and drag anywhere on it, or on any child that
//                       is not a control, and the window moves. "max" also
//                       maximises on double-click (Settings, History).
//   html[data-resize]   grab zones along the sheet's edges and corners:
//                       "all" for eight, "x" for left and right only (the
//                       bar, whose height follows its content). The sheet is
//                       html[data-resize-target], a selector.
//   html[data-maximized] set here while the window is maximized (Tauri's
//                       isMaximized, read again on every resize); the
//                       zones are hidden then.
//
// The zones sit across the glass edge, half in the transparent shadow margin
// and half on the rim, and carry data-hit so the margin's click-through
// (glass.js, overlay.js) never swallows a grab.

(() => {
  const root = document.documentElement;
  const win = () => {
    const t = window.__TAURI__;
    return t && t.window && t.window.getCurrentWindow ? t.window.getCurrentWindow() : null;
  };

  // ------------------------------------------------------------------ drag

  const CONTROL = 'button, a, input, select, textarea, label, summary, [role="button"], [role="option"], [contenteditable], [data-no-drag]';

  document.addEventListener('mousedown', (e) => {
    if (e.button !== 0) return;
    const t = e.target;
    if (!(t instanceof Element)) return;
    const region = t.closest('[data-drag]');
    if (!region || t.closest(CONTROL)) return;
    const w = win();
    if (!w) return;
    if (e.detail === 2 && region.dataset.drag === 'max') {
      e.preventDefault();
      w.toggleMaximize().catch(() => {});
      return;
    }
    // Text in a title can still be selected by a deliberate drag elsewhere;
    // here a press means "hold the window".
    e.preventDefault();
    w.startDragging().catch(() => {});
  });

  // ---------------------------------------------------------------- resize

  const DIRS = {
    n: 'North',
    s: 'South',
    e: 'East',
    w: 'West',
    ne: 'NorthEast',
    nw: 'NorthWest',
    se: 'SouthEast',
    sw: 'SouthWest',
  };
  const CURSOR = { n: 'ns-resize', s: 'ns-resize', e: 'ew-resize', w: 'ew-resize', ne: 'nesw-resize', sw: 'nesw-resize', nw: 'nwse-resize', se: 'nwse-resize' };
  // Outside the glass edge, then inside it: 7 + 3 is a 10 px band, wide
  // enough to find and narrow enough to leave the margin mostly clear.
  const OUT = 7;
  const IN = 3;
  const CORNER = 18;

  const handles = new Map();

  function makeHandle(d) {
    const h = document.createElement('div');
    h.className = `frame-handle fh-${d}`;
    h.dataset.hit = '';
    h.dataset.dir = d;
    h.setAttribute('aria-hidden', 'true');
    h.style.cssText = `position:fixed;z-index:2147483000;cursor:${CURSOR[d]};background:transparent;touch-action:none;display:none;`;
    h.addEventListener('mousedown', (e) => {
      if (e.button !== 0) return;
      e.preventDefault();
      e.stopPropagation();
      const w = win();
      if (w) w.startResizeDragging(DIRS[d]).catch(() => {});
    });
    document.body.appendChild(h);
    handles.set(d, h);
    return h;
  }

  function target() {
    const sel = root.dataset.resizeTarget;
    if (!sel) return null;
    return document.querySelector(sel);
  }

  function wanted() {
    const mode = root.dataset.resize;
    if (mode === 'all') return Object.keys(DIRS);
    if (mode === 'x') return ['e', 'w'];
    return [];
  }

  // ------------------------------------------------------------- maximized

  // html[data-maximized] while the window fills the work area: pages drop
  // their stage margin and corners, and there is no edge to resize.
  let maxed = false;
  function setMaximized(on) {
    if (on === maxed) return;
    maxed = on;
    if (on) root.dataset.maximized = '';
    else delete root.dataset.maximized;
    requestLayout();
  }

  let maxQueued = false;
  function syncMaximized() {
    if (maxQueued) return;
    maxQueued = true;
    requestAnimationFrame(() => {
      maxQueued = false;
      const w = win();
      if (!w || typeof w.isMaximized !== 'function') return;
      w.isMaximized()
        .then((m) => setMaximized(!!m))
        .catch(() => {});
    });
  }

  let queued = false;
  function layout() {
    queued = false;
    const want = new Set(wanted());
    const el = target();
    const r = !maxed && el && !el.hidden && !el.closest('[hidden]') ? el.getBoundingClientRect() : null;
    const show = r && r.width > 40 && r.height > 20;
    for (const d of Object.keys(DIRS)) {
      if (!want.has(d)) {
        const h = handles.get(d);
        if (h) h.style.display = 'none';
        continue;
      }
      const h = handles.get(d) || makeHandle(d);
      if (!show) {
        h.style.display = 'none';
        continue;
      }
      const band = OUT + IN;
      const x0 = Math.round(r.left - OUT);
      const y0 = Math.round(r.top - OUT);
      const x1 = Math.round(r.right - IN);
      const y1 = Math.round(r.bottom - IN);
      let box;
      if (d === 'n') box = [r.left + CORNER, y0, r.width - 2 * CORNER, band];
      else if (d === 's') box = [r.left + CORNER, y1, r.width - 2 * CORNER, band];
      else if (d === 'w') box = [x0, want.has('n') ? r.top + CORNER : r.top, band, want.has('n') ? r.height - 2 * CORNER : r.height];
      else if (d === 'e') box = [x1, want.has('n') ? r.top + CORNER : r.top, band, want.has('n') ? r.height - 2 * CORNER : r.height];
      else {
        const size = CORNER + OUT;
        const x = d.includes('w') ? x0 : Math.round(r.right - CORNER);
        const y = d.includes('n') ? y0 : Math.round(r.bottom - CORNER);
        box = [x, y, size, size];
      }
      h.style.left = `${Math.round(box[0])}px`;
      h.style.top = `${Math.round(box[1])}px`;
      h.style.width = `${Math.max(0, Math.round(box[2]))}px`;
      h.style.height = `${Math.max(0, Math.round(box[3]))}px`;
      h.style.display = 'block';
    }
  }

  function requestLayout() {
    if (queued) return;
    queued = true;
    requestAnimationFrame(layout);
  }

  function boot() {
    requestLayout();
    const ro = new ResizeObserver(requestLayout);
    let observed = null;
    const watch = () => {
      const el = target();
      if (el && el !== observed) {
        if (observed) ro.unobserve(observed);
        ro.observe(el);
        observed = el;
      }
      requestLayout();
    };
    watch();
    window.addEventListener('resize', () => {
      requestLayout();
      syncMaximized();
    });
    syncMaximized();
    const w = win();
    if (w && typeof w.onResized === 'function') {
      w.onResized(syncMaximized).catch(() => {});
    }
    // Showing or hiding the sheet, a mode change on <body> or a new target.
    new MutationObserver(watch).observe(document.body, {
      attributes: true,
      subtree: true,
      attributeFilter: ['hidden', 'data-mode', 'class'],
    });
    new MutationObserver(watch).observe(root, { attributes: true, attributeFilter: ['data-resize', 'data-resize-target'] });
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', boot);
  else boot();

  window.AevalFrame = { layout: requestLayout, maximized: () => maxed };
})();
