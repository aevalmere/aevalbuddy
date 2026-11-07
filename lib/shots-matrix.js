'use strict';

// The glass matrix of the visual self-check (PLAN-v2.1 Phase B). Glass takes
// its look from what is behind it, so each key surface is photographed over
// three desktops: a white document, a black editor and a colourful
// wallpaper (lib/backdrop-fixture.html). The installer check runs this at
// 100%, 125% and 150%. Every frame also goes through the contrast gate
// (lib/contrast.js), which reads the text against its real background.
//
// Names: m-<surface>-<backdrop>-<scale>.png, for example
// m-answer-white-125.png.

const contrast = require('./contrast');

const wait = (ms) => new Promise((r) => setTimeout(r, ms));

const BACKDROPS = ['white', 'black', 'wallpaper'];

const CHAT = [
  { role: 'user', text: 'what is this error and how do I fix it?' },
  {
    role: 'assistant',
    text:
      'That is a **missing peer dependency**. The install skipped it.\n\n' +
      '1. Install the package\n' +
      '2. Restart the dev server\n\n' +
      '```\nnpm i react-dom@19\n```\n\n' +
      'The warning at the top of your terminal clears once it resolves.',
  },
];

const HITS = [
  { i: 0, kind: 'literal', text: 'Invoice 2291 for March', match: 'Invoice', why: '' },
  { i: 1, kind: 'literal', text: 'See invoice terms on page 2', match: 'invoice', why: '' },
  { i: 2, kind: 'related', text: 'Please settle by Friday', match: 'settle', why: '"settle" relates to "invoice": settling is paying one' },
];

// Each scene puts the app in one state and names the window to photograph.
// They run in this order, so a scene can rely on the one before it having
// closed what it opened.
function scenes(a, w) {
  const trail = (upTo) => {
    a.resetChat();
    a.toOutput('chat:user', { gen: 60, text: 'save this file as notes.txt on the desktop' });
    a.toOutput('agent:begin', { task: 'save this file as notes.txt on the desktop' });
    a.showOutput();
    const step = (r) => a.toOutput('agent:step', { runId: 9, at: Date.now(), state: 'done', ...r });
    step({ id: 1, kind: 'plan', steps: ['Open Save As', 'Choose the desktop', 'Name it notes.txt', 'Save'] });
    step({ id: 2, kind: 'action', text: 'Press Ctrl+Shift+S in Notepad', ms: 180 });
    if (upTo === 'running') {
      step({ id: 3, kind: 'action', text: 'Type "notes.txt" in Notepad', state: 'running' });
      a.toOutput('agent:state', { runId: 9, state: 'running', step: 3 });
      return;
    }
    step({ id: 3, kind: 'action', text: 'Type "notes.txt" in Notepad', ms: 410 });
    if (upTo === 'confirm') {
      a.toOutput('agent:state', { runId: 9, state: 'confirm', step: 4 });
      a.toOutput('agent:confirm-request', {
        runId: 9,
        id: 1,
        text: 'Click "Save" in Notepad',
        reason: 'save the file',
        why: 'the control is named "Save"',
      });
      return;
    }
    step({ id: 4, kind: 'action', text: 'Click "Save" in Notepad', ms: 90 });
    a.toOutput('agent:done', { runId: 9, state: 'done', text: 'Saved **notes.txt** to your desktop.' });
  };
  return [
    {
      name: 'pill',
      // The orb is a glyph and a badge; it shows no text by design (DESIGN.md,
      // Material: "state is shown by its rim and a badge, never by a word").
      text: false,
      win: () => w().overlay,
      async set() {
        a.collapse();
        await wait(900);
      },
    },
    {
      name: 'bar',
      win: () => w().overlay,
      async set() {
        // Unlocked, empty: the placeholder and the key legend, the way the
        // bar first opens. The text an earlier shot typed is cleared, or the
        // bar shows it selected and the Find scene below searches for it.
        a.toOverlay('bar:open', { lock: null });
        a.toOverlay('bar:prefill', { text: '', mode: 'ask' });
        await a.expandPanel();
        await wait(800);
      },
    },
    {
      name: 'answer',
      win: () => w().output,
      async set() {
        a.seedChat(CHAT);
        await wait(900);
      },
    },
    {
      name: 'thinking',
      win: () => w().output,
      async set() {
        a.seedChat([{ role: 'user', text: 'what is on my screen?' }]);
        await wait(200);
        a.seedThinking();
        await wait(700);
      },
    },
    {
      name: 'error',
      win: () => w().output,
      async set() {
        a.seedChat([{ role: 'user', text: 'why is the build failing?' }]);
        await wait(200);
        a.seedError('Every free model is busy. Retry in a moment, or pick a paid model in Settings.', 'why is the build failing?');
        await wait(600);
      },
    },
    {
      name: 'find',
      win: () => w().output,
      async set() {
        a.resetChat();
        await a.setBar('find');
        a.stopFind();
        const st = { gen: 1, open: true, scope: 'window', source: 'uia', status: '2 exact · 1 related', lines: [], action: null };
        a.toOverlay('find:state', st);
        a.toOutput('find:state', st);
        a.toOutput('find:hits', { gen: 1, query: 'invoice', hits: HITS, current: 0 });
        a.toOutput('view:set', { view: 'find' });
        a.showOutput();
        await wait(900);
      },
      after() {
        a.toOutput('view:set', { view: 'chat' });
      },
    },
    {
      name: 'lens',
      win: () => w().lens,
      rect: { x: 100, y: 0, width: 640, height: 360 },
      under: true,
      async set() {
        a.lensSample();
        await wait(1200);
      },
      after() {
        a.lensClear();
      },
    },
    {
      name: 'agent-running',
      win: () => w().output,
      async set() {
        await a.setBar('agent');
        trail('running');
        await wait(900);
      },
    },
    {
      name: 'agent-confirm',
      win: () => w().output,
      async set() {
        trail('confirm');
        await wait(900);
      },
    },
    {
      name: 'agent-done',
      win: () => w().output,
      async set() {
        trail('done');
        await wait(900);
      },
      after() {
        a.resetChat();
        a.collapse();
      },
    },
    {
      name: 'settings',
      win: () => w().settings,
      async set() {
        a.openSettings();
        await wait(1800);
      },
      after() {
        a.closeSettings();
      },
    },
    {
      name: 'history',
      win: () => w().history,
      async set() {
        a.openHistory();
        await wait(1500);
      },
      async after() {
        await a.closeHistory();
      },
    },
    {
      name: 'tour',
      win: () => w().buddy,
      rect: null,
      async set() {
        a.startTour();
        await wait(1800);
      },
      after() {
        a.endTour();
      },
    },
  ];
}

