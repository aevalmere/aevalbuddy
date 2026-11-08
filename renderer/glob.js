'use strict';

// The glob (REQUESTS.md 8, DESIGN.md "The glob"): an output window opens by
// stretching out of the chat bar like a drop of liquid glass, and closes by
// pulling back into it.
//
// The bar is another window, but the two sit closer than the 20 px shadow
// margin both carry, so this window can draw over the bar's edge. One shape
// is drawn per frame (design-visual F2), in a frame where the bar is above
// and v grows from the bar into the sheet:
//
//   A  drop forms     0    - 0.18  a meniscus hangs from the bar's edge,
//                                  its top 6 px inside the bar
//   B  stretch, pinch 0.18 - 0.62  the body grows toward the sheet on a
//                                  spring; the neck thins to a point, its
//                                  tangents horizontal at both ends
//   C  recoil         0.62 - 0.82  a bead retracts into the bar and a bump
//                                  into the sheet
//   Land              0.86 - 1     the body is the sheet; the sheet's own
//                                  glass takes over from the glob
//   Words             from 0.70    the transcript arrives inside the glass
//
// The shape clips a real piece of glass (#glob is a .glass element, so
// glass.js gives it the backdrop and the scrim), a thin layer under the
// bar's own scrim so the join is one material, and the sheet's words; a
// masked stroke draws its rim. Only paths change per frame.
//
// When the sheet is not beside the bar (a cascaded or dragged window, or no
// bar), the sheet scales in from the bar's side instead (F25). Reduced
// motion: a 150 ms cross-fade, no shape change. Every timing runs on
// requestAnimationFrame and performance.now.

