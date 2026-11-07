'use strict';

// Desktop perception and action: Claude in Chrome's read_page / find /
// get_page_text / click / type / key / scroll, for any Windows app.
// Contract: SPEC-v2.md section 1.3. This header is the version of it that
// the code keeps.
//
// SHAPE
//   Main process only; renderers never touch this module. It owns one child,
//   `powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy
//   Bypass -Command -`, which reads host.ps1 and host.cs from this module
//   over stdin (powershell cannot read inside app.asar; Electron can), then
//   talks JSON lines: `{id, op, args, timeoutMs}` in, `{id, ok, result}` or
//   `{id, ok:false, error:{code, message}}` out, plus `{event}` lines.
//   No native npm module, no build step, nothing for electron-builder to
//   unpack.
//
// COORDINATES
//   Every rect and point here is PHYSICAL virtual-screen pixels: the helper
//   runs per-monitor-v2 DPI aware, so UIA, GetWindowRect, capture and
//   SendInput agree on every monitor. Rects are `{x, y, width, height}`.
//   Convert before positioning an Electron window: `toDip(rect)` (which is
//   `screen.screenToDipRect(null, rect)` inside Electron) and back with
//   `toPhysical(rect)`.
//
// LIFECYCLE
//   Lazy: nothing spawns until the first call (boot time is a gate). The
//   first spawn compiles host.cs (about 1.5s) and caches the assembly in
//   %LOCALAPPDATA%\aevalbuddy\desktop-<hash>.dll; later spawns load it.
//   start(): Promise<void>       idempotent, resolves once the helper answers
//   stop(): void                 kills the helper tree; call from before-quit
//   isReady(): boolean
//   init({ ownHandles, beforeSyntheticKey, afterSyntheticKey, onUserInput, wantInput })
//     ownHandles(): array of Buffer (getNativeWindowHandle()), number,
//       'w1A2B' handle, {handle, block:false} for a click-through layer that
//       may sit over targets (the lens), or {pid}. process.pid and Electron's
//       child pids are always added. Re-read before every call.
//     beforeSyntheticKey(key) / afterSyntheticKey(key): awaited around any
//       key() that sends Escape, so main can drop its global Esc (Stop) and
//       retake it; RegisterHotKey fires on injected input too.
//     onUserInput(ev): physical (non-injected) mouse or key input seen by a
//       low-level hook, `{kind:'mouse-move'|'mouse-button'|'wheel'|'key', x,
//       y, distance, t}`, throttled to about 10 per second. Key identity is
//       never reported. Also emitted as `events.on('user-input')`.
//   Death: a helper exit rejects in-flight calls with HELPER_DOWN and the
//   next call respawns. Three deaths inside 60s stop respawning until
//   start() is called explicitly; status().disabled says so.
//
// WINDOWS
//   listWindows({ includeMinimized = false }) ->
//     [{ handle:'w1A2B3C', title, process, pid, rect, display, isForeground,
//        isMinimized, elevated, className }]           top of z-order first
//   foregroundWindow() -> same | null    the app the person was in: never
//                                         ours, the window under ours if so
//   focusWindow(handle) -> { ok, method }  restore, SetForegroundWindow,
//                                         input/AttachThreadInput fallbacks
//   closeWindow(handle) -> { ok }        posts WM_CLOSE (the app may prompt)
//   monitors() -> [{ rect, work, dpi, scale, primary, device }]
//
// PERCEPTION
//   readTree({ windowHandle = foreground, filter = 'visible'|'interactive'|
//              'all', depth = 15, ref, maxChars = 50000, maxNodes = 3000 }) ->
//     { text, nodes, truncated, generation, windowHandle, window, readMs }
//     text is the browser toolset's format, two spaces per depth:
//       button "Send" [ref_12] (focused)
//       edit "Search" [ref_13] value="cats"
//     nodes: [{ ref, role, name, value, rect, states, patterns, parentRef,
//               automationId, depth, interactive }]
//     states: focused disabled checked mixed expanded collapsed selected
//             password offscreen readonly
//     patterns: invoke value toggle expandcollapse selectionitem rangevalue
//               scroll scrollitem text
//     'visible' leaves out offscreen nodes and unnamed layout containers,
//     'interactive' keeps only what can be acted on, 'all' adds offscreen.
//     depth counts kept nodes, not layout containers. Password values are
//     always '•••'.
//   find({ query, windowHandle, limit = 20 }) -> { nodes, text }
//     Local literal and fuzzy scoring over name, value, role, automationId
//     and helpText. Role words in the query ("button", "field", "link",
//     "tab", "checkbox", "menu") weight the role. No model call; the agent
//     layer reranks. Each node gains `score`.
//   getText({ windowHandle, rect, maxChars = 50000, ocrFallback = true }) ->
//     { text, source:'uia'|'ocr'|'mixed', spans:[{ text, rect, ref, source }] }
//     Reading order. UIA spans are visible lines of TextPattern documents
//     (with each line's rect) and Name/Value of text-bearing nodes. Under
//     200 characters of UIA text it OCRs the window (or rect) too.
//   ocr({ rect | windowHandle, lang = 'auto' }) -> { spans, text, lang }
//     Windows.Media.Ocr, local; spans are words with rects.
//   screenshot({ windowHandle, rect, maxEdge = 1568 }) ->
//     { dataUrl, rect, scale }   PNG of the screen as composed; our windows
//                                are excluded from capture already
//   zoom({ rect, maxEdge = 1568 }) -> same, enlarged up to maxEdge (4x cap)
//   elementAt({ x, y }) -> { node, windowHandle }
//
// ACTION      every target is { ref } or { x, y }
//   click(target, { button = 'left', count = 1, modifiers = '' }) ->
//     { ok, via:'invoke'|'sendinput', point }
//     A plain left single click on a ref with InvokePattern invokes it and
//     leaves the cursor alone. `physical: true` forces SendInput.
//   hover(target) -> { ok, point }
//   drag({ from:{x,y}, to:{x,y} }) -> { ok }
//   type({ text, ref, windowHandle })   ref: focus its window and the element
//                                       first. KEYEVENTF_UNICODE, so layout
//                                       independent. Stops with FOCUS_LOST
//                                       the moment the target loses focus.
//   key({ keys:'ctrl+s' | 'Backspace Backspace', repeat = 1, windowHandle })
//   scroll(target, { direction:'up'|'down'|'left'|'right', amount = 3 })
//     ScrollPattern on a ref that has it, else wheel notches at the point.
//   scrollTo({ ref }) -> { ok, node }    ScrollItemPattern.ScrollIntoView
//   setValue({ ref, value })             form_input: string, number, boolean
//                                        for checkboxes, option text for combo
//                                        boxes; patterns first, else focus +
//                                        ctrl+a + type
//   invoke({ ref, action:'invoke'|'toggle'|'select'|'expand'|'collapse'|
//            'focus' }) -> { ok, via, pending }
//   restoreCursor() -> { ok, point }     back to where it was before the
//                                        first synthetic pointer move
//   Without windowHandle, type and key go to the foreground window and are
//   refused when that is ours. Any pointer action whose point lies on one of
//   our visible windows is refused with BLOCKED_OWN_WINDOW, and a ref click
//   covered by another app's window is refused with OCCLUDED.
//
// REFS
//   'ref_N'. N comes from one counter per helper session, so it only grows
//   for a window. The same element (same UIA RuntimeId) keeps its ref across
//   reads. A ref resolves until its element is gone; then STALE_REF with the
//   browser toolset's message. A helper restart forgets every ref.
//   `generation` bumps on UIA structure-changed events for that window.
//
// ERRORS
//   Calls reject with DesktopError { code, message }. Codes: STALE_REF,
//   NOT_FOUND, BLOCKED_OWN_WINDOW, ELEVATED_TARGET, NO_PATTERN, TIMEOUT
//   (4s per op, 15s for ocr and getText with OCR), HELPER_DOWN, BAD_ARGS,
//   and beyond the spec: OCCLUDED, NOT_FOREGROUND, FOCUS_LOST, DISABLED,
//   NO_OCR, UIA_ERROR, INTERNAL, UNSUPPORTED (not Windows).

