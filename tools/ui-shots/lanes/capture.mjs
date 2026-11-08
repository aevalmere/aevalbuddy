// The capture lane's ui-shots scenes, mocks, checks and CSS lint list. The
// shape is documented at the top of tools/ui-shots/shots.mjs; shared fixtures
// are in tools/ui-shots/kit.mjs.
import { W, H } from '../kit.mjs';

export const mocks = {};
export const checks = [];
export const cssLintEnforce = ['renderer/lens.css'];

// The lens and the area selector each cover a whole display.
const full = (name, src, z) => ({ name, src, x: 0, y: 0, w: W, h: H, z, fixed: true });
// A dark editor under the window, for the backdrops shots.mjs does not paint.
const editor = full('editor', '/lib/backdrop-fixture.html?kind=black', 0);

// design-visual §4 state 52: literal hits, a related hit with its why chip as
// the current one, a concept hit.
const HITS = [
  { i: 0, kind: 'literal', x: 220, y: 180, w: 64, h: 18 },
  { i: 1, kind: 'literal', x: 420, y: 228, w: 64, h: 18 },
  { i: 2, kind: 'related', x: 220, y: 276, w: 92, h: 18, why: '"Invoice" relates to "payment": an invoice is a request for payment' },
  { i: 3, kind: 'concept', x: 220, y: 324, w: 150, h: 18, why: 'the total due is what you owe' },
];

function lensScene(name, backdrop, windows) {
  return {
    name,
    backdrop,
    windows,
    async run(t) {
      const lens = t.frame('lens');
      t.check(`${name}.ready`, (await t.calls('lens', 'lens:ready')).length === 1, 'lens.js never invoked lens:ready');
      await t.emit('lens', 'lens:draw', { hits: HITS, current: 2, frame: { chip: 'Using your computer' } });
      await t.settle(500);
      const drawn = await lens.evaluate(() => ({
        hits: document.querySelectorAll('.hit').length,
        ring: document.getElementById('ring').classList.contains('on'),
        why: document.getElementById('why').classList.contains('on'),
        multi: document.getElementById('why').classList.contains('multi'),
      }));
      t.check(`${name}.drawn`, drawn.hits === 4 && drawn.ring && drawn.why, JSON.stringify(drawn));
      await t.shot(name, { x: 0, y: 0, width: 800, height: 480 });
      // A one-line why is a capsule.
      await t.emit('lens', 'lens:draw', { hits: HITS, current: 3, frame: null });
      await t.settle(400);
      const single = await lens.evaluate(() => !document.getElementById('why').classList.contains('multi'));
      t.check(`${name}.whyCapsule`, single, 'a one-line why chip is not a capsule');
    },
  };
}

// design-visual §4 state 51.
async function drag(t, from, to) {
  await t.page.mouse.move(from.x, from.y);
  await t.page.mouse.down();
  await t.page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2);
  await t.page.mouse.move(to.x, to.y);
  await t.settle(200);
}

const key = (f, k, alt) =>
  f.evaluate(({ k: kk, alt: a }) => document.dispatchEvent(new KeyboardEvent('keydown', { key: kk, altKey: a, bubbles: true, cancelable: true })), { k, alt: !!alt });

export const scenes = [
  lensScene('lens-hits-white', 'white', [full('lens', '/renderer/lens.html', 1)]),
  lensScene('lens-hits-dark-editor', 'white', [editor, full('lens', '/renderer/lens.html', 1)]),
  {
    name: 'region-idle-white',
    backdrop: 'white',
    windows: [full('region', '/renderer/region.html', 1)],
    async run(t) {
      await t.settle(400);
      const f = t.frame('region');
      const hint = await f.evaluate(() => document.getElementById('hint').textContent.replace(/\s+/g, ' ').trim());
      t.check('regionHint', hint === 'Drag to select an area Esc Arrows move, Alt+arrows resize', hint);
      const a11y = await f.evaluate(() => document.body.getAttribute('role') === 'application' && document.body.getAttribute('aria-label') === 'Choose an area of the screen');
      t.check('regionRole', a11y, 'body has no application role and label');
      await t.shot('region-idle-white', { x: 400, y: 0, width: 800, height: 120 });
      // Keyboard: the first key says how to drive the box, then each size
      // change is announced.
      await key(f, 'ArrowRight');
      const first = await f.evaluate(() => document.getElementById('live').textContent);
      await key(f, 'ArrowRight', true);
      await t.settle(650);
      const after = await f.evaluate(() => document.getElementById('live').textContent);
      const want = `Area ${W / 2} by ${H / 2}. Arrows move it, Alt+arrows resize, Enter asks, Esc cancels.`;
      t.check('regionAnnouncesSize', first === want && after === `Area ${W / 2 + 16} by ${H / 2}.`, `${first} | ${after}`);
    },
  },
  {
    name: 'region-idle-dark',
    backdrop: 'white',
    windows: [editor, full('region', '/renderer/region.html', 1)],
    async run(t) {
      await t.settle(400);
      await t.shot('region-idle-dark', { x: 400, y: 0, width: 800, height: 120 });
    },
  },
  {
    name: 'region-dragging',
    backdrop: 'white',
    windows: [editor, full('region', '/renderer/region.html', 1)],
    async run(t) {
      await drag(t, { x: 300, y: 200 }, { x: 780, y: 470 });
      const chip = await t.frame('region').evaluate(() => document.getElementById('size').textContent);
      t.check('regionDragSize', chip === '480 × 270', chip);
      await t.shot('region-dragging', { x: 200, y: 120, width: 700, height: 440 });
      await t.page.mouse.up();
    },
  },
  {
    name: 'region-too-small',
    backdrop: 'white',
    windows: [full('region', '/renderer/region.html', 1)],
    async run(t) {
      await drag(t, { x: 300, y: 200 }, { x: 306, y: 260 });
      const small = await t.frame('region').evaluate(() => document.getElementById('size').classList.contains('too-small'));
      t.check('regionTooSmall', small, 'a 6 px wide area is not marked too small');
      await t.shot('region-too-small', { x: 200, y: 120, width: 400, height: 240 });
      await t.page.mouse.up();
    },
  },
];
