'use strict';

// SPEC-v2 B7 to B19, B27, B28, B30 against real windows on a CI runner:
// a Notepad of our own holding fixture text, and a small form with a
// password box and a Send button. Every model call is answered from
// tools/fixtures/llm (AEVAL_FAKE_LLM), so the runs are the same every time.

const { until, sleep, spawn, fs, os, path, selfcheck } = require('./smoke-v2');

const FIND_TEXT = ['Invoice 2291 for March', 'Please settle by Friday', 'total due 42.00'].join('\r\n');
const SECRET = 'hunter2-secret-9154';

async function openNotepad(c, tag) {
  const { desktop } = c;
  const file = path.join(os.tmpdir(), `${tag}.txt`);
  fs.writeFileSync(file, `${FIND_TEXT}\r\n`, 'utf8');
  const before = new Set((await desktop.listWindows({ includeMinimized: true })).map((w) => w.handle));
  const hadNotepad = [...before].length && (await desktop.listWindows()).some((w) => /^notepad$/i.test(w.process));
  spawn('notepad.exe', [file], { detached: true, stdio: 'ignore' }).unref();
  const win = await until(async () => (await desktop.listWindows()).find((w) => !before.has(w.handle) && w.title.includes(tag)), 15000, 250);
  return { win, file, hadNotepad };
}

// A WinForms window with a password box and a Send button: the smallest
// real app with an IsPassword field and a control named after a risky verb.
function openForm(tag) {
  const ps = [
    'Add-Type -AssemblyName System.Windows.Forms',
    '$f = New-Object Windows.Forms.Form',
    `$f.Text = '${tag}'`,
    '$f.Width = 360; $f.Height = 200; $f.TopMost = $true',
    '$p = New-Object Windows.Forms.TextBox',
    `$p.UseSystemPasswordChar = $true; $p.Text = '${SECRET}'; $p.Left = 20; $p.Top = 20; $p.Width = 200; $p.AccessibleName = 'Password'`,
    '$b = New-Object Windows.Forms.Button',
    "$b.Text = 'Send'; $b.Left = 20; $b.Top = 60",
    `$b.Add_Click({ $f.Text = '${tag} clicked' })`,
    '$f.Controls.Add($p); $f.Controls.Add($b)',
    '[Windows.Forms.Application]::Run($f)',
  ].join('; ');
  const p = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', ps], { detached: true, stdio: 'ignore' });
  p.unref();
  return p;
}

async function waitAgent(c, ms) {
  return until(() => !c.agent.isRunning(), ms, 100);
}