const { spawn, spawnSync } = require('child_process');
const { EventEmitter } = require('events');
const fs = require('fs');
const os = require('os');
const path = require('path');
const readline = require('readline');

const OP_TIMEOUT = 4000;
const OCR_TIMEOUT = 15000;
const START_TIMEOUT = 25000;
const DEATH_WINDOW_MS = 60000;
const DEATH_LIMIT = 3;
const POWERSHELL = path.join(
  process.env.SystemRoot || 'C:\\Windows',
  'System32',
  'WindowsPowerShell',
  'v1.0',
  'powershell.exe'
);

class DesktopError extends Error {
  constructor(code, message) {
    super(message);
    this.name = 'DesktopError';
    this.code = code;
  }

  toJSON() {
    return { code: this.code, message: this.message };
  }
}

const events = new EventEmitter();
let deps = {};
let child = null;
let starting = null;
let ready = false;
let readyInfo = null;
let nextId = 1;
const pending = new Map();
let queue = Promise.resolve();
let deaths = [];
let disabled = false;
let lastStderr = '';
let sentOwn = '';
let watching = false;
let watchOverride = null;
// The first UI Automation read in a helper session pays for loading the UIA
// client and for the target app building its tree for a first client; that
// one read gets longer than the 4s every later read is held to.
const UIA_COLD_TIMEOUT = 12000;
let uiaWarm = false;

function uiaTimeout() {
  return uiaWarm ? OP_TIMEOUT : UIA_COLD_TIMEOUT;
}

function appVersion() {
  try {
    return require('../../package.json').version;
  } catch (err) {
    return '';
  }
}

function packaged() {
  try {
    return !!(process.versions.electron && require('electron').app.isPackaged);
  } catch (err) {
    // Plain node (the build script): not packaged.
    return false;
  }
}

