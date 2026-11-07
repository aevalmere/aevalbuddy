'use strict';

// The glass engine, shared by every page (see glass.css for the layers and
// lib/backdrop.js for where the picture comes from). Loaded first, as a
// classic script; exposes window.AevalGlass.
//
// For each .glass element it inserts one layer (.gl) holding the backdrop
// (.gs), keeps that backdrop lined up with the element's place on the
// screen, measures how bright the desktop is under it and sets --scrim from
// that, and on .refract elements builds a displacement map for the edge
// band. It also keeps three document-level facts current: the tier
// (data-glass), reduced motion read live (data-motion), and one device pixel
// (--px).

(() => {
  const root = document.documentElement;
  const bridge = window.aeval;
  const S = {
    tier: 'live',
    reasons: [],
    display: null,
    sample: null, // { data, w, h }
    url: '',
    shotUrl: '',
    layers: new Set(),
    trackUntil: 0,
    raf: 0,
    sx: window.screenX,
    sy: window.screenY,
    filters: 0,
  };

  // Scrim from the brightness behind (0..1, sRGB luma, 90th percentile).
  // Chosen so --label-3 holds 4.5:1 over a white page (DESIGN.md, Palette).
  const SCRIM = {
    thin: (y) => 0.46 + 0.32 * y,
    thick: (y) => 0.72 + 0.16 * y,
  };

  // ------------------------------------------------------------ device px

  function syncPx() {
    const d = window.devicePixelRatio || 1;
    root.style.setProperty('--px', `${Math.max(1, Math.round(d)) / d}px`);
    root.dataset.dpr = String(Math.round(d * 100));
    matchMedia(`(resolution: ${d}dppx)`).addEventListener('change', () => {
      syncPx();
      for (const el of S.layers) refract(el, true);
    }, { once: true });
  }

  // --------------------------------------------------------------- motion

  const reducedMotion = matchMedia('(prefers-reduced-motion: reduce)');
  function syncMotion() {
    root.dataset.motion = reducedMotion.matches ? 'reduced' : 'full';
  }
  reducedMotion.addEventListener('change', syncMotion);

  // ----------------------------------------------------------------- tier

  const reducedTransparency = matchMedia('(prefers-reduced-transparency: reduce)');
  const forcedColors = matchMedia('(forced-colors: active)');
  function localReason() {
    if (forcedColors.matches) return 'forced-colors';
    if (reducedTransparency.matches) return 'reduced-transparency';
    return '';
  }
  function syncTier() {
    const local = localReason();
    const solid = S.tier === 'solid' || !!local;
    root.dataset.glass = solid ? 'solid' : 'live';
    root.dataset.glassWhy = [...S.reasons, local].filter(Boolean).join(' ') || 'none';
  }
  reducedTransparency.addEventListener('change', syncTier);
  forcedColors.addEventListener('change', syncTier);

  // --------------------------------------------------------------- layers

  const ro = new ResizeObserver((entries) => {
    for (const e of entries) {
      place(e.target);
      refract(e.target);
    }
  });

  function adopt(el) {
    if (el.__gl || !el.isConnected) return;
    const gl = document.createElement('span');
    gl.className = 'gl';
    gl.setAttribute('aria-hidden', 'true');
    const gs = document.createElement('span');
    gs.className = 'gs';
    gl.appendChild(gs);
    el.prepend(gl);
    el.__gl = { gl, gs, mapKey: '', fid: '', y: -1 };
    S.layers.add(el);
    ro.observe(el);
    if (el.classList.contains('interactive')) {
      el.addEventListener('pointermove', (e) => {
        const r = gl.getBoundingClientRect();
        gl.style.setProperty('--mx', `${Math.round(e.clientX - r.left)}px`);
        gl.style.setProperty('--my', `${Math.round(e.clientY - r.top)}px`);
      });
      el.addEventListener('pointerleave', () => {
        gl.style.removeProperty('--mx');
        gl.style.removeProperty('--my');
      });
    }
    place(el);
    refract(el);
  }

  function scan(node) {
    if (!(node instanceof Element)) return;
    if (node.classList.contains('glass')) adopt(node);
    for (const g of node.querySelectorAll('.glass')) adopt(g);
  }

  new MutationObserver((list) => {
    for (const m of list) {
      for (const n of m.addedNodes) scan(n);
      for (const n of m.removedNodes) {
        if (!(n instanceof Element)) continue;
        for (const el of [n, ...n.querySelectorAll('.glass')]) {
          if (el.__gl && !el.isConnected) {
            S.layers.delete(el);
            ro.unobserve(el);
          }
        }
      }
    }
  }).observe(document.documentElement, { childList: true, subtree: true });

  // ------------------------------------------------------------ placement

  function place(el) {
    const g = el.__gl;
    if (!g || !S.display) return;
    const r = g.gl.getBoundingClientRect();
    if (!r.width || !r.height) return;
    const sx = window.screenX + r.left - S.display.x - 24;
    const sy = window.screenY + r.top - S.display.y - 24;
    g.gs.style.setProperty('--bx', `${-Math.round(sx)}px`);
    g.gs.style.setProperty('--by', `${-Math.round(sy)}px`);
    measure(el, r);
  }

  function placeAll() {
    for (const el of S.layers) {
      if (!el.isConnected) S.layers.delete(el);
      else place(el);
    }
    if (S.display && S.shotUrl) {
      root.style.setProperty('--sx', `${Math.round(S.display.x - window.screenX)}px`);
      root.style.setProperty('--sy', `${Math.round(S.display.y - window.screenY)}px`);
    }
  }

  // Follow elements that are moving (a morph, a sheet rising) for a while.
  function track(ms) {
    S.trackUntil = Math.max(S.trackUntil, performance.now() + (ms || 500));
    if (S.raf) return;
    const step = () => {
      placeAll();
      if (performance.now() < S.trackUntil) S.raf = requestAnimationFrame(step);
      else S.raf = 0;
    };
    S.raf = requestAnimationFrame(step);
  }

  document.addEventListener('transitionrun', () => track(450), true);
  document.addEventListener('animationstart', () => track(450), true);

  // The window itself moved (a drag in the layout editor, a dock).
  setInterval(() => {
    if (window.screenX !== S.sx || window.screenY !== S.sy) {
      S.sx = window.screenX;
      S.sy = window.screenY;
      placeAll();
    }
  }, 250);
  window.addEventListener('resize', () => placeAll());

  // ---------------------------------------------------------------- scrim

  function measure(el, r) {
    const s = S.sample;
    const d = S.display;
    if (!s || !d) return;
    const kx = s.w / d.width;
    const ky = s.h / d.height;
    const x0 = Math.max(0, Math.floor((window.screenX + r.left - d.x) * kx));
    const y0 = Math.max(0, Math.floor((window.screenY + r.top - d.y) * ky));
    const x1 = Math.min(s.w, Math.ceil((window.screenX + r.right - d.x) * kx));
    const y1 = Math.min(s.h, Math.ceil((window.screenY + r.bottom - d.y) * ky));
    if (x1 <= x0 || y1 <= y0) return;
    const ys = [];
    for (let y = y0; y < y1; y += 1) {
      for (let x = x0; x < x1; x += 1) {
        const i = (y * s.w + x) * 4;
        ys.push((0.2126 * s.data[i] + 0.7152 * s.data[i + 1] + 0.0722 * s.data[i + 2]) / 255);
      }
    }
    ys.sort((a, b) => a - b);
    const y = ys[Math.min(ys.length - 1, Math.floor(ys.length * 0.9))];
    const g = el.__gl;
    if (Math.abs(y - g.y) < 0.02) return;
    g.y = y;
    const kind = el.classList.contains('thick') ? 'thick' : 'thin';
    el.style.setProperty('--scrim', SCRIM[kind](y).toFixed(3));
    el.dataset.backdropY = y.toFixed(2);
  }

  // ----------------------------------------------------------- refraction

  let svg = null;
  function svgHost() {
    if (svg) return svg;
    svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('width', '0');
    svg.setAttribute('height', '0');
    svg.setAttribute('aria-hidden', 'true');
    svg.style.position = 'absolute';
    svg.style.pointerEvents = 'none';
    document.body.appendChild(svg);
    return svg;
  }

  // A map for the snap layer (the element plus 24 px of bleed): neutral grey
  // inside, and in a 14 px band along the glass edge a push that reaches
  // outward, so content just past the edge bends into the rim.
  function buildMap(w, h, radius) {
    const W = w + 48;
    const H = h + 48;
    const c = document.createElement('canvas');
    c.width = W;
    c.height = H;
    const ctx = c.getContext('2d');
    const img = ctx.createImageData(W, H);
    const band = 14;
    const rr = Math.min(radius, w / 2, h / 2);
    const cx = w / 2;
    const cy = h / 2;
    for (let py = 0; py < H; py += 1) {
      for (let px = 0; px < W; px += 1) {
        // Rounded-rect signed distance, negative inside.
        const x = px - 24 - cx;
        const y = py - 24 - cy;
        const qx = Math.abs(x) - (cx - rr);
        const qy = Math.abs(y) - (cy - rr);
        const ox = Math.max(qx, 0);
        const oy = Math.max(qy, 0);
        const dist = Math.hypot(ox, oy) + Math.min(Math.max(qx, qy), 0) - rr;
        const inside = -dist;
        let nx = 0;
        let ny = 0;
        let m = 0;
        if (inside < band) {
          if (qx > 0 && qy > 0) {
            const l = Math.hypot(ox, oy) || 1;
            nx = (ox / l) * Math.sign(x);
            ny = (oy / l) * Math.sign(y);
          } else if (qx > qy) {
            nx = Math.sign(x);
          } else {
            ny = Math.sign(y);
          }
          const t = Math.max(0, Math.min(1, 1 - inside / band));
          m = t * t;
        }
        const i = (py * W + px) * 4;
        img.data[i] = Math.round(128 + 127 * nx * m);
        img.data[i + 1] = Math.round(128 + 127 * ny * m);
        img.data[i + 2] = 128;
        img.data[i + 3] = 255;
      }
    }
    ctx.putImageData(img, 0, 0);
    return { url: c.toDataURL('image/png'), W, H };
  }

  const refractTimers = new WeakMap();
  function refract(el, force) {
    const g = el.__gl;
    if (!g || !el.classList.contains('refract')) return;
    clearTimeout(refractTimers.get(el));
    refractTimers.set(el, setTimeout(() => {
      const w = Math.round(el.offsetWidth);
      const h = Math.round(el.offsetHeight);
      if (!w || !h) return;
      const radius = parseFloat(getComputedStyle(g.gl).borderTopLeftRadius) || 0;
      const key = `${w}x${h}r${radius}`;
      if (key === g.mapKey && !force) return;
      g.mapKey = key;
      if (!g.fid) g.fid = `rf${(S.filters += 1)}`;
      const host = svgHost();
      let f = host.querySelector(`#${g.fid}`);
      if (!f) {
        f = document.createElementNS('http://www.w3.org/2000/svg', 'filter');
        f.id = g.fid;
        f.setAttribute('x', '0');
        f.setAttribute('y', '0');
        f.setAttribute('width', '1');
        f.setAttribute('height', '1');
        f.setAttribute('color-interpolation-filters', 'sRGB');
        f.innerHTML = '<feImage result="map" preserveAspectRatio="none" x="0" y="0"></feImage>' +
          '<feDisplacementMap in="SourceGraphic" in2="map" xChannelSelector="R" yChannelSelector="G"></feDisplacementMap>';
        host.appendChild(f);
      }
      const map = buildMap(w, h, radius);
      const fe = f.querySelector('feImage');
      fe.setAttribute('href', map.url);
      fe.setAttribute('width', String(map.W));
      fe.setAttribute('height', String(map.H));
      f.querySelector('feDisplacementMap').setAttribute('scale', el.classList.contains('thick') ? '28' : '22');
      const gf = getComputedStyle(el).getPropertyValue('--gf').trim();
      g.gs.style.setProperty('--gfr', `url(#${g.fid}) ${gf}`);
    }, 120));
  }

  // While a shape morphs the displacement is dropped (glass.css); back 120 ms
  // after it settles.
  let morphTimer = null;
  function morph(ms) {
    root.dataset.morph = '1';
    clearTimeout(morphTimer);
    morphTimer = setTimeout(() => delete root.dataset.morph, (ms || 360) + 120);
    track((ms || 360) + 60);
  }

  // --------------------------------------------------------------- frames

  async function onFrame(f) {
    S.tier = f.tier || 'live';
    S.reasons = f.reasons || [];
    syncTier();
    if (!f.img || !f.display) return;
    S.display = f.display;
    const blob = new Blob([f.img], { type: 'image/jpeg' });
    const url = URL.createObjectURL(blob);
    try {
      const bmp = await createImageBitmap(blob);
      const c = new OffscreenCanvas(bmp.width, bmp.height);
      const ctx = c.getContext('2d', { willReadFrequently: true });
      ctx.drawImage(bmp, 0, 0);
      S.sample = { data: ctx.getImageData(0, 0, bmp.width, bmp.height).data, w: bmp.width, h: bmp.height };
      bmp.close();
      // Decoded before it is swapped in, so a refresh never shows a frame of
      // nothing behind the glass.
      const im = new Image();
      im.src = url;
      await im.decode();
    } catch (err) {
      URL.revokeObjectURL(url);
      return;
    }
    root.style.setProperty('--snap', `url("${url}")`);
    root.style.setProperty('--bw', `${f.display.width}px`);
    root.style.setProperty('--bh', `${f.display.height}px`);
    const old = S.url;
    S.url = url;
    if (old) setTimeout(() => URL.revokeObjectURL(old), 1000);
    root.dataset.snap = '1';
    root.dataset.fixture = f.fixture || '';
    if (f.full) {
      const full = URL.createObjectURL(new Blob([f.full], { type: 'image/jpeg' }));
      const oldShot = S.shotUrl;
      S.shotUrl = full;
      root.style.setProperty('--shot-bg', `url("${full}")`);
      root.dataset.shotBg = '1';
      if (oldShot) setTimeout(() => URL.revokeObjectURL(oldShot), 1000);
    } else if (S.shotUrl) {
      delete root.dataset.shotBg;
      S.shotUrl = '';
    }
    for (const el of S.layers) el.__gl.y = -1;
    placeAll();
  }

  // ------------------------------------------------------------ hit-tests

  // Is this point on something the person can see? The transparent margin
  // around the glass (lib/stage.js) lets the click through.
  function solidAt(x, y, extra) {
    const el = document.elementFromPoint(x, y);
    if (!el || el === document.documentElement || el === document.body) return false;
    return !!el.closest(`.glass, [data-hit]${extra ? `, ${extra}` : ''}`);
  }

  // Pages that ask for it (html[data-passthrough]) get the click-through
  // handled here; the bar does its own, since its pill mode predates this.
  let ignoring = false;
  function passthrough() {
    if (!bridge || root.dataset.passthrough === undefined) return;
    document.addEventListener('mousemove', (e) => {
      if (root.dataset.passthrough !== 'on') return;
      const ignore = !solidAt(e.clientX, e.clientY);
      if (ignore === ignoring) return;
      ignoring = ignore;
      bridge.invoke('glass:passthrough', { ignore });
    });
  }

  function resetPassthrough() {
    ignoring = false;
  }

  // ----------------------------------------------------------------- boot

  syncPx();
  syncMotion();
  syncTier();
  if (bridge) {
    bridge.on('glass:frame', (f) => {
      onFrame(f);
    });
    bridge.on('glass:tier', (t) => {
      S.tier = t.tier;
      S.reasons = t.reasons || [];
      syncTier();
    });
  }
  const boot = () => {
    scan(document.body);
    passthrough();
    if (bridge) bridge.invoke('glass:hello').catch(() => {});
  };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', boot);
  else boot();

  window.AevalGlass = {
    adopt,
    track,
    morph,
    placeAll,
    solidAt,
    resetPassthrough,
    state: () => ({ tier: root.dataset.glass, why: root.dataset.glassWhy, layers: S.layers.size, snap: !!S.url, display: S.display }),
  };
})();
