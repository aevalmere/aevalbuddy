'use strict';

// The backdrop behind the glass.
//
// A transparent Electron window has nothing behind it inside Chromium, so
// backdrop-filter has nothing to blur. What makes real glass possible here is
// that every one of our windows is excluded from capture: a small frame of the
// display from desktopCapturer is the desktop exactly as it is behind us,
// without us in it. Each glass window gets that frame and draws it, blurred,
// inside its own shapes (renderer/glass.js). No window style changes for any
// of this, which is the constraint acrylic broke (DESIGN.md, Material).
//
// The frame is a sixth of the display, JPEG, held only in memory: it is sent
// to our own content-protected pages and never written anywhere.
//
// Tiers. 'live' draws the frame; 'solid' draws the same shapes on an opaque
// fill. Solid is used when the user chose it, when Windows transparency
// effects are off, in high contrast, when the windows cannot be hidden from
// capture (the frame would then contain the overlay itself, and a screen
// share would see the overlay anyway), and when capture keeps failing.
// The renderer adds its own reasons: prefers-reduced-transparency and
// forced-colors.

const { desktopCapturer, screen, nativeTheme, BrowserWindow } = require('electron');
const { execFile } = require('child_process');
const path = require('path');
const { monitorEventLoopDelay, performance } = require('perf_hooks');

const SCALE = 6;
const FAST_MS = 500; // 2 Hz while something you read or type into is showing
const SLOW_MS = 5000; // only the orb is showing
const RETRY_MS = 30000;

const wins = new Set();
let opts = { pace: () => 'fast', canHide: true, userSurface: () => 'auto' };
let timer = null;
let busy = false;
let paused = 0;
let fails = 0;
let transparencyOff = false;
let lastTier = null;
let fixture = null; // { name, small: Buffer, full: Buffer, display }
const stats = { frames: 0, wall: [], encode: [], block: [], lastError: '' };

function reasons() {
  const r = [];
  if (opts.userSurface() === 'solid') r.push('setting');
  if (!opts.canHide) r.push('capture-visible');
  if (transparencyOff) r.push('transparency-off');
  try {
    if (nativeTheme.shouldUseHighContrastColors) r.push('high-contrast');
  } catch (err) {
    // Not available on this platform.
  }
  if (fails >= 3 && !fixture) r.push('capture-failed');
  return r;
}

function tier() {
  return reasons().length ? 'solid' : 'live';
}

