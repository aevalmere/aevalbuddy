// The polish lane's ui-shots scenes: every kind of menu, opened where it has
// the least room, ends inside its own window (a WebView cannot draw past its
// window, so anything outside is cut). The shape is documented at the top of
// tools/ui-shots/shots.mjs.
import { ANSWER, BAR, COMMON, openBar, streamAnswer } from '../kit.mjs';

export const mocks = {};
export const checks = [];
export const cssLintEnforce = [];

// Rust's cap on the bar window (layout.rs INPUT_MAX_HEIGHT) plus the 20 px
// stage on both sides: the mock grows the frame to the page's ask, at most
// this.
const BAR_MAX = 400 + 40;
const overlay = { name: 'overlay', src: '/renderer/index.html', x: BAR.x, y: BAR.y, w: BAR.w, h: 116, z: 2, max: BAR_MAX };
// At 200% text Rust's cap doubles with the type (layout.rs input_bounds).
const overlayFor = (t) => ({ ...overlay, max: BAR_MAX * t });

const model = (id, name) => ({ id, name });
const MODELS = {
  live: true,
  current: 'claude-sonnet-5-5',
  groups: [],
  providers: [
    { id: 'anthropic', name: 'Anthropic', groups: [{ tier: 'paid', label: 'Paid', models: [model('claude-sonnet-5-5', 'Sonnet 5.5'), model('claude-haiku-5', 'Haiku 5'), model('claude-opus-5-5', 'Opus 5.5')] }] },
    { id: 'openai', name: 'OpenAI', groups: [{ tier: 'paid', label: 'Paid', models: [model('gpt-5-mini', 'GPT-5 mini'), model('gpt-5', 'GPT-5'), model('gpt-5-nano', 'GPT-5 nano')] }] },
    { id: 'gemini', name: 'Google Gemini', groups: [{ tier: 'paid', label: 'Paid', models: [model('gemini-3-flash', 'Gemini 3 Flash'), model('gemini-3-pro', 'Gemini 3 Pro')] }] },
  ],
};

// "ok", or where the menu sits against its window.
const inside = (f, selector) =>
  f.evaluate((s) => {
    const m = document.querySelector(s);
    if (!m || m.hidden) return `${s} is not open`;
    const r = m.getBoundingClientRect();
    const w = document.documentElement.clientWidth;
    const h = document.documentElement.clientHeight;
    const ok = r.height > 0 && r.left >= -0.5 && r.top >= -0.5 && r.right <= w + 0.5 && r.bottom <= h + 0.5;
    return ok ? 'ok' : `${s} at ${Math.round(r.left)},${Math.round(r.top)} to ${Math.round(r.right)},${Math.round(r.bottom)} in a ${w}x${h} window`;
  }, selector);

// The settled state: headless Edge can hold a frame back for half a second
// when nothing else is painting, and the bar asks for its room on a frame.
const check = async (t, name, f, selector) => {
  let v = await inside(f, selector);
  for (let i = 0; i < 6 && v !== 'ok'; i += 1) {
    await t.settle(250);
    v = await inside(f, selector);
  }
  t.check(name, v === 'ok', v);
};

const setField = (f, text) =>
  f.evaluate((v) => {
    const i = document.getElementById('query-input');
    i.focus();
    i.value = v;
    i.dispatchEvent(new Event('input', { bubbles: true }));
  }, text);

const seenBefore = (f) => f.evaluate(() => localStorage.setItem('aeval.hasSentFirstRequest', '1'));

// The token's menu under a four-line question, and the @ menu with eight
// models: the bar window grows to hold them, up to Rust's cap. Once for
// each way the bar can be anchored: at the top, in the middle of the screen
// (the default, growing down from there) and at the bottom (growing up).
const barMenus = (name, tag, anchor) => ({
  name,
  backdrop: 'wallpaper',
  textScale: true,
  windows: [{ ...overlay, responses: { 'models:list': MODELS } }],
  async run(t) {
    const f = t.frame('overlay');
    // The mock grows the frame as Rust grows the window: to the cap for
    // this text size.
    await t.page.evaluate((max) => {
      document.querySelector('iframe[name="overlay"]').dataset.max = String(max);
    }, overlayFor(t.textScale).max);
    await seenBefore(f);
    await openBar(f, { inputAnchor: anchor });
    await t.settle();
    await setField(f, 'Why does the build fail\non the release branch\nbut not on main\nsince Tuesday?');
    await t.settle(200);
    // A real click: a menu opened with the pointer shows no focus ring.
    await f.click('#intent');
    await t.settle(400);
    await check(t, `menuInsideWindow.barIntent${tag}`, f, '#intent-menu');
    const ring = await f.evaluate(() => {
      const a = document.activeElement;
      return a && a.closest('.g-menu') ? getComputedStyle(a).outlineStyle : `focus on ${a && a.id}`;
    });
    t.check(`pointerMenuNoRing${tag}`, ring === 'none', ring);
    const clip = { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: overlayFor(t.textScale).max + 40 };
    await t.shot(`${name}-intent`, clip);
    await f.evaluate(() => document.getElementById('intent').click());
    await setField(f, '@');
    await t.settle(400);
    await check(t, `menuInsideWindow.barModel${tag}`, f, '#model-menu');
    await t.shot(`${name}-model`, clip);
  },
});