// The compiled-assembly cache lives in a folder any process of this user can
// write to, so an installed copy never loads from it: it loads the prebuilt
// assembly whose hash it shipped with, or compiles in memory.
function cacheDir() {
  if (packaged()) return '';
  if (process.env.AEVAL_DESKTOP_CACHE !== undefined) return process.env.AEVAL_DESKTOP_CACHE;
  return path.join(process.env.LOCALAPPDATA || os.tmpdir(), 'aevalbuddy');
}

// SHA-256 of build/desktop-<hash>.dll, written by tools/build-desktop.js
// into app.asar beside this file. The helper loads that DLL only when its
// bytes match.
function prebuiltSha() {
  try {
    const j = JSON.parse(fs.readFileSync(path.join(__dirname, 'prebuilt.json'), 'utf8'));
    return /^[0-9a-f]{64}$/.test(j.sha256) ? j.sha256 : '';
  } catch (err) {
    // No prebuilt was made (a dev checkout): the helper compiles instead.
    return '';
  }
}

function psQuote(s) {
  return `'${String(s).replace(/'/g, "''")}'`;
}

function b64(s) {
  return Buffer.from(s, 'utf8').toString('base64');
}

// One line, because `-Command -` runs stdin line by line. Every request is
// written only after the helper says ready, so powershell's own reader has
// nothing buffered when host.cs takes over stdin.
// Where a build-time copy of the compiled helper may be (tools/
// build-desktop.js): beside app.asar when packaged, build/ from source.
function prebuiltDirs() {
  const dirs = [];
  if (process.resourcesPath) dirs.push(process.resourcesPath);
  dirs.push(path.join(__dirname, '..', '..', 'build'));
  return dirs.filter((d) => {
    try {
      return fs.statSync(d).isDirectory();
    } catch (err) {
      return false;
    }
  });
}

function bootstrapLine(opts = {}) {
  const cs = fs.readFileSync(path.join(__dirname, 'host.cs'), 'utf8');
  const ps = fs.readFileSync(path.join(__dirname, 'host.ps1'), 'utf8');
  const decode = (v) => `[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('${v}'))`;
  return (
    `$AevalCs=${decode(b64(cs))};` +
    `$AevalCache=${psQuote(opts.cache !== undefined ? opts.cache : cacheDir())};` +
    `$AevalPrebuilt=${psQuote(opts.compileOnly ? '' : prebuiltDirs().join(';'))};` +
    `$AevalDllSha=${psQuote(opts.compileOnly ? '' : prebuiltSha())};` +
    `$AevalCompileOnly=${opts.compileOnly ? '$true' : '$false'};` +
    `$AevalVersion=${psQuote(appVersion())};` +
    `iex (${decode(b64(ps))})\n`
  );
}

function recordDeath() {
  const now = Date.now();
  deaths = deaths.filter((t) => now - t < DEATH_WINDOW_MS);
  deaths.push(now);
  if (deaths.length >= DEATH_LIMIT) disabled = true;
}

function failAll(code, message) {
  for (const [, p] of pending) {
    clearTimeout(p.timer);
    p.reject(new DesktopError(code, message));
  }
  pending.clear();
}

function onLine(line) {
  if (!line || line[0] !== '{') {
    if (line && line.trim()) lastStderr = `${lastStderr}\n[stdout] ${line}`.slice(-4096);
    return;
  }
  let msg;
  try {
    msg = JSON.parse(line);
  } catch (err) {
    return;
  }
  if (msg.event === 'ready') {
    ready = true;
    readyInfo = msg;
    events.emit('ready', msg);
    return;
  }
  if (msg.event === 'fatal') {
    events.emit('fatal', msg);
    lastStderr = `${lastStderr}\n[fatal] ${msg.error}`.slice(-4096);
    return;
  }
  if (msg.event === 'user-input') {
    events.emit('user-input', msg);
    if (typeof deps.onUserInput === 'function') {
      try {
        deps.onUserInput(msg);
      } catch (err) {
        // A listener error must not take the reader down.
      }
    }
    return;
  }
  const p = pending.get(msg.id);
  if (!p) return;
  pending.delete(msg.id);
  clearTimeout(p.timer);
  if (msg.ok) p.resolve(msg.result);
  else {
    const e = msg.error || {};
    p.reject(new DesktopError(e.code || 'INTERNAL', e.message || 'desktop helper error'));
  }
}