function readTransparency() {
  if (process.platform !== 'win32') return;
  execFile(
    'reg',
    ['query', 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize', '/v', 'EnableTransparency'],
    { windowsHide: true, timeout: 4000 },
    (err, out) => {
      const m = !err && /EnableTransparency\s+REG_DWORD\s+0x([0-9a-f]+)/i.exec(String(out));
      const off = !!m && parseInt(m[1], 16) === 0;
      if (off !== transparencyOff) {
        transparencyOff = off;
        broadcastTier();
      }
    }
  );
}

function alive() {
  return [...wins].filter((w) => w && !w.isDestroyed());
}

function broadcastTier() {
  const t = tier();
  lastTier = t;
  for (const w of alive()) w.webContents.send('glass:tier', { tier: t, reasons: reasons() });
  schedule(0);
}

function displayFor(win) {
  try {
    return screen.getDisplayMatching(win.getBounds());
  } catch (err) {
    return screen.getPrimaryDisplay();
  }
}

/** One capture of every display at a sixth of its size. */
async function grab() {
  const displays = screen.getAllDisplays();
  const w = Math.max(...displays.map((d) => d.size.width));
  const h = Math.max(...displays.map((d) => d.size.height));
  const t0 = performance.now();
  const sources = await desktopCapturer.getSources({
    types: ['screen'],
    thumbnailSize: { width: Math.round(w / SCALE), height: Math.round(h / SCALE) },
    fetchWindowIcons: false,
  });
  const t1 = performance.now();
  const out = new Map();
  for (const d of displays) {
    const src = sources.find((s) => s.display_id === String(d.id)) || (displays.length === 1 ? sources[0] : null);
    if (!src || src.thumbnail.isEmpty()) continue;
    out.set(d.id, src.thumbnail.toJPEG(72));
  }
  const t2 = performance.now();
  return { frames: out, wall: t1 - t0, encode: t2 - t1 };
}

// Only pages that load glass.js say hello; the lens and the area selector
// draw no glass and get no frames.
function visibleWins() {
  return alive().filter((w) => w.aevalGlass && w.isVisible());
}

function send(win, buf, display, full) {
  win.webContents.send('glass:frame', {
    img: buf,
    full: full || null,
    display: display.bounds,
    tier: tier(),
    reasons: reasons(),
    fixture: fixture ? fixture.name : '',
  });
}

async function tick() {
  timer = null;
  if (busy || paused) return schedule();
  const shown = visibleWins();
  if (!shown.length || tier() === 'solid') {
    // Solid needs no frames; capture failures are retried slowly in case
    // whatever broke it has passed.
    if (fails >= 3 && !fixture && shown.length) {
      fails = 2;
    }
    return schedule(fails >= 3 ? RETRY_MS : undefined);
  }
  busy = true;
  try {
    if (fixture) {
      for (const w of shown) send(w, fixture.small, fixture.display, fixture.full);
    } else {
      const g = await grab();
      stats.frames += 1;
      push(stats.wall, g.wall);
      push(stats.encode, g.encode);
      if (!g.frames.size) throw new Error('no screen sources');
      for (const w of shown) {
        if (w.isDestroyed()) continue;
        const d = displayFor(w);
        const buf = g.frames.get(d.id) || [...g.frames.values()][0];
        send(w, buf, d);
      }
      if (fails) {
        const was = tier();
        fails = 0;
        if (was !== tier()) broadcastTier();
      }
    }
  } catch (err) {
    stats.lastError = String(err && err.message ? err.message : err);
    fails += 1;
    if (fails === 3) broadcastTier();
  } finally {
    busy = false;
  }
  schedule();
}

function push(arr, v) {
  arr.push(v);
  if (arr.length > 200) arr.shift();
}

function schedule(ms) {
  if (timer) {
    if (ms !== 0) return;
    clearTimeout(timer);
  }
  const pace = ms !== undefined ? ms : opts.pace() === 'fast' ? FAST_MS : SLOW_MS;
  timer = setTimeout(tick, pace);
}

/**
 * @param {object} o
 * @param {() => 'fast'|'slow'} o.pace
 * @param {boolean} o.canHide windows are excluded from capture on this build
 * @param {() => 'auto'|'solid'} o.userSurface
 */
function init(o) {
  opts = { ...opts, ...o };
  readTransparency();
  try {
    nativeTheme.on('updated', () => {
      readTransparency();
      broadcastTier();
    });
  } catch (err) {
    // No theme events here.
  }
  schedule(0);
}

/** Every window that draws glass registers once, at creation. */
function register(win) {
  if (!win || wins.has(win)) return;
  wins.add(win);
  win.on('closed', () => wins.delete(win));
  win.on('show', () => schedule(0));
  win.webContents.on('did-finish-load', () => {
    if (win.isDestroyed()) return;
    win.webContents.send('glass:tier', { tier: tier(), reasons: reasons() });
    schedule(0);
  });
}

/** Something changed that the windows should see now, not at the next tick. */
function kick() {
  schedule(0);
}

/** A page has loaded glass.js and wants frames. */
function hello(win) {
  if (win && !win.isDestroyed()) win.aevalGlass = true;
  schedule(0);
}

/** While content protection is off for a live photograph, no frames. */
function pause() {
  paused += 1;
}
function resume() {
  paused = Math.max(0, paused - 1);
  schedule(0);
}

/**
 * A rendered page stands in for the desktop (the visual self-check). The
 * windows then see a known white document, black editor or wallpaper, and
 * each also gets the full-size picture so its photograph is a composite.
 */
async function useFixture(name) {
  if (!name) {
    fixture = null;
    broadcastTier();
    return;
  }
  const display = screen.getPrimaryDisplay();
  const { width, height } = display.size;
  const win = new BrowserWindow({
    width,
    height,
    show: false,
    frame: false,
    webPreferences: { offscreen: true, contextIsolation: true, nodeIntegration: false },
  });
  try {
    await win.loadFile(path.join(__dirname, 'backdrop-fixture.html'), { query: { kind: name } });
    await new Promise((r) => setTimeout(r, 400));
    let img = await win.webContents.capturePage();
    if (img.isEmpty()) {
      await new Promise((r) => setTimeout(r, 600));
      img = await win.webContents.capturePage();
    }
    const full = img.resize({ width, height });
    const small = img.resize({ width: Math.round(width / SCALE), height: Math.round(height / SCALE), quality: 'good' });
    fixture = { name, small: small.toJPEG(80), full: full.toJPEG(88), fullImage: full, display };
  } finally {
    if (!win.isDestroyed()) win.destroy();
  }
  broadcastTier();
}

/**
 * A crop of what is behind a rect at full resolution, for the Find lens's
 * magnifier. DIP rect in screen coordinates; resolves to a JPEG buffer or
 * null. One capture per current-hit change, never on a timer.
 */
async function crop(rect) {
  const d = screen.getDisplayMatching(rect);
  const rel = { x: rect.x - d.bounds.x, y: rect.y - d.bounds.y, width: rect.width, height: rect.height };
  if (fixture && fixture.fullImage) {
    return fixture.fullImage.crop(rel).toJPEG(90);
  }
  if (tier() === 'solid') return null;
  const k = d.scaleFactor || 1;
  const sources = await desktopCapturer.getSources({
    types: ['screen'],
    thumbnailSize: { width: Math.round(d.size.width * k), height: Math.round(d.size.height * k) },
    fetchWindowIcons: false,
  });
  const src = sources.find((s) => s.display_id === String(d.id)) || sources[0];
  if (!src || src.thumbnail.isEmpty()) return null;
  const sz = src.thumbnail.getSize();
  const f = sz.width / d.size.width;
  return src.thumbnail
    .crop({ x: Math.round(rel.x * f), y: Math.round(rel.y * f), width: Math.round(rel.width * f), height: Math.round(rel.height * f) })
    .toJPEG(90);
}

/** The fixture as a data URL, for photographing a window that draws no glass. */
function fixtureUrl() {
  return fixture ? `data:image/jpeg;base64,${fixture.full.toString('base64')}` : '';
}

function pct(arr, p) {
  if (!arr.length) return 0;
  const s = [...arr].sort((a, b) => a - b);
  return s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))];
}

