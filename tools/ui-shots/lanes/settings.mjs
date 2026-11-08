// The settings lane's ui-shots scenes, mocks, checks and CSS lint list. The
// shape is documented at the top of tools/ui-shots/shots.mjs; shared fixtures
// are in tools/ui-shots/kit.mjs.
import { W, H, streamAnswer } from '../kit.mjs';

export const mocks = {};
export const checks = [];
export const cssLintEnforce = [];

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