function spawnHelper() {
  if (process.platform !== 'win32') {
    return Promise.reject(new DesktopError('UNSUPPORTED', 'desktop control needs Windows'));
  }
  return new Promise((resolve, reject) => {
    const proc = spawn(
      POWERSHELL,
      ['-NoLogo', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-Command', '-'],
      { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] }
    );
    child = proc;
    ready = false;
    uiaWarm = false;
    sentOwn = '';
    watching = false;
    let settled = false;
    const settle = (err) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      events.off('ready', onReady);
      events.off('fatal', onFatal);
      if (err) reject(err);
      else resolve();
    };
    const onReady = () => settle();
    const onFatal = (m) => settle(new DesktopError('HELPER_DOWN', `desktop helper failed to start: ${m.error}`));
    events.on('ready', onReady);
    events.on('fatal', onFatal);
    const timer = setTimeout(() => {
      settle(new DesktopError('HELPER_DOWN', `desktop helper did not start within ${START_TIMEOUT / 1000}s`));
      killTree(proc);
    }, START_TIMEOUT);

    readline.createInterface({ input: proc.stdout }).on('line', onLine);
    proc.stderr.setEncoding('utf8');
    proc.stderr.on('data', (d) => {
      lastStderr = (lastStderr + d).slice(-4096);
    });
    proc.stdin.on('error', () => {});
    proc.on('error', (err) => settle(new DesktopError('HELPER_DOWN', `could not run powershell.exe: ${err.message}`)));
    proc.on('exit', (code) => {
      const wasOurs = child === proc;
      if (wasOurs) {
        child = null;
        ready = false;
        if (!proc.aevalStopping) recordDeath();
        failAll('HELPER_DOWN', `desktop helper exited (code ${code})`);
      }
      settle(new DesktopError('HELPER_DOWN', `desktop helper exited during start (code ${code}): ${lastStderr.trim().slice(-400)}`));
    });
    proc.stdin.write(bootstrapLine());
  });
}

function start() {
  if (ready && child) return Promise.resolve();
  if (starting) return starting;
  disabled = false;
  deaths = [];
  return ensure();
}

function ensure() {
  if (ready && child) return Promise.resolve();
  if (disabled) {
    return Promise.reject(
      new DesktopError('HELPER_DOWN', 'desktop control stopped after three crashes in a minute; call start() to try again')
    );
  }
  if (!starting) {
    starting = spawnHelper().finally(() => {
      starting = null;
    });
  }
  return starting;
}

function killTree(proc) {
  if (!proc || proc.exitCode !== null) return;
  proc.aevalStopping = true;
  try {
    // The helper has a conhost child; /T takes it too, so nothing under the
    // install folder outlives the app and blocks an update.
    spawnSync('taskkill', ['/T', '/F', '/PID', String(proc.pid)], { windowsHide: true, timeout: 2000 });
  } catch (err) {
    // Fall through to kill().
  }
  try {
    proc.kill();
  } catch (err) {
    // Already gone.
  }
}

function stop() {
  const proc = child;
  child = null;
  ready = false;
  starting = null;
  failAll('HELPER_DOWN', 'desktop helper stopped');
  if (!proc) return;
  proc.aevalStopping = true;
  try {
    proc.stdin.end('{"op":"quit"}\n');
  } catch (err) {
    // The pipe may already be closed.
  }
  killTree(proc);
}

process.on('exit', () => {
  if (child) killTree(child);
});

function isReady() {
  return !!(ready && child);
}

function status() {
  return {
    ready: isReady(),
    disabled,
    deaths: deaths.length,
    pid: child ? child.pid : null,
    dpi: readyInfo ? readyInfo.dpi : null,
    watching,
    stderr: lastStderr.trim().slice(-1000),
  };
}

function init(options = {}) {
  deps = { ...deps, ...options };
  sentOwn = '';
}

// ----------------------------------------------------------------- own windows

function handleOf(h) {
  if (h === null || h === undefined) return null;
  if (Buffer.isBuffer(h)) {
    if (h.length >= 8) return Number(h.readBigUInt64LE(0));
    if (h.length >= 4) return h.readUInt32LE(0);
    return null;
  }
  if (typeof h === 'number') return h;
  if (typeof h === 'bigint') return Number(h);
  if (typeof h === 'string') {
    const s = h.trim();
    if (/^w[0-9a-f]+$/i.test(s)) return parseInt(s.slice(1), 16);
    if (/^0x[0-9a-f]+$/i.test(s)) return parseInt(s.slice(2), 16);
    if (/^\d+$/.test(s)) return Number(s);
  }
  return null;
}

function handleStr(h) {
  const n = handleOf(h);
  return n === null ? null : `w${n.toString(16).toUpperCase()}`;
}

function electronPids() {
  try {
    if (!process.versions.electron || process.type !== 'browser') return [];
    const { app } = require('electron');
    return app.isReady() ? app.getAppMetrics().map((m) => m.pid) : [];
  } catch (err) {
    return [];
  }
}

function ownConfig() {
  const pids = new Set([process.pid, ...electronPids()]);
  const windows = [];
  let list = [];
  try {
    list = typeof deps.ownHandles === 'function' ? deps.ownHandles() || [] : [];
  } catch (err) {
    list = [];
  }
  for (const item of list) {
    if (item && typeof item === 'object' && !Buffer.isBuffer(item)) {
      if (item.pid) pids.add(item.pid);
      const h = handleOf(item.handle);
      if (h) windows.push({ handle: h, block: item.block !== false });
    } else {
      const h = handleOf(item);
      if (h) windows.push({ handle: h, block: true });
    }
  }
  return { pids: [...pids], windows };
}

