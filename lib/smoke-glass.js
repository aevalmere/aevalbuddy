'use strict';

// The Phase B gates of the smoke test (PLAN-v2.1 Phase B): the glass
// engine is loaded everywhere, costs what it says, and none of the window
// churn that broke the acrylic attempt has come back.
//
//   glassLoaded        every glass page has glass.js and a tier
//   noSetOpacity       no code calls setOpacity (it toggles WS_EX_LAYERED)
//   snapCost           a backdrop capture blocks the main thread < 25 ms p95
//   glassCost          CPU with the glass live, then solid (logged)
//   styleStable        the overlay's window style never changes across
//                      pill, bar, answer and back, except the two bits
//                      setIgnoreMouseEvents owns; WS_EX_NOACTIVATE is set
//                      on the pill and clear on the bar, exactly
//   collapseClickable  after a collapse, the pill takes the click
//   topmostHolds       a foreign topmost window does not bury us for 60 s

const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');

const wait = (ms) => new Promise((r) => setTimeout(r, ms));

// setIgnoreMouseEvents adds and removes WS_EX_TRANSPARENT and WS_EX_LAYERED;
// that is the click-through margin working, not churn (DECISIONS D-B3).
// setFocusable owns WS_EX_NOACTIVATE: the orb must never take focus from the
// app in front, the bar must. That bit is not free; it is checked against
// the mode exactly (D-B10).
const NOACTIVATE = 0x08000000;
const EX_MASK = ~(0x20 | 0x80000 | NOACTIVATE);
// WS_VISIBLE follows show and hide, which is not a style change either.
const STYLE_MASK = ~0x10000000;

function hex(n) {
  return `0x${(n >>> 0).toString(16)}`;
}

function sources(root) {
  const out = [path.join(root, 'main.js')];
  const walk = (dir) => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.name.endsWith('.js')) out.push(p);
    }
  };
  walk(path.join(root, 'lib'));
  return out;
}

async function cpuOver(app, ms) {
  app.getAppMetrics();
  await wait(ms);
  const m = app.getAppMetrics();
  const sum = (f) => Math.round(m.filter(f).reduce((s, p) => s + p.cpu.percentCPUUsage, 0) * 10) / 10;
  return {
    total: sum(() => true),
    gpu: sum((p) => p.type === 'GPU'),
    renderer: sum((p) => p.type === 'Tab'),
    browser: sum((p) => p.type === 'Browser'),
  };
}

/**
 * @param {object} c the smoke context from main.js
 */
