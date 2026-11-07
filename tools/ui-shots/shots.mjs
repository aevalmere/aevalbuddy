// Renderer screenshots and checks for the Tauri rewrite, split (d).
//
// The Rust side of most channels is still being ported, so the installed exe
// cannot yet put the pages in these states. This run loads the real pages
// (renderer/, settings/, history/) in Microsoft Edge, the engine WebView2
// embeds, each in a frame standing for its window on a 1600x1000 desktop
// painted with lib/backdrop-fixture.html, with tools/ui-shots/mock-bridge.js
// in place of window.aeval. It photographs every surface and asserts the
// renderer half of REQUESTS.md 3-11. `[UI] name=ok|FAIL:...` per check; exit 1
// on any FAIL.
//
//   node tools/ui-shots/shots.mjs <outDir> [--channel=msedge|chromium]

import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(HERE, '..', '..');
const OUT = path.resolve(process.argv[2] || path.join(ROOT, 'ui-shots-out'));
const CHANNEL = (process.argv.find((a) => a.startsWith('--channel=')) || '--channel=msedge').split('=')[1];
const W = 1600;
const H = 1000;
const STAGE = 20;
fs.mkdirSync(OUT, { recursive: true });

let failed = 0;
function check(name, ok, why) {
  if (!ok) failed += 1;
  console.log(`[UI] ${name}=${ok ? 'ok' : `FAIL:${why || ''}`}`);
}

// ------------------------------------------------------------- the server