/**
 * @param {(name, win, opts) => Promise} shoot the shots.js photographer
 * @param {() => object} w the windows
 * @param {object} a the shot actions
 * @returns {Promise<number>} how many frames failed the contrast gate
 */
async function run(shoot, w, a) {
  const scale = Math.round((Number(process.env.AEVAL_SHOT_SCALE) || 1) * 100);
  console.log(`[SHOT] matrix display=${a.display()} scale=${scale}`);
  let failed = 0;
  let worst = Infinity;
  for (const bg of BACKDROPS) {
    await a.useFixture(bg);
    await wait(700);
    for (const s of scenes(a, w)) {
      const name = `m-${s.name}-${bg}-${scale}`;
      try {
        await s.set();
        const win = s.win();
        await shoot(name, win, { settle: 300, backdrop: false, rect: s.rect || undefined, under: !!s.under });
        if (win && !win.isDestroyed()) {
          const c = await contrast.measure(win);
          if (c.min < worst) worst = c.min;
          // Every scene but the orb shows text. A frame where the probe found
          // none was not measured, and a gate that measured nothing has not
          // passed.
          const none = c.runs === 0 && s.text !== false ? ' FAIL no visible text measured' : '';
          const bad = c.fails.map((f) => `${f.sel} "${f.text}" ${f.ratio}<${f.need}`).join('; ');
          console.log(`[SHOT] contrast ${name} runs=${c.runs} min=${c.min}${bad ? ` FAIL ${bad}` : none || ' ok'}`);
          if (c.fails.length || none) failed += 1;
        }
      } catch (err) {
        console.log(`[SHOT] ${name}=error ${err && err.message ? err.message : err}`);
        failed += 1;
      }
      if (s.after) await s.after();
      await wait(300);
    }
  }
  const st = a.glassStatus();
  console.log(`[SHOT] matrix scale=${scale} contrastMin=${worst} failedFrames=${failed} glass=${st.tier} frames=${st.frames}`);
  return failed;
}

module.exports = { run, BACKDROPS };