async function run(c) {
  const { log, must, desktop, backdrop, app, screen, store } = c;
  const handle = (w) => desktop.handleStr(w.getNativeWindowHandle());

  // ------------------------------------------------------------ glassLoaded
  const pages = [
    ['overlay', c.overlayWin()],
    ['output', c.outputWin()],
  ];
  const seen = [];
  for (const [name, w] of pages) {
    let r = 'missing';
    if (w && !w.isDestroyed()) {
      r = await w.webContents.executeJavaScript(
        `(() => (window.AevalGlass ? 'glass' : 'none') + ':' + (document.documentElement.dataset.glass || 'notier'))()`
      );
    }
    seen.push(`${name}=${r}`);
  }
  const st = backdrop.status();
  log('glassTier', `${st.tier} reasons=${st.reasons.join('+') || 'none'} canHide=${c.canHide}`);
  must('glassLoaded', seen.every((s) => /=glass:(live|solid)$/.test(s)), seen.join(' '));

  // ----------------------------------------------------------- noSetOpacity
  const root = app.getAppPath();
  const calls = [];
  for (const f of sources(root)) {
    const lines = fs.readFileSync(f, 'utf8').split('\n');
    lines.forEach((line, i) => {
      const code = line.replace(/\/\/.*$/, '');
      if (/\.setOpacity\s*\(/.test(code)) calls.push(`${path.relative(root, f)}:${i + 1}`);
    });
  }
  must('noSetOpacity', calls.length === 0, calls.join(' '));

  // -------------------------------------------------------------- snapCost
  if (backdrop.tier() === 'live') {
    try {
      const m = await backdrop.measure(50);
      const r = (v) => Math.round(v * 10) / 10;
      log('snapCost', `n=${m.n} wallP50=${r(m.wallP50)}ms wallP95=${r(m.wallP95)}ms encodeP95=${r(m.encodeP95)}ms blockP95=${r(m.blockP95)}ms`);
      must('snapCost', m.blockP95 < 25, `main thread blocked ${r(m.blockP95)}ms p95, budget 25ms`);
    } catch (err) {
      must('snapCost', false, err.message);
    }
  } else {
    must('snapCost', false, `the glass is ${backdrop.tier()} (${backdrop.reasons().join('+')}), so there is nothing to measure`);
  }

  // ------------------------------------------------- styleStable, glassCost
  const ov = c.overlayWin();
  const out = c.outputWin();
  const hs = [handle(ov), handle(out)];
  const samples = [];
  const sample = async (label) => {
    const r = await desktop.windowStates({ handles: hs });
    for (const [i, w] of r.windows.entries()) {
      if (!w.visible) continue;
      samples.push({ label, win: i === 0 ? 'overlay' : 'output', style: w.style, ex: w.exStyle });
    }
  };
  c.collapse();
  await wait(600);
  await sample('pill');
  await c.openBar('ask');
  await wait(700);
  await sample('bar');
  c.seedAnswer();
  await wait(900);
  await sample('answer');

  // With the bar and the answer up the backdrop runs at its fast pace; this
  // is the cost a person pays while reading an answer.
  const live = await cpuOver(app, 8000);
  const was = store.getSurface();
  store.setSurface('solid');
  backdrop.broadcastTier();
  await wait(1200);
  await sample('answer-solid');
  const solid = await cpuOver(app, 8000);
  store.setSurface(was);
  backdrop.broadcastTier();
  log('glassCost', `live total=${live.total}% gpu=${live.gpu}% renderer=${live.renderer}% browser=${live.browser}% | solid total=${solid.total}% gpu=${solid.gpu}% renderer=${solid.renderer}% browser=${solid.browser}% (percent of one core, 8 s, bar and answer open)`);

  c.collapse();
  await wait(700);
  await sample('pill-again');
  const raw = samples.map((s) => `${s.label}/${s.win}:${hex(s.style)}/${hex(s.ex)}`).join(' ');
  log('styleSamples', raw);
  const drift = [];
  for (const name of ['overlay', 'output']) {
    const mine = samples.filter((s) => s.win === name);
    if (!mine.length) continue;
    const base = mine[0];
    for (const s of mine.slice(1)) {
      if ((s.style & STYLE_MASK) !== (base.style & STYLE_MASK) || (s.ex & EX_MASK) !== (base.ex & EX_MASK)) {
        drift.push(`${name} ${base.label}->${s.label} ${hex(base.style)}/${hex(base.ex)}->${hex(s.style)}/${hex(s.ex)}`);
      }
    }
  }
  for (const s of samples.filter((x) => x.win === 'overlay')) {
    const want = s.label.startsWith('pill');
    if (((s.ex & NOACTIVATE) !== 0) !== want) {
      drift.push(`overlay ${s.label} WS_EX_NOACTIVATE=${!want} (want ${want}) ${hex(s.ex)}`);
    }
  }
  must('styleStable', samples.length >= 4 && drift.length === 0, drift.join('; ') || `samples=${samples.length}`);

  // ------------------------------------------------------ collapseClickable
  await c.openBar('ask');
  await wait(600);
  c.collapse();
  await wait(c.collapseMs + 500);
  const pb = ov.getBounds();
  const centre = screen.dipToScreenPoint({ x: pb.x + Math.round(pb.width / 2), y: pb.y + Math.round(pb.height / 2) });
  const hit = await desktop.windowStates({ handles: [handle(ov)], point: centre, move: true });
  // windowStates reads the styles before it moves the pointer, so its own
  // exStyle is the orb at rest (click-through until hovered). Read it again
  // with the pointer on the orb: that is the state a click meets.
  const after = await desktop.windowStates({ handles: [handle(ov)] });
  const ex = after.windows[0].exStyle;
  const through = (ex & 0x20) !== 0;
  log('collapseClickable', `pill=${JSON.stringify(pb)} at=${hit.atPoint} overlay=${handle(ov)} ex=${hex(ex)}`);
  must('collapseClickable', hit.atPoint === handle(ov) && !through, `under the pill centre: ${hit.atPoint}, overlay ${handle(ov)}, click-through=${through}`);

  // ----------------------------------------------------------- topmostHolds
  // A topmost window from another process, over the pill. keepOnTop is
  // what holds us above it; it is off in smoke runs, so this runs it on
  // its real 4 s interval for the length of the gate.
  const title = `aeval-topmost-probe-${process.pid}`;
  const at = { x: Math.max(0, centre.x - 120), y: Math.max(0, centre.y - 120) };
  const ps = [
    'Add-Type -AssemblyName System.Windows.Forms;',
    '$f = New-Object System.Windows.Forms.Form;',
    `$f.Text = '${title}';`,
    '$f.TopMost = $true; $f.ShowInTaskbar = $false;',
    "$f.StartPosition = 'Manual';",
    `$f.Location = New-Object System.Drawing.Point(${at.x}, ${at.y});`,
    '$f.Size = New-Object System.Drawing.Size(240, 240);',
    '$t = New-Object System.Windows.Forms.Timer; $t.Interval = 90000; $t.add_Tick({ $f.Close() }); $t.Start();',
    '[void]$f.ShowDialog();',
  ].join(' ');
  const probe = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', ps], { stdio: 'ignore', windowsHide: false });
  const pin = setInterval(c.keepOnTop, 4000);
  const zs = [];
  try {
    let found = null;
    for (let i = 0; i < 40 && !found; i += 1) {
      await wait(250);
      found = (await desktop.windowStates({ title })).titled || null;
    }
    if (!found) {
      must('topmostHolds', false, 'the probe window never appeared');
    } else {
      const t0 = Date.now();
      while (Date.now() - t0 < 60000) {
        await wait(3000);
        const r = await desktop.windowStates({ handles: [handle(ov)], title });
        const p = r.titled;
        const me = r.windows[0];
        zs.push({ t: Math.round((Date.now() - t0) / 1000), me: me.z, probe: p ? p.z : -1, top: me.topmost });
      }
      // The first 6 s are the probe arriving on top; keepOnTop's interval
      // is 4 s. After that we have to be above it every time.
      const late = zs.filter((z) => z.t >= 6);
      const buried = late.filter((z) => !z.top || (z.probe >= 0 && z.me > z.probe));
      log('topmostHolds', zs.map((z) => `${z.t}s:${z.me}/${z.probe}${z.top ? '' : '!'}`).join(' '));
      must('topmostHolds', late.length >= 15 && buried.length === 0, `buried in ${buried.length} of ${late.length} samples`);
    }
  } finally {
    clearInterval(pin);
    try {
      probe.kill();
    } catch (err) {
      // Already gone; the form also closes itself after 90 s.
    }
  }
}

module.exports = { run };