const scenes = new Map();
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.png': 'image/png', '.svg': 'image/svg+xml', '.json': 'application/json' };
const server = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://x');
  if (scenes.has(url.pathname)) {
    res.writeHead(200, { 'content-type': 'text/html' });
    res.end(scenes.get(url.pathname));
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
const ctx = await browser.newContext({ viewport: { width: W, height: H }, deviceScaleFactor: 1 });
await ctx.addInitScript({ path: path.join(HERE, 'mock-bridge.js') });
const errors = [];

// ------------------------------------------------------------ the desktop

async function backdrop(kind) {
  const page = await ctx.newPage();
  await page.goto(`${BASE}/lib/backdrop-fixture.html?kind=${kind}`);
  await page.waitForTimeout(300);
  const png = await page.screenshot({ type: 'png' });
  fs.writeFileSync(path.join(OUT, `.backdrop-${kind}.png`), png);
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
  return { full, small };
}

const BACK = { wallpaper: await backdrop('wallpaper'), white: await backdrop('white') };

// --------------------------------------------------------------- fixtures

const now = Date.now();
const COMMON = {
  'glass:hello': null,
  'bar:history': [],
  'bar:history-push': [],
  'settings:get': {
    hasKey: true,
    model: 'google/gemma-3-27b-it:free',
    defaultModel: 'google/gemma-3-27b-it:free',
    brain: 'deepseek/deepseek-chat-v3-0324:free',
    brainEnabled: false,
    defaultBrain: 'deepseek/deepseek-chat-v3-0324:free',
    findModel: 'google/gemma-3-27b-it:free',
    defaultFindModel: 'google/gemma-3-27b-it:free',
    agentModel: 'google/gemma-3-27b-it:free',
    defaultAgentModel: 'google/gemma-3-27b-it:free',
    find: { scope: 'window', related: 'ask', ocr: true },
    agent: { permission: 'risky', maxSteps: 25, screenshots: true, returnCursor: true, consent: true },
    winBuild: 26100,
    showPaid: false,
    skills: [],
    skill: 'auto',
    shortcuts: { captureQuery: 'Ctrl+Alt+Space', captureRegion: '', findOnScreen: '', agentTask: '', toggleVisibility: '' },
    active: { captureQuery: true, captureRegion: null, findOnScreen: null, agentTask: null, toggleVisibility: null },
    defaults: { captureQuery: 'Ctrl+Alt+Space', captureRegion: '', findOnScreen: '', agentTask: '', toggleVisibility: '' },
    version: '2.0.0',
    update: { state: 'idle', version: '2.0.0', checkedAt: now - 3600e3 },
    canHide: true,
    saveHistory: true,
    historyCount: 4,
    launchAtLogin: true,
    launchAtLoginLive: true,
    surface: 'auto',
    glass: { tier: 'live', reasons: [] },
  },
  'models:list': {
    live: true,
    current: 'google/gemma-3-27b-it:free',
    groups: [
      {
        label: 'Free',
        models: [
          { id: 'google/gemma-3-27b-it:free', name: 'Gemma 3 27B', free: true },
          { id: 'mistralai/mistral-small-3.1-24b-instruct:free', name: 'Mistral Small 3.1', free: true },
        ],
      },
    ],
  },
  'history:list': {
    rows: [
      { id: 'a', title: 'What does this error in the build log mean?', reply: 'The linker cannot find vcruntime140.dll because the redistributable is missing…', asked: 3, at: now - 20 * 60e3 },
      { id: 'b', title: 'Summarise the quarterly numbers on this slide', reply: 'Revenue grew 12% quarter on quarter, driven by the services line…', asked: 1, at: now - 3 * 3600e3 },
      { id: 'c', title: 'Rename these files by date', reply: 'Renamed 14 files in Downloads to YYYY-MM-DD names.', asked: 2, at: now - 30 * 3600e3 },
      { id: 'd', title: 'Which of these flights is cheapest?', reply: 'The 07:45 KLM flight at €212 is the cheapest direct option.', asked: 1, at: now - 4 * 86400e3 },
    ],
  },
};

const ANSWER = [
  'The build fails because **vcruntime140.dll** is not on this machine. The linker log shows it three times, each after `LINK : fatal error LNK1104`.',
  '',
  '## Fix it',
  '1. Install the *Microsoft Visual C++ 2015-2022 Redistributable (x64)*.',
  '2. Restart the terminal so `PATH` is read again.',
  '3. Run the build again:',
  '',
  '```',
  'cargo build --release',
  '```',
  '',
  'If it still fails, the project may target x86; check `rust-toolchain.toml`.',
].join('\n');

// A window: a frame at x,y with the window's size (glass + 20 px margin).
function frameTag(name, src, x, y, w, h, z) {
  return `<iframe name="${name}" src="${src}" style="position:absolute;left:${x}px;top:${y}px;width:${w}px;height:${h}px;border:0;background:transparent;z-index:${z || 1}" allowtransparency="true"></iframe>`;
}

async function scene(name, kind, frames, mock) {
  const html = `<!DOCTYPE html><html><head><meta charset="utf-8"><title>${name}</title>
<style>html,body{margin:0;width:${W}px;height:${H}px;overflow:hidden;background:#000 url('/ui-shots-bg-${kind}.png') 0 0/${W}px ${H}px no-repeat}</style>
<script>window.__MOCK__=${JSON.stringify(mock || {})};window.__MOCK_COMMON__=${JSON.stringify(COMMON)};</script>
</head><body>${frames.map((f) => frameTag(...f)).join('')}</body></html>`;
  scenes.set(`/${name}.html`, html);
  const page = await ctx.newPage();
  page.on('pageerror', (e) => errors.push(`${name}: ${e.message}`));
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(`${name}: console: ${m.text()}`);
  });
  await page.goto(`${BASE}/${name}.html`);
  await page.waitForLoadState('load');
  const get = (n) => page.frames().find((f) => f.name() === n);
  for (const [n] of frames) {
    const f = get(n);
    await f.waitForFunction(() => document.readyState === 'complete' && !!window.AevalGlass);
    // Rust's first glass frame: the display, sixth size.
    await f.evaluate(
      ({ small }) => window.__emit('glass:frame', { img: small, display: { x: 0, y: 0, width: 1600, height: 1000 }, tier: 'live', reasons: [] }),
      { small: BACK[kind].small }
    );
  }
  await page.waitForTimeout(250);
  return { page, get };
}

// Background images for scenes, served from memory.
for (const kind of Object.keys(BACK)) {
  scenes.set(`/ui-shots-bg-${kind}.png`, null);
}
const bgServer = server.listeners('request')[0];
server.removeAllListeners('request');
server.on('request', (req, res) => {
  const m = /^\/ui-shots-bg-(\w+)\.png$/.exec(req.url);
  if (m && BACK[m[1]]) {
    res.writeHead(200, { 'content-type': 'image/png' });
    res.end(Buffer.from(BACK[m[1]].full.split(',')[1], 'base64'));
    return;
  }
  bgServer(req, res);
});

async function shot(page, name, clip) {
  await page.waitForTimeout(120);
  await page.screenshot({ path: path.join(OUT, `${name}.png`), clip });
  console.log(`[UI] shot ${name}.png`);
}

