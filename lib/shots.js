'use strict';

// Visual self-check. Boots the real app, drives every surface into a
// representative state, and writes a PNG of each one with
// webContents.capturePage. That method renders the page itself rather than
// reading the screen, so it works even though every window sets
// setContentProtection(true) and is therefore invisible to normal capture.
//
// Run it with:  AEVAL_SHOT=<dir> AEVAL_USERDATA=<scratch profile> electron .
//
// Transparent windows get a flat backdrop injected just before the shot, so
// the window rectangle is visible against the panel inside it; that is what
// makes padding and alignment errors readable in the output.

const fs = require('fs');
const path = require('path');

const BACKDROP = 'html,body{background:#3b3f4a !important}';

function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function shoot(dir, name, win, opts) {
  const o = opts || {};
  if (!win || win.isDestroyed()) {
    console.log(`[SHOT] ${name}=missing`);
    return;
  }
  let cssKey = null;
  if (o.backdrop !== false) {
    try {
      cssKey = await win.webContents.insertCSS(BACKDROP);
    } catch (err) {
      // A page that has not finished loading rejects insertCSS; the shot is
      // still worth taking without the backdrop.
    }
  }
  await wait(o.settle || 300);
  const img = o.rect
    ? await win.webContents.capturePage(o.rect)
    : await win.webContents.capturePage();
  const size = img.getSize();
  if (!size.width || !size.height) {
    console.log(`[SHOT] ${name}=empty`);
  } else {
    fs.writeFileSync(path.join(dir, `${name}.png`), img.toPNG());
    console.log(`[SHOT] ${name}=${size.width}x${size.height}`);
  }
  if (cssKey) {
    try {
      await win.webContents.removeInsertedCSS(cssKey);
    } catch (err) {
      // Nothing to undo if the page navigated in between.
    }
  }
}

const SAMPLE_CHAT = [
  { role: 'user', text: 'what is this error and how do I fix it?' },
  {
    role: 'assistant',
    text:
      'That is a **missing peer dependency**. The install skipped it.\n\n' +
      '1. Install the package:\n' +
      '2. Restart the dev server\n\n' +
      '```\nnpm i react-dom@19\n```\n\n' +
      'The warning at the top of your terminal clears once it resolves. ' +
      'See `package.json` for the pinned range.',
  },
  { role: 'user', text: 'and the second warning?' },
];

/**
 * @param {object} ctx
 * @param {string} ctx.dir output directory
 * @param {() => object} ctx.windows returns { overlay, output, settings, buddy }
 * @param {object} ctx.actions app entry points the states need
 */
async function run(ctx) {
  const dir = ctx.dir;
  fs.mkdirSync(dir, { recursive: true });
  const a = ctx.actions;
  const w = () => ctx.windows();

  // 1. Idle pill.
  await shoot(dir, '01-pill', w().overlay);

  // 2. Query panel with no key saved: the notice row is showing.
  a.clearKey();
  await a.expandPanel();
  await shoot(dir, '02-panel-nokey', w().overlay, { settle: 500 });

  // Window height should equal panel height: any difference is a dead band
  // of empty window showing under the surface.
  await a.probe('panel', w().overlay, '#panel');

  // 3. Query panel in its normal state.
  a.setKey('sk-or-v1-shotprofile-not-a-real-key');
  await wait(200);
  await shoot(dir, '03-panel', w().overlay, { settle: 400 });

  // 3b. Mid-expansion. Catching the glide part-way is the only way to prove
  // the panel is pinned to its anchored edge: the frame is narrow, and the
  // contents inside it are already in their final position rather than
  // reflowing or scaling.
  a.collapse();
  await wait(400);
  a.expandPanel();
  await shoot(dir, '03b-panel-midglide', w().overlay, { settle: 90 });
  await wait(400);

  // 4. Answer window holding a conversation.
  a.seedChat(SAMPLE_CHAT);
  await wait(500);
  await shoot(dir, '04-answer', w().output, { settle: 500 });
  await a.probe('answer', w().output, '#opanel');

  // 4b. A transcript taller than the window. The scrollbar and the fade at
  // the bottom edge are the only two things telling the user there is more
  // below, so they have to be legible in a photograph.
  const LONG_CHAT = [];
  for (let i = 0; i < 6; i += 1) {
    LONG_CHAT.push({ role: 'user', text: `follow-up number ${i + 1}` });
    LONG_CHAT.push({
      role: 'assistant',
      text: `Answer ${i + 1}. The install skipped a peer dependency, so the ` +
        'dev server starts and then fails on the first render. Pin the version ' +
        'and restart.',
    });
  }
  a.seedChat(LONG_CHAT);
  await wait(700);
  await shoot(dir, '04b-answer-scrolled', w().output, { settle: 500 });
  await a.probe('answerLong', w().output, '#opanel');
  a.seedChat(SAMPLE_CHAT);
  await wait(400);

  // 5. Answer window mid-request, thinking.
  a.seedThinking();
  await wait(400);
  await shoot(dir, '05-answer-thinking', w().output, { settle: 400 });

  // 6 and 7. Layout editor ghosts.
  a.enterHud();
  await wait(500);
  await shoot(dir, '06-hud-input', w().overlay, { settle: 300 });
  await shoot(dir, '07-hud-output', w().output, { settle: 300 });
  a.exitHud();
  await wait(400);

  // 8, 9, 10. Settings, top and bottom, then with the model list open.
  a.openSettings();
  await wait(1600);
  await shoot(dir, '08-settings-top', w().settings, { settle: 400 });
  await a.scrollSettings(9999);
  await shoot(dir, '09-settings-bottom', w().settings, { settle: 400 });
  await a.scrollSettings(0);
  await a.openModelList();
  await shoot(dir, '10-settings-models', w().settings, { settle: 500 });
  await a.closeModelList();
  // Brain mode reveals a second picker; it should not disturb the layout.
  await a.enableBrain();
  await wait(700);
  await shoot(dir, '10b-settings-brain', w().settings, { settle: 400 });

  // 11. The pointer with a callout, cropped to the area around it.
  a.showPoint();
  await wait(1400);
  await shoot(dir, '11-buddy', w().buddy, {
    settle: 600,
    rect: { x: 120, y: 60, width: 620, height: 340 },
  });

  // 12. The real thing, composited over the desktop. capturePage renders the
  // page in isolation, which cannot show an acrylic backdrop or the system
  // shadow, because neither belongs to the page. This one goes through
  // desktopCapturer with content protection briefly off, so what lands on
  // disk is what a person actually sees. Cropped to the overlay and a
  // margin, so nothing else on the desktop is written out.
  if (a.liveShot) {
    a.exitHud();
    await wait(300);
    a.closeSettings();
    await wait(300);
    a.seedChat(SAMPLE_CHAT);
    await a.expandPanel();
    await wait(900);
    const ok = await a.liveShot(path.join(dir, '12-live-overlay.png'));
    console.log(`[SHOT] 12-live-overlay=${ok || 'failed'}`);
  }

  console.log('[SHOT] done');
}

module.exports = { run, shoot };