// ----------------------------------------------------------------- calls

function send(op, args, timeoutMs) {
  return new Promise((resolve, reject) => {
    if (!child || !ready) {
      reject(new DesktopError('HELPER_DOWN', 'desktop helper is not running'));
      return;
    }
    const id = nextId++;
    const proc = child;
    // A helper that stops answering entirely is killed: the next call gets
    // a fresh one.
    const timer = setTimeout(() => {
      if (!pending.has(id)) return;
      pending.delete(id);
      reject(new DesktopError('TIMEOUT', `${op} got no answer in ${timeoutMs + 2000}ms; restarting the helper`));
      recordDeath();
      killTree(proc);
    }, timeoutMs + 2000);
    pending.set(id, { resolve, reject, timer });
    proc.stdin.write(`${JSON.stringify({ id, op, args: args || {}, timeoutMs })}\n`);
  });
}

// One op at a time, in order: input has to land in the order it was asked
// for, and the helper runs requests sequentially anyway.
function call(op, args = {}, timeoutMs = OP_TIMEOUT) {
  const run = async () => {
    await ensure();
    const own = ownConfig();
    const key = JSON.stringify(own);
    if (key !== sentOwn) {
      await send('config', own, OP_TIMEOUT);
      sentOwn = key;
    }
    // wantInput lets main keep the low-level hook off except while something
    // is listening for a takeover (an agent run, an open Find).
    const wantWatch =
      watchOverride !== null
        ? watchOverride
        : typeof deps.wantInput === 'function'
          ? !!deps.wantInput()
          : !!(typeof deps.onUserInput === 'function' || events.listenerCount('user-input') > 0);
    if (wantWatch !== watching) {
      await send('watchInput', { on: wantWatch }, OP_TIMEOUT);
      watching = wantWatch;
    }
    if (op === null) return null;
    return send(op, args, timeoutMs);
  };
  const result = queue.then(run, run);
  queue = result.catch(() => {});
  return result;
}

// ----------------------------------------------------------------- shapes

function rectObj(r) {
  if (!r) return null;
  if (Array.isArray(r)) return { x: r[0], y: r[1], width: r[2], height: r[3] };
  return r;
}

function rectArg(r) {
  if (!r) return undefined;
  if (Array.isArray(r)) return r;
  return [r.x, r.y, r.width !== undefined ? r.width : r.w, r.height !== undefined ? r.height : r.h];
}

let electronScreen;
function getScreen() {
  if (electronScreen !== undefined) return electronScreen;
  electronScreen = null;
  try {
    if (process.versions.electron && process.type === 'browser') {
      const { app, screen } = require('electron');
      if (app.isReady()) electronScreen = screen;
      else electronScreen = undefined;
    }
  } catch (err) {
    electronScreen = null;
  }
  return electronScreen || null;
}

function windowShape(w) {
  if (!w) return w;
  const out = { ...w, rect: rectObj(w.rect) };
  const scr = getScreen();
  if (scr && out.rect) {
    try {
      out.display = scr.getDisplayMatching(scr.screenToDipRect(null, out.rect)).id;
    } catch (err) {
      // Keep the device name the helper gave.
    }
  }
  return out;
}

function nodeShape(n) {
  return { ...n, rect: rectObj(n.rect) };
}

function quote(s) {
  return `"${String(s).replace(/\\/g, '\\\\').replace(/"/g, '\\"').replace(/\s*[\r\n]+\s*/g, ' ')}"`;
}

const QUIET_STATES = new Set(['readonly']);

/** One node in the browser toolset's read_page line format. */
function nodeLine(n, indent = 0) {
  let line = `${'  '.repeat(Math.max(0, indent))}${n.role}`;
  if (n.name) line += ` ${quote(n.name)}`;
  line += ` [${n.ref}]`;
  if (n.value !== undefined && n.value !== null && n.value !== '') line += ` value=${quote(n.value)}`;
  const states = (n.states || []).filter((s) => !QUIET_STATES.has(s));
  if (states.length) line += ` (${states.join(', ')})`;
  return line;
}

function renderTree(nodes, maxChars = 50000) {
  const lines = [];
  let used = 0;
  let truncated = false;
  for (const n of nodes) {
    const line = nodeLine(n, n.depth || 0);
    if (used + line.length + 1 > maxChars) {
      truncated = true;
      break;
    }
    lines.push(line);
    used += line.length + 1;
  }
  if (truncated) {
    lines.push(`[output truncated at ${maxChars} characters; pass ref to read one subtree, or filter: 'interactive']`);
  }
  return { text: lines.join('\n'), truncated };
}

// ----------------------------------------------------------------- windows

async function listWindows({ includeMinimized = false } = {}) {
  const list = await call('listWindows', { includeMinimized });
  return list.map(windowShape);
}

async function foregroundWindow() {
  return windowShape(await call('foregroundWindow'));
}

