// The bar lane's ui-shots scenes, mocks, checks and CSS lint list. The shape
// is documented at the top of tools/ui-shots/shots.mjs; shared fixtures are
// in tools/ui-shots/kit.mjs.
import { BAR, OUT1, barContext, barRect, barState, openBar } from '../kit.mjs';

export const mocks = {};
export const checks = [];
// glass.css goes on this list once the lint itself (B-145) has landed and the
// file has been run through it.
export const cssLintEnforce = [];

const overlay = { name: 'overlay', src: '/renderer/index.html', x: BAR.x, y: BAR.y, w: BAR.w, h: BAR.h, z: 2 };
const BAR_CLIP = { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: 140 };
const TALL_CLIP = { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: 300 };
const TARGET = { label: 'output-2', title: 'Why does the build fail on the release branch?' };

// The suggestions row is the first run's: every other shot is without it.
const seenBefore = (f) => f.evaluate(() => localStorage.setItem('aeval.hasSentFirstRequest', '1'));

const setField = (f, text) =>
  f.evaluate((v) => {
    const i = document.getElementById('query-input');
    i.focus();
    i.value = v;
    i.dispatchEvent(new Event('input', { bubbles: true }));
  }, text);

const fieldValue = (f) => f.evaluate(() => document.getElementById('query-input').value);

// A keydown on the field, as the page's own handlers see one. True when
// something took it.
const press = (f, init) =>
  f.evaluate((k) => {
    const e = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...k });
    document.getElementById('query-input').dispatchEvent(e);
    return e.defaultPrevented;
  }, init);

const tokenState = (f) =>
  f.evaluate(() => {
    const t = document.getElementById('intent');
    return {
      intent: t.dataset.intent,
      locked: t.dataset.locked,
      rest: t.dataset.rest,
      name: t.querySelector('.intent-name').textContent,
      label: t.getAttribute('aria-label'),
      disabled: t.getAttribute('aria-disabled'),
    };
  });

// The glass at a liquid level, the way Rust repeats it with every frame.
const setLevel = (t, name, k) =>
  t.frame(name).evaluate(
    ({ img, level }) => window.__emit('glass:frame', { img, display: { x: 0, y: 0, width: 1600, height: 1000 }, tier: 'live', reasons: [], intensity: level }),
    { img: t.backdrop.small, level: k }
  );

const glassClip = async (f) => {
  const r = await barRect(f);
  return { x: Math.round(r.x), y: Math.round(r.y), width: Math.round(r.width), height: Math.round(r.height) };
};

const base64 = async (t, clip) => (await t.page.screenshot({ clip })).toString('base64');

// Runs in the scene page, whose only job is to hold the frames: decodes a
// PNG and hands back its pixels.
const DECODE = `async (s) => {
  const im = new Image();
  im.src = 'data:image/png;base64,' + s;
  await im.decode();
  const c = document.createElement('canvas');
  c.width = im.width;
  c.height = im.height;
  const x = c.getContext('2d');
  x.drawImage(im, 0, 0);
  return x.getImageData(0, 0, c.width, c.height).data;
}`;

// The share of pixels that moved by more than `by` levels in some channel.
const changedShare = (t, a, b, by) =>
  t.page.evaluate(
    async ({ a: pa, b: pb, by: limit, decode }) => {
      const load = eval(decode);
      const [p, q] = [await load(pa), await load(pb)];
      let n = 0;
      for (let i = 0; i < p.length; i += 4) {
        if (Math.abs(p[i] - q[i]) > limit || Math.abs(p[i + 1] - q[i + 1]) > limit || Math.abs(p[i + 2] - q[i + 2]) > limit) n += 1;
      }
      return n / (p.length / 4);
    },
    { a, b, by, decode: DECODE }
  );