/**
 * The snapCost gate: n captures back to back. Reports wall time (the whole
 * async round trip), the synchronous JPEG encode, and the longest stall of
 * the main thread's event loop while each capture ran, which is the part a
 * person would feel as a hitch.
 */
async function measure(n = 50) {
  const wall = [];
  const encode = [];
  const block = [];
  const h = monitorEventLoopDelay({ resolution: 1 });
  for (let i = 0; i < n; i += 1) {
    h.reset();
    h.enable();
    const g = await grab();
    h.disable();
    wall.push(g.wall + g.encode);
    encode.push(g.encode);
    // The histogram's max is the worst delay one 1 ms tick saw, in ns.
    block.push(Math.max(g.encode, h.max / 1e6 - 1));
  }
  return {
    n,
    wallP50: pct(wall, 50),
    wallP95: pct(wall, 95),
    encodeP95: pct(encode, 95),
    blockP95: pct(block, 95),
  };
}

function status() {
  return {
    tier: tier(),
    reasons: reasons(),
    windows: alive().length,
    frames: stats.frames,
    wallP95: pct(stats.wall, 95),
    encodeP95: pct(stats.encode, 95),
    fails,
    lastError: stats.lastError,
    fixture: fixture ? fixture.name : '',
  };
}

module.exports = { init, register, kick, hello, pause, resume, tier, reasons, useFixture, fixtureUrl, crop, measure, status, SCALE, broadcastTier };
