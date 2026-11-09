'use strict';

// The glass engine, shared by every page (see glass.css for the layers and
// lib/backdrop.js for where the picture comes from). Loaded first, as a
// classic script; exposes window.AevalGlass.
//
// For each .glass element it inserts one layer (.gl) holding the backdrop
// (.gs), keeps that backdrop lined up with the element's place on the
// screen, measures how bright the desktop is under it and sets --scrim from
// that, and on .refract elements builds a displacement map for the edge
// band. It also keeps four document-level facts current: the tier
// (data-glass), reduced motion read live (data-motion), one device pixel
// (--px), and how liquid the glass is (--glass-k, data-glass-level).
//
// How liquid: Settings' "Liquid glass" slider, 0 (None: solid frosted
// panels) to 1 (Very liquid), default 0.75 (Liquid). It moves blur,
// refraction, the specular light, saturation and the scrim together, live,
// in every window: Settings writes it to localStorage, which every page of
// the app shares, so the others hear it through the storage event; Rust also
// repeats it in glass:tier and in the glass:hello reply (ARCHITECTURE.md
// section 10). The scale widened in v2.2 (glassScale 2 in the store); Rust
// migrates a stored v2.1 value, and the page key changed with it, so a
// stale v2.1 value in localStorage is never read as a v2.2 one.

