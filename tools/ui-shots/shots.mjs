// Renderer screenshots and checks for the Tauri rewrite.
//
// The real pages (renderer/, settings/, history/) load in Microsoft Edge, the
// engine WebView2 embeds, each in a frame standing for its window on a
// 1600x1000 desktop painted with lib/backdrop-fixture.html, with
// tools/ui-shots/mock-bridge.js in place of window.aeval. Every lane's scenes
// run at device scale 1 and 1.5, and the ones flagged `textScale` once more
// with `--text-scale: 2`. `[UI] name=ok|FAIL:...` per check; exit 1 on any
// FAIL or page error.
//
//   node tools/ui-shots/shots.mjs <outDir> [--channel=msedge|chromium] [--lane=<lane>]
//
// LANE FILES (BACKLOG K-31). Each lane owns tools/ui-shots/lanes/<lane>.mjs,
// which exports four things:
//
//   export const scenes = [ /* scene objects, below */ ];
//   export const mocks = { 'settings:get': { textScale: 1 } };
//   export const checks = [ { name: 'tokensDefined', run: async (ctx) => true } ];
//   export const cssLintEnforce = [ 'renderer/glass.css' ];
//
// mocks: what Rust answers a channel when the window's own `responses` do
//   not say. Merged, in lane order, over kit.mjs COMMON: plain objects merge
//   key by key (so two lanes can each add fields to settings:get), anything
//   else replaces. JSON values only.
// checks: run once, after every scene. `run(ctx)` returns true (ok), false,
//   a string (the failure) or [ok, why]. ctx = { root, read(relPath),
//   exists(relPath) }. Printed as `[UI] <name>=ok|FAIL:<why>`.
// cssLintEnforce: CSS files (repo-relative) bridge.test.mjs lints for
//   off-system values (B-145). A file listed here must pass from then on.
//
// A scene:
//
//   {
//     name: '14-thinking',              // unique; the default shot name
//     backdrop: 'wallpaper',            // kit.mjs BACKDROPS: wallpaper, white,
//                                       // black, dark-editor, photo-busy
//     windows: [                        // one frame per window, CSS px
//       { name: 'overlay', src: '/renderer/index.html', x: 480, y: 24, w: 680, h: 240, z: 1 },
//       { name: 'out1', src: '/renderer/output.html', x: 560, y: 92, w: 520, h: 520, z: 2,
//         responses: { 'output:bar': null },  // this window's own answers
//         fixed: true,                  // output:resize leaves the frame alone
//         max: 900 },                   // output:resize grows it at most this tall
//     ],
//     textScale: true,                  // also run with --text-scale: 2 (-ts200)
//     scales: [1, 1.5],                 // the default; 1.5 adds -150 to names
//     clock: true,                      // virtual clock: performance.now and
//                                       // requestAnimationFrame stand still and
//                                       // move only by t.tick or t.settle, in
//                                       // 16 ms frames, the same in every window;
//                                       // Date and timers stay real
//     media: { reducedMotion: 'reduce' }, // page.emulateMedia
//     async run(t) { ... },             // drive it, shoot it, check it
//   }
//
// run(t) gets a toolkit:
//   t.page, t.frame(name)       Playwright Page and Frame
//   t.scale, t.textScale        1 or 1.5; 1 or 2
//   t.suffix                    '', '-150', '-ts200' or '-150-ts200'
//   t.emit(name, channel, payload)   an event from Rust to that window
//   t.respond(name, channel, value)  change that window's answer from now on
//   t.calls(name, channel?)     what that window invoked, [{channel, payload}]
//   t.settle(ms = 450)          wait ms of real time. On a clock scene it
//                               also moves the virtual clock by ms, so never
//                               call it between frames you are timing: use
//                               t.tick there, and t.settle only to let
//                               something run out first
//   t.tick(ms)                  move the virtual clock by exactly ms (needs
//                               clock: true); nothing else moves it
//   t.shot(name?, clip?)        <outDir>/<name><suffix>.png; clip in CSS px,
//                               default every window plus 20 px. Never moves
//                               the virtual clock (elsewhere it settles 120 ms
//                               of real time first)
//   t.check(name, ok, why)      `[UI] <name><suffix>=ok|FAIL:<why>`
//   t.backdrop                  { kind, full, small } data URLs
// kit.mjs has the shared fixtures and helpers (BAR, ANSWER, openBar, ...).
// A scene that throws fails as `[UI] scene.<name><suffix>=FAIL:<error>`.