const settle = (page, ms) => page.waitForTimeout(ms || 450);

// The bar window: 640 wide panel, in a window 40 wider and taller.
const BAR = { x: 480, y: 24, w: 680, h: 240 };
const barState = (extra) => ({
  mode: 'panel',
  hasKey: true,
  captureOk: true,
  inputAnchor: { h: 'center', v: 'top' },
  inputWidth: 640,
  pillState: 'idle',
  agent: { running: false },
  ...extra,
});
const barContext = { screen: 'Build log LINK fatal error LNK1104 cannot open file vcruntime140.dll', title: 'Developer PowerShell', process: 'WindowsTerminal', chars: 2400 };

async function openBar(f, extra) {
  await f.evaluate(
    ({ st, cx }) => {
      window.__emit('overlay:state', st);
      window.__emit('bar:context', cx);
    },
    { st: barState(extra), cx: barContext }
  );
}

// The bar's glass rect in screen coordinates, which is what Rust answers
// for output:bar.
async function barRect(f) {
  return f.evaluate(() => {
    const r = document.getElementById('bar').getBoundingClientRect();
    return { x: window.screenX + r.left, y: window.screenY + r.top, width: r.width, height: r.height };
  });
}

async function streamAnswer(f, question, answer) {
  await f.evaluate(
    ({ q, a }) => {
      window.__emit('overlay:state', { mode: 'panel', chatMax: 640 });
      window.__emit('chat:user', { gen: 1, text: q });
      window.__emit('query:start', { gen: 1 });
      window.__emit('query:delta', { gen: 1, text: a });
      window.__emit('query:done', { gen: 1, usage: { model: 'google/gemma-3-27b-it:free', ttftMs: 900, ms: 3200, prompt: 1100, completion: 240, cost: 0 }, question: q });
    },
    { q: question, a: answer }
  );
}

// ------------------------------------------------------------------ shots

// 01, 02: the bar before and after the first request (REQUESTS.md 3, 6).
{
  const { page, get } = await scene('bar', 'wallpaper', [['overlay', '/renderer/index.html', BAR.x, BAR.y, BAR.w, BAR.h, 2]]);
  const f = get('overlay');
  await f.evaluate(() => localStorage.clear());
  await openBar(f);
  await settle(page);
  await shot(page, '01-bar-first-run', { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: 200 });
  const shown = await f.evaluate(() => !document.getElementById('examples').hidden && document.querySelectorAll('#examples button').length);
  check('suggestionsBeforeFirst', shown > 0, `examples shown=${shown}`);
  check('noLegend', await f.evaluate(() => !document.getElementById('legend')), 'the sigil legend is back beside the input');
  const hintOnToken = await f.evaluate(() => /Ctrl\+1/.test(document.getElementById('intent').title));
  check('hintOnLeftButton', hintOnToken, 'the token lost its shortcut hint');
  // Send one question.
  await f.evaluate(() => {
    const i = document.getElementById('query-input');
    i.value = 'why does the build fail';
    i.dispatchEvent(new Event('input'));
    i.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  });
  await settle(page, 200);
  const after = await f.evaluate(() => document.getElementById('examples').hidden);
  check('suggestionsGoneAfterFirst', after, 'examples still showing after a request');
  const sent = await f.evaluate(() => window.__calls.some((c) => c.channel === 'query:submit'));
  check('barSends', sent, 'Enter did not submit');
  // A new session (the bar reopened, a new context): still none.
  await f.evaluate((cx) => {
    window.__emit('overlay:state', { mode: 'pill' });
    window.__emit('overlay:state', { mode: 'panel' });
    window.__emit('bar:context', cx);
  }, barContext);
  await settle(page, 300);
  check('suggestionsNeverAgain', await f.evaluate(() => document.getElementById('examples').hidden), 'examples came back on reopen');
  await shot(page, '02-bar-after-first', { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: 140 });
  await page.close();
}

