'use strict';

// The v2 surfaces for the visual self-check (SPEC-v2 B11, B20, B23, B24):
// the bar in Find and Agent mode, Find's results and its empty and offline
// states, the lens, the agent trail running, asking, done and failed, the
// tour's four steps, and the settings rail. Each state is reached through
// the same IPC events the real flows send, so a frame shows what the
// renderer actually draws for that event.

const wait = (ms) => new Promise((r) => setTimeout(r, ms));

const HITS = [
  { i: 0, kind: 'literal', text: 'Invoice 2291 for March', match: 'Invoice', why: '' },
  { i: 1, kind: 'literal', text: 'See invoice terms on page 2', match: 'invoice', why: '' },
  { i: 2, kind: 'related', text: 'Please settle by Friday', match: 'settle', why: '"settle" relates to "invoice": settling is paying one' },
  { i: 3, kind: 'concept', text: 'total due 42.00', match: 'total due 42.00', why: 'the total due is what the invoice asks for' },
];

async function run(shoot, w, a) {
  const find = (status, lines, hits, current) => {
    const st = { gen: 1, open: true, scope: 'window', source: 'uia', status, lines, action: null };
    a.toOverlay('find:state', st);
    a.toOutput('find:state', st);
    a.toOutput('find:hits', { gen: 1, query: 'invoice', hits, current });
  };

  // 18, 19. Find in the bar, and its results.
  a.resetChat();
  await a.setBar('find');
  await wait(500);
  a.toOverlay('bar:prefill', { text: 'invoice', mode: 'find' });
  find('2 exact · 2 related', [], HITS, 0);
  a.toOutput('view:set', { view: 'find' });
  a.showOutput();
  await wait(600);
  await shoot('18-panel-find', w().overlay, { settle: 300 });
  await shoot('19-find-results', w().output, { settle: 300 });

  // 26, 27. Nothing matches; and offline, where only exact matches work.
  find('Nothing on screen matches "zebra".', ['No related matches either.'], [], -1);
  await wait(400);
  await shoot('26-find-empty', w().output, { settle: 300 });
  find('2 exact', ['Related matches need a connection.'], HITS.slice(0, 2), 0);
  await wait(400);
  await shoot('27-find-offline', w().overlay, { settle: 300 });

  // 20. The lens: two exact hits, a related one with the ring and its why
  // chip, a concept hit, and the agent's frame.
  a.lensSample();
  await wait(900);
  await shoot('20-lens', w().lens, { settle: 400, rect: { x: 100, y: 0, width: 640, height: 360 } });
  a.lensClear();
  a.toOutput('view:set', { view: 'chat' });
  await wait(300);

  // 21. Agent in the bar, with the hint an empty Enter gives.
  await a.setBar('agent');
  a.toOverlay('bar:prefill', { text: 'rename these screenshots by date', mode: 'agent' });
  await wait(500);
  await shoot('21-panel-agent', w().overlay, { settle: 300 });

  // 22 to 25. The trail.
  a.resetChat();
  await wait(200);
  a.toOutput('chat:user', { gen: 50, text: 'save this file as notes.txt on the desktop' });
  a.toOutput('agent:begin', { task: 'save this file as notes.txt on the desktop' });
  a.showOutput();
  const step = (r) => a.toOutput('agent:step', { runId: 1, at: Date.now(), state: 'done', ...r });
  step({ id: 1, kind: 'plan', steps: ['Open Save As', 'Choose the desktop', 'Name it notes.txt', 'Save'] });
  step({ id: 2, kind: 'read', text: 'Read Notepad', ms: 240 });
  step({ id: 3, kind: 'action', text: 'Press Ctrl+Shift+S in Notepad', ms: 180 });
  step({ id: 4, kind: 'action', text: 'Click "Desktop" in Notepad', ms: 320 });
  step({ id: 5, kind: 'action', text: 'Type "notes.txt" in Notepad', state: 'running' });
  a.toOutput('agent:state', { runId: 1, state: 'running', step: 5 });
  await wait(700);
  await shoot('22-agent-running', w().output, { settle: 300 });

  step({ id: 5, kind: 'action', text: 'Type "notes.txt" in Notepad', ms: 410 });
  a.toOutput('agent:state', { runId: 1, state: 'confirm', step: 6 });
  a.toOutput('agent:confirm-request', {
    runId: 1,
    id: 1,
    text: 'Click "Save" in Notepad',
    reason: 'save the file',
    why: 'the control is named "Save"',
  });
  await wait(600);
  await shoot('23-agent-confirm', w().output, { settle: 300 });

  step({ id: 6, kind: 'action', text: 'Click "Save" in Notepad', ms: 90 });
  a.toOutput('agent:done', { runId: 1, state: 'done', text: 'Saved **notes.txt** to your desktop.' });
  await wait(600);
  await shoot('24-agent-done', w().output, { settle: 300 });

  a.resetChat();
  await wait(200);
  a.toOutput('chat:user', { gen: 51, text: 'send the report to Sam' });
  a.toOutput('agent:begin', { task: 'send the report to Sam' });
  a.showOutput();
  step({ id: 11, kind: 'plan', steps: ['Open Outlook', 'Attach the report', 'Send to Sam'] });
  step({ id: 12, kind: 'action', text: 'Click "Attach file" in Outlook', state: 'failed', error: 'Refused: that window runs as administrator, and AevalBuddy does not.' });
  a.toOutput('agent:done', {
    runId: 2,
    state: 'error',
    text: 'The agent stopped: that window runs as administrator. Start Outlook normally and retry.',
  });
  await wait(600);
  await shoot('25-agent-error', w().output, { settle: 300 });
  a.resetChat();
  a.collapse();
  await wait(400);

  // 28. The tour, one frame per step.
  a.startTour();
  for (let n = 1; n <= 4; n += 1) {
    await wait(1600);
    await shoot(`28-onboard-${n}`, w().buddy, { settle: 400, backdrop: true });
    if (n === 1) a.tourEvent('key-saved');
    else if (n === 2) a.tourEvent('first-answer');
    else if (n === 3) a.tourEvent('find-hit');
  }
  a.endTour();
  a.closeSettings();
  await wait(400);

  // 29, 30. Settings: the rail at full width, then the Agent section open.
  a.openSettings();
  await wait(1600);
  await shoot('30-settings-rail', w().settings, { settle: 400 });
  await a.settingsJs(
    `(() => { document.querySelector('[data-edit="sec-agent"]').click();` +
      ` document.querySelector('#rail [data-page="pg-agent"]').click(); return true; })()`
  );
  await wait(900);
  await shoot('29-settings-agent', w().settings, { settle: 400 });
  a.closeSettings();
  await wait(300);
}

module.exports = { run };
