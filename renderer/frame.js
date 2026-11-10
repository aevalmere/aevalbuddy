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

  // ----------------------------------------------------------- window menu

  // Everything a drag does, from the keyboard (U30): Alt+Space, or a
  // right-click on a title bar, opens Move, Size, Maximize or Restore, and
  // Close. In Move and Size the arrows go 16 px at a time (Shift, 64)
  // through window:nudge; Enter keeps the result, Esc puts the window back.
  // The bar has no title bar and has its own layout editor, so this is for
  // the windows with all eight resize zones.
  const STEP = 16;
  const STEP_BIG = 64;
  const ARROWS = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
  const CLOSERS = ['#close-out', '#close-settings', '#close-history'];

  const hasMenu = () => root.dataset.resize === 'all' && !!document.querySelector('[data-drag]');
  const say = (text) => window.AevalA11y && window.AevalA11y.say(text);

  let menu = null;
  let adjust = null;
  let hint = null;

  function menuRows() {
    const rows = [];
    // A maximized window has nowhere to move to.
    if (!maxed) {
      rows.push({ name: 'Move', run: () => startAdjust(false) });
      rows.push({ name: 'Size', run: () => startAdjust(true) });
    }
    // Only the windows whose title bar maximizes on double-click.
    if (document.querySelector('[data-drag="max"]')) {
      rows.push({
        name: maxed ? 'Restore' : 'Maximize',
        run: () => {
          const w = win();
          if (w) w.toggleMaximize().catch(() => {});
        },
      });
    }
    // An answer window folds into the pill with the bar, keeping its chat.
    const minimize = document.getElementById('minimize-out');
    if (minimize) rows.push({ name: 'Minimize', run: () => minimize.click() });
    rows.push({
      name: 'Close',
      run: () => {
        const button = CLOSERS.map((s) => document.querySelector(s)).find(Boolean);
        if (button) button.click();
      },
    });
    return rows;
  }

  function openMenu(x, y) {
    closeMenu(false);
    const el = document.createElement('ul');
    el.id = 'window-menu';
    el.className = 'glass thin g-menu';
    el.dataset.hit = '';
    el.setAttribute('role', 'menu');
    el.setAttribute('aria-label', 'Window');
    const rows = menuRows();
    const items = rows.map((row) => {
      const li = document.createElement('li');
      li.setAttribute('role', 'menuitem');
      li.tabIndex = -1;
      li.textContent = row.name;
      li.addEventListener('click', () => choose(row));
      li.addEventListener('pointermove', () => li.focus({ preventScroll: true }));
      el.appendChild(li);
      return li;
    });
    const back = document.activeElement;
    document.body.appendChild(el);
    // At the pointer, flipped and clamped inside the window (glass.js).
    if (window.AevalGlass && window.AevalGlass.place) window.AevalGlass.place(el, { x, y }, { gap: 0 });
    else {
      const r = el.getBoundingClientRect();
      el.style.left = `${Math.round(Math.max(8, Math.min(x, window.innerWidth - r.width - 8)))}px`;
      el.style.top = `${Math.round(Math.max(8, Math.min(y, window.innerHeight - r.height - 8)))}px`;
    }
    menu = { el, items, rows, back };
    items[0].focus();
    if (window.AevalGlass) window.AevalGlass.placeAll();
  }

  function closeMenu(refocus) {
    if (!menu) return;
    const { el, back } = menu;
    menu = null;
    el.classList.add('closing');
    setTimeout(() => el.remove(), 120);
    if (refocus && back && back.focus) back.focus();
  }

  function choose(row) {
    closeMenu(true);
    row.run();
  }

  function menuKey(e) {
    const { items, rows } = menu;
    const at = items.indexOf(document.activeElement);
    const go = (i) => items[(i + items.length) % items.length].focus();
    if (e.key === 'ArrowDown') go(at + 1);
    else if (e.key === 'ArrowUp') go(at < 0 ? -1 : at - 1);
    else if (e.key === 'Home') go(0);
    else if (e.key === 'End') go(-1);
    else if (e.key === 'Enter' || e.key === ' ') {
      if (at >= 0) choose(rows[at]);
    } else if (e.key === 'Escape' || e.key === 'Tab') closeMenu(true);
    else if (e.key.length === 1) {
      const i = rows.findIndex((r) => r.name[0].toLowerCase() === e.key.toLowerCase());
      if (i >= 0) go(i);
    }
  }

  function nudge(d) {
    window.aeval.invoke('window:nudge', d).catch(() => {});
  }

  function startAdjust(size) {
    adjust = {
      size,
      sum: { dx: 0, dy: 0, dw: 0, dh: 0 },
      x: window.screenX,
      y: window.screenY,
      w: window.innerWidth,
      h: window.innerHeight,
    };
    const text = `${size ? 'Arrows resize the window.' : 'Arrows move the window.'} Enter keeps it, Esc puts it back.`;
    hint = document.createElement('div');
    hint.id = 'window-hint';
    hint.className = 'glass thin';
    hint.dataset.hit = '';
    hint.textContent = text;
    document.body.appendChild(hint);
    say(text);
  }

  function stopAdjust() {
    adjust = null;
    if (hint) hint.remove();
    hint = null;
  }

  // Rust stops a window at the edge of the work area, so the way back is
  // what really changed. Without a real window to measure, it is the sum of
  // the presses.
  function putBack() {
    const a = adjust;
    const undo = win()
      ? { dx: a.x - window.screenX, dy: a.y - window.screenY, dw: a.w - window.innerWidth, dh: a.h - window.innerHeight }
      : { dx: 0 - a.sum.dx, dy: 0 - a.sum.dy, dw: 0 - a.sum.dw, dh: 0 - a.sum.dh };
    if (undo.dx || undo.dy || undo.dw || undo.dh) nudge(undo);
  }

  function adjustKey(e) {
    if (e.key === 'Enter') {
      stopAdjust();
      return;
    }
    if (e.key === 'Escape') {
      putBack();
      stopAdjust();
      return;
    }
    const arrow = ARROWS[e.key];
    if (!arrow) return;
    const by = e.shiftKey ? STEP_BIG : STEP;
    const [x, y] = [arrow[0] * by, arrow[1] * by];
    const d = adjust.size ? { dx: 0, dy: 0, dw: x, dh: y } : { dx: x, dy: y, dw: 0, dh: 0 };
    for (const k of Object.keys(d)) adjust.sum[k] += d[k];
    nudge(d);
  }

  // In the capture phase: while either is open the keys are theirs.
  window.addEventListener(
    'keydown',
    (e) => {
      const own = adjust ? adjustKey : menu ? menuKey : null;
      const opens = !own && e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey && e.code === 'Space' && hasMenu();
      if (!own && !opens) return;
      e.preventDefault();
      e.stopPropagation();
      if (own) {
        own(e);
        return;
      }
      const bar = document.querySelector('[data-drag]').getBoundingClientRect();
      openMenu(bar.left + 8, bar.bottom + 4);
    },
    true
  );

  document.addEventListener('contextmenu', (e) => {
    const t = e.target;
    if (!(t instanceof Element) || !hasMenu()) return;
    if (!t.closest('[data-drag]') || t.closest(CONTROL)) return;
    e.preventDefault();
    openMenu(e.clientX, e.clientY);
  });

  document.addEventListener(
    'pointerdown',
    (e) => {
      if (menu && !menu.el.contains(e.target)) closeMenu(false);
    },
    true
  );

  // Clicking away keeps where the window got to.
  window.addEventListener('blur', () => {
    closeMenu(false);
    stopAdjust();
  });

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