import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';
import { W, H, BACKDROPS, COMMON, deepMerge } from './kit.mjs';
import { LANES, shapeProblems } from './lanes.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..');
const args = process.argv.slice(2);
const flag = (name, dflt) => (args.find((a) => a.startsWith(`--${name}=`)) || `--${name}=${dflt}`).split('=')[1];
const OUT = path.resolve(args.find((a) => !a.startsWith('--')) || path.join(ROOT, 'ui-shots-out'));
const CHANNEL = flag('channel', 'msedge');
const ONLY = flag('lane', '');
const SCALES = [1, 1.5];
// The virtual clock (B-053): every frame of a clock scene gets its own
// performance.now and requestAnimationFrame, stepped together by tickAll.
const CLOCK_SCRIPT = `(() => {
  let now = 1000;
  let queue = [];
  let nextId = 1;
  performance.now = () => now;
  window.requestAnimationFrame = (cb) => {
    const id = nextId++;
    queue.push({ id, cb });
    return id;
  };
  window.cancelAnimationFrame = (id) => {
    queue = queue.filter((q) => q.id !== id);
  };
  window.__clock = {
    tick(ms) {
      const end = now + ms;
      while (now < end) {
        now = Math.min(end, now + 16);
        const run = queue;
        queue = [];
        for (const q of run) {
          try {
            q.cb(now);
          } catch (e) {
            setTimeout(() => {
              throw e;
            });
          }
        }
      }
      return now;
    },
  };
})();`;
fs.mkdirSync(OUT, { recursive: true });

let failed = 0;
function check(name, ok, why) {
  if (!ok) failed += 1;
  console.log(`[UI] ${name}=${ok ? 'ok' : `FAIL:${String(why || '').replace(/\s+/g, ' ')}`}`);
}

const shape = shapeProblems();
for (const p of shape) console.log(`[UI] shape ${p}`);
check('laneFiles', shape.length === 0, `${shape.length} problems`);

let common = COMMON;
for (const m of Object.values(LANES)) common = deepMerge(common, m.mocks || {});

// ------------------------------------------------------------- the server

const routes = new Map();
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.png': 'image/png', '.svg': 'image/svg+xml', '.json': 'application/json', '.woff2': 'font/woff2' };
const server = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  const r = routes.get(url.pathname);
  if (r) {
    res.writeHead(200, { 'content-type': r.type });
    res.end(r.body);
    return;
  }
  const file = path.join(ROOT, decodeURIComponent(url.pathname));
  if (!file.startsWith(ROOT) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
    res.writeHead(404);
    res.end();
    return;
  }
  res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream' });
  fs.createReadStream(file).pipe(res);
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const BASE = `http://127.0.0.1:${server.address().port}`;

const browser = await chromium.launch(CHANNEL === 'chromium' ? {} : { channel: CHANNEL });
const bridge = fs.readFileSync(path.join(HERE, 'mock-bridge.js'), 'utf8');
const errors = [];
const written = new Map();

// ------------------------------------------------------------ the desktop

async function backdrop(ctx, kind, scale) {
  const page = await ctx.newPage();
  await page.goto(`${BASE}/lib/backdrop-fixture.html?kind=${kind}`);
  await page.waitForTimeout(300);
  const png = await page.screenshot({ type: 'png' });
  const full = `data:image/png;base64,${png.toString('base64')}`;
  // The glass gets what Rust sends: a sixth-size JPEG of the display.
  const small = await page.evaluate(async (src) => {
    const im = new Image();
    im.src = src;
    await im.decode();
    const c = document.createElement('canvas');
    c.width = Math.round(im.width / 6);
    c.height = Math.round(im.height / 6);
    c.getContext('2d').drawImage(im, 0, 0, c.width, c.height);
    return c.toDataURL('image/jpeg', 0.72);
  }, full);
  await page.close();
  routes.set(`/ui-shots-bg-${kind}-${scale}.png`, { type: 'image/png', body: png });
  return { kind, full, small };
}