(() => {
  const root = document.documentElement;
  const bridge = window.aeval;
  const S = {
    k: 0.75,
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
    tone: 'dark',
    lightFrames: 0,
  };

  // Scrim from the brightness behind (0..1, sRGB luma, 90th percentile, or
  // the 98th above 0.5, where thinner blur lets bright detail through).
  // Chosen so --label-3 holds 4.5:1 over a white page (DESIGN.md, Palette);
  // thin reads as the same glass as thick, a touch lighter. More liquid glass
  // lets more of a dark desktop through; over a bright one (y near 1) the
  // scrim does not move, so the contrast floor holds at every setting.
  const SCRIM = {
    thin: (y, k) => 0.58 + 0.24 * y + (0.5 - k) * 0.32 * (1 - y),
    thick: (y, k) => 0.72 + 0.16 * y + (0.5 - k) * 0.30 * (1 - y),
  };
  // Over light content the tint is light too, and thins out as the page
  // behind it gets brighter (D-20).
  const LIGHT_SCRIM = {
    thin: (y) => 0.42 + 0.3 * (1 - y),
    thick: (y) => 0.62 + 0.2 * (1 - y),
  };

  // ---------------------------------------------------------- liquidness

  const K_KEY = 'aeval.glassLevel2';
  const K_DEFAULT = 0.75;
  const clampK = (v) => Math.min(1, Math.max(0, Math.round(Number(v) * 100) / 100));
  function storedK() {
    try {
      const v = localStorage.getItem(K_KEY);
      if (v !== null && v !== '' && Number.isFinite(Number(v))) return clampK(v);
    } catch (err) {
      // Storage can be off in a locked-down profile; the default stands.
    }
    return K_DEFAULT;
  }
  S.k = storedK();

  function levelOf(k) {
    if (k <= 0) return 'none';
    if (k < 0.34) return 'low';
    if (k < 0.67) return 'mid';
    return 'high';
  }

  function applyK(k, opts) {
    const next = clampK(k);
    const changed = next !== S.k || root.style.getPropertyValue('--glass-k') === '';
    S.k = next;
    root.style.setProperty('--glass-k', String(next));
    root.dataset.glassLevel = levelOf(next);
    if (opts && opts.persist) {
      try {
        localStorage.setItem(K_KEY, String(next));
      } catch (err) {
        // Not shared, then; Rust still repeats it through glass:tier.
      }
    }
    if (!changed) return;
    syncTier();
    for (const el of S.layers) {
      if (!el.__gl) continue;
      el.__gl.y = -1;
      refract(el, true);
    }
    placeAll();
  }

  window.addEventListener('storage', (e) => {
    if (e.key === K_KEY && e.newValue !== null) applyK(e.newValue);
  });

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

  const forcedColors = matchMedia('(forced-colors: active)');
  // Glass no longer follows Windows' transparency setting (REQUESTS.md 5):
  // Chromium reports that setting as prefers-reduced-transparency, and the
  // slider is the control for it now. Forced colours (a contrast theme) and
  // the reasons only Rust can see (no capture exclusion, capture failing)
  // still make the surface solid.
  const IGNORED = new Set(['transparency-off', 'reduced-transparency']);
  function localReason() {
    if (forcedColors.matches) return 'forced-colors';
    return '';
  }
  function syncTier() {
    const local = localReason();
    const remote = S.reasons.filter((r) => !IGNORED.has(r));
    const remoteSolid = S.tier === 'solid' && (remote.length > 0 || S.reasons.length === 0);
    const chosen = S.k <= 0 ? 'none-chosen' : '';
    const solid = remoteSolid || !!local || !!chosen;
    root.dataset.glass = solid ? 'solid' : 'live';
    root.dataset.glassWhy = [...remote, local, chosen].filter(Boolean).join(' ') || 'none';
    // The solid surface is dark whatever is behind it.
    if (solid && S.tone === 'light') setTone('dark');
  }
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
    const gsc = document.createElement('span');
    gsc.className = 'gsc';
    gl.appendChild(gsc);
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

  // A title bar marked .interactive lights the sheet under it, more softly
  // than a control does (F32): the sheet's own glass tracks no pointer.
  let litSheet = null;
  function darkenSheet() {
    if (!litSheet) return;
    for (const p of ['--mx', '--my', '--lit']) litSheet.style.removeProperty(p);
    litSheet = null;
  }
  document.addEventListener('pointermove', (e) => {
    const on = e.target instanceof Element ? e.target.closest('.sheet-bar.interactive') : null;
    const host = on ? on.closest('.glass') : null;
    const gl = host && host.__gl ? host.__gl.gl : null;
    if (gl !== litSheet) darkenSheet();
    if (!gl) return;
    litSheet = gl;
    const r = gl.getBoundingClientRect();
    gl.style.setProperty('--mx', `${Math.round(e.clientX - r.left)}px`);
    gl.style.setProperty('--my', `${Math.round(e.clientY - r.top)}px`);
    gl.style.setProperty('--lit', '0.06');
  }, { passive: true });

  document.addEventListener('transitionrun', () => track(450), true);
  document.addEventListener('animationstart', () => track(450), true);

  // The window itself moved (a title-bar drag, the layout editor, a dock):
  // the displacement is dropped while it moves, as for a morph.
  setInterval(() => {
    if (window.screenX !== S.sx || window.screenY !== S.sy) {
      S.sx = window.screenX;
      S.sy = window.screenY;
      morph(200);
      placeAll();
    }
  }, 250);
  window.addEventListener('resize', () => placeAll());

  // ---------------------------------------------------------------- scrim

  // The brightness of the desktop under a screen rectangle (DIP), from the
  // sixth-size frame: the median, the 90th and the 98th percentile luma.
  function lumaUnder(left, top, right, bottom) {
    const s = S.sample;
    const d = S.display;
    if (!s || !d) return null;
    const kx = s.w / d.width;
    const ky = s.h / d.height;
    const x0 = Math.max(0, Math.floor((left - d.x) * kx));
    const y0 = Math.max(0, Math.floor((top - d.y) * ky));
    const x1 = Math.min(s.w, Math.ceil((right - d.x) * kx));
    const y1 = Math.min(s.h, Math.ceil((bottom - d.y) * ky));
    if (x1 <= x0 || y1 <= y0) return null;
    const ys = [];
    for (let y = y0; y < y1; y += 1) {
      for (let x = x0; x < x1; x += 1) {
        const i = (y * s.w + x) * 4;
        ys.push((0.2126 * s.data[i] + 0.7152 * s.data[i + 1] + 0.0722 * s.data[i + 2]) / 255);
      }
    }
    ys.sort((a, b) => a - b);
    const at = (p) => ys[Math.min(ys.length - 1, Math.floor(ys.length * p))];
    return { p50: at(0.5), p90: at(0.9), p98: at(0.98) };
  }

  // The luma the scrim is computed from.
  function scrimLuma(l) {
    return S.k > 0.5 ? l.p98 : l.p90;
  }

  function scrimOf(kind, y) {
    const k = kind === 'thick' ? 'thick' : 'thin';
    return S.tone === 'light' ? LIGHT_SCRIM[k](y) : SCRIM[k](y, S.k);
  }

  function measure(el, r) {
    const l = lumaUnder(window.screenX + r.left, window.screenY + r.top, window.screenX + r.right, window.screenY + r.bottom);
    if (!l) return;
    const y = scrimLuma(l);
    const g = el.__gl;
    if (Math.abs(y - g.y) < 0.02) return;
    g.y = y;
    const kind = el.classList.contains('thick') ? 'thick' : 'thin';
    el.style.setProperty('--scrim', scrimOf(kind, y).toFixed(3));
    el.dataset.backdropY = y.toFixed(2);
  }

  // What scrim a glass layer of this kind would get over a screen rectangle
  // (DIP, {x, y, width, height}): the glob's thin bud takes the bar's scrim
  // so the two read as one material. Before the first frame, the stylesheet's
  // starting value.
  function scrimFor(rect, level) {
    const kind = level === 'thick' ? 'thick' : 'thin';
    const r = rect || {};
    const x = Number(r.x !== undefined ? r.x : r.left);
    const y = Number(r.y !== undefined ? r.y : r.top);
    const w = Number(r.width);
    const h = Number(r.height);
    const l = [x, y, w, h].every(Number.isFinite) ? lumaUnder(x, y, x + w, y + h) : null;
    if (!l) return kind === 'thick' ? 0.86 - 0.12 * S.k : 0.66 - 0.12 * S.k;
    return Math.round(scrimOf(kind, scrimLuma(l)) * 1000) / 1000;
  }

  // ---------------------------------------------------------------- tone

  // Each window's glass turns light over light content (F10): the median
  // brightness under its largest glass layer is above 0.72 for two frames
  // running, and it goes back below 0.55. The gap keeps a page that sits
  // between the two from flickering.
  const TONE_LIGHT = 0.72;
  const TONE_DARK = 0.55;
  const TONE_FRAMES = 2;
  const TONE_FADE_MS = 360;

  function largestRect() {
    let best = null;
    for (const el of S.layers) {
      if (!el.__gl || !el.isConnected) continue;
      const r = el.__gl.gl.getBoundingClientRect();
      if (r.width * r.height > (best ? best.width * best.height : 0)) best = r;
    }
    return best;
  }

  let toneFade = null;
  function setTone(tone) {
    S.tone = tone;
    root.dataset.toneFade = '';
    clearTimeout(toneFade);
    toneFade = setTimeout(() => delete root.dataset.toneFade, TONE_FADE_MS);
    if (tone === 'light') root.dataset.tone = 'light';
    else delete root.dataset.tone;
    for (const el of S.layers) if (el.__gl) el.__gl.y = -1;
    placeAll();
  }

  function updateTone() {
    if (root.dataset.glass === 'solid') {
      S.lightFrames = 0;
      return;
    }
    const r = largestRect();
    const l = r && lumaUnder(window.screenX + r.left, window.screenY + r.top, window.screenX + r.right, window.screenY + r.bottom);
    if (!l) return;
    S.lightFrames = l.p50 > TONE_LIGHT ? S.lightFrames + 1 : 0;
    if (S.tone === 'dark' && S.lightFrames >= TONE_FRAMES) setTone('light');
    else if (S.tone === 'light' && l.p50 < TONE_DARK) setTone('dark');
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
  function buildMap(w, h, radius, band) {
    const W = w + 48;
    const H = h + 48;
    const c = document.createElement('canvas');
    c.width = W;
    c.height = H;
    const ctx = c.getContext('2d');
    const img = ctx.createImageData(W, H);
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
      // The edge band widens and the bend deepens on a k^1.5 curve, so the
      // low end stays nearly flat and the top end turns molten:
      //   k      band  thin  thick
      //   0.25    8     9    11   Frosted
      //   0.5    17    25    32   Soft (about v2.1's Liquid)
      //   0.75   27    46    58   Liquid, the default
      //   1      40    70    90   Very liquid
      // None has no refraction at all. The band is also where the scrim
      // clears (glass.css, .gsc).
      const e = S.k ** 1.5;
      const band = Math.round(4 + 36 * e);
      const base = el.classList.contains('thick') ? 28 : 22;
      const scale = Math.round(base * 3.2 * e);
      const key = `${w}x${h}r${radius}b${band}s${scale}`;
      if (key === g.mapKey && !force) return;
      g.mapKey = key;
      if (!scale) {
        g.gs.style.removeProperty('--gfr');
        g.gl.style.removeProperty('--band');
        return;
      }
      g.gl.style.setProperty('--band', `${band}px`);
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
      const map = buildMap(w, h, radius, band);
      const fe = f.querySelector('feImage');
      fe.setAttribute('href', map.url);
      fe.setAttribute('width', String(map.W));
      fe.setAttribute('height', String(map.H));
      f.querySelector('feDisplacementMap').setAttribute('scale', String(scale));
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

  // A frame arrives as a URL under aeval-frame:// on Tauri (ARCHITECTURE.md
  // D4; Rust serves it with Access-Control-Allow-Origin so its pixels can be
  // read for the scrim) and as JPEG bytes under Electron. Either way it
  // becomes a blob: URL of this page, so the CSS and the sampler see the same
  // picture and a later frame can never swap it underneath.
  async function asBlob(img) {
    if (typeof img === 'string' && img.startsWith('data:')) {
      // Decoded here rather than fetched: the pages' CSP has no data: in
      // connect-src.
      const comma = img.indexOf(',');
      const type = (/^data:([^;,]+)/.exec(img) || [])[1] || 'image/jpeg';
      const bin = atob(img.slice(comma + 1));
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i);
      return new Blob([bytes], { type });
    }
    if (typeof img === 'string') {
      const r = await fetch(img);
      if (!r.ok) throw new Error(`frame ${r.status}`);
      return r.blob();
    }
    return new Blob([img], { type: 'image/jpeg' });
  }

  async function onFrame(f) {
    S.tier = f.tier || 'live';
    S.reasons = f.reasons || [];
    if (typeof f.intensity === 'number') applyK(f.intensity);
    syncTier();
    if (!f.img || !f.display) return;
    let blob;
    try {
      blob = await asBlob(f.img);
    } catch (err) {
      return;
    }
    S.display = f.display;
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
      let full = '';
      try {
        full = URL.createObjectURL(await asBlob(f.full));
      } catch (err) {
        full = '';
      }
      const oldShot = S.shotUrl;
      if (!full) return;
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
    updateTone();
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

  // Listening on a channel Rust does not send yet throws in the bridge
  // (its allowlist); a page must still load.
  function listen(channel, fn) {
    if (!bridge) return;
    try {
      bridge.on(channel, fn);
    } catch (err) {
      // Not in this build's allowlist; the feature waits for it.
    }
  }

  // ------------------------------------------------------------ text size

  // --text-scale is the Windows text size times the person's own factor
  // (1.0..2.0); Rust sends the product as ui:text-scale. Ctrl+= and Ctrl+-
  // step the person's factor by 0.1 and Ctrl+0 puts it back to 1, in any
  // page: they see the keys before the bridge blocks browser zoom.
  const TS_KEY = 'aeval.textScale';
  function applyTextScale(scale) {
    const v = Number(scale);
    if (!Number.isFinite(v) || v <= 0) return;
    const s = Math.round(Math.min(3, Math.max(0.5, v)) * 100) / 100;
    if (s === 1) root.style.removeProperty('--text-scale');
    else root.style.setProperty('--text-scale', String(s));
    root.dataset.textScale = String(Math.round(s * 100));
    // Pages that size things from text metrics (the bar's rows) refit.
    window.dispatchEvent(new CustomEvent('aeval:text-scale', { detail: { scale: s } }));
  }

  function localFactor() {
    try {
      const v = Number(localStorage.getItem(TS_KEY));
      if (Number.isFinite(v) && v >= 1 && v <= 2) return v;
    } catch (err) {
      // Storage off: 100%.
    }
    return 1;
  }

  async function storedFactor() {
    if (bridge) {
      try {
        const s = await bridge.invoke('settings:get');
        if (s && typeof s.textScale === 'number') return s.textScale;
      } catch (err) {
        // This window may not read settings; the shared copy stands.
      }
    }
    return localFactor();
  }

  let zoomChain = Promise.resolve();
  function stepTextScale(dir) {
    zoomChain = zoomChain.then(async () => {
      const cur = await storedFactor();
      const next = dir === 0 ? 1 : Math.round(Math.min(2, Math.max(1, cur + dir * 0.1)) * 10) / 10;
      if (next === cur && dir !== 0) return;
      try {
        localStorage.setItem(TS_KEY, String(next));
      } catch (err) {
        // Rust keeps the setting either way.
      }
      if (!bridge) return;
      try {
        await bridge.invoke('settings:set-text-scale', { scale: next });
      } catch (err) {
        // A build without the channel: this page at least follows the keys.
        applyTextScale(next);
      }
    });
    return zoomChain;
  }

  // On document, so a field that takes the chord itself (Settings' shortcut
  // recorder) can claim it first with preventDefault.
  document.addEventListener('keydown', (e) => {
    if (!e.ctrlKey || e.altKey || e.metaKey || e.defaultPrevented) return;
    const k = e.key;
    let dir = null;
    if (k === '=' || k === '+') dir = 1;
    else if (k === '-' || k === '_') dir = -1;
    else if (k === '0') dir = 0;
    if (dir === null) return;
    e.preventDefault();
    stepTextScale(dir);
  });

  // ---------------------------------------------------------------- icons

  // A glyph from renderer/icons.svg: a span that glass.css masks with the
  // sprite's <view> of that name (.ico .i-<name>), filled with currentColor.
  // Extra classes size it (for example 'g-check').
  function icon(name, cls) {
    const el = document.createElement('span');
    el.className = `ico i-${name}${cls ? ` ${cls}` : ''}`;
    el.setAttribute('aria-hidden', 'true');
    return el;
  }

  // ---------------------------------------------------------- announcements

  // One way to speak (AevalA11y.say): into this page's live region while it
  // has focus, else through Rust (a11y:announce) to the window that has it,
  // since a screen reader does not voice a background window's live region.
  // Rust answers with a11y:say, spoken here. Polite for news, assertive for
  // errors and a task's question.
  function liveRegion(priority) {
    const assertive = priority === 'assertive';
    const id = assertive ? 'announce-alert' : 'announce';
    let el = document.getElementById(id);
    if (el) return el;
    if (!document.body) return null;
    el = document.createElement('div');
    el.id = id;
    el.setAttribute('role', assertive ? 'alert' : 'status');
    el.setAttribute('aria-live', assertive ? 'assertive' : 'polite');
    el.setAttribute('aria-atomic', 'true');
    // CSSOM, not a style attribute: the pages' CSP has no 'unsafe-inline'.
    Object.assign(el.style, {
      position: 'absolute',
      width: '1px',
      height: '1px',
      overflow: 'hidden',
      clipPath: 'inset(50%)',
      whiteSpace: 'nowrap',
    });
    document.body.appendChild(el);
    return el;
  }

  const speakTimers = {};
  function speak(text, priority) {
    const p = priority === 'assertive' ? 'assertive' : 'polite';
    const el = liveRegion(p);
    if (!el) return;
    // Emptied first, so the same words said twice are heard twice.
    el.textContent = '';
    clearTimeout(speakTimers[p]);
    speakTimers[p] = setTimeout(() => {
      el.textContent = String(text || '');
    }, 30);
  }

  function say(text, priority) {
    const t = String(text || '').trim();
    if (!t) return;
    const p = priority === 'assertive' ? 'assertive' : 'polite';
    if (!bridge || document.hasFocus()) {
      speak(t, p);
      return;
    }
    bridge.invoke('a11y:announce', { text: t, priority: p }).catch(() => speak(t, p));
  }

  window.AevalA11y = Object.freeze({ say, speak });

  // ---------------------------------------------------------------- popups

  // Every menu that hangs off a control or a point is placed here, so none
  // is ever cut by its window's edge (the window is the only canvas a page
  // has). It opens below its anchor when it fits, above when it fits there
  // instead, and when neither side fits it takes the roomier one and
  // scrolls inside. Across, it starts at the anchor's left (or ends at its
  // right with align: 'end') and is clamped inside the window. Use
  // position: fixed, so no scrolling ancestor clips it; the result is
  // measured and corrected, so a transformed ancestor cannot shift it either.
  // anchor: an element, a DOMRect, or a point {x, y}.
  const POP_EDGE = 8;
  function place(el, anchor, opts) {
    const o = opts || {};
    const gap = o.gap === undefined ? 4 : o.gap;
    const edge = o.edge === undefined ? POP_EDGE : o.edge;
    const a = anchor && typeof anchor.getBoundingClientRect === 'function' ? anchor.getBoundingClientRect() : anchor || {};
    const ax = Number(a.left !== undefined ? a.left : a.x) || 0;
    const ay = Number(a.top !== undefined ? a.top : a.y) || 0;
    const r = { left: ax, top: ay, right: a.right !== undefined ? a.right : ax, bottom: a.bottom !== undefined ? a.bottom : ay };
    const vw = root.clientWidth || window.innerWidth;
    const vh = root.clientHeight || window.innerHeight;
    if (o.width === 'anchor') el.style.width = `${Math.round(r.right - r.left)}px`;
    el.style.maxHeight = '';
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const below = Math.floor(vh - edge - (r.bottom + gap));
    const above = Math.floor(r.top - gap - edge);
    let side = 'below';
    if (h > below && (h <= above || above > below)) side = 'above';
    const room = Math.max(0, side === 'below' ? below : above);
    const tall = Math.min(h, room);
    if (h > room) {
      el.style.maxHeight = `${room}px`;
      el.style.overflowY = 'auto';
    }
    const left = Math.round(Math.max(edge, Math.min(o.align === 'end' ? r.right - w : r.left, vw - w - edge)));
    const top = Math.round(side === 'below' ? r.bottom + gap : r.top - gap - tall);
    el.style.left = `${left}px`;
    el.style.top = `${top}px`;
    el.style.right = 'auto';
    el.style.bottom = 'auto';
    const got = el.getBoundingClientRect();
    if (Math.abs(got.left - left) >= 0.5) el.style.left = `${Math.round(left + (left - got.left))}px`;
    if (Math.abs(got.top - top) >= 0.5) el.style.top = `${Math.round(top + (top - got.top))}px`;
    el.dataset.side = side;
    el.style.setProperty('--drop-from', side === 'below' ? '-4px' : '4px');
    return side;
  }

  // ----------------------------------------------------------------- boot

  syncPx();
  syncMotion();
  applyK(S.k);
  syncTier();
  if (bridge) {
    bridge.on('glass:frame', (f) => {
      onFrame(f);
    });
    bridge.on('glass:tier', (t) => {
      S.tier = t.tier;
      S.reasons = t.reasons || [];
      if (typeof t.intensity === 'number') applyK(t.intensity);
      syncTier();
    });
    listen('ui:text-scale', (t) => applyTextScale(t && t.scale));
    listen('a11y:say', (m) => m && speak(m.text, m.priority));
  }
  const boot = () => {
    scan(document.body);
    passthrough();
    if (bridge) {
      bridge
        .invoke('glass:hello')
        .then((r) => {
          if (r && typeof r.intensity === 'number') applyK(r.intensity);
          if (r && typeof r.textScale === 'number') applyTextScale(r.textScale);
        })
        .catch(() => {});
    }
  };
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', boot);
  else boot();

  window.AevalGlass = {
    place,
    adopt,
    track,
    morph,
    placeAll,
    solidAt,
    resetPassthrough,
    scrimFor,
    icon,
    textScale: () => Number(getComputedStyle(root).getPropertyValue('--text-scale')) || 1,
    stepTextScale,
    // Settings' slider: applied here at once, shared with every other page.
    setIntensity: (k) => applyK(k, { persist: true }),
    intensity: () => S.k,
    state: () => ({ k: S.k, tone: S.tone, tier: root.dataset.glass, why: root.dataset.glassWhy, layers: S.layers.size, snap: !!S.url, display: S.display }),
  };
})();
