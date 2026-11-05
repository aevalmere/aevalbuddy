'use strict';

// The desktop engine's acceptance checks (SPEC-v2.md B2 to B6), shared by
// the AEVAL_SMOKE run in main.js and by tools/desktop-check.js.
//
// The Notepad half opens a real window and types into it, so it only runs
// when asked: `notepad: true`, which the callers set on a CI runner or with
// AEVAL_DESKTOP_NOTEPAD=1. It touches nothing but the Notepad window it
// launched: that window is told apart from any Notepad already open by
// diffing the window list and by a unique file name in its title, every
// keystroke goes through the engine's foreground guard, and the file is a
// temp file deleted at the end. It closes without saving.

const { spawn } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

const FIXTURE = 'Invoice total due 42.00';
const TYPED = 'AevalBuddy smoke 4271';
const SET = 'Replaced by setValue 9137';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function waitFor(fn, timeoutMs, stepMs = 250) {
  const end = Date.now() + timeoutMs;
  for (;;) {
    const v = await fn();
    if (v) return v;
    if (Date.now() > end) return null;
    await sleep(stepMs);
  }
}

function inside(r, outer, slack = 2) {
  return (
    r &&
    outer &&
    r.x >= outer.x - slack &&
    r.y >= outer.y - slack &&
    r.x + r.width <= outer.x + outer.width + slack &&
    r.y + r.height <= outer.y + outer.height + slack
  );
}

/**
 * @param {object} o
 * @param {object} o.desktop        lib/desktop
 * @param {(k: string, v: any) => void} o.log
 * @param {(k: string, ok: boolean, detail?: string) => void} o.must
 * @param {boolean} [o.notepad]     run the checks that open Notepad
 * @param {{x:number,y:number}} [o.ownPoint]  a point on one of our windows
 * @param {string[]} [o.ownHandles] our window handles, as 'w…' strings
 */