(() => {
  const host = document.getElementById('glob');
  if (!host) return;
  const thin = document.getElementById('glob-thin');
  const strokeP = document.getElementById('glob-stroke');
  const fillP = document.getElementById('glob-fill');
  const barMask = document.getElementById('glob-bar');
  const root = document.documentElement;

  const D_OPEN = 480;
  const D_CLOSE = 260;
  const D_SCALE = 280;
  const D_SCALE_CLOSE = 160;
  // Close: the words leave first, then the sheet's glass hands over.
  const WORDS_OUT = 80;
  const HANDOVER_FROM = 20;
  const OVERLAP = 6;
  const T_DROP = 0.18;
  const T_PINCH = 0.62;
  const T_RECOIL = 0.82;
  const T_LAND = 0.86;
  const T_REVEAL = 0.7;
  const R = 28;
  // Beyond 14 px (the 20 px margin less the overlap) the neck would leave
  // this window; under 48 px of shared edge there is no room for it.
  const MAX_GAP = 14;
  const MIN_CROSS = 48;

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
  // The DESIGN.md curves. --ease-spring is the body: a spring with a
  // response near 0.42 s and damping 0.88, no overshoot.
  const easeSpring = bezier(0.2, 0.9, 0.25, 1);
  const easeOut = bezier(0.16, 1, 0.3, 1);
  const easeMorph = bezier(0.22, 1, 0.36, 1);
  const easeGlide = bezier(0.32, 0.72, 0, 1);

  const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
  const lerp = (a, b, t) => a + (b - a) * t;
  const smooth = (a, b, x) => {
    const t = clamp((x - a) / (b - a), 0, 1);
    return t * t * (3 - 2 * t);
  };

  // The body's growth: F2's spring, renormalised so it starts from the drop
  // at the end of phase A and reaches the sheet at T_LAND.
  const spring = (t) => easeSpring(clamp((t - 0.08) / 0.78, 0, 1));
  const G0 = spring(T_DROP);
  const growAt = (t) => clamp((spring(t) - G0) / (1 - G0), 0, 1);

  // ------------------------------------------------------------- geometry

  // Work in a frame where the bar is "above" and v grows from the bar into
  // the sheet; map to the window at the end. One shape function for all
  // four sides.
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

  function area(pts) {
    let s = 0;
    for (let i = 0; i < pts.length; i += 1) {
      const a = pts[i];
      const b = pts[(i + 1) % pts.length];
      s += a[0] * b[1] - b[0] * a[1];
    }
    return s;
  }

  // Every piece winds the same way on screen, so with the nonzero rule the
  // pieces add up to one body and none cuts a hole.
  function piece(f, pts) {
    if (pts.length < 3) return null;
    const p = pts.map((q) => f.to(q[0], q[1]));
    return area(p) < 0 ? p.reverse() : p;
  }

  function roundRect(f, q) {
    const w = q.u1 - q.u0;
    const h = q.v1 - q.v0;
    if (w < 0.5 || h < 0.5) return null;
    const r = Math.max(0, Math.min(R, w / 2, h / 2));
    const pts = [];
    const corner = (cu, cv, a0) => {
      for (let i = 0; i <= 6; i += 1) {
        const a = a0 + (Math.PI / 2) * (i / 6);
        pts.push([cu + r * Math.cos(a), cv + r * Math.sin(a)]);
      }
    };
    corner(q.u1 - r, q.v0 + r, -Math.PI / 2);
    corner(q.u1 - r, q.v1 - r, 0);
    corner(q.u0 + r, q.v1 - r, Math.PI / 2);
    corner(q.u0 + r, q.v0 + r, Math.PI);
    return piece(f, pts);
  }

  const N_NECK = 16;

  // The neck from the bar (centre u0, half-width a, starting OVERLAP inside
  // the bar at v0) to the body (centre u1, half-width b, ending inside it at
  // v1), its half-width w at the middle. Both ends leave horizontally, so
  // each join is a fillet, not a corner. The width is a cubic of its own
  // and never crosses the centre line.
  function neck(f, u0, v0, a, u1, v1, b, w) {
    const c = (8 * w - a - b) / 6;
    const left = [];
    const right = [];
    for (let i = 0; i <= N_NECK; i += 1) {
      const t = i / N_NECK;
      const m = 1 - t;
      const k0 = m * m * m;
      const k1 = 3 * m * m * t;
      const k2 = 3 * m * t * t;
      const k3 = t * t * t;
      const v = (k0 + k1) * v0 + (k2 + k3) * v1;
      const cu = (k0 + k1) * u0 + (k2 + k3) * u1;
      const off = Math.max(0.4, k0 * a + (k1 + k2) * c + k3 * b);
      left.push([cu - off, v]);
      right.push([cu + off, v]);
    }
    return piece(f, [...left, ...right.reverse()]);
  }

  function lerpRect(a, b, t) {
    return { u0: lerp(a.u0, b.u0, t), u1: lerp(a.u1, b.u1, t), v0: lerp(a.v0, b.v0, t), v1: lerp(a.v1, b.v1, t) };
  }

  // The 6 px behind an edge at v = e (dir +1: the bar's lower edge, -1: the
  // body's upper edge) where a neck or bead attaches, so the shape covers
  // the rim it joins. At the bar, the thin layer is the bar's own material.
  function behind(f, u, e, a, dir) {
    if (a < 0.5) return null;
    return piece(f, [
      [u - a, e - dir * OVERLAP],
      [u + a, e - dir * OVERLAP],
      [u + a, e + dir * 0.5],
      [u - a, e + dir * 0.5],
    ]);
  }

  // A neck that ends in a point (b = 0) is a drop: its sides leave the edge
  // horizontally and meet in a round tip. This waist makes it one.
  const dropWaist = (a) => 0.556 * a;

  // A bead on an edge at v = e: hw wide where it leaves the edge, a round
  // tip h away toward dir. The two halves of a pinched neck are two beads.
  function bead(f, u, e, hw, h, dir) {
    if (hw < 0.5 || h < 0.5) return [];
    return [behind(f, u, e, hw, dir), neck(f, u, e, hw, u, e + dir * h, 0, dropWaist(hw))];
  }

  // The drop at the end of phase A hangs DROP_DEPTH under the bar, DROP_HALF
  // wide each side where it leaves it. The body starts inside it.
  const DROP_HALF = 70;
  const DROP_DEPTH = 22;
  // The share of phase B over which the drop's neck becomes the bridge.
  const HANDOVER = 0.35;

  // The pieces at time t (0..1, linear), in window coordinates.
  function polysAt(g, t) {
    const f = g.f;
    const S = g.sheet;
    const B = g.bar;
    const u0 = g.u0;
    const out = [];
    const add = (p) => {
      if (Array.isArray(p) && Array.isArray(p[0]) && !Array.isArray(p[0][0])) out.push(p);
      else if (Array.isArray(p)) p.forEach(add);
    };
    // Where the neck lands on the body: on the straight part of its top
    // edge, nearest the bar's attach point.
    const land = (q, b) => {
      const r = Math.min(R, (q.u1 - q.u0) / 2, (q.v1 - q.v0) / 2);
      const lo = q.u0 + r + b;
      const hi = q.u1 - r - b;
      return lo <= hi ? clamp(u0, lo, hi) : (q.u0 + q.u1) / 2;
    };
    const straight = (q) => (q.u1 - q.u0) / 2 - Math.min(R, (q.u1 - q.u0) / 2, (q.v1 - q.v0) / 2) - 2;
    const tip = B.v1 + DROP_DEPTH;
    if (t < T_DROP) {
      // A: a drop swells out of the bar's edge.
      const e = easeOut(t / T_DROP);
      const a = lerp(28, DROP_HALF, e);
      add(bead(f, u0, B.v1, a, lerp(3, DROP_DEPTH, e), 1));
      return out;
    }
    // The body starts as a capsule inside the drop and grows into the sheet.
    const Q1 = { u0: u0 - 22, u1: u0 + 22, v0: B.v1 + 8, v1: tip - 1 };
    const q = lerpRect(Q1, S, growAt(t));
    add(roundRect(f, q));
    if (t < T_PINCH) {
      // B: the drop's sides become a neck from the bar to the body's top
      // edge, both ends horizontal, and the neck thins to the pinch. Its
      // waist stays narrower than both ends: a meniscus, never a tab.
      const s = smooth(T_DROP, T_PINCH, t);
      const m = smooth(0, HANDOVER, s);
      const a = lerp(DROP_HALF, 10, s);
      const bT = Math.max(0, Math.min(lerp(DROP_HALF, 18, s), straight(q)));
      const wT = Math.min(lerp(60, 1, s ** 1.4), 0.8 * Math.min(a, bT));
      add(behind(f, u0, B.v1, a, 1));
      add(neck(f, u0, B.v1, a, lerp(u0, land(q, bT), m), lerp(tip, q.v0, m), bT * m, lerp(dropWaist(a), wT, m)));
    } else if (t < T_RECOIL) {
      // C: the two halves of the neck retract, into the bar and the sheet.
      const k = 1 - easeOut((t - T_PINCH) / (T_RECOIL - T_PINCH));
      const mid = (q.v0 - B.v1) / 2;
      add(bead(f, u0, B.v1, 10 * k, Math.min(6, Math.max(0, mid)) * k, 1));
      add(bead(f, land(q, 18), q.v0, 18 * k, Math.min(5, Math.max(0, mid)) * k, -1));
    }
    return out;
  }

  const fmt = (n) => (Math.round(n * 10) / 10).toString();

  function toPath(polys, ox, oy) {
    let d = '';
    for (const p of polys) {
      d += `M${fmt(p[0][0] - ox)} ${fmt(p[0][1] - oy)}`;
      for (let i = 1; i < p.length; i += 1) d += `L${fmt(p[i][0] - ox)} ${fmt(p[i][1] - oy)}`;
      d += 'Z';
    }
    return d;
  }

  // The whole shape at time t, as an SVG path in window coordinates.
  function shapeAt(g, t) {
    return toPath(polysAt(g, t), 0, 0);
  }

  // ------------------------------------------------------------- drawing

  let run = null;
  const glOf = (el) => el && el.querySelector(':scope > .gl');

  function motionOn() {
    return root.dataset.motion !== 'reduced' && !matchMedia('(prefers-reduced-motion: reduce)').matches;
  }

  // One frame of the shape: the glob's glass and its thin layer clipped to
  // it, the rim, and (opening) the sheet's words clipped to it too.
  function drawShape(g, t, sheet, clipWords) {
    const polys = polysAt(g, t);
    const d = toPath(polys, 0, 0);
    const clip = `path(nonzero, '${d}')`;
    const gl = glOf(host);
    if (gl) gl.style.clipPath = clip;
    strokeP.setAttribute('d', d);
    fillP.setAttribute('d', d);
    // The bar's material at the join fades as the body leaves it.
    const thinOn = 1 - smooth(0.1, 0.55, t);
    if (thin) {
      thin.hidden = thinOn <= 0.001;
      thin.style.opacity = thinOn.toFixed(3);
      const tgl = glOf(thin);
      if (tgl && !thin.hidden) tgl.style.clipPath = clip;
    }
    if (clipWords) sheet.style.clipPath = `path(nonzero, '${toPath(polys, g.origin.x, g.origin.y)}')`;
  }

  // Where the bar is, in this window's coordinates. Rust answers in screen
  // DIPs (ARCHITECTURE.md section 10); null while the bar is closed.
  async function barRect() {
    let r = null;
    try {
      r = await Promise.race([window.aeval.invoke('output:bar'), new Promise((res) => setTimeout(() => res(null), 150))]);
    } catch (err) {
      r = null;
    }
    if (r && Number.isFinite(r.x) && r.width > 0) {
      const left = r.x - window.screenX;
      const top = r.y - window.screenY;
      return { left, top, right: left + r.width, bottom: top + r.height, real: true, screen: r };
    }
    return null;
  }

  function geometry(bar, sheetRect) {
    const side = sideOf(bar, sheetRect);
    const f = frameFor(side, window.innerWidth, window.innerHeight);
    const S = toFrame(f, sheetRect);
    const B = toFrame(f, bar);
    const u0 = clamp((S.u0 + S.u1) / 2, B.u0 + 24, Math.max(B.u0 + 24, B.u1 - 24));
    const cross = Math.min(S.u1, B.u1) - Math.max(S.u0, B.u0);
    const gap = S.v0 - B.v1;
    return {
      side,
      f,
      sheet: S,
      bar: B,
      u0,
      gap,
      cross,
      // The shape needs the sheet beside the bar: close enough for the neck
      // to stay in this window, and sharing enough of its edge.
      beside: gap >= -2 && gap <= MAX_GAP && cross >= MIN_CROSS,
      origin: { x: sheetRect.left, y: sheetRect.top },
    };
  }

  // The bar's own scrim on the thin layer, and the rim kept out of the bar.
  function prepare(g, bar) {
    if (window.AevalGlass) {
      window.AevalGlass.adopt(host);
      if (thin) window.AevalGlass.adopt(thin);
    }
    const tgl = glOf(thin);
    if (tgl && bar && bar.screen && window.AevalGlass && typeof window.AevalGlass.scrimFor === 'function') {
      tgl.style.setProperty('--scrim', String(window.AevalGlass.scrimFor(bar.screen, 'thin')));
    }
    if (barMask) {
      barMask.setAttribute('x', fmt(bar.left - 1));
      barMask.setAttribute('y', fmt(bar.top - 1));
      barMask.setAttribute('width', fmt(bar.right - bar.left + 2));
      barMask.setAttribute('height', fmt(bar.bottom - bar.top + 2));
    }
    host.style.opacity = '';
    host.hidden = false;
    // Its backdrop was laid out while hidden; place it for this frame.
    if (window.AevalGlass && typeof window.AevalGlass.placeAll === 'function') window.AevalGlass.placeAll();
  }

  function finish(sheet) {
    run = null;
    host.hidden = true;
    host.style.opacity = '';
    if (thin) thin.hidden = false;
    delete document.body.dataset.glob;
    sheet.classList.remove('globbing', 'reveal', 'fading', 'closing');
    sheet.style.clipPath = '';
    sheet.style.transform = '';
    sheet.style.transformOrigin = '';
    sheet.style.opacity = '';
    const gl = glOf(sheet);
    if (gl) gl.style.opacity = '';
  }

  // Runs fn(elapsed ms) each frame until it returns false or the run is
  // replaced; resolves true when it ran to the end.
  function frames(token, ms, fn) {
    return new Promise((resolve) => {
      const t0 = performance.now();
      const step = (now) => {
        if (run !== token) {
          resolve(false);
          return;
        }
        const x = Math.min(ms, Math.max(0, now - t0));
        fn(x);
        if (x < ms) requestAnimationFrame(step);
        else resolve(true);
      };
      requestAnimationFrame(step);
    });
  }

  // The state of an opening at time t: shape, words, and the hand-over.
  function openFrame(g, t, sheet) {
    const land = clamp((t - T_LAND) / (1 - T_LAND), 0, 1);
    drawShape(g, t, sheet, land === 0);
    if (land > 0) sheet.style.clipPath = '';
    sheet.classList.toggle('reveal', t >= T_REVEAL);
    const gl = glOf(sheet);
    if (gl) gl.style.opacity = land > 0 ? land.toFixed(3) : '';
    host.style.opacity = land > 0 ? (1 - land).toFixed(3) : '';
  }

  // F25: the sheet scales in from the bar's side, 0.94 to 1, with the words'
  // reveal; or out again, 1 to 0.94, the words first.
  function scaleOrigin(sheetRect, bar) {
    if (!bar) return '50% 0';
    const cx = (bar.left + bar.right) / 2 - sheetRect.left;
    const cy = (bar.top + bar.bottom) / 2 - sheetRect.top;
    return `${fmt(clamp(cx, 0, sheetRect.width))}px ${fmt(clamp(cy, 0, sheetRect.height))}px`;
  }

  function scaleFrame(sheet, e) {
    sheet.style.transform = `scale(${(0.94 + 0.06 * e).toFixed(4)})`;
    const gl = glOf(sheet);
    if (gl) gl.style.opacity = e.toFixed(3);
    sheet.classList.toggle('reveal', e >= 0.5);
  }

  async function scaleIn(sheet, rect, bar) {
    const token = {};
    run = token;
    sheet.classList.add('globbing');
    sheet.classList.remove('reveal');
    sheet.style.transformOrigin = scaleOrigin(rect, bar);
    scaleFrame(sheet, 0);
    const done = await frames(token, D_SCALE, (x) => scaleFrame(sheet, easeMorph(x / D_SCALE)));
    if (done) finish(sheet);
  }

  // Grows the sheet out of the bar. The sheet must already be laid out (not
  // hidden); its own glass waits and its words arrive inside the glob.
  async function open(sheet) {
    if (!motionOn()) {
      sheet.classList.add('fading');
      void sheet.offsetWidth;
      sheet.classList.remove('fading');
      return;
    }
    sheet.classList.add('globbing');
    sheet.classList.remove('reveal', 'closing');
    const rect = sheet.getBoundingClientRect();
    const bar = await barRect();
    const g = bar ? geometry(bar, rect) : null;
    if (!g || !g.beside) {
      await scaleIn(sheet, rect, bar);
      return;
    }
    const token = g;
    run = token;
    prepare(g, bar);
    openFrame(g, 0, sheet);
    // Refraction is dropped while the shape changes and is back as the
    // sheet's own glass takes over.
    if (window.AevalGlass) window.AevalGlass.morph(D_OPEN - 120);
    document.body.dataset.glob = 'open';
    const done = await frames(token, D_OPEN, (x) => openFrame(g, x / D_OPEN, sheet));
    if (done) finish(sheet);
  }

  // Pulls the sheet back into the bar, then resolves: the words leave, the
  // sheet's glass hands over to the glob, and the shape reverses. With the
  // bar closed, or the sheet away from it, the sheet scales out instead.
  async function close(sheet) {
    if (!motionOn() || sheet.hidden) {
      if (!sheet.hidden) {
        sheet.classList.add('fading-out');
        await new Promise((res) => setTimeout(res, 150));
      }
      return;
    }
    sheet.classList.add('closing');
    const rect = sheet.getBoundingClientRect();
    const bar = await barRect();
    const g = bar ? geometry(bar, rect) : null;
    if (!g || !g.beside) {
      const token = {};
      run = token;
      sheet.style.transformOrigin = scaleOrigin(rect, bar);
      await frames(token, D_SCALE_CLOSE, (x) => {
        const e = easeMorph(x / D_SCALE_CLOSE);
        sheet.style.transform = `scale(${(1 - 0.06 * e).toFixed(4)})`;
        sheet.style.opacity = (1 - e).toFixed(3);
      });
      if (run === token) run = null;
      return;
    }
    const token = g;
    run = token;
    const gl = glOf(sheet);
    prepare(g, bar);
    drawShape(g, 1, sheet, false);
    host.style.opacity = '0';
    if (window.AevalGlass) window.AevalGlass.morph(WORDS_OUT + D_CLOSE);
    document.body.dataset.glob = 'close';
    const total = WORDS_OUT + D_CLOSE;
    await frames(token, total, (x) => {
      if (x < WORDS_OUT) {
        // The sheet's glass hands over to the glob, in 60 ms.
        const h = clamp((x - HANDOVER_FROM) / (WORDS_OUT - HANDOVER_FROM), 0, 1);
        if (gl) gl.style.opacity = (1 - h).toFixed(3);
        host.style.opacity = h.toFixed(3);
        return;
      }
      sheet.classList.add('globbing');
      if (gl) gl.style.opacity = '0';
      host.style.opacity = '';
      drawShape(g, 1 - easeGlide((x - WORDS_OUT) / D_CLOSE), sheet, false);
    });
    delete document.body.dataset.glob;
    host.hidden = true;
    if (run === token) run = null;
  }

  // One still frame at time t, for the visual self-check's frame sequence.
  // bar is in this window's coordinates.
  function preview(sheet, bar, t) {
    const rect = sheet.getBoundingClientRect();
    const g = geometry(bar, rect);
    run = g;
    sheet.classList.add('globbing');
    sheet.classList.remove('closing');
    if (!g.beside) {
      host.hidden = true;
      sheet.style.transformOrigin = scaleOrigin(rect, bar);
      scaleFrame(sheet, easeMorph(clamp(t, 0, 1)));
      return;
    }
    prepare(g, bar);
    openFrame(g, clamp(t, 0, 1), sheet);
  }

  window.AevalGlob = { open, close, preview, shapeAt, polysAt, geometry, bezier, timing: { D_OPEN, D_CLOSE, T_REVEAL, T_LAND } };
})();