// WCAG contrast of a colour against the median of a shot: the glass under
// text, since text covers well under half of a row.
const contrastOver = (t, shot, color) =>
  t.page.evaluate(
    async ({ s, fg, decode }) => {
      const lin = (v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
      const lum = (r, g, b) => 0.2126 * lin(r / 255) + 0.7152 * lin(g / 255) + 0.0722 * lin(b / 255);
      const load = eval(decode);
      const p = await load(s);
      const all = [];
      for (let i = 0; i < p.length; i += 4) all.push(lum(p[i], p[i + 1], p[i + 2]));
      all.sort((x, y) => x - y);
      const bg = all[Math.floor(all.length / 2)];
      const f = lum(...fg);
      return (Math.max(f, bg) + 0.05) / (Math.min(f, bg) + 0.05);
    },
    { s: shot, fg: color, decode: DECODE }
  );

const accentRgb = (f) =>
  f.evaluate(() => {
    const el = document.createElement('span');
    el.style.color = 'var(--accent-text)';
    document.body.appendChild(el);
    const rgb = getComputedStyle(el).color.match(/[\d.]+/g).slice(0, 3).map(Number);
    el.remove();
    return rgb;
  });

// design-visual section 3: every token a page may use.
const TOKENS = [
  '--t-bar', '--t-body', '--t-caption', '--t-code', '--t-meta', '--t-page', '--t-row', '--t-title',
  '--font-small', '--w-body', '--w-strong',
  '--sp-1', '--sp-2', '--sp-3', '--sp-4', '--sp-5', '--sp-6', '--stage',
  '--inset-menu', '--inset-row', '--inset-text',
  '--r-sheet', '--r-card', '--r-plate', '--r-row', '--r-key', '--r-cap',
  '--shadow-control', '--shadow-sheet', '--shadow-menu',
  '--ease-glide', '--ease-morph', '--ease-out', '--ease-settle', '--ease-spring',
  '--d-fast', '--d-glob', '--d-med', '--d-morph', '--d-press', '--d-reduced', '--d-reveal', '--d-sheet',
  '--ctl', '--ctl-s', '--bar-h', '--sheet-bar-h', '--ico', '--ico-s', '--ico-i',
  '--accent-text', '--accent-bright', '--text-scale',
];

export const scenes = [
  // 01, 02: the bar before and after the first request (REQUESTS.md 3, 6).
  {
    name: 'bar',
    backdrop: 'wallpaper',
    windows: [{ name: 'overlay', src: '/renderer/index.html', x: BAR.x, y: BAR.y, w: BAR.w, h: BAR.h, z: 2 }],
    async run(t) {
      const f = t.frame('overlay');
      await f.evaluate(() => localStorage.clear());
      await openBar(f);
      await t.settle();
      await t.shot('01-bar-first-run', { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: 200 });
      const shown = await f.evaluate(() => !document.getElementById('examples').hidden && document.querySelectorAll('#examples button').length);
      t.check('suggestionsBeforeFirst', shown > 0, `examples shown=${shown}`);
      t.check('noLegend', await f.evaluate(() => !document.getElementById('legend')), 'the sigil legend is back beside the input');
      const hintOnToken = await f.evaluate(() => /Ctrl\+1/.test(document.getElementById('intent').title));
      t.check('hintOnLeftButton', hintOnToken, 'the token lost its shortcut hint');
      // Send one question.
      await f.evaluate(() => {
        const i = document.getElementById('query-input');
        i.value = 'why does the build fail';
        i.dispatchEvent(new Event('input'));
        i.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
      });
      await t.settle(200);
      const after = await f.evaluate(() => document.getElementById('examples').hidden);
      t.check('suggestionsGoneAfterFirst', after, 'examples still showing after a request');
      const sent = await f.evaluate(() => window.__calls.some((c) => c.channel === 'query:submit'));
      t.check('barSends', sent, 'Enter did not submit');
      // A new session (the bar reopened, a new context): still none.
      await f.evaluate((cx) => {
        window.__emit('overlay:state', { mode: 'pill' });
        window.__emit('overlay:state', { mode: 'panel' });
        window.__emit('bar:context', cx);
      }, barContext);
      await t.settle(300);
      t.check('suggestionsNeverAgain', await f.evaluate(() => document.getElementById('examples').hidden), 'examples came back on reopen');
      await t.shot('02-bar-after-first', { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: 140 });
    },
  },

  // B-054: the tokens resolve, the bar's double focus ring, the shared menu.
  {
    name: 'bar-focus',
    backdrop: 'wallpaper',
    textScale: true,
    windows: [overlay],
    async run(t) {
      const f = t.frame('overlay');
      await seenBefore(f);
      await openBar(f);
      await t.settle();
      const missing = await f.evaluate((names) => {
        const css = getComputedStyle(document.documentElement);
        return names.filter((n) => !css.getPropertyValue(n).trim());
      }, TOKENS);
      t.check('tokensPresent', missing.length === 0, missing.join(' '));
      await f.focus('#query-input');
      await t.shot('bar-focus-ring', BAR_CLIP);
      await press(f, { key: 'ArrowDown', altKey: true });
      await t.settle(320);
      const menu = await f.evaluate(() => {
        const m = document.getElementById('intent-menu');
        return !m.hidden && m.classList.contains('g-menu') && m.querySelectorAll('[role="option"]').length;
      });
      t.check('tokenMenuIsGMenu', menu === 3, `menu=${menu}`);
      await t.shot('bar-menu', TALL_CLIP);
    },
  },

  // B-055: the five liquid levels, then the two stops that must look apart.
  // The other two backdrops of design-visual state 45 are not in the
  // harness's list yet.
  {
    name: 'glass-levels',
    backdrop: 'wallpaper',
    windows: [overlay],
    async run(t) {
      const f = t.frame('overlay');
      await seenBefore(f);
      await openBar(f);
      await setField(f, 'What does this error mean?');
      await t.settle();
      const clip = await glassClip(f);
      const frames = {};
      for (const k of [0, 0.25, 0.5, 0.75, 1]) {
        await setLevel(t, 'overlay', k);
        await t.settle(350);
        await t.shot(`glass-level-${k * 100}`, BAR_CLIP);
        frames[k] = await base64(t, clip);
      }
      const share = await changedShare(t, frames[0.5], frames[1], 8);
      t.check('glassStopsDiffer', share > 0.05, `${(share * 100).toFixed(1)}% of the bar changed from 0.5 to 1.0`);
    },
  },
  {
    name: 'glass-levels-white',
    backdrop: 'white',
    windows: [overlay],
    async run(t) {
      const f = t.frame('overlay');
      await seenBefore(f);
      await openBar(f);
      await t.settle();
      for (const k of [0, 0.25, 0.5, 0.75, 1]) {
        await setLevel(t, 'overlay', k);
        await t.settle(350);
        await t.shot(`glass-level-${k * 100}-white`, BAR_CLIP);
      }
      // The worst case for accent text: the most liquid glass over white,
      // measured on the empty field's part of the bar.
      const r = await f.evaluate(() => {
        const b = document.getElementById('query-input').getBoundingClientRect();
        return { x: window.screenX + b.left, y: window.screenY + b.top, width: b.width, height: b.height };
      });
      const clip = { x: Math.round(r.x + r.width * 0.5), y: Math.round(r.y), width: Math.round(r.width * 0.4), height: Math.round(r.height) };
      const ratio = await contrastOver(t, await base64(t, clip), await accentRgb(f));
      t.check('accentTextContrast', ratio >= 4.5, `${ratio.toFixed(2)}:1`);
    },
  },

  // B-056: three rows, Shift+Enter, and Enter sends the line breaks.
  {
    name: 'bar-multiline',
    backdrop: 'wallpaper',
    textScale: true,
    windows: [overlay],
    async run(t) {
      const f = t.frame('overlay');
      await seenBefore(f);
      await openBar(f);
      await t.settle();
      await f.focus('#query-input');
      await t.page.keyboard.type('why does the build fail?');
      await t.page.keyboard.press('Shift+Enter');
      await t.page.keyboard.type('what does LNK1104 mean?');
      await t.page.keyboard.press('Shift+Enter');
      await t.page.keyboard.type('how do I fix it?');
      await t.settle(200);
      const rows = await f.evaluate(() => document.getElementById('bar').dataset.rows);
      t.check('shiftEnterAddsRows', Number(rows) >= 3, `rows=${rows}`);
      await t.shot('bar-multiline', TALL_CLIP);
      await t.page.keyboard.press('Enter');
      await t.settle(100);
      const sent = await t.calls('overlay', 'query:submit');
      const breaks = sent.length ? sent[0].payload.question.split('\n').length - 1 : -1;
      t.check('multilineSends', breaks === 2, `line breaks sent=${breaks}`);
    },
  },

  // B-057: the chip names the window only when there is a choice of two.
  {
    name: 'bar-target-chip',
    backdrop: 'wallpaper',
    windows: [overlay],
    async run(t) {
      const f = t.frame('overlay');
      await seenBefore(f);
      await openBar(f, { target: TARGET, visibleOutputs: 1 });
      await t.settle();
      const chip = () => f.evaluate(() => !document.getElementById('target-chip').hidden);
      const one = await chip();
      await openBar(f, { target: TARGET, visibleOutputs: 2 });
      await t.settle(300);
      const two = await chip();
      const text = await f.evaluate(() => document.getElementById('target-text').textContent);
      const desc = await f.evaluate(() => document.getElementById('target-desc').textContent);
      t.check('targetChipOnlyWithTwoWindows', !one && two, `one=${one} two=${two}`);
      t.check('targetChipCutsTitle', text.endsWith('…') && text.length <= 40, `chip says "${text}"`);
      t.check('targetChipDescribesField', desc.includes(TARGET.title), `description "${desc}"`);
      await t.shot('bar-target-chip', BAR_CLIP);
      await press(f, { key: 'Tab', ctrlKey: true });
      await press(f, { key: 'Tab', ctrlKey: true, shiftKey: true });
      const dirs = (await t.calls('overlay', 'output:cycle-target')).map((c) => c.payload.dir);
      t.check('ctrlTabCycles', dirs.join(',') === '1,-1', `dirs=${dirs.join(',')}`);
      await f.click('#target-new');
      t.check('targetChipStartsOver', (await t.calls('overlay', 'chat:new')).length === 1, 'the x did not send chat:new');
    },
  },

  // B-058, B-059, B-060, B-151, B-153: what the keys and the voice do.
  {
    name: 'bar-keys',
    backdrop: 'wallpaper',
    scales: [1],
    windows: [overlay],
    async run(t) {
      const f = t.frame('overlay');
      await seenBefore(f);
      await f.evaluate(() => localStorage.removeItem('aeval.textScale'));
      await openBar(f, { target: TARGET, visibleOutputs: 2 });
      await t.settle();

      // Esc: the text first, then the bar.
      await setField(f, 'half a thought');
      await press(f, { key: 'Escape' });
      const cleared = await fieldValue(f);
      const before = (await t.calls('overlay', 'overlay:collapse')).length;
      await press(f, { key: 'Escape' });
      const after = (await t.calls('overlay', 'overlay:collapse')).length;
      t.check('escClearsThenCloses', cleared === '' && before === 0 && after === 1, `text="${cleared}" collapses ${before} then ${after}`);
      const closeBtn = await f.evaluate(() => {
        const b = document.getElementById('close-btn');
        return `${b.getAttribute('aria-label')}|${b.title}`;
      });
      t.check('closeButtonNamed', closeBtn === 'Close the bar|Close (Esc)', closeBtn);

      // Each chord reaches the channel it names.
      const chords = [
        [{ key: 'n', ctrlKey: true }, 'chat:new'],
        [{ key: 'w', ctrlKey: true }, 'output:close'],
        [{ key: 'W', ctrlKey: true, shiftKey: true }, 'output:close-all'],
        [{ key: 'C', ctrlKey: true, shiftKey: true }, 'output:last-answer'],
        [{ key: '.', code: 'Period', ctrlKey: true }, 'query:cancel'],
        [{ key: 'R', ctrlKey: true, shiftKey: true }, 'query:retry'],
        [{ key: 'r', ctrlKey: true }, 'region:pick'],
        [{ key: ',', code: 'Comma', ctrlKey: true }, 'overlay:open-settings'],
        [{ key: 'h', ctrlKey: true }, 'overlay:open-history'],
        [{ key: 'F6' }, 'ui:focus-answer'],
      ];
      const silent = [];
      for (const [init, channel] of chords) {
        const n = (await t.calls('overlay', channel)).length;
        await press(f, init);
        await t.settle(30);
        if ((await t.calls('overlay', channel)).length !== n + 1) silent.push(channel);
      }
      t.check('barShortcutsInvoke', silent.length === 0, silent.join(' '));
      const close = await t.calls('overlay', 'output:close');
      const retry = await t.calls('overlay', 'query:retry');
      t.check('chordsNameTheTarget', close[0].payload.label === TARGET.label && retry[0].payload.label === TARGET.label, JSON.stringify([close[0], retry[0]]));

      // Ctrl+=, Ctrl+- and Ctrl+0 step the stored factor.
      await press(f, { key: '=', ctrlKey: true });
      await t.settle(60);
      await press(f, { key: '-', ctrlKey: true });
      await t.settle(60);
      await press(f, { key: '0', ctrlKey: true });
      await t.settle(60);
      const scales = (await t.calls('overlay', 'settings:set-text-scale')).map((c) => c.payload.scale);
      t.check('zoomKeysInvokeTextScale', scales.join(',') === '1.1,1,1', `scales=${scales.join(',')}`);
      await f.evaluate(() => localStorage.removeItem('aeval.textScale'));

      // The half-typed flag, and Rust's order to clear it.
      await setField(f, 'an answer for the task');
      await t.settle(200);
      const dirty = (await t.calls('overlay', 'bar:dirty')).map((c) => c.payload.dirty);
      await t.emit('overlay', 'bar:clear', {});
      await t.settle(200);
      const emptied = await fieldValue(f);
      const focused = await f.evaluate(() => document.activeElement && document.activeElement.id);
      const flags = (await t.calls('overlay', 'bar:dirty')).map((c) => c.payload.dirty);
      const ok = dirty[dirty.length - 1] === true && emptied === '' && focused === 'query-input' && flags[flags.length - 1] === false;
      t.check('dirtyFlagSentAndClearHonoured', ok, `dirty=${flags.join(',')} text="${emptied}" focus=${focused}`);

      // Speaking: here while this window has focus, through Rust when not.
      await f.evaluate(() => {
        document.activeElement.blur();
        window.top.focus();
      });
      await t.settle(60);
      const away = await f.evaluate(() => document.hasFocus());
      const sentBefore = (await t.calls('overlay', 'a11y:announce')).length;
      await f.evaluate(() => window.AevalA11y.say('Said while away', 'polite'));
      const relayed = await t.calls('overlay', 'a11y:announce');
      await f.focus('#query-input');
      await t.settle(60);
      const here = await f.evaluate(() => document.hasFocus());
      await f.evaluate(() => window.AevalA11y.say('Said right here', 'assertive'));
      await t.settle(80);
      const live = await f.evaluate(() => document.getElementById('announce-alert').textContent);
      const relayedAfter = (await t.calls('overlay', 'a11y:announce')).length;
      const routes = !away && relayed.length === sentBefore + 1 && relayed[sentBefore].payload.text === 'Said while away' && here && live === 'Said right here' && relayedAfter === relayed.length;
      t.check('sayRoutesByFocus', routes, `away=${away} relayed=${relayed.length - sentBefore} here=${here} live="${live}"`);
    },
  },

  // B-150: a glyph at rest, a word when there is something to say, and the
  // truth about Enter while a task waits.
  {
    name: 'bar-token',
    backdrop: 'wallpaper',
    textScale: true,
    windows: [overlay],
    async run(t) {
      const f = t.frame('overlay');
      await seenBefore(f);
      await openBar(f);
      await t.settle();
      const atRest = await tokenState(f);
      t.check('tokenRestsAsGlyph', atRest.rest === 'true' && atRest.locked === 'false', JSON.stringify(atRest));
      await t.shot('bar-token-rest', BAR_CLIP);
      await setField(f, 'LNK1104');
      await press(f, { key: '2', ctrlKey: true });
      await t.settle(320);
      const find = await tokenState(f);
      t.check('tokenShowsChoice', find.intent === 'find' && find.locked === 'true' && find.rest === 'false', JSON.stringify(find));
      await t.shot('bar-token-find', BAR_CLIP);
      await setField(f, '');
      await press(f, { key: 'Backspace' });

      await openBar(f, { pillState: 'agent', agent: { running: true, step: 3, state: 'asking' } });
      await t.settle(320);
      const asking = await tokenState(f);
      await t.shot('bar-token-reply', BAR_CLIP);
      await setField(f, 'the second one');
      await press(f, { key: 'Enter' });
      await t.settle(60);
      const reply = await t.calls('overlay', 'agent:continue');
      const asked = (await t.calls('overlay', 'query:submit')).length;
      const says = asking.name === 'Reply' && asking.label === 'Reply. Enter sends this to the running task.' && asking.disabled === 'true';
      t.check('tokenSaysReplyWhileAsking', says && reply.length === 1 && reply[0].payload.text === 'the second one' && asked === 0, JSON.stringify({ asking, reply, asked }));

      await setField(f, '');
      await openBar(f, { pillState: 'agent', agent: { running: true, step: 1, state: 'plan' } });
      await t.settle(320);
      const plan = await tokenState(f);
      t.check('tokenSaysStartOnPlan', plan.name === 'Start' && plan.disabled === 'true', JSON.stringify(plan));
      await t.shot('bar-token-start', BAR_CLIP);
      await openBar(f);
    },
  },

  // B-155, B-156: what the bar says with no provider, or a taken hotkey.
  {
    name: 'bar-notices',
    backdrop: 'wallpaper',
    windows: [{ ...overlay, responses: { 'query:submit': { status: 'needs-key' } } }],
    async run(t) {
      const f = t.frame('overlay');
      await seenBefore(f);
      await openBar(f, { hasKey: false, recipients: [{ provider: 'anthropic', name: 'Anthropic', ready: false }] });
      await t.settle();
      await t.shot('bar-notice-nokey', TALL_CLIP);
      const copy = await f.evaluate(() => {
        const attrs = [...document.querySelectorAll('[aria-label],[title],[placeholder]')].flatMap((e) => [e.getAttribute('aria-label'), e.title, e.getAttribute('placeholder')]);
        return `${document.body.innerText} ${attrs.filter(Boolean).join(' ')}`;
      });
      t.check('noOpenRouterInBarCopy', !/openrouter/i.test(copy), 'a bar string names OpenRouter');
      const notice = await f.evaluate(() => document.getElementById('notice-text').textContent);
      t.check('noticeNamesTheNeed', notice === 'Answers need an AI provider. Finding words on screen works without one.', notice);

      await f.focus('#query-input');
      await setField(f, 'why is the build red');
      await press(f, { key: 'Enter' });
      await t.settle(50);
      const said = 'Answers need an AI provider. Press Tab to connect one.';
      const nudged = await f.evaluate(() => document.getElementById('notice').classList.contains('nudge'));
      await t.settle(60);
      const live = await f.evaluate(() => document.getElementById('announce-alert').textContent);
      const relayed = (await t.calls('overlay', 'a11y:announce')).some((c) => c.payload.text === said);
      const kept = await fieldValue(f);
      const tabTaken = await press(f, { key: 'Tab' });
      const focus = await f.evaluate(() => document.activeElement.id);
      const ok = nudged && (live === said || relayed) && kept === 'why is the build red' && tabTaken && focus === 'notice-settings';
      t.check('needsKeyNudgesAndAnnounces', ok, JSON.stringify({ nudged, live, relayed, kept, tabTaken, focus }));

      await openBar(f, { hasKey: true, hotkeyFailed: 'Ctrl+Alt+Space' });
      await t.settle(300);
      await t.shot('bar-notice-hotkey-taken', TALL_CLIP);
      const taken = await f.evaluate(() => `${document.getElementById('notice-text').textContent}|${document.getElementById('notice-settings').textContent}`);
      t.check('noticeHotkeyTaken', /^Another app already uses Ctrl\+Alt\+Space/.test(taken) && taken.endsWith('|Change shortcut'), taken);
    },
  },

  // B-157: in a contrast theme the working rim is an outline.
  {
    name: 'forced-colors-bar',
    backdrop: 'wallpaper',
    media: { forcedColors: 'active' },
    windows: [overlay],
    async run(t) {
      const f = t.frame('overlay');
      await seenBefore(f);
      await f.evaluate((st) => window.__emit('overlay:state', st), barState({ mode: 'pill', pillState: 'busy', orbAt: { x: 300, y: 20, width: 48, height: 48 } }));
      await t.settle();
      const pill = await f.evaluate(() => {
        const p = document.getElementById('pill');
        const c = getComputedStyle(p);
        return { orbit: p.classList.contains('orbit'), style: c.outlineStyle, width: c.outlineWidth };
      });
      t.check('forcedColorsPillDashed', pill.orbit && pill.style === 'dashed' && pill.width === '2px', JSON.stringify(pill));
      await t.shot('forced-colors-bar-working');
    },
  },
  {
    name: 'forced-colors-answer',
    backdrop: 'wallpaper',
    media: { forcedColors: 'active' },
    windows: [overlay, { name: 'out1', src: '/renderer/output.html', x: OUT1.x, y: OUT1.y, w: OUT1.w, h: OUT1.h, z: 3 }],
    async run(t) {
      const out = t.frame('out1');
      await seenBefore(t.frame('overlay'));
      await openBar(t.frame('overlay'));
      await out.evaluate(() => {
        window.__emit('overlay:state', { mode: 'panel', chatMax: 640 });
        window.__emit('chat:user', { gen: 1, text: 'Why does the build fail?' });
        window.__emit('query:start', { gen: 1 });
        window.__emit('query:delta', { gen: 1, text: 'The linker cannot find' });
      });
      await t.settle();
      const rim = await out.evaluate(() => {
        const p = document.getElementById('opanel');
        const c = getComputedStyle(p);
        return { orbit: p.classList.contains('orbit'), style: c.outlineStyle, width: c.outlineWidth };
      });
      t.check('forcedColorsAnswerOutlined', rim.orbit && rim.style === 'solid' && rim.width === '2px', JSON.stringify(rim));
      await t.shot('forced-colors-answer-streaming');
    },
  },
];