async function run({ desktop, log, must, notepad = false, ownPoint, ownHandles = [] }) {
  // ---------------------------------------------------------------- B2
  let t = Date.now();
  let started = true;
  try {
    await desktop.start();
    await desktop.ping();
  } catch (err) {
    started = false;
    log('desktopError', `${err.code}:${err.message}`);
  }
  const readyMs = Date.now() - t;
  must('desktopReady', started && readyMs < 5000, `ready=${started} ms=${readyMs}`);
  log('desktopReadyMs', readyMs);
  if (!started) return;
  log('desktopDpi', desktop.status().dpi);

  const windows = await desktop.listWindows();
  must('desktopWindows', windows.length > 0, `listWindows returned ${windows.length}`);
  log('desktopWindowCount', windows.length);
  const leaked = windows.filter((w) => ownHandles.includes(w.handle));
  must('desktopOwnExcluded', leaked.length === 0, leaked.map((w) => w.title).join(', '));

  // ---------------------------------------------------------------- B5
  if (ownPoint) {
    let code = 'none';
    try {
      await desktop.click({ x: ownPoint.x, y: ownPoint.y });
      code = 'clicked';
    } catch (err) {
      code = err.code;
    }
    must('ownWindowBlocked', code === 'BLOCKED_OWN_WINDOW', code);
  }

  if (!notepad) {
    log('desktopNotepad', 'skipped (set AEVAL_DESKTOP_NOTEPAD=1, or run on CI)');
    return;
  }

  // ---------------------------------------------------------------- Notepad
  const tag = `aeval-desktop-check-${process.pid}-${Date.now().toString(36)}`;
  const file = path.join(os.tmpdir(), `${tag}.txt`);
  fs.writeFileSync(file, `${FIXTURE}\r\n`, 'utf8');
  const beforeList = await desktop.listWindows({ includeMinimized: true });
  const before = new Set(beforeList.map((w) => w.handle));
  const hadNotepad = beforeList.some((w) => /^notepad$/i.test(w.process));
  let win = null;
  try {
    spawn('notepad.exe', [file], { detached: true, stdio: 'ignore', windowsHide: false }).unref();
    win = await waitFor(async () => {
      const list = await desktop.listWindows();
      return list.find((w) => !before.has(w.handle) && w.title.includes(tag)) || null;
    }, 15000);
    must('desktopNotepadIncluded', !!win, 'no new window titled with the temp file appeared');
    if (!win) return;
    log('notepadWindow', `${win.handle}:${win.process}:${win.className}`);
    // Each step stands alone, so one that cannot run (no keyboard focus on
    // a locked session) does not hide the result of the rest.
    const step = async (name, fn) => {
      try {
        await fn();
      } catch (err) {
        must(name, false, `${err.code || 'ERROR'}:${err.message}`);
      }
    };
    await step('focusWindow', async () => {
      const f = await desktop.focusWindow(win.handle);
      log('focusMethod', f.method);
      await sleep(400);
    });

    // -------------------------------------------------------------- B3
    let editor = null;
    await step('uiaTree', async () => {
      t = Date.now();
      const tree = await desktop.readTree({ windowHandle: win.handle, filter: 'interactive' });
      const treeMs = Date.now() - t;
      const refsOk = tree.nodes.every((n) => /^ref_\d+$/.test(n.ref));
      editor = pickEditor(tree.nodes);
      must(
        'uiaTree',
        tree.nodes.length >= 5 && refsOk && !!editor && tree.text.length < 50000,
        `nodes=${tree.nodes.length} refs=${refsOk} editor=${!!editor} chars=${tree.text.length}`
      );
      log('uiaTreeNodes', tree.nodes.length);
      log('uiaTreeMs', treeMs);
      // Roles only: a tabbed Notepad restores the person's own tabs into
      // this window, and their names do not belong in a CI log.
      log('uiaTreeRoles', [...new Set(tree.nodes.map((n) => n.role))].join(','));
    });
    if (!editor) return;
    // The interactive tree can hold more than one edit; the one that types
    // is the biggest text control in the whole window, which may be a
    // Document the interactive filter leaves out.
    await step('editorPick', async () => {
      const all = await desktop.readTree({ windowHandle: win.handle, filter: 'all', depth: 30 });
      const best = pickEditor(all.nodes);
      if (best) editor = best;
      log(
        'editorCandidates',
        all.nodes
          .filter((n) => n.role === 'document' || n.role === 'edit')
          .map((n) => `${n.ref}:${n.role}:${n.name}:${(n.patterns || []).join('+')}:${(n.states || []).join('+')}:${n.rect ? n.rect.width + 'x' + n.rect.height : '-'}`)
          .join(' | ')
      );
      log('editor', `${editor.ref}:${editor.role}:${editor.name}`);
    });

    // -------------------------------------------------------------- B6
    // Zoom the text up (Notepad: Ctrl+Plus) so OCR sees large glyphs, then
    // put it back. Without focus the zoom is skipped and OCR reads 11pt.
    let zoomed = false;
    await step('notepadZoom', async () => {
      await desktop.key({ keys: 'ctrl+plus', repeat: 8, windowHandle: win.handle });
      zoomed = true;
      await sleep(500);
    });
    await step('ocrWords', async () => {
      t = Date.now();
      const o = await desktop.ocr({ windowHandle: win.handle });
      log('ocrMs', Date.now() - t);
      const words = o.spans.map((s) => s.text.toLowerCase());
      const hits = ['invoice', 'total', 'due'].filter((w) => words.includes(w));
      const rectsIn = o.spans.filter((s) => hits.includes(s.text.toLowerCase())).every((s) => inside(s.rect, win.rect));
      const detail = `hits=${hits.join('|')} words=${o.spans.length} inWindow=${rectsIn} lang=${o.lang}`;
      must('ocrWords', hits.length >= 2 && rectsIn, detail);
      log('ocrDetail', detail);
    });
    if (zoomed) await step('notepadZoomReset', () => desktop.key({ keys: 'ctrl+0', windowHandle: win.handle }));

    // -------------------------------------------------------------- B4
    await step('uiaTypeRead', async () => {
      await desktop.key({ keys: 'ctrl+end', windowHandle: win.handle });
      await desktop.type({ text: ` ${TYPED}`, ref: editor.ref });
      await sleep(300);
      const g = await desktop.getText({ windowHandle: win.handle, ocrFallback: false });
      must('uiaTypeRead', g.text.includes(TYPED) && g.source === 'uia', `source=${g.source} spans=${g.spans.length}`);
    });

    // form_input: the pattern path, no keystrokes, no focus needed.
    await step('uiaSetValue', async () => {
      t = Date.now();
      const sv = await desktop.setValue({ ref: editor.ref, value: SET });
      log('uiaSetValueMs', Date.now() - t);
      log('uiaSetValueVia', sv.via);
      await sleep(200);
      const g = await desktop.getText({ windowHandle: win.handle, ocrFallback: false });
      must('uiaSetValue', g.text.includes(SET), `via=${sv.via}`);
      log('getTextSpans', g.spans.length);
    });

    await step('findEditor', async () => {
      const found = await desktop.find({ query: 'text editor', windowHandle: win.handle, limit: 3 });
      const top = found.nodes[0];
      must('findEditor', !!top && (top.role === 'document' || top.role === 'edit'), JSON.stringify(found.nodes.map((n) => n.role)));
      log('findTop', top ? `${top.role}:${top.name}:${top.score}` : 'none');
    });

    await step('elementAt', async () => {
      const r = editor.rect;
      const at = await desktop.elementAt({ x: r.x + Math.round(r.width / 2), y: r.y + Math.round(r.height / 2) });
      log('elementAt', `${at.node.role}:${at.node.ref}`);
    });
    await step('restoreCursor', () => desktop.restoreCursor());
  } finally {
    if (win) {
      const closed = await closeWithoutSaving(desktop, win, tag, hadNotepad, log);
      must('notepadClosed', closed, 'the check tab or window is still open');
    }
    try {
      fs.unlinkSync(file);
    } catch (err) {
      log('tempFileLeft', file);
    }
  }
}