async function focusWindow(handle) {
  return call('focusWindow', { handle: handleOf(handle) });
}

async function closeWindow(handle) {
  return call('closeWindow', { handle: handleOf(handle) });
}

async function monitors() {
  const list = await call('monitors');
  return list.map((m) => ({ ...m, rect: rectObj(m.rect), work: rectObj(m.work) }));
}

// ----------------------------------------------------------------- perception

function windowArg(windowHandle) {
  return windowHandle === undefined || windowHandle === null ? undefined : handleOf(windowHandle);
}

async function readTree({
  windowHandle,
  filter = 'visible',
  depth = 15,
  ref,
  maxChars = 50000,
  maxNodes = 3000,
  strategy,
} = {}) {
  const r = await call('readTree', { windowHandle: windowArg(windowHandle), filter, depth, ref, maxNodes, strategy }, uiaTimeout());
  uiaWarm = true;
  const nodes = r.nodes.map(nodeShape);
  const rendered = renderTree(nodes, maxChars);
  return {
    text: rendered.text,
    nodes,
    truncated: !!(r.truncated || rendered.truncated),
    generation: r.generation,
    windowHandle: r.windowHandle,
    window: windowShape(r.window),
    readMs: r.readMs,
  };
}

// Words in a query that name a role rather than the element.
const ROLE_WORDS = {
  button: ['button', 'splitbutton'],
  btn: ['button', 'splitbutton'],
  link: ['hyperlink'],
  hyperlink: ['hyperlink'],
  field: ['edit', 'combobox', 'document'],
  input: ['edit', 'combobox'],
  textbox: ['edit'],
  box: ['edit', 'combobox', 'checkbox'],
  search: ['edit', 'combobox'],
  tab: ['tabitem'],
  checkbox: ['checkbox'],
  check: ['checkbox'],
  toggle: ['checkbox', 'button'],
  radio: ['radiobutton'],
  menu: ['menuitem', 'menu', 'menubar'],
  item: ['listitem', 'treeitem', 'menuitem', 'dataitem'],
  dropdown: ['combobox'],
  combo: ['combobox'],
  select: ['combobox', 'list'],
  slider: ['slider'],
  list: ['list', 'listitem'],
  row: ['dataitem', 'listitem'],
  heading: ['header', 'text'],
  title: ['titlebar', 'text'],
  image: ['image'],
  icon: ['image', 'button'],
};

