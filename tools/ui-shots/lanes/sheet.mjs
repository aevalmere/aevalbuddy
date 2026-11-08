// The sheet lane's ui-shots scenes, mocks, checks and CSS lint list. The
// shape is documented at the top of tools/ui-shots/shots.mjs; shared fixtures
// are in tools/ui-shots/kit.mjs.
//
// The answer window: the glob out of the bar, answers set as a document,
// streaming, the usage line, errors with their fixes, Start over, the Find
// view, the agent trail and its confirm card (lanes/sheet.md, design-visual
// §4 states 14-40).
import vm from 'node:vm';
import { ANSWER, BAR, OUT1, W, H, barRect, bottomOf, openBar, streamAnswer } from '../kit.mjs';

export const mocks = {};
export const cssLintEnforce = [];

const win = (name, src, r, z, extra) => ({ name, src, x: r.x, y: r.y, w: r.w, h: r.h, z, ...(extra || {}) });
const OUT = '/renderer/output.html';
const outWin = (name, r, extra) => win(name, OUT, r, 2, extra);

const Q = 'Why does the build fail?';
const USAGE = { model: 'google/gemma-3-27b-it:free', provider: 'openrouter', ttftMs: 900, ms: 3200, prompt: 1100, completion: 240, cost: 0 };

// Rust's events for one question and its whole answer, in order.
async function ask(f, opts) {
  const o = { gen: 1, q: Q, a: ANSWER, usage: USAGE, done: {}, ...(opts || {}) };
  await f.evaluate((p) => {
    window.__emit('overlay:state', { mode: 'panel', chatMax: 640 });
    window.__emit('chat:user', { gen: p.gen, text: p.q });
    window.__emit('query:start', { gen: p.gen });
    window.__emit('query:delta', { gen: p.gen, text: p.a });
    window.__emit('query:done', { gen: p.gen, usage: p.usage, question: p.q, ...p.done });
  }, o);
}

// Events in order, in one window.
const emitAll = (f, list) =>
  f.evaluate((l) => {
    for (const [c, p] of l) window.__emit(c, p);
  }, list);

// A window's frame plus a margin, in scene pixels, as the mock last sized it.
const clipOf = (t, name, pad = 20) =>
  t.page.evaluate(
    ({ n, p, w, h }) => {
      const r = document.querySelector(`iframe[name="${n}"]`).getBoundingClientRect();
      const x = Math.max(0, Math.floor(r.left - p));
      const y = Math.max(0, Math.floor(r.top - p));
      return { x, y, width: Math.min(w, Math.ceil(r.right + p)) - x, height: Math.min(h, Math.ceil(r.bottom + p)) - y };
    },
    { n: name, p: pad, w: W, h: H }
  );

// Everything a window says aloud: relayed through Rust (a11y:announce) when
// it does not have focus, else into its own live regions.
async function listenAloud(f) {
  await f.evaluate(() => {
    window.__heard = [];
    for (const [id, priority] of [
      ['announce', 'polite'],
      ['announce-alert', 'assertive'],
    ]) {
      const el = document.getElementById(id);
      if (!el) continue;
      new MutationObserver(() => {
        const text = el.textContent.trim();
        if (text) window.__heard.push({ text, priority });
      }).observe(el, { childList: true, characterData: true, subtree: true });
    }
  });
}
async function heard(t, name) {
  const relayed = (await t.calls(name, 'a11y:announce')).map((c) => c.payload || {});
  const here = await t.frame(name).evaluate(() => window.__heard || []);
  return [...relayed, ...here];
}

// Playwright leaves time running after clock.install, so a clock scene stops
// it first: from there only t.tick and t.settle move it. The clock belongs to
// the browser context and every later page replays its calls, so the scene
// starts it again on the way out; a stopped clock would freeze every scene
// after this one.
const onStoppedClock = (run) => async (t) => {
  const now = await t.page.evaluate(() => Date.now());
  await t.page.clock.pauseAt(now + 1000);
  try {
    await run(t);
  } finally {
    await t.page.clock.resume();
  }
};

const key = (f, k, mods) =>
  f.evaluate(
    ({ k: kk, m }) => {
      const e = new KeyboardEvent('keydown', { key: kk, bubbles: true, cancelable: true, ctrlKey: !!m.ctrl, shiftKey: !!m.shift, altKey: false });
      (document.activeElement || document.body).dispatchEvent(e);
    },
    { k, m: mods || {} }
  );

const last = (list) => list[list.length - 1];
const payloadOf = (list) => (list.length ? last(list).payload || {} : null);

// ------------------------------------------------------------- fixtures

// design-visual §4 state 20: every block markdown.js sets.
const FIXTURE = [
  '# Heading one',
  'A paragraph whose lines were wrapped by the model',
  'and join into one paragraph.',
  '',
  '## Heading two',
  '1. Open the project folder.',
  '2. Run the installer.',
  '',
  '3. A third step after a blank line, still the same list:',
  '   - a nested point',
  '   - another nested point',
  '4. The fourth step.',
  '',
  'Then, later:',
  '',
  '7. A list that starts at seven',
  '8. and goes on.',
  '',
  '- a tight list',
  '- of two items',
  '',
  '### Heading three',
  '| Name | Count | Note |',
  '|:-----|------:|:----:|',
  '| Apples | 12 | fresh |',
  '| Pears | 3 | `ripe` |',
  '',
  '```js',
  "const longLine = 'a very long line of code that runs past the measure and scrolls sideways in its plate';",
  'console.log(longLine);',
  '```',
  '',
  'Inline code by punctuation: `npm run build`, then `cargo test`.',
  '',
  '> A quote from the documentation, set apart.',
  '',
  '---',
  '',
  'Read [the Rust book](https://doc.rust-lang.org/book/) for more.',
].join('\n');

// Untrusted answer text that must stay text.
const HOSTILE = [
  '<img src=x onerror="window.__pwned=1">',
  '<script>window.__pwned=2</script>',
  '[click](javascript:window.__pwned=3)',
  '[data](data:text/html,<b>x</b>)',
  '<a href="https://evil.example" onclick="window.__pwned=4">x</a>',
  '`<b>code</b>` and **<i>bold</i>**',
].join('\n\n');

const LONG_TYPED = Array.from({ length: 24 }, (_, i) => `Line ${i + 1} of the message that will be typed into Slack, word for word.`).join('\n');

const PLAN = ['Open Downloads in File Explorer', 'Select the screenshots', 'Rename each to its date'];

const PROVIDERS = {
  live: true,
  current: 'google/gemma-3-27b-it:free',
  groups: [{ label: 'Free', models: [{ id: 'google/gemma-3-27b-it:free', name: 'Gemma 3 27B', free: true }] }],
  providers: [
    {
      id: 'openrouter',
      name: 'OpenRouter',
      ready: true,
      models: [
        { id: 'google/gemma-3-27b-it:free', name: 'Gemma 3 27B', free: true },
        { id: 'mistralai/mistral-small-3.1-24b-instruct:free', name: 'Mistral Small 3.1', free: true },
      ],
    },
    { id: 'anthropic', name: 'Anthropic', ready: true, models: [{ id: 'claude-3-5-haiku-latest', name: 'Claude 3.5 Haiku' }] },
  ],
};

// ---------------------------------------------------------- glob helpers

// The bar's glass rect in a window's own coordinates, from its screen rect.
const localBar = (out, rect) =>
  out.evaluate((r) => ({ left: r.x - window.screenX, top: r.y - window.screenY, right: r.x - window.screenX + r.width, bottom: r.y - window.screenY + r.height }), rect);

// The shape the glob drew this frame, from its rim's fill path, as polygons
// in the window's coordinates.
const drawnShape = (out) =>
  out.evaluate(() => {
    const host = document.getElementById('glob');
    if (host.hidden) return [];
    const d = document.getElementById('glob-fill').getAttribute('d') || '';
    return d
      .split('Z')
      .map((s) => s.trim())
      .filter(Boolean)
      .map((s) =>
        s
          .replace(/^M/, '')
          .split('L')
          .map((p) => p.trim().split(/\s+/).map(Number))
      );
  });

// F1: outside the glob's shape (and the shadow it casts, and the bar) the
// window shows only the wallpaper, so no word or glass escapes the shape.
async function outsideIsWallpaper(t, outName, barGlass) {
  const out = t.frame(outName);
  const polys = await drawnShape(out);
  const frame = await t.page.evaluate((n) => {
    const r = document.querySelector(`iframe[name="${n}"]`).getBoundingClientRect();
    return { x: r.left, y: r.top, w: r.width, h: r.height };
  }, outName);
  const clip = { x: Math.floor(frame.x), y: Math.floor(frame.y), width: Math.floor(frame.w), height: Math.floor(Math.min(frame.h, H - frame.y)) };
  const png = await t.page.screenshot({ clip });
  return t.page.evaluate(
    async ({ shot, wall, clip: c, scale, polys: ps, frame: fr, bar }) => {
      const load = (src) =>
        new Promise((res, rej) => {
          const im = new Image();
          im.onload = () => res(im);
          im.onerror = () => rej(new Error('image did not load'));
          im.src = src;
        });
      if (!window.__wallImg) window.__wallImg = await load(wall);
      const a = await load(shot);
      const w = a.naturalWidth;
      const h = a.naturalHeight;
      const ctx = (img, dx, dy) => {
        const cv = document.createElement('canvas');
        cv.width = w;
        cv.height = h;
        const x = cv.getContext('2d', { willReadFrequently: true });
        x.drawImage(img, dx, dy);
        return x.getImageData(0, 0, w, h).data;
      };
      const da = ctx(a, 0, 0);
      const db = ctx(window.__wallImg, -c.x * scale, -c.y * scale);
      const P = ps.map((p) => p.map(([x, y]) => [x + fr.x, y + fr.y]));
      const inside = (x, y) =>
        P.some((p) => {
          let on = false;
          for (let i = 0, j = p.length - 1; i < p.length; j = i, i += 1) {
            const [xi, yi] = p[i];
            const [xj, yj] = p[j];
            if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) on = !on;
          }
          return on;
        });
      const near = (x, y, m) => {
        for (const p of P) {
          for (let i = 0, j = p.length - 1; i < p.length; j = i, i += 1) {
            const [x1, y1] = p[j];
            const [x2, y2] = p[i];
            const dx = x2 - x1;
            const dy = y2 - y1;
            const l = dx * dx + dy * dy || 1;
            const k = Math.max(0, Math.min(1, ((x - x1) * dx + (y - y1) * dy) / l));
            if (Math.hypot(x - (x1 + k * dx), y - (y1 + k * dy)) < m) return true;
          }
        }
        return false;
      };
      let n = 0;
      let bad = 0;
      let worst = null;
      for (let y = c.y + 2; y < c.y + c.height - 2; y += 7) {
        for (let x = c.x + 2; x < c.x + c.width - 2; x += 7) {
          if (x > bar.left - 40 && x < bar.right + 40 && y > bar.top - 40 && y < bar.bottom + 44) continue;
          if (inside(x, y) || near(x, y, 30)) continue;
          const i = (Math.floor((y - c.y) * scale) * w + Math.floor((x - c.x) * scale)) * 4;
          const d = Math.max(Math.abs(da[i] - db[i]), Math.abs(da[i + 1] - db[i + 1]), Math.abs(da[i + 2] - db[i + 2]));
          n += 1;
          if (d > 12) {
            bad += 1;
            if (!worst || d > worst[2]) worst = [Math.round(x), Math.round(y), d];
          }
        }
      }
      return { n, bad, worst };
    },
    { shot: `data:image/png;base64,${png.toString('base64')}`, wall: t.backdrop.full, clip, scale: t.scale, polys, frame, bar: barGlass }
  );
}

// The bar's glass in scene pixels.
const barGlassOf = (t) =>
  t.frame('overlay').evaluate(() => {
    const r = document.getElementById('bar').getBoundingClientRect();
    return { left: window.screenX + r.left, top: window.screenY + r.top, right: window.screenX + r.right, bottom: window.screenY + r.bottom };
  });