const frameTag = (w) =>
  `<iframe name="${w.name}" src="${w.src}"${w.fixed ? ' data-fixed="1"' : ''}${w.max ? ` data-max="${w.max}"` : ''} style="position:absolute;left:${w.x}px;top:${w.y}px;width:${w.w}px;height:${w.h}px;border:0;background:transparent;z-index:${w.z || 1}" allowtransparency="true"></iframe>`;

function defaultClip(spec) {
  const x0 = Math.max(0, Math.min(...spec.windows.map((w) => w.x)) - 20);
  const y0 = Math.max(0, Math.min(...spec.windows.map((w) => w.y)) - 20);
  const x1 = Math.min(W, Math.max(...spec.windows.map((w) => w.x + w.w)) + 20);
  const y1 = Math.min(H, Math.max(...spec.windows.map((w) => w.y + w.h)) + 20);
  return { x: x0, y: y0, width: x1 - x0, height: y1 - y0 };
}

async function runScene(ctx, back, lane, spec, scale, textScale) {
  const suffix = `${scale === 1 ? '' : `-${Math.round(scale * 100)}`}${textScale === 1 ? '' : `-ts${textScale * 100}`}`;
  const kind = spec.backdrop || 'wallpaper';
  const mock = Object.fromEntries(spec.windows.map((w) => [w.name, { responses: w.responses || {} }]));
  const route = `/scene/${lane}/${spec.name}${suffix}.html`;
  routes.set(route, {
    type: 'text/html',
    body: `<!DOCTYPE html><html><head><meta charset="utf-8"><title>${spec.name}${suffix}</title>
<style>html,body{margin:0;width:${W}px;height:${H}px;overflow:hidden;background:#000 url('/ui-shots-bg-${kind}-${scale}.png') 0 0/${W}px ${H}px no-repeat}</style>
<script>window.__MOCK__=${JSON.stringify(mock)};window.__MOCK_COMMON__=${JSON.stringify(common)};</script>
</head><body>${spec.windows.map(frameTag).join('')}</body></html>`,
  });
  console.log(`[UI] scene ${lane}/${spec.name}${suffix}`);
  const page = await ctx.newPage();
  const tag = `${spec.name}${suffix}`;
  page.on('pageerror', (e) => errors.push(`${tag}: ${e.message}`));
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(`${tag}: console: ${m.text()}`);
  });
  try {
    if (spec.clock) await page.addInitScript({ content: CLOCK_SCRIPT });
    if (spec.media) await page.emulateMedia(spec.media);
    const tickAll = async (ms) => {
      for (const f of page.frames()) await f.evaluate((m) => window.__clock && window.__clock.tick(m), ms).catch(() => null);
    };
    // Real time passes too, for the timers; on a clock scene the virtual
    // clock keeps step with it, 50 ms at a time.
    const settle = async (ms = 450) => {
      if (!spec.clock) return page.waitForTimeout(ms);
      for (let left = ms; left > 0; left -= 50) {
        await tickAll(Math.min(50, left));
        await page.waitForTimeout(Math.min(50, left));
      }
    };
    await page.goto(`${BASE}${route}`);
    await page.waitForLoadState('load');
    const frame = (n) => {
      const f = page.frames().find((x) => x.name() === n);
      if (!f) throw new Error(`no window ${n} in scene ${spec.name}`);
      return f;
    };
    for (const w of spec.windows) {
      const f = frame(w.name);
      for (let i = 0; (await f.evaluate(() => document.readyState)) !== 'complete'; i += 1) {
        if (i > 200) throw new Error(`${w.src} never finished loading`);
        await page.waitForTimeout(50);
      }
      // Rust's first glass frame: the display, sixth size; and the text scale.
      await f.evaluate(
        ({ small, ts }) => {
          if (window.AevalGlass) window.__emit('glass:frame', { img: small, display: { x: 0, y: 0, width: 1600, height: 1000 }, tier: 'live', reasons: [] });
          if (ts !== 1) {
            document.documentElement.style.setProperty('--text-scale', String(ts));
            window.__emit('ui:text-scale', { scale: ts });
          }
        },
        { small: back.small, ts: textScale }
      );
    }
    await settle(250);
    const t = {
      page,
      frame,
      scale,
      textScale,
      suffix,
      lane,
      backdrop: back,
      emit: (n, channel, payload) => frame(n).evaluate(({ c, p }) => window.__emit(c, p), { c: channel, p: payload }),
      respond: (n, channel, value) =>
        page.evaluate(
          ({ n: name, c, v }) => {
            const m = (window.__MOCK__[name] = window.__MOCK__[name] || {});
            (m.responses = m.responses || {})[c] = v;
          },
          { n, c: channel, v: value }
        ),
      calls: (n, channel) => frame(n).evaluate((c) => window.__calls.filter((x) => !c || x.channel === c), channel || ''),
      settle,
      tick: async (ms) => {
        if (!spec.clock) throw new Error(`scene ${spec.name} has no virtual clock (clock: true)`);
        await tickAll(ms);
        await page.waitForTimeout(20);
      },
      shot: async (name, clip) => {
        if (spec.clock) await page.waitForTimeout(60);
        else await settle(120);
        const file = `${name || spec.name}${suffix}.png`;
        if (written.has(file)) check(`uniqueShot.${file}`, false, `also shot by ${written.get(file)}`);
        written.set(file, `${lane}/${spec.name}`);
        await page.screenshot({ path: path.join(OUT, file), clip: clip || defaultClip(spec) });
        console.log(`[UI] shot ${file}`);
      },
      check: (name, ok, why) => check(`${name}${suffix}`, ok, why),
    };
    await spec.run(t);
  } catch (e) {
    check(`scene.${spec.name}${suffix}`, false, e && e.stack ? e.stack.split('\n').slice(0, 3).join(' ') : e);
  } finally {
    await page.close();
  }
}

