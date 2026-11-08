// The sheet lane's ui-shots scenes, mocks, checks and CSS lint list. The
// shape is documented at the top of tools/ui-shots/shots.mjs; shared fixtures
// are in tools/ui-shots/kit.mjs.
import { ANSWER, BAR, OUT1, W, H, barRect, bottomOf, openBar, streamAnswer } from '../kit.mjs';

export const mocks = {};
export const checks = [];
export const cssLintEnforce = [];

const win = (name, src, r, z) => ({ name, src, x: r.x, y: r.y, w: r.w, h: r.h, z });

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
];