function norm(s) {
  return String(s || '')
    .toLowerCase()
    .normalize('NFKD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();
}

function bigrams(s) {
  const out = new Map();
  const t = ` ${s} `;
  for (let i = 0; i < t.length - 1; i++) {
    const g = t.slice(i, i + 2);
    out.set(g, (out.get(g) || 0) + 1);
  }
  return out;
}

function dice(a, b) {
  if (!a || !b) return 0;
  const A = bigrams(a);
  const B = bigrams(b);
  let inter = 0;
  let total = 0;
  for (const [, v] of A) total += v;
  for (const [g, v] of B) {
    total += v;
    if (A.has(g)) inter += Math.min(v, A.get(g));
  }
  return total ? (2 * inter) / total : 0;
}

/** 0..100: how well one field's text matches the (role-free) query. */
function scoreText(q, qTokens, text) {
  const s = norm(text);
  if (!s || !q) return 0;
  if (s === q) return 100;
  if (s.startsWith(q)) return 88;
  if (new RegExp(`(^| )${q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}( |$)`).test(s)) return 82;
  if (s.includes(q)) return 72;
  const words = s.split(' ');
  if (qTokens.length > 1 && qTokens.every((t) => words.some((w) => w.startsWith(t)))) return 64;
  const hit = qTokens.filter((t) => words.some((w) => w.startsWith(t) || (t.length > 3 && dice(t, w) > 0.7))).length;
  const partial = qTokens.length ? (hit / qTokens.length) * 55 : 0;
  // Short labels against a typo: "sumbit" still finds "Submit".
  const fuzzy = s.length <= 60 ? dice(q, s) * 60 : 0;
  return Math.max(partial, fuzzy);
}

function scoreNode(node, query) {
  const all = norm(query).split(' ').filter(Boolean);
  const roles = new Set();
  const rest = [];
  for (const t of all) {
    if (ROLE_WORDS[t]) ROLE_WORDS[t].forEach((r) => roles.add(r));
    else rest.push(t);
  }
  // A query made only of role words ("button") is about the role itself.
  const tokens = rest.length ? rest : all;
  const q = tokens.join(' ');
  let score = Math.max(
    scoreText(q, tokens, node.name),
    scoreText(q, tokens, node.value) * 0.85,
    scoreText(q, tokens, node.automationId) * 0.7,
    scoreText(q, tokens, node.helpText) * 0.6
  );
  if (!rest.length && roles.size) score = roles.has(node.role) ? 60 : 0;
  else if (roles.size) score += roles.has(node.role) ? 15 : -12;
  if (score <= 0) return 0;
  if (node.interactive) score += 3;
  if (!(node.states || []).includes('offscreen')) score += 2;
  return Math.min(100, Math.round(score));
}

async function find({ query, windowHandle, limit = 20, minScore = 45 } = {}) {
  if (!query || !String(query).trim()) throw new DesktopError('BAD_ARGS', 'find needs a query');
  const tree = await readTree({ windowHandle, filter: 'all', depth: 60, maxNodes: 4000, maxChars: Infinity });
  const scored = [];
  for (const n of tree.nodes) {
    if (n.depth === 0) continue;
    const score = scoreNode(n, query);
    if (score >= minScore) scored.push({ ...n, score });
  }
  scored.sort((a, b) => b.score - a.score || a.depth - b.depth);
  const nodes = scored.slice(0, Math.max(1, Math.min(100, limit)));
  return {
    nodes,
    text: nodes.map((n) => nodeLine(n)).join('\n'),
    windowHandle: tree.windowHandle,
    generation: tree.generation,
  };
}

function spanShape(s) {
  return { text: s.text, rect: rectObj(s.rect), ref: s.ref || null, source: s.source };
}

function overlaps(a, b) {
  if (!a || !b) return false;
  return a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;
}

async function getText({ windowHandle, rect, maxChars = 50000, ocrFallback = true } = {}) {
  const r = await call(
    'getText',
    { windowHandle: windowArg(windowHandle), rect: rectArg(rect), maxChars },
    ocrFallback ? OCR_TIMEOUT : OP_TIMEOUT
  );
  const spans = r.spans.map(spanShape);
  const out = { text: r.text, source: 'uia', spans, truncated: !!r.truncated, windowHandle: r.windowHandle };
  if (!ocrFallback || r.text.replace(/\s+/g, '').length >= 200) return out;
  let o;
  try {
    o = await ocr({ rect: rect || rectObj(r.windowRect), windowHandle: r.windowHandle });
  } catch (err) {
    out.ocrError = err.toJSON ? err.toJSON() : { code: 'INTERNAL', message: err.message };
    return out;
  }
  const extra = o.spans.filter((s) => !spans.some((u) => overlaps(u.rect, s.rect)));
  if (!spans.length) return { ...out, text: o.text, source: 'ocr', spans: o.spans };
  if (!extra.length) return out;
  return {
    ...out,
    source: 'mixed',
    spans: [...spans, ...extra],
    text: `${r.text}${extra.map((s) => s.text).join(' ')}\n`,
  };
}

async function ocr({ rect, windowHandle, lang = 'auto' } = {}) {
  const r = await call('ocr', { rect: rectArg(rect), windowHandle: windowArg(windowHandle), lang }, OCR_TIMEOUT);
  return { spans: r.spans.map(spanShape), text: r.text, lang: r.lang, rect: rectObj(r.rect), ocrMs: r.ocrMs };
}

async function screenshot({ windowHandle, rect, maxEdge = 1568 } = {}) {
  const r = await call('capture', { windowHandle: windowArg(windowHandle), rect: rectArg(rect), maxEdge }, 8000);
  return { dataUrl: r.dataUrl, rect: rectObj(r.rect), scale: r.scale, width: r.width, height: r.height };
}

async function zoom({ rect, maxEdge = 1568 } = {}) {
  if (!rect) throw new DesktopError('BAD_ARGS', 'zoom needs a rect');
  const r = await call('capture', { rect: rectArg(rect), maxEdge, enlarge: true }, 8000);
  return { dataUrl: r.dataUrl, rect: rectObj(r.rect), scale: r.scale, width: r.width, height: r.height };
}

async function elementAt({ x, y } = {}) {
  const r = await call('elementAt', { x, y });
  return { node: nodeShape(r.node), windowHandle: r.windowHandle };
}

async function resolve({ ref } = {}) {
  const r = await call('resolve', { ref });
  return { node: nodeShape(r.node), windowHandle: r.windowHandle };
}

// ----------------------------------------------------------------- action

function target(t = {}) {
  if (t.ref !== undefined && t.ref !== null) return { ref: t.ref };
  if (Number.isFinite(t.x) && Number.isFinite(t.y)) return { x: Math.round(t.x), y: Math.round(t.y) };
  throw new DesktopError('BAD_ARGS', 'target must be {ref} or {x, y}');
}

function pointOut(r) {
  if (r && Array.isArray(r.point)) r.point = { x: r.point[0], y: r.point[1] };
  return r;
}

async function click(t, opts = {}) {
  const o = { ...t, ...opts };
  return pointOut(
    await call('click', {
      ...target(o),
      button: o.button || 'left',
      count: o.count || (o.double ? 2 : 1),
      modifiers: o.modifiers || '',
      physical: !!o.physical,
      force: !!o.force,
    })
  );
}

async function hover(t) {
  return pointOut(await call('hover', target(t)));
}

async function drag({ from, to, steps } = {}) {
  if (!from || !to) throw new DesktopError('BAD_ARGS', 'drag needs from and to');
  const r = await call('drag', { from: target(from), to: target(to), steps });
  return { ok: r.ok, from: { x: r.from[0], y: r.from[1] }, to: { x: r.to[0], y: r.to[1] } };
}

async function type({ text, ref, windowHandle } = {}) {
  if (typeof text !== 'string') throw new DesktopError('BAD_ARGS', 'type needs text');
  return call('type', { text, ref, windowHandle: windowArg(windowHandle) }, Math.max(OP_TIMEOUT, 2000 + text.length * 2));
}

function sendsEscape(keys) {
  return String(keys)
    .split(/\s+/)
    .some((chord) => /^(esc|escape)$/i.test(chord.split('+').pop()));
}

async function key({ keys, repeat = 1, windowHandle } = {}) {
  if (!keys || !String(keys).trim()) throw new DesktopError('BAD_ARGS', 'key needs keys');
  const guard = sendsEscape(keys);
  if (guard && typeof deps.beforeSyntheticKey === 'function') await deps.beforeSyntheticKey('Escape');
  try {
    return await call('key', { keys, repeat, windowHandle: windowArg(windowHandle) });
  } finally {
    if (guard && typeof deps.afterSyntheticKey === 'function') await deps.afterSyntheticKey('Escape');
  }
}

async function scroll(t, opts = {}) {
  const o = { ...t, ...opts };
  return pointOut(
    await call('scroll', {
      ...target(o),
      direction: o.direction || 'down',
      amount: o.amount || 3,
      physical: !!o.physical,
      force: !!o.force,
    })
  );
}

async function scrollTo({ ref } = {}) {
  const r = await call('scrollTo', { ref });
  return { ok: r.ok, node: nodeShape(r.node) };
}

async function setValue({ ref, value } = {}) {
  const r = await call('setValue', { ref, value });
  if (r.node) r.node = nodeShape(r.node);
  return r;
}

async function invoke({ ref, action = 'invoke' } = {}) {
  return call('invoke', { ref, action });
}

async function restoreCursor() {
  return pointOut(await call('restoreCursor'));
}

/** Turns the physical-input watch on or off now, instead of on next call. */
async function watchUserInput(on = true) {
  watchOverride = !!on;
  await call(null);
  return { watching };
}

/** Test hook (spec B17): makes the helper emit one user-input event. */
async function simulateUserInput(kind = 'key') {
  return call('testUserInput', { kind });
}

async function ping() {
  return call('ping');
}

// ----------------------------------------------------------------- DIP

let monitorCache = null;

async function dipFallback(rect, toDip) {
  if (!monitorCache) monitorCache = await monitors();
  const primary = monitorCache.find((m) => m.primary) || monitorCache[0] || { scale: 1 };
  // Outside Electron there is no display layout to ask, so this uses the
  // primary monitor's scale for everything: exact on one monitor, close on
  // most layouts, not authoritative on mixed DPI.
  const s = toDip ? 1 / primary.scale : primary.scale;
  return {
    x: Math.round(rect.x * s),
    y: Math.round(rect.y * s),
    width: Math.round((rect.width || 0) * s),
    height: Math.round((rect.height || 0) * s),
  };
}

/** Physical rect or point to Electron DIP. */
async function toDip(r) {
  const rect = Array.isArray(r) ? rectObj(r) : { width: 0, height: 0, ...r };
  const scr = getScreen();
  if (scr) {
    if (r.width === undefined && !Array.isArray(r)) return scr.screenToDipPoint({ x: rect.x, y: rect.y });
    return scr.screenToDipRect(null, rect);
  }
  const out = await dipFallback(rect, true);
  return r.width === undefined && !Array.isArray(r) ? { x: out.x, y: out.y } : out;
}

/** Electron DIP rect or point to physical. */
async function toPhysical(r) {
  const rect = Array.isArray(r) ? rectObj(r) : { width: 0, height: 0, ...r };
  const scr = getScreen();
  if (scr) {
    if (r.width === undefined && !Array.isArray(r)) return scr.dipToScreenPoint({ x: rect.x, y: rect.y });
    return scr.dipToScreenRect(null, rect);
  }
  const out = await dipFallback(rect, false);
  return r.width === undefined && !Array.isArray(r) ? { x: out.x, y: out.y } : out;
}

module.exports = {
  // lifecycle
  start,
  stop,
  isReady,
  init,
  status,
  ping,
  events,
  // windows
  listWindows,
  foregroundWindow,
  focusWindow,
  closeWindow,
  monitors,
  // perception
  readTree,
  find,
  getText,
  ocr,
  screenshot,
  zoom,
  elementAt,
  resolve,
  // action
  click,
  hover,
  drag,
  type,
  key,
  scroll,
  scrollTo,
  setValue,
  invoke,
  restoreCursor,
  watchUserInput,
  simulateUserInput,
  // helpers
  toDip,
  toPhysical,
  handleOf,
  handleStr,
  renderTree,
  nodeLine,
  scoreNode,
  DesktopError,
  bootstrapLine,
};