// ------------------------------------------------------------- the scenes

const counts = {};
for (const scale of SCALES) {
  const ctx = await browser.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: scale });
  await ctx.addInitScript({ content: bridge });
  const backs = {};
  for (const kind of BACKDROPS) backs[kind] = await backdrop(ctx, kind, scale);
  for (const [lane, m] of Object.entries(LANES)) {
    if (ONLY && lane !== ONLY) continue;
    for (const spec of m.scenes || []) {
      if (!(spec.scales || SCALES).includes(scale)) continue;
      const back = backs[spec.backdrop || 'wallpaper'];
      if (!back) {
        check(`scene.${spec.name}`, false, `unknown backdrop ${spec.backdrop}`);
        continue;
      }
      for (const ts of spec.textScale ? [1, 2] : [1]) {
        await runScene(ctx, back, lane, spec, scale, ts);
        counts[lane] = (counts[lane] || 0) + 1;
      }
    }
  }
  await ctx.close();
}
for (const lane of Object.keys(LANES)) console.log(`[UI] lane ${lane} sceneRuns=${counts[lane] || 0}`);

// ------------------------------------------------------------- the checks

const ctxFiles = {
  root: ROOT,
  read: (rel) => fs.readFileSync(path.join(ROOT, rel), 'utf8'),
  exists: (rel) => fs.existsSync(path.join(ROOT, rel)),
};
for (const [lane, m] of Object.entries(LANES)) {
  if (ONLY && lane !== ONLY) continue;
  for (const c of m.checks || []) {
    try {
      const r = await c.run(ctxFiles);
      if (Array.isArray(r)) check(c.name, !!r[0], r[1]);
      else if (typeof r === 'string') check(c.name, false, r);
      else check(c.name, r === true, 'returned false');
    } catch (e) {
      check(c.name, false, e && e.message ? e.message : e);
    }
  }
}

for (const e of errors) console.log(`[UI] pageerror ${e}`);
check('noPageErrors', errors.length === 0, `${errors.length} page errors`);
await browser.close();
server.close();
console.log(`[UI] done failed=${failed} shots=${written.size}`);
process.exit(failed ? 1 : 0);
