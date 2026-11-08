// The settings lane's ui-shots scenes, mocks, checks and CSS lint list. The
// shape is documented at the top of tools/ui-shots/shots.mjs; shared fixtures
// are in tools/ui-shots/kit.mjs.
import { W, H, COMMON, streamAnswer } from '../kit.mjs';

export const mocks = {};

// B-145 for this lane's two stylesheets: radii are tokens, spacing sits on
// the 4 px grid (2 px only when a comment says optical or hairline), and raw
// colours live in :root alone.
export const cssLintEnforce = ['settings/settings.css', 'history/history.css'];

function cssProblems(css) {
  const out = [];
  const lineAt = (i) => css.slice(0, i).split('\n').length;
  for (const m of css.matchAll(/([^{}]*)\{([^{}]*)\}/g)) {
    const selector = m[1].replace(/\/\*[\s\S]*?\*\//g, '').trim();
    if (selector.startsWith('@')) continue;
    const inRoot = /:root/.test(selector);
    const start = lineAt(m.index + m[1].length);
    m[2].split('\n').forEach((line, k) => {
      const where = `line ${start + k}`;
      const decl = line.replace(/\/\*[\s\S]*?\*\//g, '');
      const marked = /\/\*\s*(optical|hairline)\b/.test(line);
      const spacing = decl.match(/^\s*(?:padding|margin|gap|row-gap|column-gap)(?:-[a-z-]+)?\s*:\s*([^;]+)/);
      if (spacing) {
        for (const px of spacing[1].matchAll(/(-?\d*\.?\d+)px/g)) {
          const v = Math.abs(Number(px[1]));
          if (v % 4 !== 0 && !(marked && v <= 2)) out.push(`${where}: ${line.trim()}`);
        }
      }
      const radius = decl.match(/^\s*border(?:-[a-z]+)*-radius\s*:\s*([^;]+)/);
      if (radius && /(^|[^\w-])[1-9]\d*(\.\d+)?px/.test(radius[1])) out.push(`${where}: ${line.trim()}`);
      if (!inRoot && /#[0-9a-fA-F]{3,8}\b|\b(?:rgba?|hsla?)\(/.test(decl)) out.push(`${where}: ${line.trim()}`);
    });
  }
  return out;
}

const lint = (file) => ({
  name: `cssOnSystem.${file.split('/')[0]}`,
  run: (ctx) => {
    const p = cssProblems(ctx.read(file));
    return p.length ? [false, `${file}: ${p.slice(0, 6).join(' | ')}${p.length > 6 ? ` (+${p.length - 6})` : ''}`] : true;
  },
});

export const checks = [
  lint('settings/settings.css'),
  lint('history/history.css'),
  // One CSS layer each (B-175): no second stylesheet, no inline style blocks.
  {
    name: 'oneCssLayer',
    run: (ctx) => {
      const bad = [];
      for (const [page, own] of [
        ['settings/settings.html', 'settings.css'],
        ['history/history.html', 'history.css'],
      ]) {
        const html = ctx.read(page);
        const sheets = [...html.matchAll(/<link[^>]+href="([^"]+\.css)"/g)].map((m) => m[1]);
        if (sheets.join() !== `../renderer/glass.css,${own}`) bad.push(`${page} loads ${sheets.join(', ')}`);
        if (/<style[\s>]/i.test(html)) bad.push(`${page} has a <style> block`);
      }
      return bad.length ? bad.join('; ') : true;
    },
  },
  // Icons are the shared sprite's spans (K-40), not page-local masks.
  {
    name: 'iconsFromSprite',
    run: (ctx) => {
      const bad = ['settings/settings.css', 'history/history.css'].filter((f) => /mask(-image)?\s*:/.test(ctx.read(f)));
      return bad.length ? `${bad.join(', ')} draws its own masked icons` : true;
    },
  },
];

const S = { x: 120, y: 130, w: 960, h: 700 };
const O = { x: 1080, y: 200, w: 480, h: 480 };
const HIST = { x: 400, y: 140, w: 720, h: 640 };

export const scenes = [
  // 08, 06: Settings, then the Liquid glass slider at 0, 0.5 and 1 beside an
  // output window that follows it live (REQUESTS.md 4, 5).
  {
    name: 'settings',
    backdrop: 'wallpaper',
    windows: [
      { name: 'settings', src: '/settings/settings.html', ...S, z: 2 },
      { name: 'o1', src: '/renderer/output.html', ...O, z: 1 },
    ],
    async run(t) {
      const st = t.frame('settings');
      const out = t.frame('o1');
      await streamAnswer(out, 'Summarise the quarterly numbers on this slide', 'Revenue grew **12%** quarter on quarter, driven by services. Margins held at 31%; hardware fell 4%.');
      await t.settle(600);
      await t.shot('08-settings-account', { x: S.x - 10, y: S.y - 10, width: S.w + 20, height: S.h + 20 });
      await st.evaluate(() => {
        showPage('pg-general');
        openSection('sec-surface');
      });
      await t.settle(400);
      for (const k of [0, 0.5, 1]) {
        await st.evaluate((kk) => {
          const r = document.getElementById('glass-range');
          r.value = String(kk);
          r.dispatchEvent(new Event('input', { bubbles: true }));
        }, k);
        await t.settle(700);
        const there = await out.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--glass-k').trim());
        t.check(`glassLive.${k}`, Number(there) === k, `output window has --glass-k ${there}`);
        const level = await st.evaluate(() => document.documentElement.dataset.glass);
        t.check(`glassTier.${k}`, k === 0 ? level === 'solid' : level === 'live', `data-glass=${level}`);
        await t.shot(`06-settings-glass-${String(Math.round(k * 100)).padStart(3, '0')}`, { x: S.x - 10, y: S.y - 10, width: O.x + O.w - S.x + 20, height: S.h + 20 });
      }
      const saved = await st.evaluate(() => new Promise((r) => setTimeout(() => r(window.__calls.filter((c) => c.channel === 'settings:set-glass').map((c) => c.payload.glassIntensity)), 300)));
      t.check('glassSaved', saved.length > 0 && saved[saved.length - 1] === 1, JSON.stringify(saved));
      t.check('noDuplicateHintInSettings', await st.evaluate(() => !/Ctrl\+1\/2\/3/.test(document.body.textContent)), 'Settings repeats the Ask/Find/Do hint');
      t.check('settingsResizeZones', await st.evaluate(() => document.querySelectorAll('.frame-handle').length === 8), 'no resize zones');
      t.check('settingsDrag', await st.evaluate(() => document.getElementById('titlebar').dataset.drag === 'max'), 'title bar is not a drag region');
    },
  },

  // 07: Settings maximised (REQUESTS.md 4).
  {
    name: 'desktop',
    backdrop: 'wallpaper',
    windows: [{ name: 'settings', src: '/settings/settings.html', x: 0, y: 0, w: W, h: H, z: 1 }],
    async run(t) {
      await t.frame('settings').evaluate(() => showPage('pg-general'));
      await t.settle(500);
      await t.shot('07-settings-fullscreen', { x: 0, y: 0, width: W, height: H });
    },
  },

  // 09: History.
  {
    name: 'history',
    backdrop: 'wallpaper',
    windows: [{ name: 'history', src: '/history/history.html', ...HIST, z: 1 }],
    async run(t) {
      await t.settle(600);
      await t.shot('09-history', { x: HIST.x - 10, y: HIST.y - 10, width: HIST.w + 20, height: HIST.h + 20 });
    },
  },
];

// ------------------------------------------------------- B-069 .. B-184

const BASE = COMMON['settings:get'];
const NOW = Date.now();
// Settings a little over its default size, Settings at its default (a
// 660 x 560 window), and History at its default.
const SW = { x: 440, y: 120, w: 700, h: 640 };
const SD = { x: 470, y: 160, w: 660, h: 560 };
const HW = { x: 520, y: 140, w: 500, h: 600 };

const NAMES = { openrouter: 'OpenRouter', anthropic: 'Anthropic', openai: 'OpenAI', gemini: 'Google Gemini' };
const HINTS = { openrouter: 'sk-or-v1…a3f9', anthropic: 'sk-ant-a…9Q2c', openai: 'sk-proj…77Lm', gemini: 'AIza…k2Pq' };

// One provider as K-19's settings:get reports it.
function provider(id, order, status, extra) {
  const hasKey = status !== 'not-added';
  return {
    id,
    name: NAMES[id],
    order,
    hasKey,
    keyHint: hasKey ? HINTS[id] : '',
    status,
    checkedAt: status === 'ready' ? NOW - 60e3 : 0,
    busyUntil: 0,
    credit: null,
    ...extra,
  };
}

// settings:get with these providers in this order, plus anything else.
function withProviders(list, extra) {
  return {
    ...BASE,
    hasKey: list.some((p) => p.id === 'openrouter' && p.hasKey),
    providers: list.map((p, i) => ({ ...p, order: i })),
    keyHints: Object.fromEntries(list.filter((p) => p.hasKey).map((p) => [p.id, p.keyHint])),
    keyStorage: 'encrypted',
    failover: { acrossProviders: true, askBeforePaid: true },
    textScale: 1,
    osTextScale: 1,
    overlayPos: { h: 'right', v: 'top' },
    ...extra,
  };
}

const IDS = ['openrouter', 'anthropic', 'openai', 'gemini'];
const READY_TWO = [
  provider('openrouter', 0, 'ready', { credit: 7.5 }),
  provider('anthropic', 1, 'ready'),
  provider('openai', 2, 'not-added'),
  provider('gemini', 3, 'not-added'),
];
const NONE = IDS.map((id, i) => provider(id, i, 'not-added'));
const one = (id, status, extra) => READY_TWO.map((p) => (p.id === id ? provider(id, p.order, status, extra) : p));
const byOrder = (list, ids) => ids.map((id) => list.find((p) => p.id === id));

const settingsWin = (r, responses) => ({ name: 'settings', src: '/settings/settings.html', ...r, z: 1, responses: responses || {} });
const historyWin = (r, responses) => ({ name: 'history', src: '/history/history.html', ...r, z: 1, responses: responses || {} });

// The pointer over an element inside a window.
async function hover(t, name, selector) {
  const f = t.frame(name);
  const box = await (await f.frameElement()).boundingBox();
  const at = await f.evaluate((s) => {
    const b = document.querySelector(s).getBoundingClientRect();
    return { x: b.left + b.width / 2, y: b.top + b.height / 2 };
  }, selector);
  await t.page.mouse.move(box.x + at.x, box.y + at.y);
}

// A page brought into view at once, the way showPage puts it, without the
// smooth scroll still running when the picture is taken.
const jump = (t, page) =>
  t.frame('settings').evaluate((p) => {
    const main = document.querySelector('main');
    const el = document.getElementById(p);
    main.style.scrollBehavior = 'auto';
    main.scrollTop = el.offsetTop - main.offsetTop;
    main.dispatchEvent(new Event('scroll'));
  }, page);

// A page, then a click on one of its rows (a disclosure or a provider).
async function open(t, page, row) {
  await jump(t, page);
  await t.settle(150);
  if (row) await t.frame('settings').click(`#${row} > .row-head > .edit, #${row} > .prov-head`);
}

// Every disclosure says whether it is open and names the body it opens.
const disclosuresOk = (f) =>
  f.evaluate(() => {
    const bad = [...document.querySelectorAll('.row-head > .edit, .prov-head')].filter((b) => {
      const e = b.getAttribute('aria-expanded');
      return !(e === 'true' || e === 'false') || !document.getElementById(b.getAttribute('aria-controls') || '');
    });
    return bad.length ? `${bad.length} without aria-expanded or a body: ${bad.map((b) => b.closest('[id]').id).join(', ')}` : true;
  });

// Each button's accessible name, worked out the way a screen reader does.
const buttonNames = (f) =>
  f.evaluate(() =>
    [...document.querySelectorAll('button')].map((b) => {
      const by = b.getAttribute('aria-labelledby');
      if (by) return by.split(/\s+/).map((id) => (document.getElementById(id) || {}).textContent || '').join(' ').trim();
      return (b.getAttribute('aria-label') || b.textContent || '').trim();
    })
  );

// 42: the Providers page in one state, that provider's row open.
function providerScene(name, list, run) {
  return {
    name: `settings-providers-${name}`,
    backdrop: 'wallpaper',
    windows: [settingsWin(SW, { 'settings:get': withProviders(list) })],
    async run(t) {
      await t.settle(500);
      await run(t, t.frame('settings'));
      await t.settle(450);
      const ok = await disclosuresOk(t.frame('settings'));
      t.check(`providerRowsHaveExpandedState.${name}`, ok === true, ok);
      await t.shot();
    },
  };
}

const openRow = (id) => (t) => open(t, 'pg-providers', `prov-${id}`);

scenes.push(
  providerScene('none', NONE, async () => {}),
  providerScene('ready-two', READY_TWO, async (t, f) => {
    await openRow('anthropic')(t);
    await t.settle(450);
    // Alt+Up on a row moves it, says so, keeps focus, and tells Rust; Rust's
    // next settings:get has the new order.
    await t.respond('settings', 'settings:get', withProviders(byOrder(READY_TWO, ['anthropic', 'openrouter', 'openai', 'gemini'])));
    await f.focus('#prov-anthropic-btn');
    await t.page.keyboard.press('Alt+ArrowUp');
    await t.settle(250);
    const calls = await t.calls('settings', 'settings:set-provider-order');
    const order = calls.length ? calls[calls.length - 1].payload.order : null;
    t.check('reorderInvokesOrder', JSON.stringify(order) === JSON.stringify(['anthropic', 'openrouter', 'openai', 'gemini']), JSON.stringify(order));
    const said = await f.evaluate(() => document.getElementById('prov-live').textContent);
    t.check('reorderIsAnnounced', said === 'Anthropic is now first. It answers by default.', said);
    const focus = await f.evaluate(() => document.activeElement && document.activeElement.id);
    t.check('reorderKeepsFocus', focus === 'prov-anthropic-btn', focus);
    const label = await f.evaluate(() => document.getElementById('prov-anthropic-btn').getAttribute('aria-label'));
    t.check('providerRowName', label === 'Anthropic, 1 of 4, Ready, default', label);
  }),
  providerScene('rejected', one('anthropic', 'rejected'), openRow('anthropic')),
  providerScene('no-credit', one('anthropic', 'no-credit'), openRow('anthropic')),
  providerScene('busy', one('openrouter', 'busy', { busyUntil: NOW + 25 * 60e3 }), openRow('openrouter')),
  providerScene('blocked', one('openrouter', 'blocked'), openRow('openrouter')),
  providerScene('testing', READY_TWO, async (t, f) => {
    await openRow('gemini')(t);
    await t.settle(400);
    await f.fill('#key-gemini', 'AIzaFAKE-not-a-real-key-000000000000');
    await t.emit('settings', 'provider:status', { id: 'gemini', status: 'checking' });
  }),
  providerScene('removing', READY_TWO, async (t, f) => {
    await openRow('anthropic')(t);
    await t.settle(450);
    await f.click('#prov-anthropic [data-act="remove"]');
    await t.settle(200);
    const keep = await f.evaluate(() => document.activeElement && document.activeElement.textContent);
    t.check('removeFocusesKeep', keep === 'Keep', keep);
    await t.shot('settings-providers-removing-confirm');
    await t.respond('settings', 'settings:get', withProviders(one('anthropic', 'not-added')));
    await f.click('#prov-anthropic [data-act="remove-yes"]');
    await t.settle(250);
    const calls = await t.calls('settings', 'settings:clear-key');
    t.check('removeSendsProvider', calls.length === 1 && calls[0].payload.provider === 'anthropic', JSON.stringify(calls));
    const back = await f.evaluate(() => document.activeElement && document.activeElement.id);
    t.check('removeReturnsFocus', back === 'prov-anthropic-btn', back);
  }),
  providerScene('wrong-provider', READY_TWO, async (t, f) => {
    const key = 'sk-proj-FAKE-not-a-real-key-00000000';
    await openRow('gemini')(t);
    await t.settle(400);
    await f.fill('#key-gemini', key);
    await f.click('#prov-gemini .prov-field [data-act="save"]');
    await t.settle(200);
    const msg = await f.evaluate(() => document.getElementById('prov-gemini-msg').textContent);
    t.check('wrongProviderSaysWhose', msg === 'This is an OpenAI key.', msg);
    t.check('wrongProviderSavesNothing', (await t.calls('settings', 'settings:save-key')).length === 0, 'saved under the wrong provider');
    await t.shot('settings-providers-wrong-provider-found');
    // Saved under its own provider: the row it lands in shows the hint only.
    await t.respond('settings', 'settings:save-key', { ok: true, status: 'ready', keyHint: 'sk-proj…0000', checkedAt: NOW });
    await t.respond('settings', 'settings:get', withProviders(one('openai', 'ready', { keyHint: 'sk-proj…0000' })));
    await f.click('#prov-gemini [data-act="save-under"]');
    await t.settle(400);
    const saved = (await t.calls('settings', 'settings:save-key')).map((c) => c.payload.provider);
    t.check('wrongProviderSavesUnderItsOwn', saved.length === 1 && saved[0] === 'openai', JSON.stringify(saved));
    const shown = await f.evaluate((k) => document.body.innerText.includes(k) || [...document.querySelectorAll('input')].some((i) => i.value.includes(k)), key);
    t.check('keyNeverEchoed', !shown, 'the full key is still in the page after the save');
  }),

  // 41: every Settings page at the default window size.
  {
    name: 'settings-pages',
    backdrop: 'wallpaper',
    windows: [settingsWin(SD, { 'settings:get': withProviders(READY_TWO, { recipients: [{ id: 'anthropic', name: 'Anthropic' }, { id: 'openrouter', name: 'OpenRouter' }] }) })],
    async run(t) {
      const f = t.frame('settings');
      await t.settle(500);
      for (const p of ['providers', 'models', 'answers', 'find', 'agent', 'privacy', 'shortcuts', 'general']) {
        await jump(t, `pg-${p}`);
        await t.settle(250);
        await t.shot(`settings-page-${p}`);
      }
      const ok = await disclosuresOk(f);
      t.check('everyDisclosureHasExpanded', ok === true, ok);
      const names = await buttonNames(f);
      const edits = names.filter((n) => /^edit\b/i.test(n));
      t.check('noButtonNamedEdit', edits.length === 0, edits.join(' | '));
      t.check('noNestedButtonsSettings', await f.evaluate(() => !document.querySelector('button button, button a, a button')), 'a control inside a button');
      const text = await f.evaluate(() => document.getElementById('recipients-text').textContent);
      t.check('privacyNamesRecipients', text.includes('to the first ready provider: Anthropic, then OpenRouter.'), text);
      const tasks = await f.evaluate(() => [...document.querySelectorAll('#sec-agent .note')].map((n) => n.textContent).join(' '));
      t.check('tasksSayWhatStopsThem', tasks.includes('Esc, the Stop button or the tray stops it; moving the mouse pauses it. Screenshots are never saved.'), tasks);
    },
  },

  // 48: one row hovered while another is open.
  {
    name: 'settings-row-hover-disclosing',
    backdrop: 'wallpaper',
    windows: [settingsWin(SW)],
    async run(t) {
      await open(t, 'pg-general', 'sec-startup');
      await t.settle(500);
      await hover(t, 'settings', '#sec-text-size > .row-head');
      await t.shot();
      const exp = await t.frame('settings').evaluate(() => document.querySelector('#sec-startup .edit').getAttribute('aria-expanded'));
      t.check('openRowSaysExpanded', exp === 'true', exp);
    },
  },

  // B-072: the slider at each stop, every name on its stop.
  {
    name: 'settings-glass',
    backdrop: 'wallpaper',
    windows: [settingsWin(SW)],
    async run(t) {
      const f = t.frame('settings');
      await open(t, 'pg-general', 'sec-surface');
      await t.settle(500);
      for (const k of [0, 0.25, 0.5, 0.75, 1]) {
        await f.evaluate((kk) => {
          const r = document.getElementById('glass-range');
          r.value = String(kk);
          r.dispatchEvent(new Event('input', { bubbles: true }));
        }, k);
        await t.settle(400);
        await t.shot(`settings-glass-${Math.round(k * 100)}`);
      }
      // The thumb is 22 px, so stop i is 11 px in plus i quarters of the rest.
      const off = await f.evaluate(() => {
        const r = document.getElementById('glass-range').getBoundingClientRect();
        return [...document.querySelectorAll('.glass-stops span')].map((s, i, all) => {
          const b = s.getBoundingClientRect();
          if (i === 0) return Math.abs(b.left - r.left);
          if (i === all.length - 1) return Math.abs(b.right - r.right);
          return Math.abs((b.left + b.right) / 2 - (r.left + 11 + ((r.width - 22) * i) / (all.length - 1)));
        });
      });
      t.check('sliderLabelsCentredOnStops', off.length === 5 && off.every((d) => d <= 1), `offsets ${off.map((d) => d.toFixed(2)).join(', ')}`);
      const snapped = await f.evaluate(() => {
        const r = document.getElementById('glass-range');
        r.value = '0.53';
        r.dispatchEvent(new Event('change', { bubbles: true }));
        return Number(r.value);
      });
      t.check('sliderSnapsToStop', snapped === 0.5, String(snapped));
    },
  },

  // 47, 50: maximized, both pages full bleed.
  {
    name: 'settings-maximized',
    backdrop: 'wallpaper',
    windows: [settingsWin({ x: 0, y: 0, w: W, h: H })],
    async run(t) {
      const f = t.frame('settings');
      await f.evaluate(() => {
        document.documentElement.dataset.maximized = '';
      });
      await t.settle(400);
      await t.shot('settings-maximized', { x: 0, y: 0, width: W, height: H });
      const bleed = await f.evaluate(() => getComputedStyle(document.body).paddingTop === '0px' && getComputedStyle(document.getElementById('app')).borderTopLeftRadius === '0px');
      t.check('settingsFullBleed', bleed, 'stage margin or corner radius left when maximized');
    },
  },
  {
    name: 'history-maximized',
    backdrop: 'wallpaper',
    windows: [historyWin({ x: 0, y: 0, w: W, h: H })],
    async run(t) {
      const f = t.frame('history');
      await f.evaluate(() => {
        document.documentElement.dataset.maximized = '';
      });
      await t.settle(500);
      await t.shot('history-maximized', { x: 0, y: 0, width: W, height: H });
      const bleed = await f.evaluate(() => getComputedStyle(document.body).paddingTop === '0px' && getComputedStyle(document.getElementById('app')).borderTopLeftRadius === '0px');
      t.check('historyFullBleed', bleed, 'stage margin or corner radius left when maximized');
    },
  },

  // One title bar system in both pages: the shared 44 px sheet bar, a title,
  // and a 28 px close.
  {
    name: 'settings-history-bars',
    backdrop: 'wallpaper',
    windows: [settingsWin({ x: 60, y: 120, w: 700, h: 640 }), historyWin({ x: 820, y: 120, w: 500, h: 600 })],
    async run(t) {
      await t.settle(500);
      const bars = [];
      for (const n of ['settings', 'history']) {
        bars.push(
          await t.frame(n).evaluate(() => {
            const bar = document.getElementById('titlebar');
            const close = bar.querySelector('.sheet-bar-tools > .g-icon');
            const r = close && close.getBoundingClientRect();
            return {
              sheetBar: bar.classList.contains('sheet-bar') && !!bar.querySelector('.sheet-bar-title'),
              height: Math.round(bar.getBoundingClientRect().height),
              close: r ? Math.round(r.width) : 0,
              icon: !!(close && close.querySelector('.ico.i-x')),
            };
          })
        );
      }
      t.check('oneTitleBarSystem', bars.every((b) => b.sheetBar && b.height === 44 && b.close === 28 && b.icon), JSON.stringify(bars));
      await t.shot();
    },
  },

  // B-074: Text size.
  {
    name: 'settings-text-size',
    backdrop: 'wallpaper',
    windows: [settingsWin(SW, { 'settings:get': { ...BASE, textScale: 1, osTextScale: 1.25 } })],
    async run(t) {
      const f = t.frame('settings');
      await open(t, 'pg-general', 'sec-text-size');
      await t.settle(500);
      await f.evaluate(() => {
        const r = document.getElementById('text-scale');
        r.value = '150';
        r.dispatchEvent(new Event('input', { bubbles: true }));
      });
      await t.settle(400);
      const calls = await t.calls('settings', 'settings:set-text-scale');
      t.check('textSizeRowInvokes', calls.length > 0 && calls[calls.length - 1].payload.scale === 1.5, JSON.stringify(calls));
      await t.shot();
    },
  },

  // 43: Models, Automatic, per provider, and an open picker.
  {
    name: 'settings-models-automatic',
    backdrop: 'wallpaper',
    windows: [settingsWin(SW)],
    async run(t) {
      await jump(t, 'pg-models');
      await t.settle(400);
      await t.shot();
    },
  },
  {
    name: 'settings-models-anthropic',
    backdrop: 'wallpaper',
    windows: [settingsWin(SW, { 'settings:get': withProviders(READY_TWO, { model: 'qwen/qwen-2.5-vl-72b-instruct:free' }) })],
    async run(t) {
      const f = t.frame('settings');
      await jump(t, 'pg-models');
      await t.settle(300);
      await f.click('#models-for .seg-option[data-provider="anthropic"]');
      await t.settle(300);
      const listed = (await t.calls('settings', 'models:list')).map((c) => c.payload.provider || 'openrouter');
      t.check('modelListPerProvider', listed.includes('anthropic'), JSON.stringify(listed));
      await f.click('#sec-model > .row-head > .edit');
      await t.settle(450);
      await t.shot();
      await f.click('#model-combo-btn');
      await t.settle(300);
      await t.shot('settings-models-anthropic-open');
      await f.click('#model-combo-list [role="option"][aria-selected="false"]');
      await t.settle(200);
      const calls = await t.calls('settings', 'settings:save-model');
      const last = calls[calls.length - 1];
      t.check('modelSaveSendsProvider', !!last && last.payload.provider === 'anthropic', JSON.stringify(calls));
    },
  },
  {
    name: 'settings-combo-open',
    backdrop: 'wallpaper',
    windows: [settingsWin(SW, { 'settings:get': { ...BASE, model: 'mistralai/mistral-small-3.1-24b-instruct:free' } })],
    async run(t) {
      const f = t.frame('settings');
      await jump(t, 'pg-models');
      await t.settle(300);
      await f.click('#sec-model > .row-head > .edit');
      await t.settle(450);
      await f.click('#model-combo-btn');
      await t.settle(300);
      const st = await f.evaluate(() => ({
        groups: [...document.querySelectorAll('#model-combo-list [role="group"]')].every((g) => !!g.getAttribute('aria-label')),
        checks: document.querySelectorAll('#model-combo-list [role="option"] > .g-check').length === document.querySelectorAll('#model-combo-list [role="option"]').length,
        focus: document.activeElement && document.activeElement.getAttribute('role'),
      }));
      t.check('menuIsTheSharedMenu', st.groups && st.checks && st.focus === 'option', JSON.stringify(st));
      await t.shot();
    },
  },

  // Choice groups are one tab stop, and the arrows move and choose.
  {
    name: 'settings-find-choices',
    backdrop: 'wallpaper',
    windows: [settingsWin(SW)],
    async run(t) {
      const f = t.frame('settings');
      await open(t, 'pg-find', 'sec-find');
      await t.settle(450);
      await f.focus('[data-find-scope="window"]');
      await t.page.keyboard.press('ArrowDown');
      await t.settle(150);
      const st = await f.evaluate(() => {
        const g = document.querySelector('[data-find-scope]').closest('[role="radiogroup"]');
        const radios = [...g.querySelectorAll('[role="radio"]')];
        return {
          focused: document.activeElement.dataset.findScope,
          tabbable: radios.filter((r) => r.tabIndex === 0).map((r) => r.dataset.findScope),
        };
      });
      const sent = (await t.calls('settings', 'settings:set-find')).map((c) => c.payload.scope).filter(Boolean);
      t.check('radiosRoveWithArrows', st.focused === 'screen' && sent.includes('screen'), JSON.stringify({ st, sent }));
      t.check('radiosOneTabStop', st.tabbable.length === 1, JSON.stringify(st.tabbable));
      await t.shot();
    },
  },

  // Delete all asks in place, with Keep focused.
  {
    name: 'settings-delete-all',
    backdrop: 'wallpaper',
    windows: [settingsWin(SW)],
    async run(t) {
      const f = t.frame('settings');
      await open(t, 'pg-privacy', 'sec-history');
      await t.settle(450);
      await f.click('#clear-history');
      await t.settle(150);
      const st = await f.evaluate(() => ({
        text: document.getElementById('history-confirm-text').textContent,
        shown: !document.getElementById('history-confirm').hidden,
        focus: document.activeElement.id,
      }));
      t.check('deleteAllConfirmsInline', st.shown && st.text === 'Delete all 4 conversations?' && st.focus === 'history-confirm-no', JSON.stringify(st));
      await t.shot();
    },
  },

  // 44: Shortcuts, recording, a conflict with free alternatives, and F1.
  {
    name: 'settings-shortcuts',
    backdrop: 'wallpaper',
    windows: [settingsWin(SW)],
    async run(t) {
      const f = t.frame('settings');
      await open(t, 'pg-shortcuts', 'sec-shortcuts');
      await t.settle(450);
      await f.click('.sc-row[data-action="captureQuery"] .sc-change');
      await t.settle(150);
      await t.shot('settings-shortcuts-capturing');
      await t.respond('settings', 'shortcuts:set', { ok: false, error: 'Another app already uses that shortcut. Try another.', free: ['Ctrl+Alt+K', 'Ctrl+Shift+Space'] });
      await t.page.keyboard.press('Control+Alt+J');
      await t.settle(200);
      const free = await f.evaluate(() => [...document.querySelectorAll('#shortcut-free [data-free]')].map((b) => b.dataset.free));
      await t.shot('settings-shortcuts-conflict');
      await t.respond('settings', 'shortcuts:set', { ok: true, shortcuts: { ...BASE.shortcuts, captureQuery: 'Ctrl+Alt+K' }, active: { ...BASE.active, captureQuery: true } });
      await f.click('#shortcut-free [data-free="Ctrl+Alt+K"]');
      await t.settle(200);
      const sets = (await t.calls('settings', 'shortcuts:set')).map((c) => c.payload.accelerator);
      t.check('shortcutConflictOffersFree', free.join() === 'Ctrl+Alt+K,Ctrl+Shift+Space' && sets[0] === 'Ctrl+Alt+J' && sets[sets.length - 1] === 'Ctrl+Alt+K', JSON.stringify({ free, sets }));
      const f1 = await f.evaluate(() => {
        document.activeElement.blur();
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'F1', bubbles: true }));
        return document.activeElement === document.querySelector('#pg-shortcuts h1');
      });
      t.check('f1OpensShortcuts', f1, 'F1 did not go to Shortcuts');
    },
  },

  // General, at the usual text size and at 200%.
  {
    name: 'settings-general',
    backdrop: 'wallpaper',
    textScale: true,
    windows: [settingsWin(SW, { 'settings:get': withProviders(READY_TWO) })],
    async run(t) {
      await jump(t, 'pg-general');
      await t.settle(400);
      await t.shot();
      const pos = await t.frame('settings').evaluate(() => document.getElementById('layout-summary').textContent);
      t.check('barPositionSummary', pos === 'Top right', pos);
    },
  },

  // 49, 50: History.
  {
    name: 'history-list',
    backdrop: 'wallpaper',
    windows: [historyWin(HW)],
    async run(t) {
      const f = t.frame('history');
      await t.settle(500);
      await t.shot();
      t.check('noNestedButtons', await f.evaluate(() => !document.querySelector('button button, button [role="button"], [role="button"] button')), 'a button inside a button');
      const rest = await f.evaluate(() => getComputedStyle(document.querySelector('.search-field')).outlineStyle);
      t.check('searchQuietAtRest', rest === 'none', rest);
      await f.focus('.item-open');
      await t.page.keyboard.press('ArrowDown');
      const second = await f.evaluate(() => document.activeElement === document.querySelectorAll('.item-open')[1]);
      await t.page.keyboard.press('ArrowUp');
      await t.page.keyboard.press('ArrowUp');
      const search = await f.evaluate(() => document.activeElement.id);
      t.check('historyArrowsMove', second && search === 'search', JSON.stringify({ second, search }));
    },
  },
  {
    name: 'history-row-hover',
    backdrop: 'wallpaper',
    windows: [historyWin(HW)],
    async run(t) {
      await t.settle(500);
      await hover(t, 'history', '.item[data-id="b"] .item-open');
      await t.shot();
    },
  },
  {
    name: 'history-search-results',
    backdrop: 'wallpaper',
    windows: [historyWin(HW)],
    async run(t) {
      const f = t.frame('history');
      await t.settle(400);
      await t.respond('history', 'history:list', { rows: COMMON['history:list'].rows.filter((r) => /build/i.test(r.title + r.reply)), enabled: true });
      await f.fill('#search', 'build');
      await t.settle(500);
      const queried = (await t.calls('history', 'history:list')).map((c) => c.payload && c.payload.query);
      t.check('historySearchAsksRust', queried.includes('build'), JSON.stringify(queried));
      await t.shot();
    },
  },
  {
    name: 'history-empty',
    backdrop: 'wallpaper',
    windows: [historyWin(HW, { 'history:list': { rows: [], enabled: true } })],
    async run(t) {
      await t.settle(500);
      await t.shot();
    },
  },
  {
    name: 'history-off',
    backdrop: 'wallpaper',
    windows: [historyWin(HW, { 'history:list': { rows: [], enabled: false } })],
    async run(t) {
      const f = t.frame('history');
      await t.settle(500);
      const st = await f.evaluate(() => ({ text: document.getElementById('empty-text').textContent, turnOn: !document.getElementById('empty-turn-on').hidden }));
      t.check('historyOffSaysOff', st.text === 'History is off, so nothing is being saved. Turn it on in Settings, Privacy.' && st.turnOn, JSON.stringify(st));
      await t.shot();
    },
  },
  {
    name: 'history-delete-all',
    backdrop: 'wallpaper',
    windows: [historyWin(HW)],
    async run(t) {
      const f = t.frame('history');
      await t.settle(500);
      await f.click('#clear-all');
      await t.settle(150);
      const st = await f.evaluate(() => ({ text: document.getElementById('confirm-text').textContent, focus: document.activeElement.id }));
      t.check('historyDeleteAllInline', st.text === 'Delete all 4 conversations?' && st.focus === 'confirm-no', JSON.stringify(st));
      await t.shot();
    },
  },
  {
    name: 'history-deleted-undo',
    backdrop: 'wallpaper',
    windows: [historyWin(HW)],
    async run(t) {
      const f = t.frame('history');
      await t.settle(500);
      await f.click('.item[data-id="b"] .del');
      await t.settle(150);
      const st = await f.evaluate(() => ({
        gone: !document.querySelector('.item[data-id="b"]'),
        note: document.getElementById('note').textContent,
        undo: !document.getElementById('undo').hidden,
      }));
      await t.shot();
      const early = (await t.calls('history', 'history:remove')).length;
      await f.click('#undo');
      await t.settle(150);
      const back = await f.evaluate(() => !!document.querySelector('.item[data-id="b"]'));
      // Deleted again and left alone: the six seconds run out and Rust is told.
      await f.focus('.item[data-id="b"] .item-open');
      await t.page.keyboard.press('Delete');
      await t.settle(6400);
      const late = (await t.calls('history', 'history:remove')).map((c) => c.payload.id);
      t.check(
        'deleteIsUndoable',
        st.gone && st.note === 'Deleted "Summarise the quarterly numbers on this slide".' && st.undo && early === 0 && back && late.join() === 'b',
        JSON.stringify({ st, early, back, late })
      );
    },
  }
);