// Closing uses patterns only (Invoke on buttons, WM_CLOSE on the window), so
// it works even when the check never got the keyboard focus.
//
// Classic Notepad asks about unsaved text in a separate dialog. The tabbed
// Windows 11 one asks inside its window, and only when a tab (not the
// window) closes: closing the window keeps unsaved tabs for next time, which
// would leave this check's text in the person's session. So the tab closes
// through its own Close Tab button, and the window only when Notepad was not
// running before the check; what is left in it then is the person's restored
// session, which Notepad keeps as it was.
// The text area: the largest document or edit, preferring one that exposes
// text or a writable value over a small field such as a find box.
function pickEditor(nodes) {
  const area = (n) => (n.rect ? n.rect.width * n.rect.height : 0);
  const score = (n) =>
    area(n) * ((n.patterns || []).includes('text') || (n.patterns || []).includes('value') ? 2 : 1) *
    (n.role === 'document' ? 1.5 : 1);
  return (
    nodes
      .filter((n) => n.role === 'document' || n.role === 'edit')
      .sort((a, b) => score(b) - score(a))[0] || null
  );
}

async function closeWithoutSaving(desktop, win, tag, hadNotepad, log) {
  let tabbed = false;
  try {
    const tree = await desktop.readTree({ windowHandle: win.handle, filter: 'all' });
    const tabs = tree.nodes.filter((n) => n.role === 'tabitem');
    tabbed = tabs.length > 0;
    if (tabbed) {
      const mine = tabs.find((n) => (n.name || '').includes(tag));
      const close =
        mine && tree.nodes.find((n) => n.parentRef === mine.ref && n.role === 'button' && /close/i.test(n.name || ''));
      if (!close) throw Object.assign(new Error('no Close Tab button on the check tab'), { code: 'NOT_FOUND' });
      await desktop.invoke({ ref: close.ref });
    } else {
      await desktop.closeWindow(win.handle);
    }
  } catch (err) {
    log('closeError', `${err.code}:${err.message}`);
    return false;
  }
  const done = await waitFor(async () => {
    const list = await desktop.listWindows();
    if (!list.some((w) => w.title.includes(tag))) return true;
    for (const w of list.filter((x) => x.pid === win.pid)) {
      try {
        const hit = await desktop.find({ query: "don't save", windowHandle: w.handle, limit: 1, minScore: 70 });
        if (hit.nodes.length) {
          await desktop.click({ ref: hit.nodes[0].ref });
          log('dontSave', `${hit.nodes[0].role}:${hit.nodes[0].name}`);
        }
      } catch (err) {
        // The dialog may vanish between the read and the click.
      }
    }
    return false;
  }, 10000, 400);
  if (done && tabbed && !hadNotepad) {
    const still = (await desktop.listWindows({ includeMinimized: true })).find((w) => w.handle === win.handle);
    if (still) await desktop.closeWindow(win.handle);
  }
  return !!done;
}

module.exports = { run, pickEditor, FIXTURE, TYPED };
