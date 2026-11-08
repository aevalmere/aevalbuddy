// The harness lane's ui-shots scenes, mocks, checks and CSS lint list. The
// shape is documented at the top of tools/ui-shots/shots.mjs; shared fixtures
// are in tools/ui-shots/kit.mjs.
//
// These scenes prove the framework itself (B-053): the bar and an answer over
// the two hardest backdrops, the bar at the shipped default width, and the
// glob opening on the virtual clock, photographed at real times.
import { ANSWER, BAR, BAR_DEFAULT, OUT1, barRect, bottomOf, openBar, streamAnswer } from '../kit.mjs';

export const mocks = {};
export const checks = [];
// Harness's own stylesheet, enforced from day one (B-145).
export const cssLintEnforce = ['src-tauri/src/backdrop/contrast-hide.css'];

const win = (name, src, r, z) => ({ name, src, x: r.x, y: r.y, w: r.w, h: r.h, z });

const overBackdrop = (backdrop) => ({
  name: `backdrop-${backdrop}`,
  backdrop,
  windows: [win('overlay', '/renderer/index.html', BAR, 1), win('out1', '/renderer/output.html', OUT1, 2)],
  async run(t) {
    await t.frame('overlay').evaluate(() => localStorage.setItem('aeval.hasSentFirstRequest', '1'));
    await openBar(t.frame('overlay'));
    await streamAnswer(t.frame('out1'), 'Why does the build fail?', ANSWER);
    await t.settle(900);
    const bottom = await bottomOf(t.page, 'out1');
    await t.shot(`11-${backdrop}-bar-output`, { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: bottom });
  },
});

export const scenes = [
  overBackdrop('dark-editor'),
  overBackdrop('photo-busy'),

  // The bar at the width it ships with (600 px), beside the 640 px scenes.
  {
    name: 'bar-default-width',
    backdrop: 'wallpaper',
    windows: [win('overlay', '/renderer/index.html', BAR_DEFAULT, 1)],
    async run(t) {
      const f = t.frame('overlay');
      await f.evaluate(() => localStorage.setItem('aeval.hasSentFirstRequest', '1'));
      await openBar(f, { inputWidth: 600 });
      await t.settle();
      const width = await f.evaluate(() => Math.round(document.getElementById('bar').getBoundingClientRect().width));
      t.check('barDefaultWidth', Math.abs(width - 600) <= 2, `bar is ${width} px wide`);
      const clip = { x: BAR_DEFAULT.x - 20, y: 0, width: BAR_DEFAULT.w + 40, height: 140 };
      await t.shot('12-bar-default-600', clip);
    },
  },

  // The glob opening with time stopped: each picture is the frame the page
  // draws that many milliseconds after open(), whatever the runner's speed.
  {
    name: 'glob-clock',
    backdrop: 'wallpaper',
    clock: true,
    windows: [win('overlay', '/renderer/index.html', BAR, 1), win('out1', '/renderer/output.html', OUT1, 2)],
    async run(t) {
      const bar = t.frame('overlay');
      const out = t.frame('out1');
      await bar.evaluate(() => localStorage.setItem('aeval.hasSentFirstRequest', '1'));
      await openBar(bar);
      await t.settle(300);
      const rect = await barRect(bar);
      await out.evaluate((r) => {
        window.__barRect = r;
      }, rect);
      await streamAnswer(out, 'Why does the build fail?', ANSWER);
      // Let the answer's own open run out, then open again from the start.
      await t.settle(1200);
      await out.evaluate(() => {
        window.__globDone = false;
        window.AevalGlob.open(document.getElementById('opanel')).then(() => {
          window.__globDone = true;
        });
      });
      // open() first asks where the bar is (real time); the frames count
      // from its first animation frame.
      for (let i = 0; (await out.evaluate(() => document.body.dataset.glob || '')) !== 'open'; i += 1) {
        if (i > 80) throw new Error('the glob never started');
        await t.page.waitForTimeout(25);
      }
      const clip = { x: BAR.x - 20, y: 0, width: BAR.w + 40, height: OUT1.y + OUT1.h };
      let at = 0;
      for (const ms of [48, 144, 240, 336]) {
        await t.tick(ms - at);
        at = ms;
        await t.shot(`13-glob-clock-${String(ms).padStart(3, '0')}`, clip);
      }
      const early = await out.evaluate(() => window.__globDone);
      await t.tick(200);
      const late = await out.evaluate(() => window.__globDone);
      t.check('globOnClock', !early && late, `done at 336 ms=${early}, at 536 ms=${late} (open takes 440 ms)`);
    },
  },
];
