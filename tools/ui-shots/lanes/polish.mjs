// The polish lane's ui-shots scenes: every kind of menu, opened where it has
// the least room, ends inside its own window (a WebView cannot draw past its
// window, so anything outside is cut). The shape is documented at the top of
// tools/ui-shots/shots.mjs.
import { ANSWER, BAR, openBar, streamAnswer } from '../kit.mjs';

export const mocks = {};
export const checks = [];
export const cssLintEnforce = [];

// Rust's cap on the bar window (layout.rs INPUT_MAX_HEIGHT) plus the 20 px
// stage on both sides: the mock grows the frame to the page's ask, at most
// this.
const BAR_MAX = 400 + 40;
const overlay = { name: 'overlay', src: '/renderer/index.html', x: BAR.x, y: BAR.y, w: BAR.w, h: 116, z: 2, max: BAR_MAX };

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

const check = async (t, name, f, selector) => {
  const v = await inside(f, selector);
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

export const scenes = [
  // The token's menu under a four-line question, and the @ menu with eight
  // models: the bar window grows to hold them, up to Rust's cap.
  {
    name: 'menu-bounds-bar',
    backdrop: 'wallpaper',
    windows: [{ ...overlay, responses: { 'models:list': MODELS } }],
    async run(t) {
      const f = t.frame('overlay');
      await seenBefore(f);
      await openBar(f);
      await t.settle();
      await setField(f, 'Why does the build fail\non the release branch\nbut not on main\nsince Tuesday?');
      await t.settle(200);
      await f.evaluate(() => document.getElementById('intent').click());
      await t.settle(400);
      await check(t, 'menuInsideWindow.barIntent', f, '#intent-menu');
      await t.shot('menu-bounds-bar-intent', { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: BAR_MAX + 40 });
      await f.evaluate(() => document.getElementById('intent').click());
      await setField(f, '@');
      await t.settle(400);
      await check(t, 'menuInsideWindow.barModel', f, '#model-menu');
      await t.shot('menu-bounds-bar-model', { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: BAR_MAX + 40 });
    },
  },

  // Retry's model menu from an answer in a short window: it cannot fit
  // under the chevron, so it opens above it or scrolls, inside the window.
  {
    name: 'menu-bounds-retry',
    backdrop: 'wallpaper',
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
    },
  },

  // The window menu from a right-click near the window's bottom right.
  {
    name: 'menu-bounds-window',
    backdrop: 'wallpaper',
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
    windows: [{ name: 'settings', src: '/settings/settings.html', x: 470, y: 160, w: 660, h: 520, z: 1 }],
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
      await t.shot('menu-bounds-combo');
    },
  },

  // History's row menu from a right-click on the last row.
  {
    name: 'menu-bounds-history',
    backdrop: 'wallpaper',
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