export const scenes = [
  barMenus('menu-bounds-bar', '', { h: 'center', v: 'top' }),
  barMenus('menu-bounds-bar-middle', '.middle', { h: 'center', v: 'center' }),
  barMenus('menu-bounds-bar-bottom', '.bottom', { h: 'center', v: 'bottom' }),

  // Retry's model menu from an answer in a short window: it cannot fit
  // under the chevron, so it opens above it or scrolls, inside the window.
  {
    name: 'menu-bounds-retry',
    backdrop: 'wallpaper',
    textScale: true,
    windows: [{ name: 'out1', src: '/renderer/output.html', x: 560, y: 120, w: 520, h: 360, z: 2, fixed: true, responses: { 'models:list': MODELS } }],
    async run(t) {
      const f = t.frame('out1');
      await streamAnswer(f, 'Why does the build fail?', ANSWER);
      await t.settle(600);
      await f.evaluate(() => {
        const c = document.getElementById('chat');
        c.scrollTop = c.scrollHeight;
        document.querySelector('.msg.assistant .retry-more').click();
      });
      await t.settle(400);
      await check(t, 'menuInsideWindow.retry', f, '.retry-menu');
      await t.shot('menu-bounds-retry');
      // The list is one tab stop, and it is the option that has focus.
      const stops = await f.evaluate(() => {
        const opts = [...document.querySelectorAll('.retry-menu [role="option"]')];
        const zero = opts.filter((o) => o.tabIndex === 0);
        return { options: opts.length, stops: zero.length, isFocused: zero.length === 1 && zero[0] === document.activeElement };
      });
      t.check('retryMenuOneTabStop', stops.options > 1 && stops.stops === 1 && stops.isFocused, JSON.stringify(stops));
    },
  },

  // The window menu from a right-click near the window's bottom right.
  {
    name: 'menu-bounds-window',
    backdrop: 'wallpaper',
    textScale: true,
    windows: [{ name: 'out1', src: '/renderer/output.html', x: 560, y: 120, w: 420, h: 240, z: 2, fixed: true }],
    async run(t) {
      const f = t.frame('out1');
      await streamAnswer(f, 'Why does the build fail?', ANSWER);
      await t.settle(600);
      await f.evaluate(() => {
        const w = document.documentElement.clientWidth;
        const h = document.documentElement.clientHeight;
        document.getElementById('sheet-bar').dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: w - 24, clientY: h - 24 }));
      });
      await t.settle(300);
      await check(t, 'menuInsideWindow.windowMenu', f, '#window-menu');
    },
  },

  // A Settings combo near the bottom of a short Settings window: the list
  // opens above the button instead of being cut by the window or the page.
  {
    name: 'menu-bounds-combo',
    backdrop: 'wallpaper',
    textScale: true,
    windows: [{ name: 'settings', src: '/settings/settings.html', x: 470, y: 160, w: 660, h: 520, z: 1, responses: { 'settings:get': { ...COMMON['settings:get'], model: 'mistralai/mistral-small-3.1-24b-instruct:free' } } }],
    async run(t) {
      const f = t.frame('settings');
      await t.settle(300);
      await f.evaluate(() => {
        const main = document.querySelector('main');
        main.style.scrollBehavior = 'auto';
        main.scrollTop = document.getElementById('pg-models').offsetTop - main.offsetTop;
        main.dispatchEvent(new Event('scroll'));
      });
      await t.settle(300);
      await f.click('#sec-model > .row-head > .edit');
      await t.settle(500);
      await f.evaluate(() => {
        const main = document.querySelector('main');
        const b = document.getElementById('model-combo-btn').getBoundingClientRect();
        const m = main.getBoundingClientRect();
        main.style.scrollBehavior = 'auto';
        main.scrollTop += b.bottom - (m.bottom - 24);
      });
      await t.settle(200);
      await f.click('#model-combo-btn');
      await t.settle(400);
      await check(t, 'menuInsideWindow.combo', f, '#model-combo-list');
      // A model with no second line sits in the middle of its row, level
      // with the check, not in the row's top half.
      const off = await f.evaluate(() => {
        const rows = [...document.querySelectorAll('#model-combo-list [role="option"]')].filter((o) => !o.querySelector('.opt-detail'));
        return rows.map((o) => {
          const r = o.getBoundingClientRect();
          const n = o.querySelector('.opt-name').getBoundingClientRect();
          return Math.abs((n.top + n.bottom) / 2 - (r.top + r.bottom) / 2);
        });
      });
      t.check('comboNameCentred', off.length > 0 && off.every((d) => d <= 1), JSON.stringify(off));
      await t.shot('menu-bounds-combo');
    },
  },

  // History's row menu from a right-click on the last row.
  {
    name: 'menu-bounds-history',
    backdrop: 'wallpaper',
    textScale: true,
    windows: [{ name: 'history', src: '/history/history.html', x: 520, y: 140, w: 500, h: 420, z: 1 }],
    async run(t) {
      const f = t.frame('history');
      await t.settle(600);
      await f.evaluate(() => {
        const items = document.querySelectorAll('.item');
        const last = items[items.length - 1];
        const r = last.getBoundingClientRect();
        last.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: document.documentElement.clientWidth - 16, clientY: Math.min(r.bottom, document.documentElement.clientHeight - 16) }));
      });
      await t.settle(300);
      await check(t, 'menuInsideWindow.historyRow', f, '#row-menu');
    },
  },
];