// 03, 04: one output window under the bar, and its glob frames (REQUESTS.md
// 7, 8, 9, 11).
const OUT1 = { x: 560, y: 92, w: 520, h: 520 };
{
  const mock = {};
  const { page, get } = await scene(
    'output',
    'wallpaper',
    [
      ['overlay', '/renderer/index.html', BAR.x, BAR.y, BAR.w, BAR.h, 1],
      ['out1', '/renderer/output.html', OUT1.x, OUT1.y, OUT1.w, OUT1.h, 2],
    ],
    mock
  );
  const bar = get('overlay');
  const out = get('out1');
  await bar.evaluate(() => localStorage.setItem('aeval.hasSentFirstRequest', '1'));
  await openBar(bar);
  await settle(page, 300);
  const rect = await barRect(bar);
  await out.evaluate((r) => {
    // Rust's answer to output:bar for this window.
    window.__barRect = r;
  }, rect);
  await streamAnswer(out, 'Why does the build fail?', ANSWER);
  await settle(page, 900);
  await shot(page, '03-output-answer', { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: OUT1.y + OUT1.h });
  check('noOutputInput', await out.evaluate(() => !document.querySelector('#opanel input, #opanel textarea')), 'an input exists in the output window');
  check('outputTitleBar', await out.evaluate(() => !!document.querySelector('#sheet-bar[data-drag]') && document.getElementById('sheet-title').textContent.includes('build')), 'no draggable title bar with the question');
  check('outputControls', await out.evaluate(() => ['copy-all', 'history-btn', 'new-chat', 'close-out'].every((id) => !!document.getElementById(id))), 'missing a title-bar control');
  check('noDockUi', await out.evaluate(() => !document.getElementById('dock-toggle') && !document.getElementById('hud')), 'dock or answer ghost still in the output page');
  check('resizeZones', await out.evaluate(() => document.querySelectorAll('.frame-handle[data-hit]').length === 8 && [...document.querySelectorAll('.frame-handle')].every((h) => h.style.display === 'block')), 'eight resize zones not laid out');
  // Glob frames: the shape at four moments, drawn from the bar's real rect.
  const local = await out.evaluate((r) => ({ left: r.x - window.screenX, top: r.y - window.screenY, right: r.x - window.screenX + r.width, bottom: r.y - window.screenY + r.height }), rect);
  const timing = await out.evaluate((b) => {
    const sheet = document.getElementById('opanel');
    const g = window.AevalGlob.geometry(b, sheet.getBoundingClientRect());
    const t0 = performance.now();
    for (let i = 0; i < 600; i += 1) window.AevalGlob.shapeAt(g, (i % 60) / 60);
    return (performance.now() - t0) / 600;
  }, local);
  check('globFrameCost', timing < 1.5, `${timing.toFixed(3)} ms per shape`);
  console.log(`[UI] globShapeMs=${timing.toFixed(3)}`);
  const steps = [['a', 0.08], ['b', 0.3], ['c', 0.52], ['d', 0.7], ['e', 0.9]];
  for (const [k, t] of steps) {
    await out.evaluate(({ b, t: tt }) => window.AevalGlob.preview(document.getElementById('opanel'), b, tt), { b: local, t });
    await page.waitForTimeout(160);
    await shot(page, `04${k}-glob-${String(Math.round(t * 100)).padStart(2, '0')}`, { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: OUT1.y + OUT1.h });
  }
  // A real open, measured: it must finish and hand the sheet back.
  const ran = await out.evaluate(async () => {
    const sheet = document.getElementById('opanel');
    const t0 = performance.now();
    let frames = 0;
    let longest = 0;
    let last = t0;
    let on = true;
    const tick = (now) => {
      frames += 1;
      longest = Math.max(longest, now - last);
      last = now;
      if (on) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
    await window.AevalGlob.open(sheet);
    on = false;
    return { ms: performance.now() - t0, frames, longest, clean: !sheet.classList.contains('globbing') && document.getElementById('glob').hidden };
  });
  console.log(`[UI] globOpen ms=${ran.ms.toFixed(0)} frames=${ran.frames} longestFrameMs=${ran.longest.toFixed(1)}`);
  check('globOpens', ran.clean && ran.ms < 900, JSON.stringify(ran));
  check('globSmooth', ran.longest < 34, `longest frame ${ran.longest.toFixed(1)} ms`);
  // Reduced motion: a fade, no glob.
  const rm = await out.evaluate(async () => {
    document.documentElement.dataset.motion = 'reduced';
    const sheet = document.getElementById('opanel');
    await window.AevalGlob.open(sheet);
    const r = document.getElementById('glob').hidden;
    document.documentElement.dataset.motion = 'full';
    return r;
  });
  check('globReducedMotion', rm, 'the glob ran under reduced motion');
  await page.close();
}

// 05: several outputs at once, the bar answering a task's question
// (REQUESTS.md 9, 11).
{
  const O1 = { x: 140, y: 230, w: 500, h: 560 };
  const O2 = { x: 690, y: 300, w: 440, h: 420 };
  const O3 = { x: 1150, y: 120, w: 420, h: 520 };
  const { page, get } = await scene('multi', 'wallpaper', [
    ['overlay', '/renderer/index.html', BAR.x, BAR.y, BAR.w, BAR.h, 4],
    ['o1', '/renderer/output.html', O1.x, O1.y, O1.w, O1.h, 1],
    ['o2', '/renderer/output.html', O2.x, O2.y, O2.w, O2.h, 2],
    ['o3', '/renderer/output.html', O3.x, O3.y, O3.w, O3.h, 3],
  ]);
  const bar = get('overlay');
  await bar.evaluate(() => localStorage.setItem('aeval.hasSentFirstRequest', '1'));
  await openBar(bar, { pillState: 'agent', agent: { running: true, step: 3, state: 'asking' } });
  await streamAnswer(get('o1'), 'Why does the build fail?', ANSWER);
  await get('o2').evaluate(() => {
    window.__emit('overlay:state', { mode: 'panel', chatMax: 520 });
    window.__emit('view:set', { view: 'find' });
    window.__emit('find:state', { gen: 1, open: true, status: '3 exact · 2 related', lines: [] });
    window.__emit('find:hits', {
      gen: 1,
      query: 'invoice total',
      current: 0,
      hits: [
        { i: 0, kind: 'literal', text: 'Invoice total: €1,240.00', match: 'Invoice total' },
        { i: 1, kind: 'literal', text: 'Invoice total (incl. VAT)', match: 'Invoice total' },
        { i: 2, kind: 'literal', text: 'See invoice total on page 2', match: 'invoice total' },
        { i: 3, kind: 'related', text: 'Amount due: €1,240.00', why: 'The same figure, named as the amount due' },
        { i: 4, kind: 'related', text: 'Balance €1,240.00', why: 'The outstanding balance matches the total' },
      ],
    });
  });
  await get('o3').evaluate(() => {
    window.__emit('overlay:state', { mode: 'panel', chatMax: 520 });
    window.__emit('agent:begin', { task: 'Rename the screenshots in Downloads by date' });
    window.__emit('agent:step', { id: 1, kind: 'plan', state: 'done', steps: ['Open Downloads', 'Sort screenshots by date', 'Rename each to YYYY-MM-DD'] });
    window.__emit('agent:step', { id: 2, kind: 'act', state: 'done', text: 'Opened File Explorer at Downloads', ms: 1800 });
    window.__emit('agent:step', { id: 3, kind: 'act', state: 'done', text: 'Selected 14 screenshots' });
    window.__emit('agent:step', { id: 4, kind: 'ask', state: 'running', text: 'Keep the time of day in the names?' });
    window.__emit('agent:state', { state: 'asking', step: 3 });
  });
  await settle(page, 900);
  await shot(page, '05-outputs-multi', { x: 0, y: 0, width: W, height: H });
  const ph = await bar.evaluate(() => document.getElementById('query-input').placeholder);
  check('barAnswersTask', /question/i.test(ph), `placeholder "${ph}"`);
  await bar.evaluate(() => {
    const i = document.getElementById('query-input');
    i.value = 'No, just the date';
    i.dispatchEvent(new Event('input'));
    i.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  });
  const cont = await bar.evaluate(() => window.__calls.find((c) => c.channel === 'agent:continue'));
  check('askUserRoutedFromBar', !!cont && cont.payload && cont.payload.text === 'No, just the date', JSON.stringify(cont));
  check('noInputInAnyOutput', (await Promise.all(['o1', 'o2', 'o3'].map((n) => get(n).evaluate(() => document.querySelectorAll('input, textarea').length)))).every((n) => n === 0), 'an output has an input');
  await page.close();
}

// 06: Settings with the Liquid glass slider at 0, 0.5 and 1, beside an
// output window that follows it live (REQUESTS.md 4, 5).
{
  const S = { x: 120, y: 130, w: 960, h: 700 };
  const O = { x: 1080, y: 200, w: 480, h: 480 };
  const { page, get } = await scene('settings', 'wallpaper', [
    ['settings', '/settings/settings.html', S.x, S.y, S.w, S.h, 2],
    ['o1', '/renderer/output.html', O.x, O.y, O.w, O.h, 1],
  ]);
  const st = get('settings');
  const out = get('o1');
  await streamAnswer(out, 'Summarise the quarterly numbers on this slide', 'Revenue grew **12%** quarter on quarter, driven by services. Margins held at 31%; hardware fell 4%.');
  await settle(page, 600);
  await shot(page, '08-settings-account', { x: S.x - 10, y: S.y - 10, width: S.w + 20, height: S.h + 20 });
  await st.evaluate(() => {
    showPage('pg-general');
    openSection('sec-surface');
  });
  await settle(page, 400);
  for (const k of [0, 0.5, 1]) {
    await st.evaluate((kk) => {
      const r = document.getElementById('glass-range');
      r.value = String(kk);
      r.dispatchEvent(new Event('input', { bubbles: true }));
    }, k);
    await settle(page, 700);
    const there = await out.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--glass-k').trim());
    check(`glassLive.${k}`, Number(there) === k, `output window has --glass-k ${there}`);
    const level = await st.evaluate(() => document.documentElement.dataset.glass);
    check(`glassTier.${k}`, k === 0 ? level === 'solid' : level === 'live', `data-glass=${level}`);
    await shot(page, `06-settings-glass-${String(Math.round(k * 100)).padStart(3, '0')}`, { x: S.x - 10, y: S.y - 10, width: O.x + O.w - S.x + 20, height: S.h + 20 });
  }
  const saved = await st.evaluate(() => new Promise((r) => setTimeout(() => r(window.__calls.filter((c) => c.channel === 'settings:set-glass').map((c) => c.payload.glassIntensity)), 300)));
  check('glassSaved', saved.length > 0 && saved[saved.length - 1] === 1, JSON.stringify(saved));
  check('noDuplicateHintInSettings', await st.evaluate(() => !/Ctrl\+1\/2\/3/.test(document.body.textContent)), 'Settings repeats the Ask/Find/Do hint');
  check('settingsResizeZones', await st.evaluate(() => document.querySelectorAll('.frame-handle').length === 8), 'no resize zones');
  check('settingsDrag', await st.evaluate(() => document.getElementById('titlebar').dataset.drag === 'max'), 'title bar is not a drag region');
  // Full-screen: the same page at a maximised size keeps its measure.
  await page.close();
}

// 07: Settings maximised (REQUESTS.md 4), and History.
{
  const { page, get } = await scene('desktop', 'wallpaper', [['settings', '/settings/settings.html', 0, 0, W, H, 1]]);
  const st = get('settings');
  await st.evaluate(() => showPage('pg-general'));
  await settle(page, 500);
  await shot(page, '07-settings-fullscreen', { x: 0, y: 0, width: W, height: H });
  await page.close();
}
{
  const Hh = { x: 400, y: 140, w: 720, h: 640 };
  const { page } = await scene('history', 'wallpaper', [['history', '/history/history.html', Hh.x, Hh.y, Hh.w, Hh.h, 1]]);
  await settle(page, 600);
  await shot(page, '09-history', { x: Hh.x - 10, y: Hh.y - 10, width: Hh.w + 20, height: Hh.h + 20 });
  await page.close();
}

// 10: the bar and an answer over a white document, where contrast is
// hardest.
{
  const { page, get } = await scene('white', 'white', [
    ['overlay', '/renderer/index.html', BAR.x, BAR.y, BAR.w, BAR.h, 1],
    ['out1', '/renderer/output.html', OUT1.x, OUT1.y, OUT1.w, OUT1.h, 2],
  ]);
  await get('overlay').evaluate(() => localStorage.setItem('aeval.hasSentFirstRequest', '1'));
  await openBar(get('overlay'));
  await streamAnswer(get('out1'), 'Why does the build fail?', ANSWER);
  await settle(page, 900);
  await shot(page, '10-white-bar-output', { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: OUT1.y + OUT1.h });
  await page.close();
}

for (const e of errors) console.log(`[UI] pageerror ${e}`);
check('noPageErrors', errors.length === 0, `${errors.length} page errors`);
await browser.close();
server.close();
console.log(`[UI] done failed=${failed}`);
process.exit(failed ? 1 : 0);
