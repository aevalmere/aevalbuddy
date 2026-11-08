// The bar lane's ui-shots scenes, mocks, checks and CSS lint list. The shape
// is documented at the top of tools/ui-shots/shots.mjs; shared fixtures are
// in tools/ui-shots/kit.mjs.
import { BAR, barContext, openBar } from '../kit.mjs';

export const mocks = {};
export const checks = [];
export const cssLintEnforce = [];

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
];