// A scene with the bar and one output window, the answer in it and the bar's
// rect where Rust would answer output:bar.
async function barAndAnswer(t, outName = 'out1', meta) {
  const bar = t.frame('overlay');
  const out = t.frame(outName);
  await bar.evaluate(() => localStorage.setItem('aeval.hasSentFirstRequest', '1'));
  await openBar(bar);
  await t.settle(300);
  const rect = await barRect(bar);
  await out.evaluate((r) => {
    window.__barRect = r;
  }, rect);
  if (meta) await emitAll(out, [['output:meta', meta]]);
  await streamAnswer(out, Q, ANSWER);
  await t.settle(1000);
  return { bar, out, rect, local: await localBar(out, rect) };
}

// B-145's three rules, held to the sheet's regions of overlay.css.
function lintSheetCss(text) {
  const lines = text.split(/\r?\n/);
  const at = (name) => lines.findIndex((l) => l.startsWith('/* ------') && new RegExp(`-+ ${name}(\\W|$)`).test(l));
  const regions = [
    [at('answer sheet'), at('layout editor')],
    [at('find'), at('reduced motion')],
  ];
  if (regions.flat().some((n) => n < 0)) return ['a region banner in overlay.css has moved'];
  const bad = [];
  let depth = 0;
  let root = false;
  let inComment = false;
  lines.forEach((raw, i) => {
    let code = '';
    let rest = raw;
    while (rest) {
      if (inComment) {
        const e = rest.indexOf('*/');
        if (e < 0) break;
        inComment = false;
        rest = rest.slice(e + 2);
      } else {
        const b = rest.indexOf('/*');
        if (b < 0) {
          code += rest;
          break;
        }
        code += rest.slice(0, b);
        inComment = true;
        rest = rest.slice(b + 2);
      }
    }
    const opens = (code.match(/\{/g) || []).length;
    const closes = (code.match(/\}/g) || []).length;
    if (depth === 0 && opens && /^\s*:root\b/.test(code)) root = true;
    if (!root && regions.some(([a, b]) => i > a && i < b)) {
      const n = i + 1;
      if (/border-radius\s*:[^;]*\b(4|5|8|10|12|14|18|20)px/.test(code)) bad.push(`${n} radius`);
      const sp = code.match(/^\s*(padding[\w-]*|gap|row-gap|column-gap)\s*:\s*([^;]*)/);
      if (sp) {
        for (const m of sp[2].replace(/var\([^)]*\)/g, '').matchAll(/(\d+(?:\.\d+)?)px/g)) {
          const v = Number(m[1]);
          if (v % 4 === 0) continue;
          if (v === 2 && /\/\*\s*(hairline|optical)\b/.test(raw)) continue;
          bad.push(`${n} grid ${m[0]}`);
        }
      }
      if (/rgba?\(|hsla?\(|#[0-9a-fA-F]{3,8}\b/.test(code)) bad.push(`${n} colour`);
    }
    depth += opens - closes;
    if (depth <= 0) {
      depth = 0;
      root = false;
    }
  });
  return bad;
}

// ------------------------------------------------------------ file checks

export const checks = [
  // B-183: overlay.css is shared with the bar, so it cannot join
  // cssLintEnforce whole until B-148; the sheet's regions are held to B-145's
  // three rules here.
  {
    name: 'sheetCssLint',
    run: (ctx) => {
      const bad = lintSheetCss(ctx.read('renderer/overlay.css'));
      return bad.length ? `${bad.length} violations: ${bad.slice(0, 8).join(', ')}` : true;
    },
  },

  // B-063, and the hard rule: answer text never becomes markup, script or a
  // link that leaves http(s). markdown.js runs under node with no DOM.
  {
    name: 'markdownNeverInjects',
    run: async (ctx) => {
      const window = {};
      vm.runInNewContext(ctx.read('renderer/markdown.js'), { window, URL });
      const html = window.AevalMarkdown.render(HOSTILE);
      const tags = [...html.matchAll(/<([a-z][a-z0-9]*)\b/gi)].map((m) => m[1].toLowerCase());
      const allowed = new Set(['p', 'a', 'span', 'code', 'strong', 'em', 'br', 'h2', 'h3', 'h4', 'ul', 'ol', 'li', 'div', 'pre', 'button', 'table', 'thead', 'tbody', 'tr', 'th', 'td', 'blockquote', 'hr']);
      const stray = tags.filter((x) => !allowed.has(x));
      const hrefs = [...html.matchAll(/\bhref="([^"]*)"/g)].map((m) => m[1]);
      const badHref = hrefs.filter((h) => !/^https?:\/\//.test(h));
      // Inside real tags only: escaped text may well spell "onerror=".
      const handlers = [...html.matchAll(/<[a-z][^>]*>/gi)].some((m) => /\son[a-z]+\s*=/i.test(m[0]));
      const raw = /<(img|script|i|b)\b/i.test(html);
      if (stray.length || badHref.length || handlers || raw) return `tags ${stray.join(',')} hrefs ${badHref.join(',')} handlers ${handlers} raw ${raw}`;
      return true;
    },
  },
];

export const scenes = [
  // 03, 04: one output window under the bar, and its glob frames
  // (REQUESTS.md 7, 8, 9, 11).
  {
    name: 'output',
    backdrop: 'wallpaper',
    windows: [win('overlay', '/renderer/index.html', BAR, 1), win('out1', '/renderer/output.html', OUT1, 2)],
    async run(t) {
      const { page } = t;
      const bar = t.frame('overlay');
      const out = t.frame('out1');
      await bar.evaluate(() => localStorage.setItem('aeval.hasSentFirstRequest', '1'));
      await openBar(bar);
      await t.settle(300);
      const rect = await barRect(bar);
      await out.evaluate((r) => {
        // Rust's answer to output:bar for this window.
        window.__barRect = r;
      }, rect);
      await streamAnswer(out, 'Why does the build fail?', ANSWER);
      await t.settle(900);
      await t.shot('03-output-answer', { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: await bottomOf(page, 'out1') });
      t.check('noOutputInput', await out.evaluate(() => !document.querySelector('#opanel input, #opanel textarea')), 'an input exists in the output window');
      t.check('outputTitleBar', await out.evaluate(() => !!document.querySelector('#sheet-bar[data-drag]') && document.getElementById('sheet-title').textContent.includes('build')), 'no draggable title bar with the question');
      t.check('outputControls', await out.evaluate(() => ['copy-all', 'history-btn', 'new-chat', 'close-out'].every((id) => !!document.getElementById(id))), 'missing a title-bar control');
      t.check('noDockUi', await out.evaluate(() => !document.getElementById('dock-toggle') && !document.getElementById('hud')), 'dock or answer ghost still in the output page');
      t.check('resizeZones', await out.evaluate(() => document.querySelectorAll('.frame-handle[data-hit]').length === 8 && [...document.querySelectorAll('.frame-handle')].every((h) => h.style.display === 'block')), 'eight resize zones not laid out');
      // Glob frames: the shape at four moments, drawn from the bar's real rect.
      const local = await out.evaluate((r) => ({ left: r.x - window.screenX, top: r.y - window.screenY, right: r.x - window.screenX + r.width, bottom: r.y - window.screenY + r.height }), rect);
      // Timing is measured at scale 1 only, as it always was: the 1.5 run is
      // for the pictures.
      if (t.scale === 1) {
        const timing = await out.evaluate((b) => {
          const sheet = document.getElementById('opanel');
          const g = window.AevalGlob.geometry(b, sheet.getBoundingClientRect());
          const t0 = performance.now();
          for (let i = 0; i < 600; i += 1) window.AevalGlob.shapeAt(g, (i % 60) / 60);
          return (performance.now() - t0) / 600;
        }, local);
        t.check('globFrameCost', timing < 1.5, `${timing.toFixed(3)} ms per shape`);
        console.log(`[UI] globShapeMs=${timing.toFixed(3)}`);
      }
      const steps = [['a', 0.08], ['b', 0.3], ['c', 0.52], ['d', 0.7], ['e', 0.9]];
      for (const [k, at] of steps) {
        await out.evaluate(({ b, t: tt }) => window.AevalGlob.preview(document.getElementById('opanel'), b, tt), { b: local, t: at });
        await page.waitForTimeout(160);
        await t.shot(`04${k}-glob-${String(Math.round(at * 100)).padStart(2, '0')}`, { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: OUT1.y + OUT1.h });
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
      console.log(`[UI] globOpen${t.suffix} ms=${ran.ms.toFixed(0)} frames=${ran.frames} longestFrameMs=${ran.longest.toFixed(1)}`);
      t.check('globOpens', ran.clean && ran.ms < 900, JSON.stringify(ran));
      if (t.scale === 1) t.check('globSmooth', ran.longest < 34, `longest frame ${ran.longest.toFixed(1)} ms`);
      // Reduced motion: a fade, no glob.
      const rm = await out.evaluate(async () => {
        document.documentElement.dataset.motion = 'reduced';
        const sheet = document.getElementById('opanel');
        await window.AevalGlob.open(sheet);
        const r = document.getElementById('glob').hidden;
        document.documentElement.dataset.motion = 'full';
        return r;
      });
      t.check('globReducedMotion', rm, 'the glob ran under reduced motion');
    },
  },

  // 05: several outputs at once, the bar answering a task's question
  // (REQUESTS.md 9, 11).
  {
    name: 'multi',
    backdrop: 'wallpaper',
    windows: [
      win('overlay', '/renderer/index.html', BAR, 4),
      win('o1', '/renderer/output.html', { x: 140, y: 230, w: 500, h: 560 }, 1),
      win('o2', '/renderer/output.html', { x: 690, y: 300, w: 440, h: 420 }, 2),
      win('o3', '/renderer/output.html', { x: 1150, y: 120, w: 420, h: 520 }, 3),
    ],
    async run(t) {
      const bar = t.frame('overlay');
      await bar.evaluate(() => localStorage.setItem('aeval.hasSentFirstRequest', '1'));
      await openBar(bar, { pillState: 'agent', agent: { running: true, step: 3, state: 'asking' } });
      await streamAnswer(t.frame('o1'), 'Why does the build fail?', ANSWER);
      await t.frame('o2').evaluate(() => {
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
      await t.frame('o3').evaluate(() => {
        window.__emit('overlay:state', { mode: 'panel', chatMax: 520 });
        window.__emit('agent:begin', { task: 'Rename the screenshots in Downloads by date' });
        window.__emit('agent:step', { id: 1, kind: 'plan', state: 'done', steps: ['Open Downloads', 'Sort screenshots by date', 'Rename each to YYYY-MM-DD'] });
        window.__emit('agent:step', { id: 2, kind: 'act', state: 'done', text: 'Opened File Explorer at Downloads', ms: 1800 });
        window.__emit('agent:step', { id: 3, kind: 'act', state: 'done', text: 'Selected 14 screenshots' });
        window.__emit('agent:step', { id: 4, kind: 'ask', state: 'running', text: 'Keep the time of day in the names?' });
        window.__emit('agent:state', { state: 'asking', step: 3 });
      });
      await t.settle(900);
      await t.shot('05-outputs-multi', { x: 0, y: 0, width: W, height: H });
      const ph = await bar.evaluate(() => document.getElementById('query-input').placeholder);
      t.check('barAnswersTask', /question/i.test(ph), `placeholder "${ph}"`);
      await bar.evaluate(() => {
        const i = document.getElementById('query-input');
        i.value = 'No, just the date';
        i.dispatchEvent(new Event('input'));
        i.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
      });
      const cont = await bar.evaluate(() => window.__calls.find((c) => c.channel === 'agent:continue'));
      t.check('askUserRoutedFromBar', !!cont && cont.payload && cont.payload.text === 'No, just the date', JSON.stringify(cont));
      const inputs = await Promise.all(['o1', 'o2', 'o3'].map((n) => t.frame(n).evaluate(() => document.querySelectorAll('input, textarea').length)));
      t.check('noInputInAnyOutput', inputs.every((n) => n === 0), 'an output has an input');
    },
  },

  // 10: the bar and an answer over a white document, where contrast is
  // hardest.
  {
    name: 'white',
    backdrop: 'white',
    windows: [win('overlay', '/renderer/index.html', BAR, 1), win('out1', '/renderer/output.html', OUT1, 2)],
    async run(t) {
      await t.frame('overlay').evaluate(() => localStorage.setItem('aeval.hasSentFirstRequest', '1'));
      await openBar(t.frame('overlay'));
      await streamAnswer(t.frame('out1'), 'Why does the build fail?', ANSWER);
      await t.settle(900);
      await t.shot('10-white-bar-output', { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: await bottomOf(t.page, 'out1') });
    },
  },

  // ------------------------------------------------------------ the glob

  // B-061, B-062 (§4 state 36): the open frame by frame, each the frame the
  // page draws at that point of the timeline, and the real open on the
  // virtual clock.
  {
    name: 'glob-open',
    backdrop: 'wallpaper',
    clock: true,
    windows: [win('overlay', '/renderer/index.html', BAR, 1), outWin('out1', OUT1)],
    run: onStoppedClock(async (t) => {
      const { out, local } = await barAndAnswer(t);
      const glass = await barGlassOf(t);
      const clip = { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: OUT1.y + OUT1.h };
      const bud = await out.evaluate((b) => {
        const sheet = document.getElementById('opanel').getBoundingClientRect();
        const g = window.AevalGlob.geometry(b, sheet);
        const area = (p) => Math.abs(p.reduce((s, q, i) => s + q[0] * p[(i + 1) % p.length][1] - p[(i + 1) % p.length][0] * q[1], 0)) / 2;
        const shape = window.AevalGlob.polysAt(g, 0.04).reduce((s, p) => s + area(p), 0);
        return shape / (sheet.width * sheet.height);
      }, local);
      t.check('globBudArea', bud < 0.08, `the shape at t=0.04 is ${(bud * 100).toFixed(1)}% of the sheet`);
      const leaks = [];
      let sampled = 0;
      for (const pct of [0, 4, 10, 18, 30, 45, 62, 72, 82, 90, 100]) {
        await out.evaluate(({ b, at }) => window.AevalGlob.preview(document.getElementById('opanel'), b, at), { b: local, at: pct / 100 });
        await t.shot(`glob-open-${pct}`, clip);
        const r = await outsideIsWallpaper(t, 'out1', glass);
        sampled += r.n;
        if (r.bad) leaks.push(`${pct}%: ${r.bad} of ${r.n} at ${JSON.stringify(r.worst)}`);
      }
      t.check('globWordsNeverOutsideShape', !leaks.length && sampled > 0, leaks.join('; ') || 'nothing sampled');
      // The real open, on the virtual clock: 480 ms (F2), then the sheet is
      // handed back.
      await out.evaluate(() => {
        window.__globDone = false;
        window.AevalGlob.open(document.getElementById('opanel')).then(() => {
          window.__globDone = true;
        });
      });
      await t.tick(470);
      const early = await out.evaluate(() => window.__globDone);
      await t.tick(60);
      const done = await out.evaluate(() => ({
        done: window.__globDone,
        clean: !document.getElementById('opanel').classList.contains('globbing') && document.getElementById('glob').hidden,
        clip: document.getElementById('opanel').style.clipPath,
      }));
      t.check('globOpenOnClock', !early && done.done && done.clean && !done.clip, `done at 470 ms=${early}; at 530 ms ${JSON.stringify(done)}`);
    }),
  },

  // B-062 (§4 state 37): the close, frame by frame.
  {
    name: 'glob-close',
    backdrop: 'wallpaper',
    clock: true,
    windows: [win('overlay', '/renderer/index.html', BAR, 1), outWin('out1', OUT1)],
    run: onStoppedClock(async (t) => {
      const { out, local } = await barAndAnswer(t);
      const clip = { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: OUT1.y + OUT1.h };
      for (const pct of [0, 25, 50, 75, 100]) {
        await out.evaluate(({ b, f }) => window.AevalGlob.previewClose(document.getElementById('opanel'), b, f), { b: local, f: pct / 100 });
        await t.shot(`glob-close-${pct}`, clip);
      }
      const polys = await drawnShape(out);
      const area = polys.reduce((s, p) => s + Math.abs(p.reduce((a, q, i) => a + q[0] * p[(i + 1) % p.length][1] - p[(i + 1) % p.length][0] * q[1], 0)) / 2, 0);
      const sheet = await out.evaluate(() => {
        const r = document.getElementById('opanel').getBoundingClientRect();
        return r.width * r.height;
      });
      t.check('globCloseEndsInBar', polys.length > 0 && area / sheet < 0.03, `the last close frame is ${((area / sheet) * 100).toFixed(1)}% of the sheet`);
    }),
  },

  // B-062 (§4 state 38): a sheet above the bar grows up out of its top edge.
  {
    name: 'glob-above-bar',
    backdrop: 'wallpaper',
    windows: [win('overlay', '/renderer/index.html', { x: 480, y: 700, w: 680, h: 240 }, 1), outWin('out1', { x: 560, y: 208, w: 520, h: 520 }, { fixed: true })],
    async run(t) {
      const { out, local } = await barAndAnswer(t, 'out1', { grow: 'up', front: true, target: false });
      await out.evaluate(({ b }) => window.AevalGlob.preview(document.getElementById('opanel'), b, 0.3), { b: local });
      await t.shot('glob-above-bar', { x: 460, y: 180, width: 720, height: 620 });
      const side = await out.evaluate((b) => window.AevalGlob.geometry(b, document.getElementById('opanel').getBoundingClientRect()).side, local);
      t.check('globGrowsUpFromBar', side === 'bottom', `side ${side}`);
    },
  },

  // B-062 (§4 state 39): a sheet beside the bar's end.
  {
    name: 'glob-side',
    backdrop: 'wallpaper',
    windows: [win('overlay', '/renderer/index.html', { x: 20, y: 300, w: 680, h: 240 }, 1), outWin('out1', { x: 668, y: 250, w: 520, h: 520 }, { fixed: true })],
    async run(t) {
      const { out, local } = await barAndAnswer(t);
      const g = await out.evaluate((b) => {
        const r = window.AevalGlob.geometry(b, document.getElementById('opanel').getBoundingClientRect());
        return { side: r.side, beside: r.beside };
      }, local);
      await out.evaluate(({ b }) => window.AevalGlob.preview(document.getElementById('opanel'), b, 0.3), { b: local });
      await t.shot('glob-side', { x: 0, y: 230, width: 1220, height: 560 });
      t.check('globFromBarSide', g.side === 'left' && g.beside, JSON.stringify(g));
    },
  },

  // B-062 (§4 state 40, F25): a cascaded sheet away from the bar scales in
  // from the bar's side instead of drawing a neck across the desktop.
  {
    name: 'glob-cascaded',
    backdrop: 'wallpaper',
    windows: [win('overlay', '/renderer/index.html', BAR, 1), outWin('out1', { x: 760, y: 320, w: 520, h: 520 }, { fixed: true })],
    async run(t) {
      const { out, local } = await barAndAnswer(t);
      const state = await out.evaluate((b) => {
        const sheet = document.getElementById('opanel');
        window.AevalGlob.preview(sheet, b, 0.5);
        return { beside: window.AevalGlob.geometry(b, sheet.getBoundingClientRect()).beside, glob: !document.getElementById('glob').hidden, transform: sheet.style.transform };
      }, local);
      await t.shot('glob-cascaded', { x: 460, y: 0, width: 840, height: 860 });
      t.check('globFallsBackToScale', !state.beside && !state.glob && /scale\(0\.9[4-9]/.test(state.transform), JSON.stringify(state));
    },
  },

  // B-062: the join between the bar and the drop at 150%, where a seam
  // would show.
  {
    name: 'glob-seam',
    backdrop: 'wallpaper',
    scales: [1.5],
    windows: [win('overlay', '/renderer/index.html', BAR, 1), outWin('out1', OUT1)],
    async run(t) {
      const { out, local } = await barAndAnswer(t);
      await out.evaluate(({ b }) => window.AevalGlob.preview(document.getElementById('opanel'), b, 0.12), { b: local });
      await t.shot('glob-seam', { x: OUT1.x + 120, y: 56, width: 280, height: 90 });
    },
  },

  // ------------------------------------------------------------- answers

  // B-063, B-161 (§4 state 20): every block of the typeset answer.
  {
    name: 'sheet-markdown-fixture',
    backdrop: 'wallpaper',
    windows: [outWin('out1', { x: 560, y: 20, w: 520, h: 520 }, { max: 980 })],
    async run(t) {
      const f = t.frame('out1');
      await ask(f, { q: 'Show me the formatting', a: FIXTURE });
      await t.settle(700);
      await t.shot('sheet-markdown-fixture', await clipOf(t, 'out1'));
      const s = await f.evaluate(() => {
        const body = document.querySelector('.msg.assistant .md-body');
        const ols = [...body.querySelectorAll(':scope > ol')];
        const first = ols[0];
        const items = first ? [...first.children].filter((li) => li.tagName === 'LI') : [];
        const table = body.querySelector('.md-table > table');
        const th = table ? [...table.querySelectorAll('thead th')] : [];
        const rows = table ? table.querySelectorAll('tbody tr').length : 0;
        const link = body.querySelector('a[href="https://doc.rust-lang.org/book/"]');
        const host = link && link.nextElementSibling;
        const h = (sel) => {
          const e = body.querySelector(sel);
          return e ? e.textContent.trim() : null;
        };
        return {
          ol: { lists: ols.length, items: items.length, nested: !!(items[2] && items[2].querySelector('ul')), start7: !!body.querySelector('ol[start="7"]') },
          table: { th: th.map((x) => x.textContent.trim()), align: th.map((x) => x.dataset.align || ''), rows, code: !!(table && table.querySelector('td code')) },
          heads: { h1: h('h2.md-h.md-h1'), h2: h('h3.md-h.md-h2'), h3: h('h4.md-h.md-h3') },
          link: link ? { text: link.textContent, href: link.dataset.href, host: host && host.classList.contains('md-host') ? host.textContent : null } : null,
        };
      });
      t.check('looseListIsOneOl', s.ol.items === 4 && s.ol.nested && s.ol.start7, JSON.stringify(s.ol));
      t.check('tableRenders', s.table.th.join('|') === 'Name|Count|Note' && s.table.rows === 2 && s.table.align.join('|') === 'left|right|center' && s.table.code, JSON.stringify(s.table));
      t.check('headingsAreHeadings', s.heads.h1 === 'Heading one' && s.heads.h2 === 'Heading two' && s.heads.h3 === 'Heading three', JSON.stringify(s.heads));
      t.check('linksShowHost', !!s.link && s.link.text === 'the Rust book' && s.link.host === '(doc.rust-lang.org)', JSON.stringify(s.link));
      // A link click goes to Rust, never navigates.
      await f.evaluate(() => document.querySelector('a[href="https://doc.rust-lang.org/book/"]').click());
      const opened = payloadOf(await t.calls('out1', 'open-external'));
      t.check('linkOpensThroughRust', !!opened && opened.url === 'https://doc.rust-lang.org/book/' && (await f.evaluate(() => location.pathname.endsWith('output.html'))), JSON.stringify(opened));
      // Copy code, in one click, with a check for 1.5 s.
      const code = await f.evaluate(() => {
        const btn = document.querySelector('.md-code .md-copy');
        const text = btn.closest('.md-code').querySelector('pre code').textContent;
        btn.click();
        return { text, label: btn.getAttribute('aria-label') };
      });
      await t.settle(150);
      const copied = payloadOf(await t.calls('out1', 'clipboard:write'));
      const done = await f.evaluate(() => document.querySelector('.md-code .md-copy').hasAttribute('data-done'));
      t.check('codeCopyButtonCopies', !!copied && copied.text === code.text && code.label === 'Copy code' && done, JSON.stringify({ copied, code, done }));
    },
  },

  // B-063: hostile answer text in the real page stays text.
  {
    name: 'sheet-markdown-inert',
    backdrop: 'wallpaper',
    scales: [1],
    windows: [outWin('out1', OUT1)],
    async run(t) {
      const f = t.frame('out1');
      await ask(f, { q: 'Render this', a: `Here it is:\n\n${[
        '<img src=x onerror="window.__pwned=1">',
        '<script>window.__pwned=2</script>',
        '[click](javascript:window.__pwned=3)',
        '<a href="https://evil.example" onclick="window.__pwned=4">x</a>',
      ].join('\n\n')}` });
      await t.settle(500);
      await f.evaluate(() => {
        for (const a of document.querySelectorAll('.md-body a')) a.click();
      });
      const r = await f.evaluate(() => ({
        pwned: window.__pwned || 0,
        img: document.querySelectorAll('.md-body img, .md-body script, .md-body [onerror], .md-body [onclick]').length,
        hrefs: [...document.querySelectorAll('.md-body a')].map((a) => a.getAttribute('href')),
      }));
      const ext = (await t.calls('out1', 'open-external')).map((c) => c.payload && c.payload.url);
      t.check('answerHtmlIsInert', !r.pwned && !r.img && r.hrefs.every((h) => /^https?:\/\//.test(h)) && ext.every((u) => /^https?:\/\//.test(u)), JSON.stringify({ r, ext }));
    },
  },

  // B-160 (§4 state 15): the answer mid-stream; earlier blocks stay put.
  {
    name: 'sheet-mid-stream',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      const parts = ANSWER.split(/(?<=\n)/);
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['chat:user', { gen: 1, text: Q }],
        ['query:start', { gen: 1 }],
        ['query:delta', { gen: 1, text: `${parts[0]}${parts[1]}` }],
      ]);
      await t.settle(200);
      await f.evaluate(() => {
        const first = document.querySelector('.msg.assistant .md-body').firstElementChild;
        window.__first = first;
        const r = document.createRange();
        r.selectNodeContents(first);
        const sel = window.getSelection();
        sel.removeAllRanges();
        sel.addRange(r);
        window.__selected = String(sel);
      });
      // Ten more deltas, a frame or more apart.
      const rest = parts.slice(2).join('');
      const step = Math.ceil(rest.length / 10);
      for (let i = 0; i < 10; i += 1) {
        await f.evaluate((text) => window.__emit('query:delta', { gen: 1, text }), rest.slice(i * step, (i + 1) * step));
        await t.settle(60);
      }
      const mid = await f.evaluate(() => {
        const body = document.querySelector('.msg.assistant .md-body');
        return {
          same: body.firstElementChild === window.__first,
          selection: String(window.getSelection()),
          wanted: window.__selected,
          tail: !!body.querySelector('.md-tail'),
        };
      });
      await t.shot('sheet-mid-stream', await clipOf(t, 'out1'));
      t.check('streamingKeepsEarlierNodes', mid.same && mid.tail, JSON.stringify({ same: mid.same, tail: mid.tail }));
      t.check('selectionSurvivesStream', mid.wanted.length > 20 && mid.selection === mid.wanted, JSON.stringify({ selection: mid.selection.slice(0, 40), wanted: mid.wanted.slice(0, 40) }));
      await emitAll(f, [['query:done', { gen: 1, usage: USAGE, question: Q }]]);
      await t.settle(200);
      const after = await f.evaluate(() => !document.querySelector('.md-tail'));
      t.check('streamTailDotGoesOnDone', after, 'the trailing dot stayed after done');
    },
  },

  // B-066, B-068, B-164, B-065 (copy waits for a second answer).
  {
    name: 'sheet-first-answer',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await listenAloud(f);
      await ask(f);
      await t.settle(700);
      await t.shot('sheet-first-answer', await clipOf(t, 'out1'));
      const one = await f.evaluate(() => {
        const u = document.querySelector('.msg.user');
        const r = u.getBoundingClientRect();
        return {
          isTitle: u.classList.contains('is-title'),
          hidden: r.width <= 1 && r.height <= 1,
          title: document.getElementById('sheet-title').textContent,
          copyAll: document.getElementById('copy-all').hidden,
        };
      });
      t.check('questionNotShownTwice', one.isTitle && one.hidden && one.title === Q, JSON.stringify(one));
      const said = (await heard(t, 'out1')).map((x) => x.text).filter((x) => /^Answer ready\./.test(x));
      t.check('answerAnnounceIsShort', said.length === 1 && said[0].length <= 220 && /Press F6 to read it\.$/.test(said[0]), JSON.stringify(said));
      const glass = payloadOf(await t.calls('out1', 'output:glass'));
      const rect = await f.evaluate(() => {
        const r = document.getElementById('opanel').getBoundingClientRect();
        return { top: Math.round(r.top), height: Math.ceil(r.height) };
      });
      t.check('sheetReportsGlassRect', !!glass && Math.abs(glass.top - rect.top) <= 1 && Math.abs(glass.height - rect.height) <= 2, JSON.stringify({ glass, rect }));
      // A follow-up shows its question as a chip; Copy all appears.
      await ask(f, { gen: 2, q: 'And on Linux?', a: 'On Linux the same error means **libstdc++** is missing.' });
      await t.settle(500);
      const two = await f.evaluate(() => {
        const u = document.querySelectorAll('.msg.user')[1];
        return { shown: u.getBoundingClientRect().height > 10, copyAll: !document.getElementById('copy-all').hidden };
      });
      t.check('copyAllWaitsForTwoAnswers', one.copyAll && two.copyAll && two.shown, JSON.stringify({ one: one.copyAll, two }));
    },
  },

  // B-162: the quiet usage line, then the "What was sent" panel.
  {
    name: 'sheet-done',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      const sent = { provider: 'openrouter', providerName: 'OpenRouter', model: 'google/gemma-3-27b-it:free', modelName: 'Gemma 3 27B', image: 'screen', imageSize: [1920, 1080], history: 2, free: true };
      await ask(f, { done: { sent, msgId: 7 } });
      await t.settle(700);
      await t.shot('sheet-done', await clipOf(t, 'out1'));
      const u = await f.evaluate(() => {
        const e = document.querySelector('.msg.assistant .usage');
        return { text: e.textContent, title: e.title };
      });
      t.check('usageLineIsShort', u.text === 'Gemma 3 27B via OpenRouter · 3.2 s' && /tokens/.test(u.title) && /google\/gemma-3-27b-it:free/.test(u.title), JSON.stringify(u));
      await f.evaluate(() => document.querySelector('.sent-btn').click());
      await t.settle(300);
      await t.shot('sheet-sent-panel', await clipOf(t, 'out1'));
      const p = await f.evaluate(() => {
        const btn = document.querySelector('.sent-btn');
        const panel = document.getElementById(btn.getAttribute('aria-controls'));
        return { expanded: btn.getAttribute('aria-expanded'), shown: !panel.hidden, text: panel.innerText };
      });
      const want = ['Sent to OpenRouter (Gemma 3 27B):', 'Your question', 'A screenshot of the whole screen, 1920 x 1080', '2 earlier messages from this conversation', "Free models' providers may keep and train on what they receive."];
      t.check('sentPanelSaysWhatWasSent', p.expanded === 'true' && p.shown && want.every((w) => p.text.includes(w)), JSON.stringify(p));
    },
  },

  // B-166 (§4 state 18): an error with its fixes.
  {
    name: 'sheet-error-short',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1)],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['chat:user', { gen: 1, text: Q }],
        ['query:start', { gen: 1 }],
        [
          'query:error',
          {
            gen: 1,
            message: 'OpenRouter did not accept the key.',
            code: 'auth',
            provider: 'openrouter',
            question: Q,
            actions: [
              { id: 'fix-key', label: 'Fix key', provider: 'openrouter' },
              { id: 'retry', label: 'Retry' },
            ],
          },
        ],
      ]);
      await t.settle(500);
      await t.shot('sheet-error-short', await clipOf(t, 'out1'));
      const glyph = await f.evaluate(() => {
        const g = document.querySelector('.error-row > .row-glyph');
        return g ? g.classList.contains('i-info') && getComputedStyle(g).backgroundColor : null;
      });
      await f.evaluate(() => [...document.querySelectorAll('.error-row button')].find((b) => b.textContent === 'Fix key').click());
      await f.evaluate(() => document.querySelector('.error-row .retry-split .retry-btn').click());
      await t.settle(100);
      const fix = payloadOf(await t.calls('out1', 'overlay:open-settings'));
      const retry = payloadOf(await t.calls('out1', 'query:retry'));
      t.check('errorActionsInvoke', !!fix && fix.section === 'prov-openrouter' && !!retry && retry.label === 'out1' && !!glyph, JSON.stringify({ fix, retry, glyph }));
    },
  },

  // B-166: a long error puts its fixes under the text.
  {
    name: 'sheet-error-long',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1)],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['chat:user', { gen: 1, text: Q }],
        ['query:start', { gen: 1 }],
        [
          'query:error',
          {
            gen: 1,
            message: 'Gemma 3 27B could not answer: the free endpoint on OpenRouter has used up its daily allowance for this key. Paid models still work, or the free ones come back after midnight UTC.',
            code: 'quota',
            provider: 'openrouter',
            model: 'google/gemma-3-27b-it:free',
            question: Q,
            actions: [
              { id: 'choose-model', label: 'Choose model' },
              { id: 'add-credit', label: 'Add credit', provider: 'openrouter' },
              { id: 'copy-details', label: 'Copy details' },
            ],
          },
        ],
      ]);
      await t.settle(500);
      await t.shot('sheet-error-long', await clipOf(t, 'out1'));
      const long = await f.evaluate(() => document.querySelector('.error-row').classList.contains('long'));
      await f.evaluate(() => {
        const b = [...document.querySelectorAll('.error-row button')];
        b.find((x) => x.textContent === 'Add credit').click();
        b.find((x) => x.textContent === 'Choose model').click();
        b.find((x) => x.textContent === 'Copy details').click();
      });
      await t.settle(100);
      const credit = payloadOf(await t.calls('out1', 'open-external'));
      const models = payloadOf(await t.calls('out1', 'overlay:open-settings'));
      const copied = payloadOf(await t.calls('out1', 'clipboard:write'));
      t.check('longErrorFixesWrap', long && !!credit && /openrouter\.ai\/settings\/credits/.test(credit.url) && !!models && models.section === 'pg-models' && !!copied && /quota/.test(copied.text), JSON.stringify({ long, credit, models, copied }));
    },
  },

  // B-166: busy is a calm note that counts down.
  {
    name: 'sheet-busy-countdown',
    backdrop: 'wallpaper',
    clock: true,
    windows: [outWin('out1', OUT1)],
    run: onStoppedClock(async (t) => {
      const f = t.frame('out1');
      await listenAloud(f);
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['chat:user', { gen: 1, text: Q }],
        ['query:start', { gen: 1 }],
        [
          'query:error',
          {
            gen: 1,
            tone: 'note',
            code: 'busy',
            message: 'Gemma 3 27B is busy on OpenRouter. Trying again in 12 s.',
            retryInMs: 12000,
            provider: 'openrouter',
            question: Q,
            actions: [
              { id: 'try-now', label: 'Try now' },
              { id: 'choose-model', label: 'Choose model' },
            ],
          },
        ],
      ]);
      const before = await f.evaluate(() => document.querySelector('.error-row .error-text').textContent);
      await t.tick(3000);
      await t.shot('sheet-busy-countdown', await clipOf(t, 'out1'));
      const after = await f.evaluate(() => ({
        text: document.querySelector('.error-row .error-text').textContent,
        note: document.querySelector('.error-row').classList.contains('note'),
        clock: !!document.querySelector('.error-row > .row-glyph.i-clock'),
      }));
      const said = (await heard(t, 'out1')).filter((x) => /busy/.test(x.text));
      t.check('busyCountsDown', /12 s/.test(before) && /\b(8|9) s\./.test(after.text) && after.note && after.clock && said.length === 1 && said[0].priority === 'polite', JSON.stringify({ before, after, said }));
    }),
  },

  // B-166 (§3.4): a paid hop asks first.
  {
    name: 'sheet-paid-hop',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1)],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['chat:user', { gen: 1, text: Q }],
        ['query:start', { gen: 1 }],
        [
          'query:error',
          {
            gen: 1,
            tone: 'note',
            code: 'paid-hop',
            message: 'OpenRouter is busy. Claude 3.5 Haiku on Anthropic can answer now, for about $0.002.',
            provider: 'openrouter',
            question: Q,
            actions: [
              { id: 'hop', label: 'Use Claude 3.5 Haiku', provider: 'anthropic', model: 'claude-3-5-haiku-latest' },
              { id: 'never-ask', label: "Don't ask again" },
              { id: 'wait', label: 'Wait for OpenRouter' },
            ],
          },
        ],
      ]);
      await t.settle(400);
      await t.shot('sheet-paid-hop', await clipOf(t, 'out1'));
      const inputs = await f.evaluate(() => document.querySelectorAll('#opanel input, #opanel textarea').length);
      await f.evaluate(() => {
        document.querySelector('.error-row .row-check').click();
        [...document.querySelectorAll('.error-row button')].find((b) => b.textContent === 'Use Claude 3.5 Haiku').click();
      });
      await t.settle(150);
      const failover = payloadOf(await t.calls('out1', 'settings:set-failover'));
      const retry = payloadOf(await t.calls('out1', 'query:retry'));
      t.check('paidHopAsksFirst', !inputs && !!failover && failover.askBeforePaid === false && !!retry && retry.provider === 'anthropic' && retry.model === 'claude-3-5-haiku-latest' && retry.label === 'out1', JSON.stringify({ inputs, failover, retry }));
    },
  },

  // B-172: Retry can pick another model.
  {
    name: 'sheet-retry-menu',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { responses: { 'models:list': PROVIDERS }, max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await ask(f);
      await t.settle(600);
      const label = await f.evaluate(() => {
        const more = document.querySelector('.msg.assistant .retry-more');
        more.click();
        return more.getAttribute('aria-label');
      });
      await t.settle(300);
      await t.shot('sheet-retry-menu', await clipOf(t, 'out1'));
      const menu = await f.evaluate(() => {
        const m = document.querySelector('.retry-menu');
        return m ? { groups: [...m.querySelectorAll('[role="group"]')].map((g) => g.getAttribute('aria-label')), options: m.querySelectorAll('[role="option"]').length, focused: document.activeElement && document.activeElement.getAttribute('role') } : null;
      });
      await f.evaluate(() => [...document.querySelectorAll('.retry-menu [role="option"]')].find((o) => o.textContent.includes('Claude 3.5 Haiku')).click());
      await t.settle(100);
      const retry = payloadOf(await t.calls('out1', 'query:retry'));
      t.check('retryWithModelInvokes', label === 'Retry with another model' && !!menu && menu.groups.join('|') === 'OpenRouter|Anthropic' && menu.focused === 'option' && !!retry && retry.provider === 'anthropic' && retry.model === 'claude-3-5-haiku-latest' && retry.label === 'out1', JSON.stringify({ label, menu, retry }));
    },
  },

  // B-168: Start over keeps the window and offers Undo.
  {
    name: 'sheet-started-over',
    backdrop: 'wallpaper',
    clock: true,
    windows: [outWin('out1', OUT1, { max: 900 })],
    run: onStoppedClock(async (t) => {
      const f = t.frame('out1');
      await ask(f);
      await t.settle(600);
      const btn = await f.evaluate(() => {
        const b = document.getElementById('new-chat');
        return { label: b.getAttribute('aria-label'), title: b.title };
      });
      await emitAll(f, [['chat:reset', { undoMs: 10000 }]]);
      await t.settle(300);
      await t.shot('sheet-started-over', await clipOf(t, 'out1'));
      const row = await f.evaluate(() => {
        const r = document.querySelector('.reset-row');
        return r ? { text: r.querySelector('.error-text').textContent, undo: !!r.querySelector('button'), shown: !document.getElementById('opanel').hidden } : null;
      });
      await f.evaluate(() => document.querySelector('.reset-row button').click());
      await t.settle(100);
      const undo = payloadOf(await t.calls('out1', 'output:undo-reset'));
      // Rust says no (something new was asked): the row says where it went.
      await emitAll(f, [['chat:reset', { undoMs: 10000 }]]);
      await t.respond('out1', 'output:undo-reset', { ok: false });
      await f.evaluate(() => document.querySelector('.reset-row button').click());
      await t.settle(100);
      const refused = await f.evaluate(() => document.querySelector('.reset-row .error-text').textContent);
      await t.tick(10100);
      const empty = await f.evaluate(() => {
        const e = document.querySelector('.empty-sheet');
        return e ? e.textContent : null;
      });
      t.check(
        'startOverOffersUndo',
        btn.label === 'Start over in this window' && btn.title === 'Start over' && !!row && row.text === 'Started over. The last conversation is in History.' && row.undo && row.shown && !!undo && undo.label === 'out1' && refused === "Couldn't bring it back here. It is in History." && empty === 'Your next question goes here.',
        JSON.stringify({ btn, row, undo, refused, empty })
      );
    }),
  },

  // B-065 (§4 state 22): the title bar at rest and under the pointer.
  {
    name: 'sheet-title',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await ask(f);
      await t.settle(600);
      await t.page.mouse.move(10, 990);
      await t.settle(300);
      const rest = await f.evaluate(() => ['copy-all', 'history-btn', 'new-chat', 'close-out'].map((id) => {
        const b = document.getElementById(id);
        return b.hidden ? 'hidden' : getComputedStyle(b).opacity;
      }));
      await t.shot('sheet-title-rest', { x: OUT1.x, y: OUT1.y, width: OUT1.w, height: 120 });
      await t.page.mouse.move(OUT1.x + 200, OUT1.y + 20 + 22);
      await t.settle(400);
      const hover = await f.evaluate(() => ['history-btn', 'new-chat', 'close-out'].map((id) => getComputedStyle(document.getElementById(id)).opacity));
      await t.shot('sheet-title-hover', { x: OUT1.x, y: OUT1.y, width: OUT1.w, height: 120 });
      t.check('titleToolsWhenWanted', rest[0] === 'hidden' && rest[1] === '0' && rest[2] === '0' && rest[3] === '1' && hover.every((o) => o === '1'), JSON.stringify({ rest, hover }));
      const bar = await f.evaluate(() => {
        const b = document.getElementById('sheet-bar');
        const k = document.getElementById('sheet-kind');
        const title = getComputedStyle(document.getElementById('sheet-title'));
        return {
          shared: b.classList.contains('sheet-bar') && b.classList.contains('interactive') && b.hasAttribute('data-drag'),
          height: Math.round(b.getBoundingClientRect().height),
          kind: k.classList.contains('ico') && k.classList.contains('i-bubble') && k.getAttribute('aria-label') === 'Answer' && Math.round(k.getBoundingClientRect().width) === 16,
          font: `${title.fontWeight} ${title.fontSize}`,
          glyphs: [...b.querySelectorAll('button .ico')].length,
          svgs: b.querySelectorAll('svg').length,
        };
      });
      t.check('outputTitleBarIsSheetBar', bar.shared && bar.height === 44 && bar.kind && bar.font === '600 13px' && bar.glyphs === 4 && bar.svgs === 0, JSON.stringify(bar));
    },
  },

  // B-163 (§4 state 22): front and back outputs, and the target's rim.
  {
    name: 'sheet-front-back',
    backdrop: 'wallpaper',
    windows: [outWin('o1', { x: 300, y: 120, w: 520, h: 420 }, { fixed: true }), outWin('o2', { x: 700, y: 260, w: 520, h: 420 }, { fixed: true })],
    async run(t) {
      const a = t.frame('o1');
      const b = t.frame('o2');
      await ask(a);
      await ask(b, { q: 'Summarise the quarterly numbers', a: 'Revenue grew **12%** quarter on quarter, driven by services.' });
      await emitAll(a, [['output:meta', { grow: 'down', front: false, target: false }]]);
      await emitAll(b, [['output:meta', { grow: 'down', front: true, target: false }]]);
      await t.settle(700);
      await t.shot('sheet-front-back', { x: 280, y: 100, width: 960, height: 600 });
      const looks = await Promise.all([a, b].map((f) => f.evaluate(() => ({ front: document.body.dataset.front, title: getComputedStyle(document.getElementById('sheet-title')).color }))));
      t.check('frontBackDiffer', looks[0].front === '0' && looks[1].front === '1' && looks[0].title !== looks[1].title, JSON.stringify(looks));
      await emitAll(b, [['output:meta', { target: true }]]);
      await t.settle(400);
      await t.shot('sheet-target-rim', { x: 680, y: 240, width: 560, height: 460 });
      const target = await b.evaluate(() => document.body.dataset.target);
      t.check('targetWearsRim', target === '1', `data-target=${target}`);
    },
  },

  // B-064 (§4 state 21): a maximized answer keeps the 640 px measure.
  {
    name: 'sheet-maximized',
    backdrop: 'wallpaper',
    windows: [outWin('out1', { x: 0, y: 0, w: 1600, h: 1000 }, { fixed: true })],
    async run(t) {
      const f = t.frame('out1');
      await ask(f, { q: 'Show me the formatting', a: `${ANSWER}\n\n${FIXTURE}` });
      await t.settle(700);
      await t.shot('sheet-maximized-1600', { x: 0, y: 0, width: 1600, height: 1000 });
      const m = await f.evaluate(() => {
        const p = [...document.querySelectorAll('.msg.assistant .md-p')].map((e) => e.getBoundingClientRect());
        const sheet = document.getElementById('opanel').getBoundingClientRect();
        return { widest: Math.max(...p.map((r) => r.width)), left: Math.round(p[0].left - sheet.left), sheet: Math.round(sheet.width) };
      });
      t.check('readingMeasure640', m.widest <= 640 && m.widest >= 560 && m.sheet > 1400 && m.left > 300, JSON.stringify(m));
    },
  },

  // ----------------------------------------------------------------- find

  // B-169 (§4 state 24): nothing found offers the next step.
  {
    name: 'find-empty',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1)],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 520 }],
        ['view:set', { view: 'find' }],
        ['find:hits', { gen: 1, query: 'quarterly revenue', hits: [], current: -1 }],
        ['find:state', { gen: 1, open: true, scope: 'window', status: 'Nothing on screen matches "quarterly revenue".', lines: ['No related matches either.'], counts: { exact: 0, related: 0 } }],
      ]);
      await t.settle(500);
      await t.shot('find-empty', await clipOf(t, 'out1'));
      const labels = await f.evaluate(() => [...document.querySelectorAll('.find-next button')].map((b) => b.textContent));
      const keys = await f.evaluate(() => /Ctrl|Enter|Tab|Esc/.test(document.getElementById('find-view').innerText));
      await f.evaluate(() => [...document.querySelectorAll('.find-next button')].find((b) => b.textContent === 'Look on the whole screen').click());
      await t.respond('out1', 'find:to-ask', { status: 'no-hit' });
      await f.evaluate(() => document.querySelector('.find-next button').click());
      await t.settle(100);
      const scope = payloadOf(await t.calls('out1', 'find:scope'));
      const toAsk = (await t.calls('out1', 'find:to-ask')).length;
      const submit = payloadOf(await t.calls('out1', 'query:submit'));
      t.check(
        'findEmptyOffersNextSteps',
        labels.join('|') === 'Ask about "quarterly revenue"|Look on the whole screen' && !keys && !!scope && scope.scope === 'screen' && toAsk === 1 && !!submit && submit.question === 'quarterly revenue',
        JSON.stringify({ labels, keys, scope, toAsk, submit })
      );
    },
  },

  // B-169 (§4 state 25): the results list works from the keyboard.
  {
    name: 'find-results',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1)],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 520 }],
        ['view:set', { view: 'find' }],
        ['find:state', { gen: 1, open: true, scope: 'window', status: '3 exact · 2 related', lines: [], counts: { exact: 3, related: 2 } }],
        [
          'find:hits',
          {
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
          },
        ],
      ]);
      await t.settle(500);
      await t.shot('find-results', await clipOf(t, 'out1'));
      await f.focus('.find-row.current');
      const steps = [];
      for (const k of ['ArrowDown', 'ArrowDown', 'End', 'Home', 'ArrowUp']) {
        await t.page.keyboard.press(k);
        await t.settle(60);
        steps.push(
          await f.evaluate(() => ({
            focus: document.activeElement && document.activeElement.dataset.i,
            tabs: [...document.querySelectorAll('.find-row')].map((r) => r.tabIndex).join(''),
          }))
        );
      }
      const sent = (await t.calls('out1', 'find:step')).map((c) => c.payload && c.payload.by && c.payload.by.index);
      const roles = await f.evaluate(() => document.getElementById('find-view').getAttribute('role') === 'listbox' && [...document.querySelectorAll('.find-row')].every((r) => r.getAttribute('role') === 'option'));
      const focus = steps.map((s) => s.focus).join(',');
      t.check('findListboxArrows', roles && focus === '1,2,4,0,0' && steps[2].tabs === '-1-1-1-10' && sent.join(',') === '1,2,4,0', JSON.stringify({ steps, sent, roles }));
    },
  },

  // ------------------------------------------------------------ the trail

  // B-165 (§4 state 29): a run in progress, the plan as the checklist.
  {
    name: 'sheet-task-running',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['agent:begin', { task: 'Rename the screenshots in Downloads by date' }],
        ['agent:step', { id: 1, kind: 'plan', text: 'Plan', state: 'done', steps: PLAN }],
        ['agent:step', { id: 2, kind: 'action', state: 'done', text: 'Opened File Explorer at Downloads', planIndex: 0, ms: 1800 }],
        ['agent:step', { id: 3, kind: 'action', state: 'done', text: 'Sorted by date taken', planIndex: 1 }],
        ['agent:step', { id: 4, kind: 'action', state: 'running', text: 'Selecting 14 screenshots', planIndex: 1 }],
        ['agent:state', { state: 'running', step: 3 }],
      ]);
      await t.settle(500);
      await t.shot('sheet-task-running', await clipOf(t, 'out1'));
      const s = await f.evaluate(() => ({
        items: [...document.querySelectorAll('.plan-item')].map((li) => li.className.replace('plan-item', '').trim()),
        acts: [...document.querySelectorAll('.plan-item')].map((li) => li.querySelectorAll('.plan-act').length),
        loose: document.querySelectorAll('.trail-rows > .trow.action').length,
        title: !!document.querySelector('.trail-title'),
      }));
      t.check('planChecksOff', s.items.join(',') === 'done,running,pending' && s.acts.join(',') === '1,2,0' && s.loose === 0 && !s.title, JSON.stringify(s));
      const sr = await f.evaluate(() => {
        const rows = [...document.querySelectorAll('.plan-item, .plan-act, .trail-rows > .trow:not(.plan):not(.thinking)')];
        const prefix = (el) => {
          const s = el.querySelector('.sr-only');
          return s ? s.textContent : '';
        };
        return rows.map((r) => prefix(r));
      });
      const okPrefix = sr.length >= 6 && sr.every((p) => /^(Planned|Now|Done|Skipped|Failed): $/.test(p));
      t.check('trailRowsHaveSrPrefixes', okPrefix && sr[0] === 'Done: ' && sr.includes('Now: ') && sr.includes('Planned: '), JSON.stringify(sr));
    },
  },

  // B-165 (§4 state 33): a finished run, steps opened; acts with no plan
  // index check items off by count.
  {
    name: 'sheet-task-done-expanded',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['agent:begin', { task: 'Rename the screenshots in Downloads by date' }],
        ['agent:step', { id: 1, kind: 'plan', text: 'Plan', state: 'done', steps: PLAN }],
        ['agent:step', { id: 2, kind: 'action', state: 'done', text: 'Opened File Explorer at Downloads' }],
        ['agent:step', { id: 3, kind: 'action', state: 'done', text: 'Selected 14 screenshots' }],
      ]);
      await t.settle(200);
      const mid = await f.evaluate(() => [...document.querySelectorAll('.plan-item')].map((li) => li.className.replace('plan-item', '').trim()).join(','));
      await emitAll(f, [
        ['agent:step', { id: 4, kind: 'action', state: 'done', text: 'Renamed 14 files to YYYY-MM-DD' }],
        ['agent:done', { state: 'done', text: 'Renamed **14 screenshots** in Downloads by the date they were taken.', usage: { calls: 6, ms: 42000 } }],
      ]);
      await t.settle(300);
      await t.shot('sheet-task-done-collapsed', await clipOf(t, 'out1'));
      await f.evaluate(() => document.querySelector('.trail-toggle').click());
      await t.settle(300);
      await t.shot('sheet-task-done-expanded', await clipOf(t, 'out1'));
      const end = await f.evaluate(() => ({
        items: [...document.querySelectorAll('.plan-item')].map((li) => li.className.replace('plan-item', '').trim()).join(','),
        open: document.querySelector('.trail-toggle').getAttribute('aria-expanded'),
        toggle: document.querySelector('.trail-toggle').textContent,
        status: document.querySelector('.trail-status').textContent,
      }));
      t.check('planChecksOffByCount', mid === 'done,done,running' && end.items === 'done,done,done' && end.open === 'true' && end.toggle === '3 steps' && end.status === 'Done', JSON.stringify({ mid, end }));
    },
  },

  // B-066: a question from a task says so once, in the row.
  {
    name: 'sheet-task-asking',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await listenAloud(f);
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['agent:begin', { task: 'Rename the screenshots in Downloads by date' }],
        ['agent:step', { id: 1, kind: 'plan', text: 'Plan', state: 'done', steps: PLAN }],
        ['agent:step', { id: 2, kind: 'action', state: 'done', text: 'Opened File Explorer at Downloads', planIndex: 0 }],
        ['agent:step', { id: 3, kind: 'ask', state: 'running', text: 'Keep the time of day in the names?' }],
        ['agent:state', { state: 'asking', step: 2 }],
      ]);
      await t.settle(500);
      await t.shot('sheet-task-asking', await clipOf(t, 'out1'));
      const s = await f.evaluate(() => {
        const note = document.querySelector('.trow.ask .trow-note.waiting');
        return { status: document.querySelector('.trail-status').textContent, note: note && note.textContent, color: note && getComputedStyle(note).color, head: document.querySelector('.trail-head').innerText };
      });
      const said = (await heard(t, 'out1')).filter((x) => /question/.test(x.text));
      t.check(
        'taskAskSaysItOnce',
        s.status === 'Step 2' && s.note === 'Answer in the bar' && !/Task/.test(s.head) && said.length === 1 && said[0].priority === 'assertive' && said[0].text === 'The task has a question: Keep the time of day in the names? Type your answer in the bar.',
        JSON.stringify({ s, said })
      );
    },
  },

  // B-066: a failed run shows the failure once.
  {
    name: 'sheet-task-failed',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['agent:begin', { task: 'Rename the screenshots in Downloads by date' }],
        ['agent:step', { id: 1, kind: 'plan', text: 'Plan', state: 'done', steps: PLAN }],
        ['agent:step', { id: 2, kind: 'action', state: 'done', text: 'Opened File Explorer at Downloads', planIndex: 0 }],
        ['agent:step', { id: 3, kind: 'action', state: 'failed', text: 'Click Rename', error: "The Rename button wasn't on screen.", planIndex: 1 }],
        ['agent:done', { state: 'error', text: 'Open File Explorer yourself, then run the task again.' }],
      ]);
      await t.settle(500);
      await t.shot('sheet-task-failed', await clipOf(t, 'out1'));
      const s = await f.evaluate(() => ({
        items: [...document.querySelectorAll('.plan-item')].map((li) => li.className.replace('plan-item', '').trim()).join(','),
        failed: !!document.querySelector('.plan-act.failed'),
        reasonTimes: document.getElementById('chat').innerText.split("The Rename button wasn't on screen.").length - 1,
        remedy: document.querySelector('.error-row .error-text') && document.querySelector('.error-row .error-text').textContent,
      }));
      t.check('failureShownOnce', s.items === 'done,failed,pending' && s.failed && s.reasonTimes === 1 && s.remedy === 'Open File Explorer yourself, then run the task again.', JSON.stringify(s));
    },
  },

  // B-170: steps are heard, at most one every 2 s; failures at once.
  {
    name: 'sheet-task-heard',
    backdrop: 'wallpaper',
    clock: true,
    scales: [1],
    windows: [outWin('out1', OUT1, { max: 900 })],
    run: onStoppedClock(async (t) => {
      const f = t.frame('out1');
      await listenAloud(f);
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['agent:begin', { task: 'Rename the screenshots in Downloads by date' }],
        ['agent:step', { id: 1, kind: 'plan', text: 'Plan', state: 'done', steps: PLAN }],
        ['agent:step', { id: 2, kind: 'action', state: 'running', text: 'Open File Explorer', planIndex: 0 }],
        ['agent:step', { id: 3, kind: 'action', state: 'running', text: 'Go to Downloads', planIndex: 0 }],
        ['agent:step', { id: 4, kind: 'action', state: 'running', text: 'Sort by date', planIndex: 1 }],
      ]);
      await t.tick(100);
      const steps = async () => (await heard(t, 'out1')).filter((x) => /^Step \d+: /.test(x.text)).map((x) => x.text);
      const first = await steps();
      await t.tick(2100);
      const second = await steps();
      await emitAll(f, [['agent:step', { id: 5, kind: 'action', state: 'failed', text: 'Click Rename', error: 'Not on screen', planIndex: 2 }]]);
      await t.tick(100);
      const fail = (await heard(t, 'out1')).filter((x) => /failed/.test(x.text));
      t.check(
        'stepAnnouncesThrottled',
        first.length === 1 && first[0] === 'Step 1: Open File Explorer' && second.length === 2 && second[1] === 'Step 3: Sort by date' && fail.length === 1 && fail[0].priority === 'assertive',
        JSON.stringify({ first, second, fail })
      );
    }),
  },

  // B-007 (and B-183's settle): the confirm card shows the whole text that
  // will be typed, and Allow waits until it has been seen.
  {
    name: 'sheet-confirm-long-text',
    backdrop: 'wallpaper',
    clock: true,
    windows: [outWin('out1', OUT1, { max: 900 })],
    run: onStoppedClock(async (t) => {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['agent:begin', { task: 'Post the release notes in #general' }],
        ['agent:state', { state: 'confirm', step: 4 }],
        ['agent:confirm-request', { id: 'c1', text: 'Type the release notes into Slack', reason: 'it sends text to other people', value: LONG_TYPED, truncated: true }],
      ]);
      await t.tick(800);
      await t.shot('sheet-confirm-long-text', await clipOf(t, 'out1'));
      const card = await f.evaluate(() => {
        const c = document.querySelector('.confirm-card');
        const v = c.querySelector('.confirm-value');
        const btn = (txt) => [...c.querySelectorAll('.confirm-actions button')].find((b) => b.textContent === txt);
        const reason = c.querySelector('.confirm-reason');
        return {
          label: v && v.getAttribute('aria-label'),
          mono: v && /mono|consol|cascadia/i.test(getComputedStyle(v).fontFamily),
          scrolls: v && v.scrollHeight > v.clientHeight,
          more: !!c.querySelector('.confirm-more'),
          allow: btn('Allow').getAttribute('aria-disabled'),
          skip: btn('Skip').getAttribute('aria-disabled'),
          described: !!reason && c.getAttribute('aria-describedby') === reason.id,
          hint: !!c.querySelector('.confirm-hint') || /Tab to a button/.test(c.innerText),
        };
      });
      // Allow does nothing yet.
      await f.evaluate(() => [...document.querySelectorAll('.confirm-actions button')].find((b) => b.textContent === 'Allow').click());
      const early = (await t.calls('out1', 'agent:confirm')).length;
      await f.evaluate(() => {
        const v = document.querySelector('.confirm-value');
        v.scrollTop = v.scrollHeight;
        v.dispatchEvent(new Event('scroll'));
      });
      await t.tick(300);
      const settling = await f.evaluate(() => [...document.querySelectorAll('.confirm-actions button')].find((b) => b.textContent === 'Allow').getAttribute('aria-disabled'));
      await t.tick(500);
      const ready = await f.evaluate(() => [...document.querySelectorAll('.confirm-actions button')].find((b) => b.textContent === 'Allow').getAttribute('aria-disabled'));
      await f.evaluate(() => [...document.querySelectorAll('.confirm-actions button')].find((b) => b.textContent === 'Allow').click());
      const sent = payloadOf(await t.calls('out1', 'agent:confirm'));
      t.check(
        'confirmAllowWaitsForFullText',
        card.label === 'Text that will be typed' && card.mono && card.scrolls && card.more && card.allow === 'true' && card.skip === null && card.described && !card.hint && early === 0 && settling === 'true' && ready === null && !!sent && sent.id === 'c1' && sent.allow === true,
        JSON.stringify({ card, early, settling, ready, sent })
      );
      // A short confirm: both buttons sweep for 700 ms, then work.
      await emitAll(f, [['agent:confirm-request', { id: 'c2', text: 'Click Send', reason: 'it sends a message' }]]);
      await t.tick(100);
      const sweep = await f.evaluate(() => [...document.querySelectorAll('.confirm-actions button')].map((b) => `${b.getAttribute('aria-disabled')}:${b.classList.contains('settling')}`));
      await f.evaluate(() => [...document.querySelectorAll('.confirm-actions button')].find((b) => b.textContent === 'Skip').click());
      const tooSoon = (await t.calls('out1', 'agent:confirm')).filter((c) => c.payload && c.payload.id === 'c2').length;
      await t.tick(700);
      const after = await f.evaluate(() => [...document.querySelectorAll('.confirm-actions button')].map((b) => `${b.getAttribute('aria-disabled')}:${b.classList.contains('settling')}`));
      t.check('confirmSweepThenEnable', sweep.join(',') === 'true:true,true:true' && tooSoon === 0 && after.join(',') === 'null:false,null:false', JSON.stringify({ sweep, tooSoon, after }));
    }),
  },

  // B-171 (§4 state 27): the consent sheet names who receives the screen.
  {
    name: 'sheet-consent',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        [
          'agent:consent-request',
          {
            task: 'Rename the screenshots in Downloads by date',
            provider: 'openrouter',
            providerName: 'OpenRouter',
            model: 'google/gemma-3-27b-it:free',
            modelName: 'Gemma 3 27B',
            free: true,
            recipients: [{ id: 'openrouter', name: 'OpenRouter', model: 'google/gemma-3-27b-it:free', modelName: 'Gemma 3 27B', free: true }],
          },
        ],
      ]);
      await t.settle(500);
      await t.shot('sheet-consent', await clipOf(t, 'out1'));
      const text = await f.evaluate(() => document.querySelector('.msg.consent').innerText);
      t.check(
        'consentNamesProvider',
        text.includes("Each step sends a screenshot and the window's text to OpenRouter (Gemma 3 27B).") && text.includes('Move the mouse to pause it. Press Esc to stop it.') && text.includes("Free models' providers may keep and train on what they receive."),
        text
      );
    },
  },

  // ------------------------------------------------------------ the keys

  // B-067, B-167: Esc moves focus and never closes or stops; the shortcuts.
  {
    name: 'sheet-keys',
    backdrop: 'wallpaper',
    scales: [1],
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await ask(f);
      await ask(f, { gen: 2, q: 'And on Linux?', a: 'The same error on Linux means **libstdc++** is missing.' });
      await t.settle(500);
      const count = async (c) => (await t.calls('out1', c)).length;
      await key(f, 'Escape');
      const escPanel = { focus: await count('ui:focus-bar'), collapse: await count('overlay:collapse'), close: await count('output:close'), cancel: await count('query:cancel') };
      await emitAll(f, [['overlay:state', { mode: 'pill' }]]);
      await key(f, 'Escape');
      const blur = payloadOf(await t.calls('out1', 'output:blur'));
      await emitAll(f, [['overlay:state', { mode: 'panel', chatMax: 640 }]]);
      t.check('outputEscNeverCollapses', escPanel.focus === 1 && !escPanel.collapse && !escPanel.close && !escPanel.cancel && !!blur && blur.label === 'out1' && !(await count('overlay:collapse')), JSON.stringify({ escPanel, blur }));
      const got = {};
      const press = async (name, k, mods, channel, test) => {
        const before = await count(channel);
        await key(f, k, mods);
        await t.settle(60);
        const list = await t.calls('out1', channel);
        got[name] = list.length > before && (!test || test(last(list).payload || {}));
      };
      await press('F6', 'F6', {}, 'ui:focus-bar');
      await press('Ctrl+N', 'n', { ctrl: true }, 'ui:focus-bar', (p) => p.newConversation === true);
      await press('Ctrl+Tab', 'Tab', { ctrl: true }, 'output:cycle-target', (p) => p.dir === 1);
      await press('Ctrl+Shift+Tab', 'Tab', { ctrl: true, shift: true }, 'output:cycle-target', (p) => p.dir === -1);
      await press('Ctrl+Shift+W', 'W', { ctrl: true, shift: true }, 'output:close-all');
      await press('Ctrl+Shift+C', 'C', { ctrl: true, shift: true }, 'clipboard:write', (p) => /> And on Linux\?/.test(p.text));
      await press('Ctrl+Shift+R', 'R', { ctrl: true, shift: true }, 'query:retry', (p) => p.label === 'out1');
      await press('Ctrl+,', ',', { ctrl: true }, 'overlay:open-settings');
      await press('Ctrl+H', 'h', { ctrl: true }, 'overlay:open-history');
      await emitAll(f, [['query:start', { gen: 3 }]]);
      await press('Ctrl+.', '.', { ctrl: true }, 'query:cancel');
      // Last: the close glob runs before the window is told to go.
      await key(f, 'w', { ctrl: true });
      await t.settle(500);
      const closed = payloadOf(await t.calls('out1', 'output:close'));
      got['Ctrl+W'] = !!closed && closed.label === 'out1';
      const tips = await f.evaluate(() => {
        const a = (id) => {
          const e = document.getElementById(id);
          return `${e.title}|${e.getAttribute('aria-keyshortcuts')}`;
        };
        const stop = document.querySelector('.stop-btn');
        return [a('close-out'), a('copy-all'), a('history-btn'), `${stop.title}|${stop.getAttribute('aria-keyshortcuts')}`];
      });
      const wantTips = ['Close (Ctrl+W)|Control+W', 'Copy conversation (Ctrl+Shift+C)|Control+Shift+C', 'History (Ctrl+H)|Control+H', 'Stop (Ctrl+.)|Control+.'];
      const missed = Object.entries(got).filter(([, ok]) => !ok).map(([k]) => k);
      t.check('outputShortcutsInvoke', !missed.length && tips.join(',') === wantTips.join(','), JSON.stringify({ missed, tips }));
    },
  },

  // ----------------------------------------------------------- refinements

  // B-183: Do it only for steps; text 16 px in from the glass in every kind.
  {
    name: 'sheet-kinds',
    backdrop: 'wallpaper',
    scales: [1],
    windows: [
      outWin('o1', { x: 40, y: 120, w: 500, h: 560 }, { fixed: true }),
      outWin('o2', { x: 560, y: 120, w: 480, h: 560 }, { fixed: true }),
      outWin('o3', { x: 1060, y: 120, w: 500, h: 560 }, { fixed: true }),
    ],
    async run(t) {
      const a = t.frame('o1');
      await ask(a, { q: 'How do I pair headphones?', a: 'Pair them in Settings:\n\n1. Open Settings.\n2. Click Bluetooth & devices.\n3. Press Add device.' });
      const steps = await a.evaluate(() => !document.querySelector('.msg.assistant .do-it').hidden && document.querySelector('.msg.assistant .do-it').title);
      await ask(a, { gen: 2, q: 'What is in the box?', a: 'In the box:\n\n1. The headphones.\n2. A USB-C cable.\n3. A carrying case.' });
      const plain = await a.evaluate(() => document.querySelectorAll('.msg.assistant .do-it')[1].hidden);
      t.check('doItOnlyForSteps', !!steps && plain, JSON.stringify({ steps, plain }));
      await emitAll(t.frame('o2'), [
        ['overlay:state', { mode: 'panel', chatMax: 520 }],
        ['view:set', { view: 'find' }],
        ['find:state', { gen: 1, open: true, scope: 'window', status: '1 exact', lines: [], counts: { exact: 1, related: 0 } }],
        ['find:hits', { gen: 1, query: 'invoice', current: 0, hits: [{ i: 0, kind: 'literal', text: 'Invoice total: €1,240.00', match: 'Invoice' }] }],
      ]);
      await emitAll(t.frame('o3'), [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['agent:begin', { task: 'Rename the screenshots' }],
        ['agent:step', { id: 1, kind: 'plan', text: 'Plan', state: 'done', steps: PLAN }],
        ['agent:step', { id: 2, kind: 'action', state: 'running', text: 'Opening File Explorer' }],
      ]);
      await t.settle(600);
      await t.shot('sheet-kinds');
      const inset = (f, sel) =>
        f.evaluate((s) => {
          const e = document.querySelector(s);
          if (!e) return null;
          const r = document.createRange();
          r.selectNodeContents(e);
          const text = r.getClientRects()[0] || e.getBoundingClientRect();
          return Math.round(text.left - document.getElementById('opanel').getBoundingClientRect().left);
        }, sel);
      const got = {
        answer: await inset(a, '.msg.assistant .md-p'),
        find: await inset(t.frame('o2'), '.find-row .find-snippet'),
        plan: await inset(t.frame('o3'), '.plan-item .plan-text'),
        status: await inset(t.frame('o3'), '.trail-status'),
      };
      t.check('textInsetIs16InEveryKind', Object.values(got).every((v) => v === 16), JSON.stringify(got));
    },
  },

  // ------------------------------------------- the rest of design-visual §4

  // State 14: thinking, then a free model's long wait.
  {
    name: 'sheet-thinking',
    backdrop: 'wallpaper',
    clock: true,
    windows: [outWin('out1', OUT1)],
    run: onStoppedClock(async (t) => {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['chat:user', { gen: 1, text: Q }],
        ['query:start', { gen: 1, modelName: 'Gemma 3 27B' }],
        ['query:stage', { gen: 1, label: 'Reading the screen' }],
      ]);
      await t.settle(500);
      await t.shot('sheet-thinking', await clipOf(t, 'out1'));
      const stage = await f.evaluate(() => document.querySelector('.stage-label').textContent);
      await t.tick(12000);
      await t.shot('sheet-long-wait', await clipOf(t, 'out1'));
      const wait = await f.evaluate(() => {
        const e = document.querySelector('.stage-label');
        return { text: e.textContent, title: e.title };
      });
      t.check('longWaitNamesTheModel', stage === 'Reading the screen' && /^Waiting for Gemma 3 27B · \d+ s$/.test(wait.text) && /queue/.test(wait.title), JSON.stringify({ stage, wait }));
    }),
  },

  // State 15: a stream stopped inside an open fence.
  {
    name: 'sheet-mid-stream-fence',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['chat:user', { gen: 1, text: Q }],
        ['query:start', { gen: 1 }],
        ['query:delta', { gen: 1, text: 'Run this:\n\n```js\nconst a = 1;\nconst b' }],
      ]);
      await t.settle(300);
      await t.shot('sheet-mid-stream-fence', await clipOf(t, 'out1'));
      const s = await f.evaluate(() => {
        const body = document.querySelector('.msg.assistant .md-body');
        const code = body.querySelector('.md-code pre code');
        return { code: code ? code.textContent : null, fence: body.textContent.includes('```') };
      });
      t.check('openFenceIsCode', !!s.code && s.code.includes('const b') && !s.fence, JSON.stringify(s));
    },
  },

  // State 16: Copy pressed, Do it shown, and a guessed question that may
  // have been a word to find.
  {
    name: 'sheet-done-actions',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await ask(f, { q: 'pair my headphones', a: 'Pair them in Settings:\n\n1. Open Settings.\n2. Click Bluetooth & devices.\n3. Press Add device.', done: { inferred: true } });
      await t.settle(600);
      await f.evaluate(() => document.querySelector('.msg.assistant .copy-msg').click());
      await t.settle(200);
      await t.shot('sheet-done-copied', await clipOf(t, 'out1'));
      const s = await f.evaluate(() => {
        const b = [...document.querySelectorAll('.msg.assistant .copy-msg')];
        return { copy: b[0].textContent, doIt: !document.querySelector('.msg.assistant .do-it').hidden, find: b.some((x) => x.textContent === 'Find this on screen instead') };
      });
      await f.evaluate(() => [...document.querySelectorAll('.msg.assistant .copy-msg')].find((b) => b.textContent === 'Find this on screen instead').click());
      await t.settle(60);
      const open = payloadOf(await t.calls('out1', 'find:open'));
      t.check('doneOffersNextActions', s.copy === 'Copied' && s.doIt && s.find && !!open && open.prefill === 'pair my headphones', JSON.stringify({ s, open }));
    },
  },

  // State 17: two turns, scrolled: the hairline and the bottom fade.
  {
    name: 'sheet-two-turn-scrolled',
    backdrop: 'wallpaper',
    windows: [outWin('out1', { ...OUT1, h: 440 }, { fixed: true })],
    async run(t) {
      const f = t.frame('out1');
      await ask(f);
      await ask(f, { gen: 2, q: 'And on Linux?', a: ANSWER });
      await emitAll(f, [['overlay:state', { mode: 'panel', chatMax: 320 }]]);
      await t.settle(500);
      await f.evaluate(() => {
        const c = document.getElementById('chat');
        c.scrollTop = Math.round((c.scrollHeight - c.clientHeight) / 2);
      });
      await t.settle(300);
      await t.shot('sheet-two-turn-scrolled', await clipOf(t, 'out1'));
      const s = await f.evaluate(() => ({ scrolled: document.body.dataset.scrolled, more: document.body.dataset.more }));
      t.check('scrolledShowsBothEdges', s.scrolled === '1' && s.more === '1', JSON.stringify(s));
    },
  },

  // State 19: stopped mid-answer keeps what arrived.
  {
    name: 'sheet-stopped',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['chat:user', { gen: 1, text: Q }],
        ['query:start', { gen: 1 }],
        ['query:delta', { gen: 1, text: ANSWER.slice(0, 180) }],
      ]);
      await t.settle(200);
      await emitAll(f, [['query:done', { gen: 1, question: Q }]]);
      await t.settle(400);
      await t.shot('sheet-stopped', await clipOf(t, 'out1'));
      const s = await f.evaluate(() => ({
        text: document.querySelector('.msg.assistant .md-body').textContent.length,
        stop: document.querySelector('.msg.assistant .stop-btn').hidden,
        busy: document.querySelector('.msg.assistant').classList.contains('busy'),
        tail: !!document.querySelector('.md-tail'),
      }));
      t.check('stoppedKeepsWhatArrived', s.text > 100 && s.stop && !s.busy && !s.tail, JSON.stringify(s));
    },
  },

  // State 21: dragged to a fixed height, the transcript scrolls inside it.
  {
    name: 'sheet-fixed-height',
    backdrop: 'wallpaper',
    windows: [outWin('out1', { ...OUT1, h: 420 }, { fixed: true })],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640, outputFixedHeight: true }],
        ['chat:user', { gen: 1, text: Q }],
        ['query:start', { gen: 1 }],
        ['query:delta', { gen: 1, text: `${ANSWER}\n\n${ANSWER}` }],
        ['query:done', { gen: 1, usage: USAGE, question: Q }],
      ]);
      await t.settle(700);
      await t.shot('sheet-fixed-height', await clipOf(t, 'out1'));
      const s = await f.evaluate(() => {
        const c = document.getElementById('chat');
        return { fixed: document.body.dataset.fixedHeight, scrolls: c.scrollHeight > c.clientHeight };
      });
      const resizes = (await t.calls('out1', 'output:resize')).length;
      t.check('fixedHeightStopsFollowing', s.fixed === '1' && s.scrolls && resizes === 0, JSON.stringify({ s, resizes }));
    },
  },

  // State 23: Find still reading offers nothing yet.
  {
    name: 'find-reading',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1)],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 520 }],
        ['view:set', { view: 'find' }],
        ['find:hits', { gen: 1, query: 'invoice', hits: [], current: -1 }],
        ['find:state', { gen: 1, open: true, reading: true, scope: 'window', status: 'Looking…', lines: [], counts: { exact: 0, related: 0 } }],
      ]);
      await t.settle(500);
      await t.shot('find-reading', await clipOf(t, 'out1'));
      const next = await f.evaluate(() => document.querySelectorAll('.find-next button').length);
      t.check('findReadingOffersNothingYet', next === 0, `${next} buttons`);
    },
  },

  // State 24: offscreen rows, while related matching runs.
  {
    name: 'find-results-related',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1)],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 520 }],
        ['view:set', { view: 'find' }],
        ['find:state', { gen: 1, open: true, scope: 'window', stage: 'related', status: '2 exact · looking for related matches', lines: [], counts: { exact: 2, related: 0 } }],
        [
          'find:hits',
          {
            gen: 1,
            query: 'invoice total',
            current: 0,
            hits: [
              { i: 0, kind: 'literal', text: 'Invoice total: €1,240.00', match: 'Invoice total' },
              { i: 1, kind: 'literal', text: 'See invoice total on page 2', match: 'invoice total', offscreen: true },
            ],
          },
        ],
      ]);
      await t.settle(500);
      await t.shot('find-results-related', await clipOf(t, 'out1'));
      const off = await f.evaluate(() => document.querySelectorAll('.find-row.offscreen').length);
      t.check('findMarksOffscreenRows', off === 1, `${off} offscreen rows`);
    },
  },

  // State 26: offline, nothing to ask with.
  {
    name: 'find-offline',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1)],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 520 }],
        ['view:set', { view: 'find' }],
        ['find:hits', { gen: 1, query: 'invoice', hits: [], current: -1 }],
        ['find:state', { gen: 1, open: true, scope: 'window', stage: 'offline', status: 'Nothing on screen matches "invoice".', lines: ['Related matches need a connection.'], counts: { exact: 0, related: 0 } }],
      ]);
      await t.settle(500);
      await t.shot('find-offline', await clipOf(t, 'out1'));
      const labels = await f.evaluate(() => [...document.querySelectorAll('.find-next button')].map((b) => b.textContent));
      t.check('findOfflineOffersNoAsk', labels.join('|') === 'Look on the whole screen', labels.join('|'));
    },
  },

  // States 28, 32, 35: planning, plan approval, paused, stopped.
  {
    name: 'sheet-task-states',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['agent:begin', { task: 'Rename the screenshots in Downloads by date' }],
        ['agent:step', { id: 1, kind: 'thinking', state: 'running', text: 'Working out the steps' }],
      ]);
      await t.settle(500);
      await t.shot('sheet-task-planning', await clipOf(t, 'out1'));
      const planning = await f.evaluate(() => document.querySelector('.trail-status').textContent);
      await emitAll(f, [
        ['agent:step', { id: 1, kind: 'remove' }],
        ['agent:step', { id: 2, kind: 'plan', text: 'Plan', state: 'done', steps: PLAN }],
        ['agent:state', { state: 'plan', message: 'Enter starts the task.', step: 0 }],
      ]);
      await t.settle(300);
      await t.shot('sheet-task-plan-approval', await clipOf(t, 'out1'));
      const start = await f.evaluate(() => {
        const b = document.querySelector('.trail-banner button');
        const text = b.textContent;
        b.click();
        return text;
      });
      await t.settle(60);
      const started = (await t.calls('out1', 'agent:continue')).length;
      await emitAll(f, [
        ['agent:step', { id: 3, kind: 'action', state: 'running', text: 'Opening File Explorer', planIndex: 0 }],
        ['agent:state', { state: 'paused', message: 'Paused because the mouse moved.', step: 1 }],
      ]);
      await t.settle(300);
      await t.shot('sheet-task-paused', await clipOf(t, 'out1'));
      const paused = await f.evaluate(() => ({ button: document.querySelector('.trail-banner button').textContent, text: document.querySelector('.trail-banner .banner-text').textContent }));
      await emitAll(f, [['agent:done', { state: 'stopped', text: 'Stopped. Nothing else was changed.' }]]);
      await t.settle(300);
      await t.shot('sheet-task-stopped', await clipOf(t, 'out1'));
      const stopped = await f.evaluate(() => ({ status: document.querySelector('.trail-status').textContent, stop: document.querySelector('.msg.trail .stop-btn').hidden, banner: !!document.querySelector('.trail-banner') }));
      t.check(
        'taskStatesSayWhatIsNext',
        planning === 'Planning…' && start === 'Start' && started === 1 && paused.button === 'Continue' && paused.text === 'Paused because the mouse moved.' && stopped.status === 'Stopped' && stopped.stop && !stopped.banner,
        JSON.stringify({ planning, start, started, paused, stopped })
      );
    },
  },

  // State 30: an open-link confirm shows where it goes.
  {
    name: 'sheet-confirm-url',
    backdrop: 'wallpaper',
    windows: [outWin('out1', OUT1, { max: 900 })],
    async run(t) {
      const f = t.frame('out1');
      await emitAll(f, [
        ['overlay:state', { mode: 'panel', chatMax: 640 }],
        ['agent:begin', { task: 'Pay the electricity bill' }],
        ['agent:state', { state: 'confirm', step: 2 }],
        ['agent:confirm-request', { id: 'u1', text: 'Open this link', reason: 'it leaves this page', url: { full: 'https://bank.example/login?next=/pay', host: 'bank.example', rest: '/login?next=/pay' } }],
      ]);
      await t.settle(1000);
      await t.shot('sheet-confirm-url', await clipOf(t, 'out1'));
      const u = await f.evaluate(() => {
        const e = document.querySelector('.confirm-url');
        return { host: e.querySelector('strong').textContent, text: e.textContent, title: e.title };
      });
      t.check('confirmShowsTheWholeUrl', u.host === 'bank.example' && u.text === 'bank.example/login?next=/pay' && u.title === 'https://bank.example/login?next=/pay', JSON.stringify(u));
    },
  },

  // State 38: reduced motion fades and draws no shape.
  {
    name: 'glob-reduced',
    backdrop: 'wallpaper',
    clock: true,
    media: { reducedMotion: 'reduce' },
    windows: [win('overlay', '/renderer/index.html', BAR, 1), outWin('out1', OUT1)],
    run: onStoppedClock(async (t) => {
      const { out } = await barAndAnswer(t);
      await out.evaluate(() => {
        window.__globDone = false;
        window.AevalGlob.open(document.getElementById('opanel')).then(() => {
          window.__globDone = true;
        });
      });
      await t.tick(60);
      await t.shot('glob-reduced-fade', { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: OUT1.y + OUT1.h });
      const s = await out.evaluate(() => ({ done: window.__globDone, host: document.getElementById('glob').hidden, clip: document.getElementById('opanel').style.clipPath }));
      t.check('reducedMotionDrawsNoShape', s.done && s.host && !s.clip, JSON.stringify(s));
    }),
  },

  // B-065: output, settings and history share one title bar. Settings and
  // history move onto .sheet-bar in the settings lane, so the check speaks
  // once both pages carry it.
  {
    name: 'one-title-bar',
    backdrop: 'wallpaper',
    scales: [1],
    windows: [
      outWin('out1', { x: 20, y: 40, w: 520, h: 420 }),
      win('settings', '/settings/settings.html', { x: 560, y: 40, w: 520, h: 420 }, 3),
      win('history', '/history/history.html', { x: 1080, y: 40, w: 500, h: 420 }, 4),
    ],
    async run(t) {
      await ask(t.frame('out1'));
      await t.settle(600);
      const bar = (name) =>
        t.frame(name).evaluate(() => {
          const b = document.querySelector('.sheet-bar');
          if (!b) return null;
          const title = getComputedStyle(b.querySelector('.sheet-bar-title'));
          return { height: Math.round(b.getBoundingClientRect().height), font: `${title.fontWeight} ${title.fontSize} ${title.fontFamily}` };
        });
      const bars = [await bar('out1'), await bar('settings'), await bar('history')];
      if (bars.some((b) => !b)) return;
      t.check('oneTitleBarSystem', bars.every((b) => b.height === bars[0].height && b.font === bars[0].font), JSON.stringify(bars));
    },
  },
];