async function run(c) {
  const { log, must, desktop, find, agent, ai, store } = c;
  const step = async (name, fn) => {
    try {
      await fn();
    } catch (err) {
      must(name, false, `${err.code || 'ERROR'}:${err.message}`);
    }
  };
  process.env.AEVAL_FAKE_LLM = '1';
  const hadKey = store.hasApiKey();
  if (!hadKey) store.setApiKey('sk-or-v1-smoke-fake');
  store.setAgentSettings({ consent: true, permission: 'risky', maxSteps: 25, screenshots: false });
  store.setFindSettings({ scope: 'window', related: true, ocr: true });
  const tag = `aeval-v2-${process.pid}-${Date.now().toString(36)}`;
  const np = await openNotepad(c, tag);
  must('v2NotepadOpen', !!np.win, 'no Notepad window with the fixture file');
  if (!np.win) return;
  const win = np.win;
  ai.fake.setVar('notepad', win.handle);
  try {
    await desktop.focusWindow(win.handle);
    await sleep(500);

    // ------------------------------------------------------------ Find
    await step('findLiteralMs', async () => {
      await c.openBar('find');
      await until(() => find.state().spans > 0 && !find.state().reading, 8000);
      const t = Date.now();
      const r = find.query('invoice');
      const ms = Date.now() - t;
      log('findLiteral', `${r.literal} hits in ${ms}ms (engine ${r.ms}ms, ${find.state().spans} spans, ${find.state().source})`);
      must('findLiteralMs<50', r.literal >= 1 && ms < 50, `literal=${r.literal} ms=${ms}`);
    });

    await step('lensHits', async () => {
      await sleep(400);
      const lw = c.lens.windows().filter((w) => w.isVisible());
      const drawn = find.state().hits.filter((h) => h.rect).length;
      let count = -1;
      if (lw[0]) count = await lw[0].webContents.executeJavaScript(`document.querySelectorAll('.hit').length`);
      const h = find.state().hits[0];
      let through = 'unknown';
      if (h && h.rect) {
        const at = await desktop.elementAt({ x: h.rect.x + Math.round(h.rect.width / 2), y: h.rect.y + Math.round(h.rect.height / 2) });
        through = at.windowHandle === win.handle ? 'notepad' : at.windowHandle;
      }
      must('lensHits', lw.length >= 1 && count === drawn && lw[0].isFocusable() === false && through === 'notepad', `windows=${lw.length} drawn=${count}/${drawn} under=${through}`);
    });

    await step('findStep', async () => {
      const ring = async () => {
        const lw = c.lens.windows().find((w) => w.isVisible());
        return lw ? lw.webContents.executeJavaScript(`document.getElementById('ring').style.transform`) : '';
      };
      find.query('e');
      await sleep(300);
      const n = find.state().hits.length;
      const r0 = await ring();
      const a = (await find.step(1)).current;
      await sleep(300);
      const r1 = await ring();
      let wrapped = false;
      for (let i = 0; i < n + 1; i++) if ((await find.step(1)).current === 0) wrapped = true;
      must('findStep', n >= 2 && a === 1 && r0 !== r1 && wrapped, `hits=${n} current=${a} ring ${r0} -> ${r1} wrapped=${wrapped}`);
    });

    await step('findRelated', async () => {
      find.query('payment');
      const hit = await until(() => find.state().hits.find((h) => h.kind === 'related' && h.why), 5000);
      must('findRelated', !!hit, JSON.stringify(find.state().stage));
      if (hit) log('findRelatedWhy', hit.why);
    });

    await step('findConcept', async () => {
      find.query('how much do I owe');
      const hit = await until(() => find.state().hits.find((h) => h.kind === 'concept'), 6000);
      must('findConcept', !!hit, JSON.stringify(find.state().stage));
      if (hit) log('findConceptHit', `${hit.match} :: ${hit.why}`);
    });

    await step('findOffline', async () => {
      process.env.AEVAL_OFFLINE = '1';
      try {
        const r = find.query('invoice');
        await sleep(200);
        const st = find.state().status;
        must('findOffline', r.literal >= 1 && (st.lines || []).includes('Related matches need a connection.'), JSON.stringify(st));
      } finally {
        delete process.env.AEVAL_OFFLINE;
      }
    });
    c.closeFind();
    await sleep(300);

    // ------------------------------------------------------------ Ask
    await step('askHasUiaText', async () => {
      await desktop.focusWindow(win.handle);
      await sleep(400);
      const before = ai.fake.requests.length;
      const r = await c.handleQuerySubmit('what does this file say?');
      const bodies = ai.fake.requests.slice(before).map((x) => x.body).join('\n');
      must('askHasUiaText', r.status === 'done' && bodies.includes('EXACT TEXT FROM THE APP') && bodies.includes('Invoice 2291'), `status=${r.status} requests=${ai.fake.requests.length - before}`);
      c.collapse();
    });

    await step('pointFromUia', async () => {
      await desktop.focusWindow(win.handle);
      await sleep(300);
      await desktop.key({ keys: 'ctrl+shift+s', windowHandle: win.handle });
      const dlg = await until(async () => (await desktop.listWindows()).find((w) => w.pid === win.pid && w.className === '#32770'), 8000, 250);
      if (!dlg) throw Object.assign(new Error('no Save As dialog'), { code: 'NOT_FOUND' });
      const tree = await desktop.readTree({ windowHandle: dlg.handle, filter: 'interactive' });
      const btn = tree.nodes.find((n) => n.role === 'button' && /^save$/i.test(n.name || ''));
      const p = await c.resolveUiaPoint('Save', dlg.handle);
      let off = -1;
      if (btn && p) {
        const want = c.screen.screenToDipPoint({ x: btn.rect.x + btn.rect.width / 2, y: btn.rect.y + btn.rect.height / 2 });
        off = Math.max(Math.abs(want.x - p.x), Math.abs(want.y - p.y));
      }
      must('pointFromUia', !!btn && !!p && off >= 0 && off <= 4, `button=${!!btn} point=${p ? `${p.x},${p.y}` : 'none'} off=${off}`);
      await desktop.key({ keys: 'Escape', windowHandle: dlg.handle });
      await sleep(400);
    });

    // ------------------------------------------------------------ agent
    await step('agentRun', async () => {
      await desktop.focusWindow(win.handle);
      ai.fake.setVar('agentText', ' Agent typed 5810');
      const r = await c.startAgentRun({ task: 'smoke notepad' });
      if (r.status !== 'started') throw new Error(`start: ${r.status}`);
      await waitAgent(c, 30000);
      const run = agent.current();
      const g = await desktop.getText({ windowHandle: win.handle, ocrFallback: false });
      const rows = run.rows.filter((x) => x.kind !== 'remove' && x.kind !== 'thinking');
      const saved = c.history.get(c.chatId());
      const text = saved ? JSON.stringify(saved.messages) : '';
      must(
        'agentRun',
        run.endState === 'done' && g.text.includes('Agent typed 5810') && rows.length === 4 && !!saved && !text.includes('data:image') && text.includes('"trail"'),
        `state=${run.endState} typed=${g.text.includes('Agent typed 5810')} rows=${rows.map((x) => x.kind).join('+')} saved=${!!saved}`
      );
      must('agentFocusKept', run.focusViolations === 0, `${run.focusViolations} steps found an AevalBuddy window in front`);
    });

    await step('agentJsonMode', async () => {
      ai.fake.setVar('agentText', ' Json typed 6620');
      await desktop.focusWindow(win.handle);
      const r = await c.startAgentRun({ task: 'smoke notepad json', jsonMode: true });
      if (r.status !== 'started') throw new Error(`start: ${r.status}`);
      await waitAgent(c, 30000);
      const run = agent.current();
      const g = await desktop.getText({ windowHandle: win.handle, ocrFallback: false });
      const body = ai.fake.requests.filter((x) => x.purpose === 'agent').pop();
      must('agentJsonMode', run.endState === 'done' && !run.useTools && g.text.includes('Json typed 6620') && body && !body.body.includes('"tools"'), `state=${run.endState} tools=${run.useTools}`);
    });

    await step('agentStopMs<300', async () => {
      const r = await c.startAgentRun({ task: 'smoke stop' });
      if (r.status !== 'started') throw new Error(`start: ${r.status}`);
      await sleep(600);
      const before = agent.stats().dispatched;
      let stoppedAt = 0;
      const on = (p) => {
        if (p.state === 'stopped') stoppedAt = Date.now();
      };
      c.bus.on('agent:state', on);
      const t = Date.now();
      agent.stop();
      await sleep(1200);
      c.bus.off('agent:state', on);
      const ms = stoppedAt ? stoppedAt - t : -1;
      must('agentStopMs<300', ms >= 0 && ms < 300 && agent.stats().dispatched === before, `ms=${ms} dispatched after=${agent.stats().dispatched - before}`);
    });

    await step('agentTakeover', async () => {
      const r = await c.startAgentRun({ task: 'smoke takeover' });
      if (r.status !== 'started') throw new Error(`start: ${r.status}`);
      await sleep(500);
      await desktop.simulateUserInput('key');
      const paused = await until(() => agent.current().state === 'paused', 3000);
      must('agentTakeover', !!paused, `state=${agent.current().state}`);
      agent.stop();
      await waitAgent(c, 3000);
    });

    await step('agentMaxSteps', async () => {
      const r = await c.startAgentRun({ task: 'smoke limit', maxSteps: 10 });
      if (r.status !== 'started') throw new Error(`start: ${r.status}`);
      await waitAgent(c, 30000);
      const run = agent.current();
      must('agentMaxSteps', run.endState === 'limit' && /10 steps/.test(run.final), `${run.endState}: ${run.final}`);
    });
  } finally {
    c.closeFind();
    const closed = await selfcheck.closeWithoutSaving(desktop, win, tag, np.hadNotepad, log);
    log('v2NotepadClosed', closed);
    try {
      fs.unlinkSync(np.file);
    } catch (err) {
      // left in temp
    }
  }

  // ------------------------------------------------------------ form
  const ftag = `aeval-form-${process.pid}`;
  const proc = openForm(ftag);
  const form = await until(async () => (await desktop.listWindows()).find((w) => w.title === ftag), 20000, 300);
  await step('agentConfirmGate', async () => {
    if (!form) throw Object.assign(new Error('the check form never opened'), { code: 'NOT_FOUND' });
    await desktop.focusWindow(form.handle);
    await sleep(400);
    const tree = await desktop.readTree({ windowHandle: form.handle, filter: 'all' });
    const send = tree.nodes.find((n) => n.role === 'button' && n.name === 'Send');
    if (!send) throw new Error(`no Send button: ${tree.nodes.map((n) => n.role).join(',')}`);
    ai.fake.setVar('sendRef', send.ref);
    let req = null;
    const on = (p) => {
      req = p;
    };
    c.bus.once('agent:confirm-request', on);
    const before = ai.fake.requests.length;
    const r = await c.startAgentRun({ task: 'smoke confirm gate' });
    if (r.status !== 'started') throw new Error(`start: ${r.status}`);
    await until(() => req, 15000);
    if (req) agent.confirm(req.id, false);
    await waitAgent(c, 15000);
    const title = (await desktop.listWindows()).find((w) => w.handle === form.handle);
    must('agentConfirmGate', !!req && /Send/.test(req.text) && title && !/clicked/.test(title.title), `request=${req ? req.text : 'none'} title=${title ? title.title : 'gone'}`);
    const bodies = ai.fake.requests.slice(before).map((x) => x.body).join('\n');
    must('passwordRedacted', bodies.length > 0 && !bodies.includes(SECRET) && bodies.includes('Password'), `requests=${ai.fake.requests.length - before}`);
  });
  if (form) await desktop.closeWindow(form.handle).catch(() => {});
  try {
    process.kill(proc.pid);
  } catch (err) {
    // already gone
  }
  if (!hadKey) store.clearApiKey();
  store.setAgentSettings({ consent: false });
  delete process.env.AEVAL_FAKE_LLM;
}

module.exports = { run };
