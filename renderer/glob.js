'use strict';

// The glob (REQUESTS.md 8, DESIGN.md "The glob"): an output window opens by
// stretching out of the chat bar like a drop of liquid glass, and closes by
// pulling back into it.
//
// The bar is another window, but the two sit closer than the 20 px shadow
// margin both carry, so this window can draw over the bar's edge. One shape
// is drawn: the sheet's rounded rectangle, starting as a small bud against
// the bar's edge, and a neck joining it to the bar that thins as the bud
// grows, pinches off, and leaves two small beads that are absorbed, one into
// the bar and one into the sheet. The shape is a single SVG path, recomputed
// each frame; it clips a real piece of the page's glass (#glob is a .glass
// element, so glass.js gives it the backdrop and the scrim) and a masked
// stroke draws its rim. Only a path changes per frame, so it holds 60 fps.
//
// Reduced motion: a 150 ms cross-fade, no shape change.

(() => {
  const host = document.getElementById('glob');
  if (!host) return;
  const strokeP = document.getElementById('glob-stroke');
  const fillP = document.getElementById('glob-fill');
  const root = document.documentElement;

  const D_OPEN = 440;
  const D_CLOSE = 260;
  const PINCH = 0.6;
  const BEAD_END = 0.84;
  const R = 28;

  // cubic-bezier as in CSS, solved for x by Newton then bisection.
  function bezier(x1, y1, x2, y2) {
    const cx = 3 * x1;
    const bx = 3 * (x2 - x1) - cx;
    const ax = 1 - cx - bx;
    const cy = 3 * y1;
    const by = 3 * (y2 - y1) - cy;
    const ay = 1 - cy - by;
    const sx = (t) => ((ax * t + bx) * t + cx) * t;
    const sy = (t) => ((ay * t + by) * t + cy) * t;
    const dx = (t) => (3 * ax * t + 2 * bx) * t + cx;
    return (x) => {
      if (x <= 0) return 0;
      if (x >= 1) return 1;
      let t = x;
      for (let i = 0; i < 6; i += 1) {
        const e = sx(t) - x;
        const d = dx(t);
        if (Math.abs(e) < 1e-5 || Math.abs(d) < 1e-6) break;
        t -= e / d;
      }
      if (t < 0 || t > 1 || Math.abs(sx(t) - x) > 1e-3) {
        let lo = 0;
        let hi = 1;
        t = x;
        for (let i = 0; i < 24; i += 1) {
          if (sx(t) < x) lo = t;
          else hi = t;
          t = (lo + hi) / 2;
        }
      }
      return sy(t);
    };
  }
  // The DESIGN.md curves: shape changes on --ease-morph, the close on
  // --ease-glide (window-level, no overshoot).
  const easeMorph = bezier(0.22, 1, 0.36, 1);
  const easeGlide = bezier(0.32, 0.72, 0, 1);

  const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
  const lerp = (a, b, t) => a + (b - a) * t;
  const smooth = (a, b, x) => {
    const t = clamp((x - a) / (b - a), 0, 1);
    return t * t * (3 - 2 * t);
  };

  // ------------------------------------------------------------- geometry

  // Work in a frame where the bar is "above" and v grows from the bar into
  // the sheet; map to the window at the end. That keeps one shape function
  // for all four sides.
  function frameFor(side, W, H) {
    if (side === 'bottom') return { to: (u, v) => [u, H - v], from: (x, y) => [x, H - y] };
    if (side === 'left') return { to: (u, v) => [v, u], from: (x, y) => [y, x] };
    if (side === 'right') return { to: (u, v) => [W - v, u], from: (x, y) => [y, W - x] };
    return { to: (u, v) => [u, v], from: (x, y) => [x, y] };
  }

  // A rect in window coordinates as { u0, u1, v0, v1 } in the side's frame.
  function toFrame(f, r) {
    const a = f.from(r.left, r.top);
    const b = f.from(r.right, r.bottom);
    return { u0: Math.min(a[0], b[0]), u1: Math.max(a[0], b[0]), v0: Math.min(a[1], b[1]), v1: Math.max(a[1], b[1]) };
  }

  function sideOf(bar, sheet) {
    if (bar.bottom <= sheet.top + 8) return 'top';
    if (bar.top >= sheet.bottom - 8) return 'bottom';
    if (bar.right <= sheet.left + 8) return 'left';
    if (bar.left >= sheet.right - 8) return 'right';
    return 'top';
  }

  const pt = (p) => `${p[0].toFixed(1)} ${p[1].toFixed(1)}`;

  // Clockwise on screen (SVG sweep 1), so with the nonzero rule every piece
  // adds to the shape and none cuts a hole.
  function roundRect(f, q) {
    const r = Math.max(0, Math.min(R, (q.u1 - q.u0) / 2, (q.v1 - q.v0) / 2));
    const pts = [];
    const corner = (cu, cv, a0) => {
      for (let i = 0; i <= 6; i += 1) {
        const a = a0 + (Math.PI / 2) * (i / 6);
        pts.push(f.to(cu + r * Math.cos(a), cv + r * Math.sin(a)));
      }
    };
    corner(q.u1 - r, q.v0 + r, -Math.PI / 2);
    corner(q.u1 - r, q.v1 - r, 0);
    corner(q.u0 + r, q.v1 - r, Math.PI / 2);
    corner(q.u0 + r, q.v0 + r, Math.PI);
    return poly(pts);
  }

  function area(pts) {
    let s = 0;
    for (let i = 0; i < pts.length; i += 1) {
      const a = pts[i];
      const b = pts[(i + 1) % pts.length];
      s += a[0] * b[1] - b[0] * a[1];
    }
    return s;
  }

  function poly(pts) {
    if (pts.length < 3) return '';
    const p = area(pts) < 0 ? pts.slice().reverse() : pts;
    return `M${pt(p[0])}${p.slice(1).map((q) => `L${pt(q)}`).join('')}Z`;
  }

  function cubicPts(p0, c1, c2, p1, n) {
    const out = [];
    for (let i = 0; i <= n; i += 1) {
      const t = i / n;
      const m = 1 - t;
      out.push([
        m * m * m * p0[0] + 3 * m * m * t * c1[0] + 3 * m * t * t * c2[0] + t * t * t * p1[0],
        m * m * m * p0[1] + 3 * m * m * t * c1[1] + 3 * m * t * t * c2[1] + t * t * t * p1[1],
      ]);
    }
    return out;
  }

  // The neck: wide where it meets the bar and the sheet (a meniscus), thin
  // in the middle.
  function neck(f, u0, v0, a, u1, v1, b, w) {
    const d = v1 - v0;
    const left = cubicPts([u0 - a, v0], [u0 - w, v0 + d * 0.55], [u1 - w, v1 - d * 0.55], [u1 - b, v1], 12);
    const right = cubicPts([u1 + b, v1], [u1 + w, v1 - d * 0.55], [u0 + w, v0 + d * 0.55], [u0 + a, v0], 12);
    return poly([...left, ...right].map((p) => f.to(p[0], p[1])));
  }

  // A bead: half an ellipse on an edge, bulging by h toward dir (+1 into
  // the sheet side, -1 back toward the bar).
  function bead(f, u, v, half, h, dir) {
    if (half < 0.5 || h < 0.5) return '';
    const pts = [];
    for (let i = 0; i <= 12; i += 1) {
      const a = Math.PI * (i / 12);
      pts.push(f.to(u - half * Math.cos(a), v + dir * h * Math.sin(a)));
    }
    pts.push(f.to(u + half, v - dir * 2));
    pts.push(f.to(u - half, v - dir * 2));
    return poly(pts);
  }

  // The whole shape at time t (0..1, linear).
  function shapeAt(g, t) {
    const f = g.f;
    const S = g.sheet;
    const B = g.bar;
    const grow = easeMorph(clamp(t / 0.88, 0, 1));
    const budW = Math.min((S.u1 - S.u0) * 0.5, Math.max(96, (B.u1 - B.u0) * 0.4));
    const budH = 26;
    const start = B.v1 + 4;
    const cu = lerp(g.u0, (S.u0 + S.u1) / 2, grow);
    const w = lerp(budW, S.u1 - S.u0, grow);
    const q = {
      u0: cu - w / 2,
      u1: cu + w / 2,
      v0: lerp(start, S.v0, grow),
      v1: lerp(start + budH, S.v1, grow),
    };
    let d = roundRect(f, q);
    const gap = q.v0 - B.v1;
    const a = Math.min(56, budW * 0.42);
    const u1 = clamp(g.u0, q.u0 + R, q.u1 - R);
    if (gap > 3 && t < PINCH) {
      const waist = lerp(a * 0.72, 1.2, smooth(0, PINCH, t));
      d += neck(f, g.u0, B.v1 - 2, a, u1, q.v0 + 8, a, waist);
    } else if (gap > 3 && t < BEAD_END) {
      const k = 1 - smooth(PINCH, BEAD_END, t);
      d += bead(f, g.u0, B.v1 - 2, a * 0.7 * k, 10 * k, 1);
      d += bead(f, u1, q.v0 + 2, a * 0.7 * k, 9 * k, -1);
    }
    return d;
  }

  // ---------------------------------------------------------------- runs

  let run = null;

  function draw(d) {
    const gl = host.querySelector(':scope > .gl');
    if (gl) gl.style.clipPath = `path('${d}')`;
    strokeP.setAttribute('d', d);
    fillP.setAttribute('d', d);
  }

  function motionOn() {
    return root.dataset.motion !== 'reduced' && !matchMedia('(prefers-reduced-motion: reduce)').matches;
  }

  // Where the bar is, in this window's coordinates. Rust answers in screen
  // DIPs (ARCHITECTURE.md section 10); without an answer the glob comes out
  // of the sheet's top edge, where the bar usually is.
  async function barRect(sheet) {
    let r = null;
    try {
      r = await Promise.race([
        window.aeval.invoke('output:bar'),
        new Promise((res) => setTimeout(() => res(null), 150)),
      ]);
    } catch (err) {
      r = null;
    }
    if (r && Number.isFinite(r.x) && r.width > 0) {
      const left = r.x - window.screenX;
      const top = r.y - window.screenY;
      return { left, top, right: left + r.width, bottom: top + r.height, real: true };
    }
    const w = Math.min(sheet.width * 0.7, 420);
    const left = sheet.left + (sheet.width - w) / 2;
    const bottom = sheet.top - 12;
    return { left, top: bottom - 56, right: left + w, bottom, real: false };
  }

  function geometry(bar, sheetRect) {
    const side = sideOf(bar, sheetRect);
    const f = frameFor(side, window.innerWidth, window.innerHeight);
    const S = toFrame(f, sheetRect);
    const B = toFrame(f, bar);
    const u0 = clamp((S.u0 + S.u1) / 2, B.u0 + 24, Math.max(B.u0 + 24, B.u1 - 24));
    return { side, f, sheet: S, bar: B, u0 };
  }

  function animate(g, from, to, ms, ease, onFrame) {
    return new Promise((resolve) => {
      const t0 = performance.now();
      const step = (now) => {
        if (run !== g) return resolve(false);
        const x = clamp((now - t0) / ms, 0, 1);
        const t = lerp(from, to, ease ? ease(x) : x);
        draw(shapeAt(g, t));
        if (onFrame) onFrame(t);
        if (x < 1) requestAnimationFrame(step);
        else resolve(true);
      };
      requestAnimationFrame(step);
    });
  }

  function finish(sheet) {
    run = null;
    host.hidden = true;
    sheet.classList.remove('globbing', 'reveal', 'fading');
  }

  // Grows the sheet out of the bar. The sheet must already be laid out
  // (not hidden); its own glass and content stay invisible until the glob
  // has become exactly its shape.
  async function open(sheet) {
    if (!motionOn()) {
      sheet.classList.add('fading');
      void sheet.offsetWidth;
      sheet.classList.remove('fading');
      return;
    }
    sheet.classList.add('globbing');
    sheet.classList.remove('reveal');
    const rect = sheet.getBoundingClientRect();
    const g = geometry(await barRect(rect), rect);
    run = g;
    host.hidden = false;
    draw(shapeAt(g, 0));
    if (window.AevalGlass) {
      window.AevalGlass.adopt(host);
      window.AevalGlass.morph(D_OPEN);
    }
    document.body.dataset.glob = 'open';
    const done = await animate(g, 0, 1, D_OPEN, null, (t) => {
      if (t > 0.55) sheet.classList.add('reveal');
    });
    delete document.body.dataset.glob;
    if (done) finish(sheet);
  }

  // Pulls the sheet back into the bar, then resolves. With the bar closed
  // (Rust answers null) it fades.
  async function close(sheet) {
    if (!motionOn() || sheet.hidden) return;
    const rect = sheet.getBoundingClientRect();
    const bar = await barRect(rect);
    if (!bar.real) {
      sheet.classList.add('fading-out');
      await new Promise((res) => setTimeout(res, 150));
      return;
    }
    const g = geometry(bar, rect);
    run = g;
    host.hidden = false;
    draw(shapeAt(g, 1));
    sheet.classList.add('globbing');
    if (window.AevalGlass) window.AevalGlass.morph(D_CLOSE);
    document.body.dataset.glob = 'close';
    await animate(g, 1, 0, D_CLOSE, easeGlide);
    delete document.body.dataset.glob;
    host.hidden = true;
    run = null;
  }

  window.AevalGlob = { open, close, shapeAt, geometry, bezier };
})();
