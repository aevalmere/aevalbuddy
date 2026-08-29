'use strict';

const {
  app,
  BrowserWindow,
  globalShortcut,
  ipcMain,
  screen,
  desktopCapturer,
  clipboard,
  shell,
  Menu,
} = require('electron');
const path = require('path');

// Automated checks point this at a scratch directory so they get their own
// settings store and single-instance lock, never colliding with a real
// session of the app. Must run before lib/store.js is required, because the
// store binds to the userData path at construction.
if (process.env.AEVAL_USERDATA) {
  app.setPath('userData', process.env.AEVAL_USERDATA);
}

const crash = require('./lib/crash');
crash.install();

const store = require('./lib/store');
const ai = require('./lib/openrouter');
const skills = require('./lib/skills');
const motion = require('./lib/motion');
const cursorBuddy = require('./lib/cursor-buddy');
const tray = require('./lib/tray');
const region = require('./lib/region');
const history = require('./lib/history');

const IS_WINDOWS = process.platform === 'win32';

// WDA_EXCLUDEFROMCAPTURE, which is what makes these windows invisible to a
// screen share, needs Windows 10 build 19041. Below that setContentProtection
// is a silent no-op, and a user who believes the overlay is hidden when it is
// not is worse off than one who was told. os.release() reports the NT version,
// so the third segment is the build.
const WIN_BUILD = IS_WINDOWS ? Number(require('os').release().split('.')[2]) || 0 : 0;
const CAN_HIDE_FROM_CAPTURE = !IS_WINDOWS || WIN_BUILD >= 19041;

// Every window of this app is a tool window: never in the taskbar, never in
// Alt-Tab, and pinned one level above the ordinary always-on-top band so a
// media player or another overlay cannot sit in front of it. skipTaskbar
// alone only asks the shell to drop the button; WS_EX_TOOLWINDOW, which is
// what type:'toolbar' sets on Windows, takes it out of the window list
// altogether. The relative level of 1 is the part that answers "it hides
// behind things".
function pinOnTop(win) {
  if (!win || win.isDestroyed()) return;
  win.setAlwaysOnTop(true, 'screen-saver', 1);
  win.setVisibleOnAllWorkspaces(true, { visibleOnFullScreen: true });
}

function hideFromCapture(win) {
  if (!win || win.isDestroyed() || !CAN_HIDE_FROM_CAPTURE) return;
  win.setContentProtection(true);
}

const PILL = { width: 104, height: 40 };
// Widths are user-set now; these are the shipping defaults and the bounds a
// drag is clamped to.
const PANEL_WIDTH = 420;
const INPUT_MIN_HEIGHT = 62;
const INPUT_MAX_HEIGHT = 280;
const DEFAULT_INPUT_HEIGHT = 76;
const OUTPUT_MIN_HEIGHT = 90;
// How tall the answer window is allowed to grow on its own. This was a flat
// 620, which is a third of a 1440p screen and most of a laptop one; it now
// follows the display, so a long answer uses the room that is actually there
// before it starts scrolling.
const OUTPUT_MAX_FRACTION = 0.66;
const OUTPUT_MAX_FLOOR = 380;
function outputMaxHeight() {
  return Math.max(OUTPUT_MAX_FLOOR, Math.round(workArea().height * OUTPUT_MAX_FRACTION));
}
const DEFAULT_OUTPUT_HEIGHT = 120;
// The layout ghosts take the real widths, so what is dragged is the size
// the window will actually be.
function hudInputSize() {
  return { width: store.getInputWidth(), height: 140 };
}

function hudOutputSize() {
  return { width: store.getOutputWidth(), height: 200 };
}
const DOCK_GAP = 8;
const SNAP_MARGIN = 16;
const SNAP_DISTANCE = 110;
const DOCK_DISTANCE = 150;
const DEFAULT_PROMPT = "What's on my screen and what should I do?";

const ACTION_LABELS = {
  captureQuery: 'Capture and ask',
  captureRegion: 'Ask about an area',
  toggleVisibility: 'Show or hide overlay',
};

// A drag emits 'move' continuously on Windows; snapping runs only after the
// window has been still this long, so the ghost never teleports mid-drag.
const DROP_SETTLE_MS = 160;
const CHAT_HISTORY_LIMIT = 12;
// The stored answer style when nothing is pinned: the router picks per
// request from the screen and the question.
const AUTO_SKILL = 'auto';

// Window transition lengths. Exits run at roughly half their entrance, per
// DESIGN.md; the table there is the source of these numbers.
const T_EXPAND = 260;
const T_COLLAPSE = 170;
const T_OUT_IN = 300;
const T_OUT_OUT = 150;
const T_OUT_GROW = 200;

let overlayWin = null; // the input window: pill and query bar
let outputWin = null; // the answer window
let settingsWin = null;
let overlayMode = 'pill'; // 'pill' | 'panel' | 'hud'
let overlayHidden = false;
let outputActive = false;
let currentInputHeight = DEFAULT_INPUT_HEIGHT;
let currentOutputHeight = DEFAULT_OUTPUT_HEIGHT;
let pendingShot = null;
// The area this conversation is about, in screen coordinates, or null for the
// whole screen. It outlives one message on purpose: having drawn a box around
// a stack trace, "and the line above it?" should look at the same box rather
// than silently widening to the desktop.
let activeRegion = null;
// The last question actually sent, so the error row can re-send that instead
// of whatever is (not) in the input box.
let lastQuestion = '';
let lastCaptureOk = true;
let currentAbort = null;
let queryGen = 0;
let queryBusy = false;
// One id per conversation, so history replaces the growing chat rather than
// appending a new row after every answer.
let chatId = newChatId();
let historyWin = null;

function newChatId() {
  return `c${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}
let captureModeActive = false;
let escapeRegistered = false;
let hudInputPending = null;
let hudOutputPending = null;
let hudSnapInProgress = false;
let inputDropTimer = null;
let outputDropTimer = null;
// The conversation, newest last: { role: 'user'|'assistant', text }. Held in
// memory only; New chat or quitting clears it.
let chat = [];
// Every action in ACTION_LABELS, so `settings:get` never reports an `active`
// map missing a key the panel draws a row for.
const lastRegisterResults = { captureQuery: false, captureRegion: false, toggleVisibility: false };

const WALKTHROUGH = [
  {
    win: 'overlay',
    targetSelector: '#settings-btn',
    clickScope: '#settings-btn, #notice-settings',
    message: 'Click here to add your API key',
    advanceOn: 'settings-opened',
  },
  {
    win: 'settings',
    targetSelector: '#api-key-input',
    clickScope: 'section[aria-labelledby="key-heading"]',
    message: 'Paste your OpenRouter API key, then press Save key',
    advanceOn: 'key-saved',
  },
];

// ---------------------------------------------------------------- positioning

// The display the overlay lives on.
//
// Capture and the pointer were moved onto the cursor's display in v0.6,
// because on a two-monitor desk reading the primary is a coin flip. Every
// window was left behind on getPrimaryDisplay(), so the app answered about the
// screen the user was working on and opened the panel on the other one -- the
// same defect, in the half that the user can actually see.
//
// Latched rather than read live. Reading the cursor's display on every bounds
// calculation would walk the panel to another monitor mid-answer because the
// user reached for their mouse, and every one of these functions runs during a
// glide. It is re-latched where the user summons the overlay, which is the
// moment their intent is known.
let latchedDisplayId = null;

function overlayDisplay() {
  if (latchedDisplayId !== null) {
    const held = screen.getAllDisplays().find((d) => d.id === latchedDisplayId);
    // Gone means unplugged: fall through rather than position against a
    // rectangle that is no longer on anybody's desk.
    if (held) return held;
  }
  return activeDisplay();
}

/**
 * Points the overlay at the screen the user is on.
 *
 * @returns {boolean} whether that changed the answer, so the caller can move
 *          the windows before animating them: a glide from a stale rect is a
 *          window flying across two monitors.
 */
function latchDisplay() {
  const before = latchedDisplayId;
  latchedDisplayId = activeDisplay().id;
  return before !== null && before !== latchedDisplayId;
}

function workArea() {
  return overlayDisplay().workArea;
}

function clampNum(n, min, max) {
  return Math.max(min, Math.min(max, n));
}

// Places a box from an anchor config: each axis hugs an edge (with offset)
// or centers. The pill, input bar, answer window, and both HUD ghosts all
// position through this, so everything shares the same spatial logic.
function boundsForSize(size, pos) {
  const wa = workArea();
  const p = pos;
  let x;
  if (p.h === 'left') x = wa.x + p.offX;
  else if (p.h === 'right') x = wa.x + wa.width - p.offX - size.width;
  else x = wa.x + Math.round((wa.width - size.width) / 2);
  let y;
  if (p.v === 'top') y = wa.y + p.offY;
  else if (p.v === 'bottom') y = wa.y + wa.height - p.offY - size.height;
  else y = wa.y + Math.round((wa.height - size.height) / 2);
  x = clampNum(x, wa.x, wa.x + wa.width - size.width);
  y = clampNum(y, wa.y, wa.y + wa.height - size.height);
  return { x, y, width: size.width, height: size.height };
}

function pillBounds() {
  return boundsForSize(PILL, store.getOverlayPos());
}

function inputBounds(height) {
  const h = clampNum(Math.round(height), INPUT_MIN_HEIGHT, INPUT_MAX_HEIGHT);
  return boundsForSize({ width: store.getInputWidth(), height: h }, store.getOverlayPos());
}

// Docked answers sit against the input bar, on whichever side has room:
// below a top-anchored input, above a bottom-anchored one.
function dockedBounds(inputRect, size, inputV) {
  const wa = workArea();
  const y =
    inputV === 'bottom'
      ? inputRect.y - DOCK_GAP - size.height
      : inputRect.y + inputRect.height + DOCK_GAP;
  return {
    x: clampNum(inputRect.x, wa.x, wa.x + wa.width - size.width),
    y: clampNum(y, wa.y, wa.y + wa.height - size.height),
    width: size.width,
    height: size.height,
  };
}

function outputBounds(height) {
  const wa = workArea();
  // A height the user dragged to wins over the transcript's own measurement.
  const manual = store.getOutputHeight();
  const h = manual
    ? clampNum(manual, OUTPUT_MIN_HEIGHT, wa.height - 40)
    : clampNum(Math.round(height), OUTPUT_MIN_HEIGHT, outputMaxHeight());
  const size = { width: store.getOutputWidth(), height: h };
  const op = store.getOutputPos();
  if (op.docked && overlayWin && !overlayWin.isDestroyed()) {
    return dockedBounds(overlayWin.getBounds(), size, store.getOverlayPos().v);
  }
  return boundsForSize(size, op);
}

// Derives an anchor config from a free-dropped rect: each axis hugs the
// nearer edge, keeping the drop position as the offset.
function posFromRect(rect) {
  const wa = workArea();
  const h = rect.x + rect.width / 2 < wa.x + wa.width / 2 ? 'left' : 'right';
  const v = rect.y + rect.height / 2 < wa.y + wa.height / 2 ? 'top' : 'bottom';
  return {
    h,
    v,
    offX: h === 'left' ? rect.x - wa.x : wa.x + wa.width - rect.x - rect.width,
    offY: v === 'top' ? rect.y - wa.y : wa.y + wa.height - rect.y - rect.height,
  };
}

// The nine game-HUD anchors: corners, edge midpoints, and center.
function snapAnchors() {
  const anchors = [];
  for (const h of ['left', 'center', 'right']) {
    for (const v of ['top', 'center', 'bottom']) {
      anchors.push({ h, v, offX: SNAP_MARGIN, offY: SNAP_MARGIN });
    }
  }
  return anchors;
}

function centerDistance(a, b) {
  return Math.hypot(
    a.x + a.width / 2 - (b.x + b.width / 2),
    a.y + a.height / 2 - (b.y + b.height / 2)
  );
}

function findSnap(rect, size) {
  let best = null;
  for (const pos of snapAnchors()) {
    const target = boundsForSize(size, pos);
    const dist = centerDistance(target, rect);
    if (dist <= SNAP_DISTANCE && (!best || dist < best.dist)) {
      best = { pos, rect: target, dist };
    }
  }
  return best;
}

// -------------------------------------------------------------------- windows

// Right-click menu with the editing commands a frameless window otherwise
// lacks: copy on selections, plus cut/paste inside text fields.
function attachContextMenu(win) {
  win.webContents.on('context-menu', (_e, params) => {
    const template = [];
    if (params.isEditable) {
      template.push(
        { role: 'cut' },
        { role: 'copy' },
        { role: 'paste' },
        { type: 'separator' },
        { role: 'selectAll' }
      );
    } else if (params.selectionText && params.selectionText.trim()) {
      template.push({ role: 'copy' }, { role: 'selectAll' });
    }
    if (template.length) {
      Menu.buildFromTemplate(template).popup({ window: win });
    }
  });
}

// Acrylic was tried here and reverted. Windows 11 does give a real blurred
// backdrop through backgroundMaterial, and in an isolated two-window test it
// looked markedly better than a transparent surface. In the actual app it
// broke the things that matter more: an opaque window stops behaving like a
// reliable topmost overlay, it needs the layered style added back the moment
// setOpacity touches it, and the churn showed up as artifacts on the
// desktop. Depth is done in CSS instead, and the windows stay transparent,
// which is the recipe that works.
function baseWindowOptions() {
  return {
    frame: false,
    transparent: true,
    hasShadow: false,
    // Draggable edges. The height of the input bar stays pinned to its
    // content by giving it an equal minimum and maximum on that axis, so a
    // drag can only change the width, which is the only axis where a
    // one-line field has anything to gain.
    resizable: true,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    show: false,
    ...(IS_WINDOWS ? { type: 'toolbar' } : {}),
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  };
}

function createOverlay() {
  overlayWin = new BrowserWindow({ ...pillBounds(), ...baseWindowOptions(), focusable: false });
  pinOnTop(overlayWin);
  // Core Cluely-defining feature: exclude the overlay from screen capture.
  // On Windows this maps to WDA_EXCLUDEFROMCAPTURE and is reliable on
  // Windows 10 2004+. Some capture backends (older OS builds, some hardware
  // capture cards) may still see the window; the app keeps working either
  // way, the overlay is just visible in those recordings.
  hideFromCapture(overlayWin);
  // Click-through while idle; forward:true keeps mousemove events flowing so
  // the renderer can re-enable hit-testing when the cursor is over the pill.
  overlayWin.setIgnoreMouseEvents(true, { forward: true });
  overlayWin.loadFile(path.join(__dirname, 'renderer', 'index.html'));
  overlayWin.once('ready-to-show', () => {
    // showInactive keeps focus in whatever app the user was using.
    overlayWin.showInactive();
  });
  overlayWin.webContents.on('did-finish-load', () => {
    sendOverlayState();
    // Window motion lives in the main process, where no media query exists;
    // the overlay renderer answers for it once, at boot.
    overlayWin.webContents
      .executeJavaScript('matchMedia("(prefers-reduced-motion: reduce)").matches')
      .then((value) => motion.setReducedMotion(value))
      .catch(() => {});
  });
  attachContextMenu(overlayWin);
  overlayWin.on('move', onInputMove);
  overlayWin.on('resize', onInputResize);
  overlayWin.on('closed', () => {
    overlayWin = null;
  });
  applyInputLimits();
}

function createOutput() {
  outputWin = new BrowserWindow({
    ...boundsForSize({ width: PANEL_WIDTH, height: DEFAULT_OUTPUT_HEIGHT }, store.getOverlayPos()),
    ...baseWindowOptions(),
    focusable: true,
  });
  pinOnTop(outputWin);
  hideFromCapture(outputWin);
  outputWin.loadFile(path.join(__dirname, 'renderer', 'output.html'));
  outputWin.webContents.on('did-finish-load', () => {
    sendOverlayState();
    // A reloaded answer window (dev watcher) rebuilds its transcript.
    if (chat.length && outputWin && !outputWin.isDestroyed()) {
      outputWin.webContents.send('chat:sync', { messages: chat });
    }
  });
  attachContextMenu(outputWin);
  outputWin.on('move', onOutputMove);
  outputWin.on('resize', onOutputResize);
  outputWin.on('closed', () => {
    outputWin = null;
  });
  outputLimits();
}

function openSettings() {
  if (settingsWin && !settingsWin.isDestroyed()) {
    settingsWin.show();
    settingsWin.focus();
    cursorBuddy.notify('settings-opened');
    return;
  }
  // Tall enough to show a whole step without scrolling, but never taller
  // than the work area; on a short screen the panel scrolls instead of
  // hanging off the bottom edge.
  const wa = workArea();
  const saved = store.getSettingsSize();
  settingsWin = new BrowserWindow({
    width: saved.width,
    height: clampNum(saved.height, 360, wa.height - 40),
    minWidth: 380,
    minHeight: 360,
    maxWidth: 900,
    frame: false,
    transparent: true,
    resizable: true,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    // Settings was the one window still advertising the app: it kept a
    // taskbar button and sat in the normal z-order, so opening it put a
    // labelled entry on the bar and let other windows cover it.
    skipTaskbar: true,
    alwaysOnTop: true,
    ...(IS_WINDOWS ? { type: 'toolbar' } : {}),
    show: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  // The key-entry screen stays out of screen shares as well.
  hideFromCapture(settingsWin);
  pinOnTop(settingsWin);
  attachContextMenu(settingsWin);
  settingsWin.loadFile(path.join(__dirname, 'settings', 'settings.html'));
  settingsWin.once('ready-to-show', () => {
    settingsWin.show();
    cursorBuddy.notify('settings-opened');
  });
  // The size survives closing, so the panel opens where it was left.
  let settingsResizeTimer = null;
  settingsWin.on('resize', () => {
    clearTimeout(settingsResizeTimer);
    settingsResizeTimer = setTimeout(() => {
      if (settingsWin && !settingsWin.isDestroyed()) {
        store.setSettingsSize(settingsWin.getBounds());
      }
    }, DROP_SETTLE_MS);
  });
  settingsWin.on('closed', () => {
    settingsWin = null;
    captureModeActive = false;
    applyRegistrations();
    cursorBuddy.notify('settings-closed');
  });
}

// ------------------------------------------------------------ expand/collapse

// The mode reaches the renderer before the window starts growing, so the
// panel is already painted inside the pill-sized frame and the growth reveals
// it. Doing it the other way round is what produced the opening flash: a
// full-size window showing pill content for one frame.
function expandPanel() {
  if (!overlayWin || overlayMode === 'hud') return;
  const wasPill = overlayMode === 'pill';
  const from = overlayWin.getBounds();
  overlayMode = 'panel';
  overlayHidden = false;
  overlayWin.setIgnoreMouseEvents(false);
  overlayWin.setFocusable(true);
  // setFocusable can drop the z-order flag on Windows; reassert it.
  pinOnTop(overlayWin);
  sendOverlayState();
  const to = inputBounds(currentInputHeight);
  if (wasPill) {
    // The renderer measures itself while the glide is running, so its height
    // report is applied once the glide lands rather than fighting it.
    markApp(overlayWin);
    unlockInput();
    motion.animate(overlayWin, { from, to, duration: T_EXPAND, onDone: syncInputBounds });
  } else {
    motion.stop(overlayWin);
    setInputBounds(to);
  }
  overlayWin.show();
  overlayWin.focus();
  applyRegistrations();
}

function collapsePanel() {
  if (!overlayWin) return;
  abortCurrentQuery();
  hideOutput();
  const wasPanel = overlayMode === 'panel';
  overlayMode = 'pill';
  currentInputHeight = DEFAULT_INPUT_HEIGHT;
  overlayWin.blur();
  overlayWin.setFocusable(false);
  pinOnTop(overlayWin);
  overlayWin.setIgnoreMouseEvents(true, { forward: true });
  overlayWin.showInactive();
  const to = pillBounds();
  if (wasPanel) {
    // The renderer keeps showing the panel until the frame is nearly pill
    // sized, so the contents shrink away instead of vanishing at frame one.
    markApp(overlayWin);
    unlockInput();
    motion.animate(overlayWin, {
      to,
      duration: T_COLLAPSE,
      onDone: () => {
        applyInputLimits();
        markApp(overlayWin);
        sendOverlayState();
      },
    });
  } else {
    motion.stop(overlayWin);
    setInputBounds(to);
    sendOverlayState();
  }
  applyRegistrations();
}

// Arrives folded against the edge it grows away from, at zero opacity, so
// there is no first frame of an unpainted window.
function showOutput() {
  if (!outputWin || outputWin.isDestroyed()) return;
  const reopening = !outputActive;
  outputActive = true;
  pinOnTop(outputWin);
  const to = outputBounds(currentOutputHeight);
  if (!reopening) {
    resizeOutput(to);
    return;
  }
  const op = store.getOutputPos();
  const anchorV = op.docked ? store.getOverlayPos().v : op.v;
  const anchorH = op.docked ? store.getOverlayPos().h : op.h;
  markApp(outputWin);
  outputWin.setBounds(motion.shrunkFrom(to, anchorV, anchorH));
  outputWin.showInactive();
  // The fade is the renderer's entrance animation, not window opacity.
  motion.animate(outputWin, { to, duration: T_OUT_IN, onDone: syncOutputBounds });
}

// Applies a height the renderer reported while a glide was in flight.
function keepOnTop() {
  for (const win of [overlayWin, outputWin, settingsWin]) {
    if (win && !win.isDestroyed() && win.isVisible()) pinOnTop(win);
  }
}

function syncInputBounds() {
  keepOnTop();
  if (overlayMode !== 'panel' || !overlayWin || overlayWin.isDestroyed()) return;
  const want = inputBounds(currentInputHeight);
  const have = overlayWin.getBounds();
  if (have.height !== want.height || have.y !== want.y) setInputBounds(want);
  else applyInputLimits();
  markApp(overlayWin);
  repositionOutputIfDocked();
}

function syncOutputBounds() {
  keepOnTop();
  if (!outputActive || !outputWin || outputWin.isDestroyed()) return;
  const want = outputBounds(currentOutputHeight);
  const have = outputWin.getBounds();
  markApp(outputWin);
  if (have.height !== want.height || have.y !== want.y) outputWin.setBounds(want);
}

// The grown height survives hide so a conversation reopens at its size;
// only New chat resets it.
function hideOutput() {
  if (!outputActive) {
    if (outputWin && !outputWin.isDestroyed()) outputWin.hide();
    return;
  }
  outputActive = false;
  if (!outputWin || outputWin.isDestroyed()) return;
  // Hiding is immediate. An exit animation here has to survive being taken
  // over half way through, and the failure mode is a window left invisible
  // with no way back, which is worse than no exit animation at all.
  motion.stop(outputWin);
  outputWin.hide();
}

// Growing during a conversation glides too, so a long answer does not make
// the window edge jump.
function resizeOutput(to) {
  if (!outputWin || outputWin.isDestroyed()) return;
  markApp(outputWin);
  outputLimits();
  // A height reported while this glide is in flight is refused by the
  // resize handler and only remembered, so the glide has to re-apply what
  // is remembered when it lands. Without this the window kept whatever
  // height it was already growing towards and every later report was
  // dropped: the answer went on getting longer inside a window that had
  // stopped growing, which is what a transcript that will not scroll looks
  // like from the outside.
  motion.animate(outputWin, { to, duration: T_OUT_GROW, onDone: syncOutputBounds });
}

function repositionOutputIfDocked() {
  if (!outputActive || !store.getOutputPos().docked) return;
  if (!outputWin || outputWin.isDestroyed()) return;
  // An answer window that is still gliding open must not be cut short here:
  // its own landing calls syncOutputBounds, which recomputes the docked
  // position against wherever the input bar ended up.
  if (motion.isAnimating(outputWin)) return;
  // Otherwise following the input bar is immediate; a glide would lag.
  markApp(outputWin);
  outputWin.setBounds(outputBounds(currentOutputHeight));
}

function sendOverlayState() {
  // Anchors ride along so each renderer can grow its surface out of the
  // correct corner (transform-origin follows the anchored edge).
  const payload = {
    mode: overlayMode,
    hasKey: store.hasApiKey(),
    captureOk: lastCaptureOk,
    inputAnchor: store.getOverlayPos(),
    outputPos: store.getOutputPos(),
    pillState: queryBusy ? 'busy' : chat.length ? 'chat' : 'idle',
    inputWidth: store.getInputWidth(),
    outputWidth: store.getOutputWidth(),
    // A dragged height means the transcript fills the window and scrolls
    // inside it, instead of the window following the transcript.
    outputFixedHeight: store.getOutputHeight() > 0,
    // The transcript's own ceiling. It has to agree with the window's, or the
    // window stops growing while the transcript still has room, and the
    // bottom of the answer is simply cut off.
    chatMax: outputMaxHeight() - 56,
    // The panel shows a chip for an active area so the constraint is never
    // invisible: a follow-up that only sees a corner of the screen has to say
    // so before the answer does.
    region: activeRegion
      ? { width: activeRegion.width, height: activeRegion.height }
      : null,
    busy: queryBusy,
  };
  for (const win of [overlayWin, outputWin]) {
    if (win && !win.isDestroyed()) {
      win.webContents.send('overlay:state', payload);
    }
  }
}

// ------------------------------------------------------------------ HUD edit

function enterHudEditor() {
  if (!overlayWin || !outputWin || overlayMode === 'hud') return;
  unlockInput();
  if (cursorBuddy.isActive()) cursorBuddy.cancel();
  abortCurrentQuery();
  // Layout editing is a direct-manipulation mode; no glide may be left
  // running to move a ghost the user is dragging.
  motion.stop(overlayWin);
  motion.stop(outputWin);
  overlayMode = 'hud';
  overlayHidden = false;
  outputActive = false;
  hudInputPending = store.getOverlayPos();
  hudOutputPending = store.getOutputPos();

  overlayWin.setIgnoreMouseEvents(false);
  overlayWin.setFocusable(true);
  overlayWin.setMovable(true);
  unlockInput();
  overlayWin.setBounds(boundsForSize(hudInputSize(), hudInputPending));
  lockSize(overlayWin, hudInputSize().width, hudInputSize().height);
  pinOnTop(overlayWin);
  overlayWin.show();
  overlayWin.focus();

  outputWin.setMovable(true);
  unlockSize(outputWin);
  outputWin.setBounds(hudOutputGhostBounds());
  lockSize(outputWin, hudOutputSize().width, hudOutputSize().height);
  pinOnTop(outputWin);
  outputWin.showInactive();

  applyRegistrations();
  sendOverlayState();
  outputWin.webContents.send('hud:docked', { docked: hudOutputPending.docked });
}

function hudOutputGhostBounds() {
  if (hudOutputPending.docked && overlayWin && !overlayWin.isDestroyed()) {
    return dockedBounds(overlayWin.getBounds(), hudOutputSize(), hudInputPending.v);
  }
  return boundsForSize(hudOutputSize(), hudOutputPending);
}

function exitHudEditor() {
  if (!overlayWin || overlayMode !== 'hud') return;
  store.setOverlayPos(hudInputPending || posFromRect(overlayWin.getBounds()));
  store.setOutputPos(hudOutputPending || store.getOutputPos());
  hudInputPending = null;
  hudOutputPending = null;
  overlayWin.setMovable(false);
  applyInputLimits();
  if (outputWin && !outputWin.isDestroyed()) {
    outputWin.setMovable(false);
    outputLimits();
    outputWin.hide();
  }
  collapsePanel();
  if (settingsWin && !settingsWin.isDestroyed()) settingsWin.focus();
}

function withSnapGuard(fn) {
  hudSnapInProgress = true;
  fn();
  setImmediate(() => {
    hudSnapInProgress = false;
  });
}

// Windows emits 'move' continuously while a window is being dragged, so
// running snap logic per event yanks the ghost out of the user's hand (that
// bug made a docked answer ghost impossible to pull free: every event
// re-docked it). Instead each event just resets a settle timer, and the
// snap decision runs once, after the drag stops.

// The input bar may only be resized sideways: its height belongs to its
// content. That is enforced with matching minimum and maximum heights, which
// means the app has to unlock them before changing the height itself,
// because setBounds is clamped by the same limits a drag is.
// Resizability is expressed entirely through size limits, never through
// setResizable. On a frameless window, clearing the resizable flag strips the
// thick-frame style and setting it back does not restore it, so a window
// switched off once stays unresizable for the rest of the session. Equal
// minimum and maximum sizes pin a window just as firmly and are reversible.
function lockSize(win, w, h) {
  if (!win || win.isDestroyed()) return;
  win.setMinimumSize(w, h);
  win.setMaximumSize(w, h);
}

function unlockSize(win) {
  if (!win || win.isDestroyed()) return;
  win.setMinimumSize(1, 1);
  win.setMaximumSize(4000, 4000);
}

function outputLimits() {
  if (!outputWin || outputWin.isDestroyed()) return;
  outputWin.setMinimumSize(store.MIN_W, OUTPUT_MIN_HEIGHT);
  outputWin.setMaximumSize(store.MAX_W, Math.max(240, workArea().height));
}

function applyInputLimits() {
  if (!overlayWin || overlayWin.isDestroyed()) return;
  const b = overlayWin.getBounds();
  if (overlayMode !== 'panel') {
    // The pill is far narrower than the panel minimum, and the layout ghost
    // is dragged rather than resized. Leaving the panel limits in place here
    // makes Windows widen the pill to satisfy them, and the app then reads
    // that back as a width the user chose.
    lockSize(overlayWin, b.width, b.height);
    return;
  }
  // Width is the user's, height belongs to the content.
  overlayWin.setMinimumSize(store.MIN_W, b.height);
  overlayWin.setMaximumSize(store.MAX_W, b.height);
}

function unlockInput() {
  unlockSize(overlayWin);
}

// Telling an app-driven bounds change apart from a drag. Without this the
// collapse animation would run the window down to pill width and the last
// frame would be stored as a chosen width.
const appBoundsAt = new Map();

function markApp(win) {
  if (win && !win.isDestroyed()) appBoundsAt.set(win.id, Date.now());
}

function appDriven(win) {
  if (!win || win.isDestroyed()) return true;
  if (motion.isAnimating(win)) return true;
  return Date.now() - (appBoundsAt.get(win.id) || 0) < 500;
}

// Every programmatic change to the input bar goes through here: unlock the
// height axis, move, re-lock, and mark it as the app moving itself.
function setInputBounds(rect) {
  if (!overlayWin || overlayWin.isDestroyed()) return;
  markApp(overlayWin);
  unlockInput();
  overlayWin.setBounds(rect);
  applyInputLimits();
}

let inputResizeTimer = null;
let outputResizeTimer = null;

function onInputResize() {
  // Only the open panel is user-resizable; the pill and the layout ghost
  // change size for reasons that are never a drag.
  if (overlayMode !== 'panel' || !overlayWin) return;
  clearTimeout(inputResizeTimer);
  inputResizeTimer = setTimeout(() => {
    if (!overlayWin || overlayWin.isDestroyed() || appDriven(overlayWin)) return;
    store.setInputWidth(overlayWin.getBounds().width);
    applyInputLimits();
    sendOverlayState();
    repositionOutputIfDocked();
  }, DROP_SETTLE_MS);
}

function onOutputResize() {
  if (overlayMode === 'hud' || !outputWin || !outputActive) return;
  clearTimeout(outputResizeTimer);
  outputResizeTimer = setTimeout(() => {
    if (!outputWin || outputWin.isDestroyed() || appDriven(outputWin)) return;
    const b = outputWin.getBounds();
    store.setOutputWidth(b.width);
    // Dragging the height is how the answer window stops sizing itself to
    // the transcript; New chat hands that back.
    store.setOutputHeight(b.height);
    currentOutputHeight = b.height;
    sendOverlayState();
  }, DROP_SETTLE_MS);
}

function onInputMove() {
  if (overlayMode !== 'hud' || hudSnapInProgress || !overlayWin) return;
  // A docked answer ghost follows the drag live.
  if (hudOutputPending && hudOutputPending.docked && outputWin && !outputWin.isDestroyed()) {
    withSnapGuard(() => outputWin.setBounds(hudOutputGhostBounds()));
  }
  clearTimeout(inputDropTimer);
  inputDropTimer = setTimeout(settleInputDrag, DROP_SETTLE_MS);
}

function settleInputDrag() {
  if (overlayMode !== 'hud' || !overlayWin || overlayWin.isDestroyed()) return;
  const rect = overlayWin.getBounds();
  const snap = findSnap(rect, hudInputSize());
  if (snap) {
    withSnapGuard(() => overlayWin.setBounds(snap.rect));
    hudInputPending = snap.pos;
    overlayWin.webContents.send('hud:snapped', {});
  } else {
    hudInputPending = posFromRect(rect);
  }
  if (hudOutputPending && hudOutputPending.docked && outputWin && !outputWin.isDestroyed()) {
    withSnapGuard(() => outputWin.setBounds(hudOutputGhostBounds()));
  }
}

function onOutputMove() {
  if (overlayMode !== 'hud' || hudSnapInProgress || !outputWin) return;
  clearTimeout(outputDropTimer);
  outputDropTimer = setTimeout(settleOutputDrag, DROP_SETTLE_MS);
}

// Docking to the input bar wins over anchor snapping; otherwise the same
// nine anchors apply. Dragging clear of the dock zone detaches.
function settleOutputDrag() {
  if (overlayMode !== 'hud' || !outputWin || outputWin.isDestroyed() || !overlayWin) return;
  const rect = outputWin.getBounds();
  const dockSlot = dockedBounds(overlayWin.getBounds(), hudOutputSize(), hudInputPending.v);
  if (centerDistance(dockSlot, rect) <= DOCK_DISTANCE) {
    withSnapGuard(() => outputWin.setBounds(dockSlot));
    hudOutputPending = { ...hudOutputPending, docked: true };
    outputWin.webContents.send('hud:docked', { docked: true });
    outputWin.webContents.send('hud:snapped', {});
    return;
  }
  const snap = findSnap(rect, hudOutputSize());
  if (snap) {
    withSnapGuard(() => outputWin.setBounds(snap.rect));
    hudOutputPending = { ...snap.pos, docked: false };
    outputWin.webContents.send('hud:snapped', {});
  } else {
    hudOutputPending = { ...posFromRect(rect), docked: false };
  }
  outputWin.webContents.send('hud:docked', { docked: false });
}

// The dock state is also a button, so separating the windows never depends
// on drag precision.
function setHudDock(docked) {
  if (overlayMode !== 'hud' || !outputWin || outputWin.isDestroyed() || !overlayWin) return;
  if (docked) {
    hudOutputPending = { ...hudOutputPending, docked: true };
    withSnapGuard(() => outputWin.setBounds(hudOutputGhostBounds()));
  } else {
    // Nudge the ghost clearly out of the dock zone in the direction it
    // already grows, so the split is visible immediately.
    const rect = outputWin.getBounds();
    const dir = hudInputPending && hudInputPending.v === 'bottom' ? -1 : 1;
    const freed = posFromRect({ ...rect, y: rect.y + dir * (DOCK_DISTANCE + 20) });
    hudOutputPending = { ...freed, docked: false };
    withSnapGuard(() => outputWin.setBounds(boundsForSize(hudOutputSize(), hudOutputPending)));
  }
  outputWin.webContents.send('hud:docked', { docked: hudOutputPending.docked });
  outputWin.webContents.send('hud:snapped', {});
}

// ------------------------------------------------------------- screen capture

// How much detail the reader gets. The old numbers were 1280 across at JPEG
// quality 70, stepping down to 35 to stay under a megabyte: on a 2560-wide
// display that is a halving, and quality 35 turns 12px UI text into grey
// mush. Everything downstream is reading that image, so every wrong answer
// about a variable name, an error string or a button label started here. The
// long edge now goes to 1600 at quality 88, and the floor is high enough that
// text stays legible even when the budget bites.
const SHOT_LONG_EDGE = 1600;
const SHOT_QUALITY = 88;
const SHOT_QUALITY_FLOOR = 62;
const SHOT_BUDGET = 1600 * 1024;

// The screen the user is working on, which is the one their pointer is on.
// This used to be the primary display unconditionally, and on a two-monitor
// desk that is a coin flip: the app read one screen, answered about it, and
// pointed at it, while the user was looking at the other one.
function activeDisplay() {
  try {
    return screen.getDisplayNearestPoint(screen.getCursorScreenPoint());
  } catch (err) {
    // getCursorScreenPoint can fail on a locked or switching session.
    return screen.getPrimaryDisplay();
  }
}

/**
 * A JPEG of the screen, or of one area of it, plus what the pointer needs to
 * map an answer's coordinates back onto the display.
 *
 * The whole screen is downscaled to the long-edge budget, which on a 2560x1440
 * desktop is 0.62x and turns 12px interface text into about seven pixels. An
 * area is captured at the display's native resolution and cropped instead, so
 * the same budget buys a 600x300 selection at 1:1. That difference is the
 * difference between a model reading a variable name and guessing at it.
 *
 * @param {object} [opts]
 * @param {{x, y, width, height}} [opts.region] screen coordinates
 */
async function captureScreen(opts) {
  const want = (opts && opts.region) || null;
  try {
    const target = want ? screen.getDisplayMatching(want) : activeDisplay();
    const { width, height } = target.size;
    // Native pixels for an area (so the crop loses nothing), long-edge budget
    // for the whole screen (so a 4K desktop is not decoded at full size for
    // an image that is about to be shrunk anyway).
    const scale = want
      ? target.scaleFactor || 1
      : Math.min(1, SHOT_LONG_EDGE / Math.max(width, height));
    const sources = await desktopCapturer.getSources({
      types: ['screen'],
      thumbnailSize: {
        width: Math.round(width * scale),
        height: Math.round(height * scale),
      },
    });
    if (!sources.length) return null;
    // Only an id match is trustworthy on a multi-monitor desk. Without one,
    // the primary is a better guess than source order, which follows no rule
    // the app can rely on.
    const source =
      sources.find((s) => s.display_id === String(target.id)) ||
      sources.find((s) => s.display_id === String(screen.getPrimaryDisplay().id)) ||
      sources[0];
    if (source.thumbnail.isEmpty()) return null;

    let image = source.thumbnail;
    let shotRegion = null;
    if (want) {
      // The thumbnail does not always come back at exactly the size asked
      // for, so the crop is scaled by what actually arrived rather than by
      // what was requested.
      const got = image.getSize();
      const kx = got.width / width;
      const ky = got.height / height;
      const x = clampNum(Math.round((want.x - target.bounds.x) * kx), 0, Math.max(0, got.width - 1));
      const y = clampNum(Math.round((want.y - target.bounds.y) * ky), 0, Math.max(0, got.height - 1));
      image = image.crop({
        x,
        y,
        width: clampNum(Math.round(want.width * kx), 1, got.width - x),
        height: clampNum(Math.round(want.height * ky), 1, got.height - y),
      });
      // Only a very large selection needs shrinking at all, which is the
      // whole point of choosing one.
      const cs = image.getSize();
      const long = Math.max(cs.width, cs.height);
      if (long > SHOT_LONG_EDGE) {
        const k = SHOT_LONG_EDGE / long;
        image = image.resize({
          width: Math.round(cs.width * k),
          height: Math.round(cs.height * k),
          quality: 'best',
        });
      }
      shotRegion = { ...want };
    }
    if (image.isEmpty()) return null;

    // Step quality down only as far as still-readable text allows. A slower
    // upload beats an unreadable screen.
    let quality = SHOT_QUALITY;
    let buf = image.toJPEG(quality);
    while (buf.length > SHOT_BUDGET && quality > SHOT_QUALITY_FLOOR) {
      quality -= 8;
      buf = image.toJPEG(quality);
    }
    // The image size, and the rectangle it covers on screen, let point
    // directives map back from the view to screen coordinates. For an area
    // that rectangle is the selection, not the display.
    const size = image.getSize();
    return {
      base64: buf.toString('base64'),
      bytes: buf.length,
      width: size.width,
      height: size.height,
      display: { ...target.bounds },
      region: shotRegion,
    };
  } catch (err) {
    // Capture can fail on locked desktops or exotic display drivers; the
    // query flow degrades to text-only instead of crashing.
    return null;
  }
}

// A latched display change leaves every window sitting on the old screen.
// Move them before anything animates: a glide interpolates rectangles, so
// growing the panel out of a pill on another monitor is a window flying across
// the desk rather than a surface opening.
function rehomeWindows() {
  if (overlayWin && !overlayWin.isDestroyed()) {
    motion.stop(overlayWin);
    setInputBounds(overlayMode === 'panel' ? inputBounds(currentInputHeight) : pillBounds());
  }
  if (outputActive && outputWin && !outputWin.isDestroyed()) {
    motion.stop(outputWin);
    markApp(outputWin);
    outputWin.setBounds(outputBounds(currentOutputHeight));
  }
}

async function triggerCapture() {
  if (!overlayWin || overlayMode === 'hud') return;
  overlayHidden = false;
  if (latchDisplay()) rehomeWindows();
  pendingShot = await captureScreen({ region: activeRegion });
  lastCaptureOk = !!pendingShot;
  expandPanel();
  // An existing conversation comes back with the panel.
  if (chat.length) showOutput();
}

/**
 * Draw a box, then ask about what is inside it.
 *
 * The selection sticks to the conversation rather than to the one message, so
 * follow-ups keep looking at the same rectangle. Clearing it is one click on
 * the chip in the panel, and New chat drops it too.
 */
async function triggerRegionCapture() {
  if (!overlayWin || overlayMode === 'hud' || region.isActive()) return;
  if (latchDisplay()) rehomeWindows();
  const pick = region.select(overlayDisplay());
  // Claim Escape for the selector now that one is open, and hand it back
  // however the selection ends.
  applyRegistrations();
  const rect = await pick;
  applyRegistrations();
  if (!rect) {
    // Cancelled. If the panel was already open it stays exactly as it was.
    sendOverlayState();
    return;
  }
  activeRegion = rect;
  overlayHidden = false;
  pendingShot = await captureScreen({ region: rect });
  lastCaptureOk = !!pendingShot;
  expandPanel();
  if (chat.length) showOutput();
  sendOverlayState();
}

// Back to the whole screen. The next message re-captures, so nothing has to be
// re-asked to make it take effect.
function clearRegion() {
  if (!activeRegion) return;
  activeRegion = null;
  sendOverlayState();
}

// ------------------------------------------------------------------- hotkeys

function onCaptureHotkey() {
  triggerCapture();
}

function onRegionHotkey() {
  triggerRegionCapture();
}

function onToggleHotkey() {
  if (!overlayWin || overlayMode === 'hud') return;
  overlayHidden = !overlayHidden;
  if (overlayHidden) {
    if (cursorBuddy.isActive()) cursorBuddy.cancel();
    motion.stop(overlayWin);
    motion.stop(outputWin);
    overlayWin.hide();
    if (outputWin && !outputWin.isDestroyed()) outputWin.hide();
  } else {
    // Coming back from hidden is a summon like any other, so it lands on the
    // screen the user is on now rather than the one they were on when they
    // hid it.
    if (latchDisplay()) rehomeWindows();
    if (overlayMode === 'panel') {
      overlayWin.show();
      overlayWin.focus();
    } else {
      overlayWin.showInactive();
    }
    if (outputActive && outputWin && !outputWin.isDestroyed()) {
      outputWin.showInactive();
    }
  }
  // Hidden is the state the tray exists to reverse, so its label has to say
  // which way round things currently are.
  tray.refresh({ hidden: overlayHidden });
  applyRegistrations();
}

function onEscapePressed() {
  // First claim, above everything: choosing an area is a modal gesture with a
  // full-screen scrim over the desktop.
  //
  // The selector has its own Escape handler, and while the panel is open that
  // handler never runs -- Escape is a global shortcut then, and a global
  // shortcut is consumed before any window sees the key. So pressing Escape
  // during a selection collapsed the panel and left the scrim covering the
  // screen, which is the worst of both: the thing the user wanted dismissed
  // stayed, and the thing they were using went away.
  if (region.isActive()) {
    region.cancel();
    return;
  }
  if (cursorBuddy.isActive()) {
    cursorBuddy.cancel();
    return;
  }
  if (overlayMode === 'hud') {
    exitHudEditor();
    return;
  }
  if (settingsWin && !settingsWin.isDestroyed() && settingsWin.isFocused()) {
    settingsWin.close();
    return;
  }
  // Same reasoning as settings: a focused window closes itself before Escape
  // means anything to the overlay behind it.
  if (historyWin && !historyWin.isDestroyed() && historyWin.isFocused()) {
    historyWin.close();
    return;
  }
  if (overlayMode === 'panel') {
    collapsePanel();
  }
}

// Re-derives every global shortcut from current state. Escape is registered
// globally only while the panel, the HUD editor, or the walkthrough is open,
// so the app never swallows Escape system-wide while idle. Everything is
// suspended while the settings panel is recording a new combo.
function applyRegistrations() {
  globalShortcut.unregisterAll();
  escapeRegistered = false;
  if (captureModeActive) return;
  const shortcuts = store.getShortcuts();
  lastRegisterResults.captureQuery = globalShortcut.register(
    shortcuts.captureQuery,
    onCaptureHotkey
  );
  lastRegisterResults.captureRegion = globalShortcut.register(
    shortcuts.captureRegion,
    onRegionHotkey
  );
  lastRegisterResults.toggleVisibility = globalShortcut.register(
    shortcuts.toggleVisibility,
    onToggleHotkey
  );
  // The tray menu prints the current combos beside each item.
  tray.refresh({ hidden: overlayHidden });
  if (overlayMode !== 'pill' || cursorBuddy.isActive() || region.isActive()) {
    escapeRegistered = globalShortcut.register('Escape', onEscapePressed);
  }
}

const MODIFIER_RE = /^(Ctrl|Control|CommandOrControl|Alt|AltGr|Super|Meta)\+/;

function validAccelerator(accelerator) {
  if (typeof accelerator !== 'string' || !accelerator.trim()) return false;
  if (!MODIFIER_RE.test(accelerator)) return false;
  const parts = accelerator.split('+');
  const key = parts[parts.length - 1];
  return key.length > 0 && !/^(Ctrl|Control|CommandOrControl|Alt|AltGr|Shift|Super|Meta)$/.test(key);
}

// --------------------------------------------------------------- query flow

// How many models one request may burn through before giving up. Two
// stand-ins is enough to ride out the usual free-tier queue and few enough
// that a genuine outage still surfaces quickly.
const MAX_MODEL_ATTEMPTS = 3;

// "google/gemma-4-26b-a4b-it:free" reads as "gemma-4-26b-a4b-it" in a status
// line the user glances at for half a second.
function shortModel(id) {
  const name = String(id || '').split('/').pop().replace(/:free$/, '');
  return name.length > 28 ? `${name.slice(0, 27)}\u2026` : name;
}

/**
 * Runs a model call, and moves to the next free model of the same kind when
 * the failure is one a different model could answer.
 *
 * Free models are queued and rate limited by design: a 429 is an ordinary
 * Tuesday, not an exception. Showing the user an error and asking them to go
 * and pick a different model in Settings is making them do the app's job, and
 * it is the difference between the app feeling broken and feeling reliable.
 *
 * Paid models never get a stand-in. Quietly moving a billed request onto a
 * different billed model is not the app's decision to make.
 *
 * @param {object} o
 * @param {'vision'|'text'} o.kind
 * @param {string} o.model the first choice
 * @param {(model: string) => Promise<any>} o.run
 * @param {() => boolean} [o.canRetry] false once bytes have reached the user
 * @param {(model: string) => void} [o.onSwitch]
 */
async function withFailover(o) {
  const tried = [];
  let attempt = o.model;
  let lastErr = null;
  // How many models this request actually burned, stamped on whatever error
  // escapes. The user-facing copy used to assert that a stand-in had been
  // tried whether or not one ever was; now the sentence follows the count.
  const stamp = (err) => {
    if (err && typeof err === 'object') err.attempts = tried.length;
    return err;
  };
  while (attempt) {
    tried.push(attempt);
    if (tried.length > 1 && o.onSwitch) o.onSwitch(attempt);
    try {
      return { model: attempt, value: await o.run(attempt) };
    } catch (err) {
      lastErr = err;
      if (err.name === 'AbortError') throw err;
      // Half an answer on screen cannot be replaced by another model's half.
      if (o.canRetry && !o.canRetry()) throw stamp(err);
      if (!ai.shouldFailover(err) || !/:free$/.test(attempt)) throw stamp(err);
      attempt = tried.length >= MAX_MODEL_ATTEMPTS ? null : ai.nextFreeModel(o.kind, tried);
    }
  }
  throw stamp(lastErr);
}

function abortCurrentQuery() {
  if (currentAbort) {
    currentAbort.abort();
    currentAbort = null;
  }
}

/**
 * One conversation turn.
 *
 * @param {string} question
 * @param {object} [opts]
 * @param {boolean} [opts.retry] re-run the turn already at the end of the
 *        transcript, rather than adding a second copy of the same question
 */
async function handleQuerySubmit(question, opts) {
  const o = opts || {};
  const q = String(question || '').trim();
  if (!o.retry && /^help me[.!]?$/i.test(q)) {
    startWalkthrough();
    return { status: 'walkthrough' };
  }
  if (!store.hasApiKey()) {
    sendOverlayState();
    return { status: 'needs-key' };
  }
  abortCurrentQuery();
  const gen = ++queryGen;
  const text = q || DEFAULT_PROMPT;
  lastQuestion = text;
  // A retry re-runs the question already sitting unanswered at the end of the
  // transcript. Pushing it again would show the user their own question twice
  // and send the model a duplicate turn. If the transcript has moved on since
  // the failure, it is an ordinary new message after all.
  const last = chat[chat.length - 1];
  const resume = !!o.retry && !!last && last.role === 'user' && last.text === text;
  const priorTurns = (resume ? chat.slice(0, -1) : chat).slice(-CHAT_HISTORY_LIMIT);
  if (!resume) chat.push({ role: 'user', text });
  const model = store.getModel();
  const useBrain = store.getBrainEnabled();
  const send = (channel, payload) => {
    if (outputWin && !outputWin.isDestroyed()) {
      outputWin.webContents.send(channel, { gen, ...payload });
    }
  };
  // Echo the message and the thinking state before the capture so sending
  // feels instant; the capture happens under the thinking dots.
  if (!resume) send('chat:user', { text });
  send('query:start', {});
  queryBusy = true;
  sendOverlayState();
  showOutput();
  // Every message captures the screen fresh, so follow-ups talk about what
  // is on screen now, not what was there at the first hotkey press.
  pendingShot = await captureScreen({ region: activeRegion });
  lastCaptureOk = !!pendingShot;
  sendOverlayState();
  const shot = pendingShot;
  const viaRegion = !!(shot && shot.region);
  currentAbort = new AbortController();
  const signal = currentAbort.signal;
  // Point directives are stripped out of the stream before display and
  // collected; the pointer appears once the answer finishes.
  const extractor = ai.createPointExtractor((t) => send('query:delta', { text: t }));
  // Which model an error should name: the reader until it hands over, the
  // brain afterwards.
  let answering = model;
  try {
    // Brain mode: a vision model reads the screen first, then a stronger
    // text-only model answers from that reading. The reader call is not
    // streamed, so the user watches the thinking dots through it; naming the
    // stage is better than an unexplained pause.
    //
    // Every failure here degrades instead of stopping. A reader that errors
    // or loses the format drops the request back to one stage, and a brain
    // that dies before producing a word is retried on the model that can see
    // the screen. The user gets an answer unless the vision model itself is
    // unreachable.
    let screenReport = null;
    if (useBrain && shot) {
      send('query:stage', { label: 'Reading your screen' });
      try {
        const read = await withFailover({
          kind: 'vision',
          model,
          onSwitch: (m) =>
            send('query:stage', { label: `Busy \u2014 reading with ${shortModel(m)}` }),
          run: (m) =>
            ai.readScreen({
              apiKey: store.getApiKey(),
              model: m,
              imageBase64: shot.base64,
              imageWidth: shot.width,
              imageHeight: shot.height,
              question: text,
              region: viaRegion,
              signal,
            }),
        });
        screenReport = read.value;
      } catch (readErr) {
        if (readErr.name === 'AbortError') throw readErr;
        screenReport = null;
      }
    }

    // Automatic answer style: the reader has actually looked at the screen,
    // so its task word outranks anything guessed from the wording.
    const pinned = store.getSkillId();
    const chosen =
      pinned === AUTO_SKILL
        ? skills.route({ question: text, task: ai.extractTask(screenReport) }).id
        : pinned;
    const skill = skills.get(chosen);

    const base = {
      apiKey: store.getApiKey(),
      question: text,
      imageWidth: shot ? shot.width : 0,
      imageHeight: shot ? shot.height : 0,
      viaRegion,
      history: priorTurns,
      // The answering procedure travels with every request, which is what
      // holds a small fast model to a useful answer shape.
      skill,
      signal,
      onDelta: (t) => extractor.push(t),
    };

    if (screenReport) {
      answering = store.getBrain();
      send('query:stage', { label: 'Thinking' });
      try {
        const spoke = await withFailover({
          kind: 'text',
          model: answering,
          canRetry: () => !extractor.text,
          onSwitch: (m) =>
            send('query:stage', { label: `Busy \u2014 thinking with ${shortModel(m)}` }),
          run: (m) => ai.streamQuery({ ...base, model: m, screenReport }),
        });
        answering = spoke.model;
      } catch (brainErr) {
        // Nothing reached the user yet, so the request can still be served
        // by the model that already saw the screen.
        if (brainErr.name === 'AbortError' || extractor.text) throw brainErr;
        answering = model;
        send('query:stage', { label: 'Answering directly' });
        await ai.streamQuery({ ...base, model, imageBase64: shot.base64 });
      }
    } else {
      answering = model;
      if (useBrain && shot) send('query:stage', { label: 'Answering directly' });
      const spoke = await withFailover({
        kind: 'vision',
        model,
        canRetry: () => !extractor.text,
        onSwitch: (m) => send('query:stage', { label: `Busy \u2014 asking ${shortModel(m)}` }),
        run: (m) =>
          ai.streamQuery({
            ...base,
            model: m,
            imageBase64: shot ? shot.base64 : null,
          }),
      });
      answering = spoke.model;
    }
    extractor.flush();
    chat.push({ role: 'assistant', text: extractor.text });
    send('query:done', {});
    rememberChat(answering);
    showModelPoints(
      extractor.points,
      shot,
      ai.parseElements(screenReport, shot ? { width: shot.width, height: shot.height } : null)
    );
    return { status: 'done' };
  } catch (err) {
    extractor.flush();
    if (extractor.text) {
      chat.push({ role: 'assistant', text: extractor.text });
      rememberChat(answering);
    }
    const message = ai.friendlyError(err, err.stage === 'reader' ? model : answering, err.attempts);
    if (message) {
      // The error row offers Retry, which re-sends this exact question rather
      // than whatever the cleared input box would have sent.
      send('query:error', { message, question: text });
    } else {
      send('query:done', {});
    }
    return { status: 'error' };
  } finally {
    if (currentAbort && currentAbort.signal === signal) currentAbort = null;
    queryBusy = false;
    sendOverlayState();
  }
}

// A coordinate pair from a model that looked at the screen, as a fraction of
// the captured view.
//
// The contract is the 0-1000 grid, the same one the reader's table uses; see
// GRID_RULE. Reading those numbers as view pixels instead, which is what this
// did, put the centre of the screen at roughly a third across and dropped
// everything down the right and bottom edges for being out of range.
//
// Over 1000 the pair can only be view pixels, so it is read as such rather
// than thrown away. Below it the two readings are genuinely indistinguishable
// and the stated grid wins; that is what the prompt is for.
function pointFraction(p, shot) {
  const x = Number(p.x);
  const y = Number(p.y);
  if (!Number.isFinite(x) || !Number.isFinite(y) || x < 0 || y < 0) return null;
  if (x > 1000 || y > 1000) {
    if (x > shot.width || y > shot.height) return null;
    return { fx: x / shot.width, fy: y / shot.height };
  }
  return { fx: x / 1000, fy: y / 1000 };
}

// Turns directives into screen coordinates. Two paths, because two kinds of
// model produce them, but one coordinate system: both arrive on the 0-1000
// grid and both map across by ratio, which keeps them right on a display at
// any DPI.
//
// A model that saw the screen gives the pair itself. A brain working from a
// reading gives a name, which is looked up in the reader's ELEMENTS table. A
// name that matches nothing, or that matches two rows equally well, is
// dropped: a pointer at the wrong place is worse than no pointer, and it was
// the blind model's invented digits that put it there.
function showModelPoints(points, shot, elements) {
  if (!points || !points.length || !shot || !shot.width || !shot.height) return;
  if (overlayMode === 'hud') return;
  // The rectangle the model was actually looking at. For an area that is the
  // selection, not the display: a grid coordinate is a fraction of the view,
  // and the view is the crop. Mapping it onto the whole screen instead would
  // reintroduce exactly the drift the grid fix removed, scaled by however
  // small the selection was.
  const b = shot.region || shot.display;
  const toScreen = (fx, fy, label) => ({
    x: Math.round(b.x + fx * b.width),
    y: Math.round(b.y + fy * b.height),
    label: label || 'Here',
  });
  const mapped = [];
  for (const p of points) {
    if (p.x === null || p.x === undefined) {
      const el = ai.matchElement(p.label, elements);
      if (el) mapped.push(toScreen(el.x / 1000, el.y / 1000, p.label || el.label));
      continue;
    }
    const f = pointFraction(p, shot);
    if (!f) continue;
    mapped.push(toScreen(f.fx, f.fy, p.label));
  }
  if (!mapped.length) return;
  // The pointer window has to open on the screen the shot came from, not on
  // whichever one Windows calls primary.
  cursorBuddy.showPoints(mapped.slice(0, 3), screen.getDisplayMatching(b));
  applyRegistrations();
}

// Saved after every answer rather than at the end of the conversation: a
// crash, a hard quit, or simply forgetting to press New chat should not lose
// what was already said.
function rememberChat(model) {
  history.save({ id: chatId, model, messages: chat });
  notifyHistoryChanged();
}

function notifyHistoryChanged() {
  if (historyWin && !historyWin.isDestroyed()) {
    historyWin.webContents.send('history:changed', {});
  }
}

function openHistory() {
  if (historyWin && !historyWin.isDestroyed()) {
    historyWin.show();
    historyWin.focus();
    return;
  }
  const wa = workArea();
  const saved = store.getHistorySize();
  historyWin = new BrowserWindow({
    width: saved.width,
    height: clampNum(saved.height, 360, wa.height - 40),
    minWidth: 380,
    minHeight: 360,
    maxWidth: 900,
    frame: false,
    transparent: true,
    resizable: true,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    ...(IS_WINDOWS ? { type: 'toolbar' } : {}),
    show: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  // Past conversations are the most private thing this app holds, so the
  // window that lists them stays out of screen shares like every other one.
  hideFromCapture(historyWin);
  pinOnTop(historyWin);
  attachContextMenu(historyWin);
  historyWin.loadFile(path.join(__dirname, 'history', 'history.html'));
  historyWin.once('ready-to-show', () => {
    if (historyWin && !historyWin.isDestroyed()) historyWin.show();
  });
  // Windows emits 'resize' continuously through a drag, and every store.set is
  // a synchronous write of the whole settings file. Settings, twenty lines
  // above, settles first for exactly this reason; this one wrote hundreds of
  // times per drag.
  let historyResizeTimer = null;
  historyWin.on('resize', () => {
    clearTimeout(historyResizeTimer);
    historyResizeTimer = setTimeout(() => {
      if (historyWin && !historyWin.isDestroyed()) store.setHistorySize(historyWin.getBounds());
    }, DROP_SETTLE_MS);
  });
  historyWin.on('closed', () => {
    historyWin = null;
  });
}

function closeHistory() {
  if (historyWin && !historyWin.isDestroyed()) historyWin.close();
}

/** Brings a stored conversation back as the live one. */
function openConversation(id) {
  const entry = history.get(id);
  if (!entry) return { ok: false };
  abortCurrentQuery();
  chatId = entry.id;
  chat = entry.messages.map((m) => ({ role: m.role, text: m.text }));
  // Reopened from text alone: the screen it was about is long gone, so the
  // next message starts from the whole screen again.
  activeRegion = null;
  lastQuestion = '';
  store.setOutputHeight(0);
  currentOutputHeight = DEFAULT_OUTPUT_HEIGHT;
  closeHistory();
  overlayHidden = false;
  expandPanel();
  showOutput();
  if (outputWin && !outputWin.isDestroyed()) {
    outputWin.webContents.send('chat:sync', { messages: chat });
  }
  sendOverlayState();
  return { ok: true };
}

// Windows only reads the Run key at login, so this is written once at boot and
// again whenever the switch moves.
function applyLaunchAtLogin() {
  const on = store.getLaunchAtLogin();
  // Unpackaged, this would register the Electron binary rather than the app.
  // The preference is still remembered; it just is not handed to Windows.
  if (!app.isPackaged) return on;
  try {
    app.setLoginItemSettings({ openAtLogin: on, path: process.execPath, args: [] });
  } catch (err) {
    // A locked-down profile can refuse the Run key. Nothing else depends on
    // it, so the app carries on without startup.
  }
  return on;
}

function resetChat() {
  abortCurrentQuery();
  chat = [];
  chatId = newChatId();
  // A new conversation is about the whole screen again. Carrying a box over
  // from the last one is a constraint the user cannot see.
  activeRegion = null;
  lastQuestion = '';
  // New chat hands sizing back to the transcript.
  store.setOutputHeight(0);
  currentOutputHeight = DEFAULT_OUTPUT_HEIGHT;
  if (outputWin && !outputWin.isDestroyed()) {
    outputWin.webContents.send('chat:reset', {});
  }
  hideOutput();
  if (overlayMode === 'panel' && overlayWin && !overlayWin.isDestroyed()) {
    overlayWin.focus();
  }
}

// ------------------------------------------------------------------ walkthrough

function startWalkthrough() {
  if (overlayMode === 'hud') return;
  cursorBuddy.start(WALKTHROUGH);
  applyRegistrations();
}

cursorBuddy.init({
  getWindowFor: (name) => (name === 'overlay' ? overlayWin : settingsWin),
  ensureStepPrereq: (step) => {
    if (step.win === 'overlay' && overlayMode !== 'panel') expandPanel();
  },
  onDone: applyRegistrations,
  onCancel: applyRegistrations,
});

// ------------------------------------------------------------------------ IPC

function registerIpc() {
  ipcMain.handle('overlay:trigger', () => triggerCapture());
  ipcMain.handle('overlay:collapse', () => {
    if (overlayMode === 'hud') exitHudEditor();
    else collapsePanel();
  });
  ipcMain.handle('overlay:resize', (_e, { height }) => {
    if (overlayMode !== 'panel' || !overlayWin) return;
    currentInputHeight = height;
    // A glide already ends at the panel size and re-syncs on landing.
    if (motion.isAnimating(overlayWin)) return;
    setInputBounds(inputBounds(height));
    repositionOutputIfDocked();
  });
  ipcMain.handle('output:resize', (_e, { height }) => {
    if (overlayMode === 'hud' || !outputActive || !outputWin) return;
    currentOutputHeight = height;
    if (motion.isAnimating(outputWin)) return;
    resizeOutput(outputBounds(height));
  });
  ipcMain.handle('overlay:set-ignore', (_e, { ignore }) => {
    if (!overlayWin || overlayMode !== 'pill') return;
    overlayWin.setIgnoreMouseEvents(ignore, { forward: true });
  });
  ipcMain.handle('overlay:open-settings', () => openSettings());

  ipcMain.handle('query:submit', (_e, { question }) => handleQuerySubmit(question));
  ipcMain.handle('query:cancel', () => abortCurrentQuery());
  // Re-sends the question that failed. The input box was cleared on send, so
  // without this the only "retry" available was an empty box, which submits
  // the default question instead of the one that did not work.
  ipcMain.handle('query:retry', () => {
    if (!lastQuestion) return { status: 'nothing-to-retry' };
    return handleQuerySubmit(lastQuestion, { retry: true });
  });
  ipcMain.handle('chat:new', () => resetChat());

  ipcMain.handle('region:pick', () => triggerRegionCapture());
  ipcMain.handle('region:clear', () => clearRegion());

  ipcMain.handle('overlay:open-history', () => openHistory());
  ipcMain.handle('history:list', (_e, payload) => ({
    rows: history.list(payload && payload.query),
  }));
  ipcMain.handle('history:open', (_e, { id }) => openConversation(id));
  ipcMain.handle('history:remove', (_e, { id }) => ({ ok: history.remove(id) }));
  ipcMain.handle('history:clear', () => ({ ok: history.clear() }));
  ipcMain.handle('history:close', () => closeHistory());

  ipcMain.handle('hud:enter', () => enterHudEditor());
  ipcMain.handle('hud:exit', () => exitHudEditor());
  ipcMain.handle('hud:set-dock', (_e, { docked }) => setHudDock(!!docked));

  ipcMain.handle('open-external', (_e, { url }) => {
    if (/^https?:\/\//i.test(String(url || ''))) {
      shell.openExternal(url);
    }
  });

  ipcMain.handle('settings:get', () => ({
    hasKey: store.hasApiKey(),
    model: store.getModel(),
    defaultModel: ai.DEFAULT_MODEL,
    brain: store.getBrain(),
    brainEnabled: store.getBrainEnabled(),
    defaultBrain: ai.DEFAULT_BRAIN,
    showPaid: store.getShowPaidModels(),
    skills: skills.list(),
    skill: store.getSkillId(),
    shortcuts: store.getShortcuts(),
    active: { ...lastRegisterResults },
    defaults: { ...store.DEFAULT_SHORTCUTS },
    version: app.getVersion(),
    // Below Windows 10 build 19041 the call that hides these windows from a
    // screen share does nothing at all. Believing the overlay is hidden when
    // it is not is the worst way to find out.
    canHide: CAN_HIDE_FROM_CAPTURE,
    saveHistory: store.getSaveHistory(),
    historyCount: history.count(),
    launchAtLogin: store.getLaunchAtLogin(),
    // Unpackaged, the Run key would point at the Electron binary, so the
    // switch is shown as remembered-but-inactive rather than silently lying.
    launchAtLoginLive: app.isPackaged,
  }));

  ipcMain.handle('settings:test-key', () => ai.testKey(store.getApiKey()));

  ipcMain.handle('settings:save-skill', (_e, { skill }) => ({
    ok: true,
    skill: store.setSkillId(skill),
  }));

  ipcMain.handle('settings:save-key', (_e, { key }) => {
    const value = String(key || '').trim();
    if (!value) {
      return { ok: false, error: 'Enter a key before saving.' };
    }
    store.setApiKey(value);
    sendOverlayState();
    cursorBuddy.notify('key-saved');
    return { ok: true };
  });

  ipcMain.handle('settings:clear-key', () => {
    store.clearApiKey();
    sendOverlayState();
    return { ok: true };
  });

  ipcMain.handle('settings:save-model', (_e, { model }) => {
    const saved = store.setModel(model);
    return { ok: true, model: saved };
  });

  ipcMain.handle('models:list', async (_e, payload) => {
    const kind = payload && payload.kind === 'text' ? 'text' : 'vision';
    const result = await ai.listModels({ kind, includePaid: store.getShowPaidModels() });
    return {
      ...result,
      kind,
      current: kind === 'text' ? store.getBrain() : store.getModel(),
    };
  });

  ipcMain.handle('settings:save-brain', (_e, { model }) => ({
    ok: true,
    brain: store.setBrain(model),
  }));

  ipcMain.handle('settings:set-brain-enabled', (_e, { on }) => ({
    ok: true,
    brainEnabled: store.setBrainEnabled(on),
  }));

  ipcMain.handle('settings:set-save-history', (_e, { on }) => {
    const value = store.setSaveHistory(on);
    history.setEnabled(value);
    // Turning it off stops new writes; it does not silently delete what is
    // already there. Clear all is a separate, deliberate act.
    return { ok: true, saveHistory: value, historyCount: history.count() };
  });

  ipcMain.handle('settings:clear-history', () => {
    history.clear();
    notifyHistoryChanged();
    return { ok: true, historyCount: history.count() };
  });

  ipcMain.handle('settings:set-launch-at-login', (_e, { on }) => {
    store.setLaunchAtLogin(on);
    return { ok: true, launchAtLogin: applyLaunchAtLogin() };
  });

  ipcMain.handle('settings:set-show-paid', (_e, { on }) => ({
    ok: true,
    showPaid: store.setShowPaidModels(on),
  }));

  ipcMain.handle('clipboard:write', (_e, { text }) => {
    clipboard.writeText(String(text || ''));
    return { ok: true };
  });

  ipcMain.handle('settings:close', () => {
    if (settingsWin && !settingsWin.isDestroyed()) settingsWin.close();
  });

  ipcMain.handle('shortcuts:capture-mode', (_e, { on }) => {
    captureModeActive = !!on;
    applyRegistrations();
  });

  ipcMain.handle('shortcuts:set', (_e, { action, accelerator }) => {
    if (!Object.prototype.hasOwnProperty.call(ACTION_LABELS, action)) {
      return { ok: false, error: 'Unknown action.' };
    }
    if (!validAccelerator(accelerator)) {
      return {
        ok: false,
        error: 'Include Ctrl, Alt, or Win plus a key, for example Ctrl+Shift+A.',
      };
    }
    const shortcuts = store.getShortcuts();
    const clash = Object.entries(shortcuts).find(
      ([other, acc]) => other !== action && acc === accelerator
    );
    if (clash) {
      return {
        ok: false,
        error: `Already used by "${ACTION_LABELS[clash[0]]}". Pick a different combo.`,
      };
    }
    // Probe availability with everything of ours unregistered, so the probe
    // only fails when another application owns the combo.
    globalShortcut.unregisterAll();
    escapeRegistered = false;
    const available = globalShortcut.register(accelerator, () => {});
    if (available) globalShortcut.unregister(accelerator);
    if (!available) {
      applyRegistrations();
      return { ok: false, error: 'That shortcut is already in use - try another.' };
    }
    store.setShortcut(action, accelerator);
    applyRegistrations();
    return {
      ok: true,
      shortcuts: store.getShortcuts(),
      active: { ...lastRegisterResults },
    };
  });

  ipcMain.handle('shortcuts:reset', () => {
    const shortcuts = store.resetShortcuts();
    applyRegistrations();
    return { ok: true, shortcuts, active: { ...lastRegisterResults } };
  });

  ipcMain.handle('buddy:resolve-result', (_e, payload) => {
    cursorBuddy.handleResolveResult(payload);
  });
  ipcMain.handle('buddy:dismiss', () => cursorBuddy.notify('dismiss'));
  ipcMain.handle('buddy:advance', () => cursorBuddy.advance());
  ipcMain.handle('buddy:set-ignore', (_e, { ignore }) => {
    cursorBuddy.setClickThrough(ignore);
  });

  ipcMain.handle('app:quit', () => app.quit());
}

// ------------------------------------------------------------------ lifecycle

const gotLock = app.requestSingleInstanceLock();
if (!gotLock) {
  app.quit();
} else {
  app.on('second-instance', () => {
    if (overlayWin) triggerCapture();
  });

  app.whenReady().then(() => {
    registerIpc();
    region.registerIpc();
    history.setEnabled(store.getSaveHistory());
    createOverlay();
    createOutput();
    applyLaunchAtLogin();
    // Before applyRegistrations, which refreshes the tray labels with the
    // accelerators it has just registered.
    tray.init({
      capture: () => triggerCapture(),
      captureRegion: () => triggerRegionCapture(),
      toggleVisibility: () => onToggleHotkey(),
      newChat: () => resetChat(),
      openHistory: () => openHistory(),
      openSettings: () => openSettings(),
      quit: () => app.quit(),
      shortcuts: () => store.getShortcuts(),
    });
    applyRegistrations();
    // Warms the model catalogue in the background. Failover picks the next
    // free model from the curated order, and the curated order is a
    // preference, not evidence that a model still exists; without this the
    // cache was only ever filled by opening Settings, so the ordinary path -
    // launch, ask, hit a queued free model - was the one running blind. No
    // key needed, and a failure is silent.
    ai.warmCatalogue();
    if (
      !lastRegisterResults.captureQuery ||
      !lastRegisterResults.captureRegion ||
      !lastRegisterResults.toggleVisibility
    ) {
      console.warn(
        '[aevalbuddy] some shortcuts failed to register:',
        JSON.stringify(lastRegisterResults)
      );
    }
    // The walkthrough auto-runs once, on the very first launch with no key.
    // After that it only appears when asked for (typing "help me").
    if (
      !store.hasApiKey() &&
      !store.getWalkthroughAutoShown() &&
      !process.env.AEVAL_SMOKE &&
      !process.env.AEVAL_SHOT
    ) {
      store.setWalkthroughAutoShown();
      setTimeout(startWalkthrough, 1000);
    }
    if (process.env.AEVAL_SMOKE) {
      setTimeout(runSmoke, 800);
    }
    if (process.env.AEVAL_SHOT) {
      setTimeout(runShots, 1200);
    }
    // Unplugging the monitor the overlay is latched to would otherwise strand
    // every window on a rectangle that is no longer on anybody's desk, and the
    // one control that could move them lives in the window that vanished with
    // it. A resolution or DPI change moves the work area under them for the
    // same result. Both re-latch and re-place.
    const onDisplaysChanged = () => {
      if (overlayMode === 'hud') return;
      latchedDisplayId = null;
      latchDisplay();
      rehomeWindows();
      keepOnTop();
    };
    screen.on('display-removed', onDisplaysChanged);
    screen.on('display-added', onDisplaysChanged);
    screen.on('display-metrics-changed', onDisplaysChanged);
    if (!process.env.AEVAL_SMOKE && !process.env.AEVAL_SHOT) {
      setInterval(keepOnTop, 4000);
    }
  });

  // Every window, including the ones the lib modules create.
  app.on('browser-window-created', (event, win) => crash.guard(win));

  app.on('window-all-closed', () => {
    app.quit();
  });

  app.on('will-quit', () => {
    globalShortcut.unregisterAll();
    // The last conversation is usually still inside the debounce.
    history.flushNow();
    tray.destroy();
  });
}

// Visual self-check: photographs every surface so the layout can be looked
// at rather than guessed at. See lib/shots.js.
async function runShots() {
  const shots = require('./lib/shots');
  const evalIn = (win, code) =>
    win && !win.isDestroyed() ? win.webContents.executeJavaScript(code) : Promise.resolve();
  try {
    await shots.run({
      dir: process.env.AEVAL_SHOT,
      windows: () => ({
        overlay: overlayWin,
        output: outputWin,
        settings: settingsWin,
        history: historyWin,
        region: region.debugWindow(),
        buddy: cursorBuddy.debugWindow(),
      }),
      actions: {
        clearKey: () => {
          store.clearApiKey();
          sendOverlayState();
        },
        setKey: (k) => {
          store.setApiKey(k);
          sendOverlayState();
        },
        expandPanel: async () => {
          expandPanel();
        },
        collapse: () => collapsePanel(),
        // Puts real text in the query field. The field grows with its content
        // now, so its photograph is no longer one fixed rectangle.
        typeQuery: (text) =>
          evalIn(
            overlayWin,
            `(() => { const el = document.getElementById('query-input');` +
              ` el.value = ${JSON.stringify(String(text))};` +
              ` el.dispatchEvent(new Event('input')); return true; })()`
          ),
        seedChat: (messages) => {
          chat = messages.slice();
          showOutput();
          outputWin.webContents.send('chat:sync', { messages });
        },
        seedThinking: () => {
          outputWin.webContents.send('query:start', { gen: ++queryGen });
        },
        // The failure path is a surface too, and the one most likely to be
        // seen on a bad day: an error the user can act on rather than read.
        seedError: (message, question) => {
          outputWin.webContents.send('query:error', { gen: queryGen, message, question });
        },
        // The chip that says a conversation is looking at a rectangle.
        setRegion: (r) => {
          activeRegion = r;
          sendOverlayState();
        },
        openHistory: () => {
          history.setEnabled(true);
          history.clear();
          history.save({
            id: 'shot-1',
            model: 'z-ai/glm-5.2:free',
            messages: [
              { role: 'user', text: 'what is this error and how do I fix it?' },
              {
                role: 'assistant',
                text: 'A missing peer dependency. Install react-dom@19 and restart the dev server.',
              },
            ],
          });
          history.save({
            id: 'shot-2',
            model: 'z-ai/glm-5.2:free',
            messages: [
              { role: 'user', text: 'make this paragraph shorter and less formal' },
              { role: 'assistant', text: 'Here is a tighter version that keeps the ask up front.' },
            ],
          });
          history.save({
            id: 'shot-3',
            model: 'nvidia/nemotron-nano-12b-v2-vl:free',
            messages: [
              { role: 'user', text: 'which of these two buttons actually submits the form?' },
              { role: 'assistant', text: 'The one on the right. The left one resets the fields.' },
            ],
          });
          openHistory();
        },
        showRegionSelector: () => region.select(screen.getPrimaryDisplay()),
        dragRegion: (box) =>
          evalIn(
            region.debugWindow(),
            `(() => { const at = (t, x, y) => document.dispatchEvent(` +
              `new MouseEvent(t, { clientX: x, clientY: y, button: 0, bubbles: true }));` +
              ` at('mousedown', ${box.x}, ${box.y});` +
              ` at('mousemove', ${box.x + box.width}, ${box.y + box.height});` +
              ` return true; })()`
          ),
        cancelRegion: () => region.cancel(),
        enterHud: () => enterHudEditor(),
        exitHud: () => exitHudEditor(),
        openSettings: () => openSettings(),
        scrollSettings: (top) =>
          evalIn(settingsWin, `document.querySelector('main').scrollTop = ${top}; true`),
        openModelList: () =>
          evalIn(
            settingsWin,
            `(() => { document.querySelector('[data-edit="sec-model"]').click();` +
              ` document.getElementById('model-combo-btn').click(); return true; })()`
          ),
        enableBrain: () =>
          evalIn(
            settingsWin,
            `(() => { document.querySelector('[data-edit="sec-brain"]').click();` +
              ` const c = document.getElementById('brain-enabled');` +
              ` c.checked = true; c.dispatchEvent(new Event('change')); return true; })()`
          ),
        closeModelList: () => evalIn(settingsWin, `document.body.click(); true`),
        closeHistory: () => closeHistory(),
        probe: async (name, win, selector) => {
          if (!win || win.isDestroyed()) return;
          const inner = await evalIn(
            win,
            `(() => { const el = document.querySelector('${selector}');` +
              ` return el ? Math.ceil(el.getBoundingClientRect().height) : -1; })()`
          );
          console.log(`[SHOT] fit.${name}=window${win.getBounds().height}:surface${inner}`);
        },
        // Content protection makes these windows invisible to any normal
        // capture, which is the point of the app; it is turned off for the
        // length of one frame here and turned straight back on.
        liveShot: async (outPath) => {
          const wins = [overlayWin, outputWin].filter((x) => x && !x.isDestroyed());
          for (const win of wins) win.setContentProtection(false);
          await new Promise((r) => setTimeout(r, 500));
          try {
            const display = screen.getPrimaryDisplay();
            const sources = await desktopCapturer.getSources({
              types: ['screen'],
              thumbnailSize: { width: display.size.width, height: display.size.height },
            });
            const src =
              sources.find((s) => s.display_id === String(display.id)) || sources[0];
            const shot = src.thumbnail;
            const full = shot.getSize();
            const k = full.width / display.size.width;
            const rects = wins.map((w) => w.getBounds());
            const x0 = Math.min(...rects.map((r) => r.x)) - 40;
            const y0 = Math.min(...rects.map((r) => r.y)) - 40;
            const x1 = Math.max(...rects.map((r) => r.x + r.width)) + 40;
            const y1 = Math.max(...rects.map((r) => r.y + r.height)) + 40;
            const crop = {
              x: Math.max(0, Math.round(x0 * k)),
              y: Math.max(0, Math.round(y0 * k)),
              width: Math.round((x1 - x0) * k),
              height: Math.round((y1 - y0) * k),
            };
            require('fs').writeFileSync(outPath, shot.crop(crop).toPNG());
            return `${crop.width}x${crop.height}`;
          } finally {
            for (const win of wins) {
              if (!win.isDestroyed()) win.setContentProtection(true);
            }
          }
        },
        closeSettings: () => {
          if (settingsWin && !settingsWin.isDestroyed()) settingsWin.close();
        },
        showPoint: () =>
          cursorBuddy.showPoints([
            { x: 420, y: 230, label: 'Press Save to apply the change' },
          ]),
      },
    });
    app.exit(0);
  } catch (err) {
    console.error('[SHOT] error', err);
    app.exit(1);
  }
}

// Headless self-check used by automated verification: boots the real app,
// exercises capture, shortcut registration, panel expansion, and the HUD
// editor round trip, prints machine-readable lines, then exits.
async function runSmoke() {
  const log = (label, value) => console.log(`[SMOKE] ${label}=${value}`);
  // An invariant, not an observation. These are the things that cannot be
  // false in a build worth shipping, so they end the run rather than
  // printing a `false` nobody reads.
  const failures = [];
  const must = (label, ok, detail) => {
    log(label, ok ? 'ok' : `FAIL${detail ? `:${detail}` : ''}`);
    if (!ok) failures.push(label);
  };
  try {
    must('windows', !!(overlayWin && outputWin), 'overlay or output missing');
    log('version', app.getVersion());
    log('shortcut.captureQuery', lastRegisterResults.captureQuery);
    log('shortcut.captureRegion', lastRegisterResults.captureRegion);
    // Nothing else in the app is reachable once the overlay is hidden, so the
    // tray coming up is not a nicety.
    must('tray', tray.isReady(), 'no way back in once the overlay is hidden');
    log('shortcut.toggleVisibility', lastRegisterResults.toggleVisibility);
    log('model', store.getModel());
    const modelList = await ai.listModels();
    const tiers = modelList.groups.map((g) => `${g.tier}${g.models.length}`).join(',');
    log('modelList', `${tiers}:${modelList.live ? 'live' : 'fallback'}`);
    // Nothing paid may appear until it is asked for, and the default has to
    // be one of the free ones.
    log('modelListPaidHidden', modelList.groups.every((g) => g.tier === 'free'));
    const freeGroup = modelList.groups.find((g) => g.tier === 'free');
    must(
      'modelDefaultOffered',
      !!(freeGroup && freeGroup.models.some((m) => m.id === ai.DEFAULT_MODEL)),
      `${ai.DEFAULT_MODEL} is not in the live free list`
    );
    const paidList = await ai.listModels({ includePaid: true });
    log('modelListWithPaid', paidList.groups.map((g) => `${g.tier}${g.models.length}`).join(','));
    const brainList = await ai.listModels({ kind: 'text' });
    const brainFree = brainList.groups.find((g) => g.tier === 'free');
    must(
      'brainListDefault',
      !!(brainFree && brainFree.models.some((m) => m.id === ai.DEFAULT_BRAIN)),
      `${ai.DEFAULT_BRAIN} is not in the live free list`
    );
    // The brain never sees an image, so its prompt must describe a reading
    // and still teach the point directive.
    const brainPrompt = ai.makeSystemPrompt({
      imageWidth: 1280,
      imageHeight: 800,
      skill: skills.get(skills.DEFAULT_ID),
      viaReader: true,
    });
    log(
      'brainPrompt',
      brainPrompt.includes('cannot see the screen') && brainPrompt.includes('[[point')
        ? 'ok'
        : 'missing'
    );
    log('brainDefault', store.getBrain());
    log('brainEnabledDefault', store.getBrainEnabled());
    // Every skill file parses and the chosen one reaches the system prompt,
    // along with the coordinate contract the pointer depends on.
    const skillList = skills.list();
    log('skills', `${skillList.length}:${skillList.map((s) => s.id).join('+')}`);
    const prompt = ai.makeSystemPrompt({
      imageWidth: 1280,
      imageHeight: 800,
      skill: skills.get(skills.route({ question: 'why did this fail' }).id),
    });
    log(
      'skillInPrompt',
      prompt.includes('Screen assistant') && prompt.includes('0 to 1000 grid')
        ? 'ok'
        : 'missing'
    );
    // The view size is never quoted at the model: it does not know how large
    // the image it received is, and a number it cannot check is a number it
    // converts wrongly. Instructions also have to arrive as lines, not as one
    // paragraph with the pointer rule buried in it.
    log(
      'promptShape',
      `${prompt.includes('1280x800') ? 'leaksize' : 'nosize'}:${
        prompt.split('\n').length > 8 ? 'lines' : 'runon'
      }`
    );
    // Both stages teach the same coordinate system. Two conventions in one app
    // is what sent every point up and to the left.
    log(
      'gridShared',
      ai.makeSystemPrompt({ imageWidth: 1280, imageHeight: 800, viaReader: true }).includes('point ')
        ? 'ok'
        : 'missing'
    );
    log('skillFallback', skills.get('no-such-skill').id);
    log('skillDefault', store.getSkillId());
    const routed = ['why did the build fail', 'what is this dialog', 'reword this email', 'help']
      .map((q) => skills.route({ question: q }).id)
      .join(',');
    log('skillRoute', routed);
    log('skillRouteFromScreen', skills.route({ question: 'x', task: 'rewrite' }).id);
    // Resizing. The pill must never be forced up to the panel minimum: that
    // was the bug that stored a widened pill as a chosen width.
    log('widthDefaults', `${store.getInputWidth()}:${store.getOutputWidth()}`);
    collapsePanel();
    applyInputLimits();
    log('pillWidthHeld', overlayWin.getBounds().width);
    const cmin = overlayWin.getMinimumSize();
    const cmax = overlayWin.getMaximumSize();
    log('pillPinned', cmin[0] === cmax[0] && cmin[1] === cmax[1]);
    await triggerCapture();
    // The expand glide is still running for a moment; the panel only becomes
    // user-resizable once it lands and re-applies its limits.
    await new Promise((r) => setTimeout(r, T_EXPAND + 120));
    const pmin = overlayWin.getMinimumSize();
    const pmax = overlayWin.getMaximumSize();
    log('panelWidthRange', `${pmin[0]}-${pmax[0]}`);
    log('panelHeightPinned', pmin[1] === pmax[1]);
    log('panelWidthHeld', overlayWin.getBounds().width);
    log('widthAfterCycle', store.getInputWidth());
    log('outputAutoHeight', store.getOutputHeight() === 0);
    // The query field takes more than one line. The questions this app is for
    // arrive as a pasted stack trace, and a text input silently dropped every
    // newline in one. Driven through the real element, because "it is a
    // textarea now" is a fact about the markup and this is a fact about what
    // the user gets: three lines in, three lines out, and a taller panel.
    const oneRow = await overlayWin.webContents.executeJavaScript(
      `(() => { const el = document.getElementById('query-input');` +
        ` el.value = ''; el.dispatchEvent(new Event('input'));` +
        ` return Math.round(el.getBoundingClientRect().height); })()`
    );
    const multi = await overlayWin.webContents.executeJavaScript(
      `(() => { const el = document.getElementById('query-input');` +
        ` el.value = 'line one\\nline two\\nline three';` +
        ` el.dispatchEvent(new Event('input'));` +
        ` return { tag: el.tagName, h: Math.round(el.getBoundingClientRect().height),` +
        ` lines: el.value.split('\\n').length }; })()`
    );
    await new Promise((r) => setTimeout(r, 120));
    log('queryFieldRows', `${oneRow}px->${multi.h}px`);
    must(
      'queryFieldTakesMultipleLines',
      multi.tag === 'TEXTAREA' && multi.lines === 3 && multi.h > oneRow,
      `${multi.tag} kept ${multi.lines} line(s) at ${multi.h}px against ${oneRow}px for one`
    );
    // And the panel followed it, rather than growing a field behind a window
    // that stayed one row tall.
    must(
      'panelFollowsTheField',
      overlayWin.getBounds().height > DEFAULT_INPUT_HEIGHT,
      `panel is ${overlayWin.getBounds().height}px for a three-line question`
    );
    await overlayWin.webContents.executeJavaScript(
      `(() => { const el = document.getElementById('query-input');` +
        ` el.value = ''; el.dispatchEvent(new Event('input')); return true; })()`
    );
    // Window motion resolves its curve and its anchored start rectangles.
    const rect = { x: 100, y: 40, width: 420, height: 200 };
    const grown = motion.shrunkFrom(rect, 'top', 'right');
    log(
      'motionAnchor',
      grown.y === rect.y && grown.x + grown.width === rect.x + rect.width
        ? 'top-right-held'
        : `wrong:${JSON.stringify(grown)}`
    );
    const shot = await captureScreen();
    log('capture', shot ? `ok:${shot.bytes}bytes:${shot.width}x${shot.height}` : 'failed');
    // The window and the screenshot have to be talking about the same monitor.
    // Capture moved onto the cursor's display in v0.6 and every window was
    // left on getPrimaryDisplay(), so the app answered about one screen and
    // opened the panel on another. On a one-monitor machine this is trivially
    // true, which is the point: it is the two-monitor desk that cannot be
    // asked, so the invariant has to be the one that holds on both.
    latchDisplay();
    const wa = workArea();
    const inside = (r) =>
      r.x >= wa.x && r.y >= wa.y && r.x + r.width <= wa.x + wa.width && r.y + r.height <= wa.y + wa.height;
    log('displays', screen.getAllDisplays().length);
    must(
      'windowsOnCaptureDisplay',
      overlayDisplay().id === latchedDisplayId &&
        overlayDisplay().id === activeDisplay().id &&
        inside(pillBounds()) &&
        inside(inputBounds(DEFAULT_INPUT_HEIGHT)),
      `overlay positions against display ${overlayDisplay().id}, capture reads ${activeDisplay().id}`
    );
    // An unplugged monitor must not leave the windows on a rectangle nobody
    // has. A latch pointing at an id that is gone falls back to the live one.
    latchedDisplayId = -1;
    must(
      'staleDisplayFallsBack',
      overlayDisplay().id === activeDisplay().id,
      'a latched display that no longer exists did not fall back'
    );
    latchDisplay();
    // Markdown renders inside the real answer window, not a simulation.
    const mdHtml = await outputWin.webContents.executeJavaScript(
      `(() => { const d = document.createElement('div');` +
        ` window.renderMarkdownInto(d, '# T\\n- a\\n- b\\n**s** and \\u0060c\\u0060');` +
        ` return d.innerHTML; })()`
    );
    log(
      'markdown',
      mdHtml.includes('<li>') && mdHtml.includes('<strong>') && mdHtml.includes('md-h')
        ? 'ok'
        : `unexpected:${mdHtml.slice(0, 80)}`
    );
    // Scrolling. A transcript taller than the answer window has to scroll
    // inside it. When it does not, the window clips and the top of the answer
    // is simply gone, with no way to reach it.
    const longChat = [];
    for (let i = 0; i < 8; i += 1) {
      longChat.push({ role: 'user', text: `question number ${i}` });
      longChat.push({ role: 'assistant', text: `Paragraph ${i} of the answer. `.repeat(24) });
    }
    chat = longChat.slice();
    showOutput();
    outputWin.webContents.send('chat:sync', { messages: longChat });
    await new Promise((r) => setTimeout(r, 800));
    const scrollProbe = await outputWin.webContents.executeJavaScript(
      `(() => { const c = document.getElementById('chat');` +
        ` c.scrollTop = 1e6;` +
        ` return [c.scrollHeight, c.clientHeight, c.scrollTop, getComputedStyle(c).overflowY].join(':');` +
        ` })()`
    );
    log('transcriptScroll', scrollProbe);
    // A real wheel event, not an assignment to scrollTop: the question is
    // whether turning the wheel over the transcript moves it.
    await outputWin.webContents.executeJavaScript(
      `document.getElementById('chat').scrollTop = 0; true`
    );
    // The window has just been resized around the seeded transcript; a wheel
    // event delivered into a layout that has not settled lands on nothing.
    await new Promise((r) => setTimeout(r, 300));
    let wheeled = 0;
    for (let attempt = 0; attempt < 5 && !wheeled; attempt += 1) {
      outputWin.webContents.sendInputEvent({
        type: 'mouseWheel',
        x: Math.round(outputWin.getBounds().width / 2),
        y: Math.round(outputWin.getBounds().height / 2),
        deltaX: 0,
        deltaY: -160,
        canScroll: true,
      });
      await new Promise((r) => setTimeout(r, 250));
      wheeled = await outputWin.webContents.executeJavaScript(
        `document.getElementById('chat').scrollTop`
      );
    }
    log('transcriptWheel', wheeled > 0 ? `moved:${Math.round(wheeled)}` : 'stuck');
    // The transcript's ceiling and the window's have to be the same number,
    // or one of them clips the other.
    const chatMax = await outputWin.webContents.executeJavaScript(
      `getComputedStyle(document.getElementById('chat')).maxHeight`
    );
    log('chatCeiling', `${chatMax}:window${outputMaxHeight()}`);
    const thumb = await outputWin.webContents.executeJavaScript(
      `(() => { const s = getComputedStyle(document.documentElement);` +
        ` return s.getPropertyValue('--scroll-thumb').trim(); })()`
    );
    log('scrollThumb', thumb || 'unset');
    log('answerHeightUnderLongChat', outputWin.getBounds().height);
    // The window has to follow the transcript. A height reported during a
    // glide used to be dropped, which left long answers inside a window that
    // had stopped growing.
    const surface = await outputWin.webContents.executeJavaScript(
      `Math.ceil(document.getElementById('opanel').getBoundingClientRect().height)`
    );
    log(
      'answerFollowsTranscript',
      outputWin.getBounds().height === surface ? 'exact' : `window${outputWin.getBounds().height}:surface${surface}`
    );
    const moreFlags = await outputWin.webContents.executeJavaScript(
      `(() => { const c = document.getElementById('chat');` +
        ` c.scrollTop = 0; c.dispatchEvent(new Event('scroll'));` +
        ` const top = document.body.dataset.more;` +
        ` c.scrollTop = c.scrollHeight; c.dispatchEvent(new Event('scroll'));` +
        ` return top + ':' + document.body.dataset.more; })()`
    );
    log('moreIndicator', moreFlags);
    resetChat();

    const SHOT_1600 = { width: 1600, height: 900 };
    // Point directives split across deltas are stripped and collected, and
    // the space the directive was sitting in closes behind it.
    const ex = ai.createPointExtractor(() => {});
    ex.push('Press Save [[point x=1');
    ex.push('20 y=340 | Save button]] to finish.');
    ex.flush();
    log(
      'pointExtract',
      `${ex.points.length}:${ex.text.includes('[[') ? 'leaked' : 'clean'}:${
        ex.text.includes('  ') ? 'doublespace' : 'tight'
      }`
    );
    // The shapes a model reaches for when it drifts off the stated one. Each
    // has to yield a point, and none may put brackets on screen: a malformed
    // directive is swallowed, because showing it loses the pointer anyway and
    // shows the machinery as well.
    const DRIFT = [
      'a [[point x=120, y=340 | Save]] b',
      'a [[point 120, 340 | Save]] b',
      'a [[point (500, 500)]] b',
      'a [[point x=120.5 y=340]] b',
      'a [[point x=120 y=340 - Save]] b',
      'a [[point at the Save button]] b',
    ];
    const drift = DRIFT.map((line) => {
      const e = ai.createPointExtractor(() => {});
      e.push(line);
      e.flush();
      return e.points.length === 1 && !e.text.includes('[[');
    });
    log('pointShapes', `${drift.filter(Boolean).length}/${DRIFT.length}`);
    // Ordinary text that happens to hold double brackets is not a directive.
    const arr = ai.createPointExtractor(() => {});
    arr.push('the matrix [[1,2],[3,4]] is fine');
    arr.flush();
    log('pointNonDirective', arr.text.includes('[[1,2]') ? 'kept' : 'eaten');
    await triggerCapture();
    log('mode', overlayMode);
    log('hasKey', store.hasApiKey());
    // Off the taskbar and off Alt-Tab: skipTaskbar asks the shell to drop the
    // button, the toolbar window type takes it out of the window list.
    const base = baseWindowOptions();
    log('taskbarHidden', `${base.skipTaskbar}:${base.type || 'default'}`);
    log('overlayOnTop', overlayWin.isAlwaysOnTop());
    log('outputOnTop', outputWin.isAlwaysOnTop());
    openSettings();
    await new Promise((r) => setTimeout(r, 600));
    log(
      'settingsWindow',
      settingsWin ? `onTop${settingsWin.isAlwaysOnTop()}` : 'missing'
    );
    settingsWin.close();
    log('captureProtection', CAN_HIDE_FROM_CAPTURE ? 'on' : `unsupported:build${WIN_BUILD}`);
    // The reader's table, and the two directive shapes that read from it.
    const sampleReport = [
      'SCREEN: VS Code',
      'STATE: the build failed',
      'FOCUS: the terminal',
      'TASK: debug',
      'TEXT: Cannot find module react-dom',
      'ELEMENTS:',
      '- Run and Debug | 120, 340',
      '- Terminal tab | 480,905',
    ].join('\n');
    const els = ai.parseElements(sampleReport);
    log('readerElements', els.map((e) => `${e.label}@${e.x},${e.y}`).join('+') || 'none');
    log(
      'pointByName',
      `${(ai.matchElement('the terminal tab', els) || {}).label}:${
        ai.matchElement('Publish to production', els) ? 'wrongmatch' : 'dropped'
      }`
    );
    // The row shapes the reader drifts into. All six describe the same
    // control; all six have to survive, because a dropped row is a name the
    // brain uses and the app cannot resolve.
    const ROWS = [
      '- Save button | 120, 340',
      '- Save button (120, 340)',
      '- Save button | x=120, y=340',
      '- Save button: 120, 340',
      '1. Save button | 120, 340',
      '- **Save button** | 0.12, 0.34',
    ];
    const rows = ROWS.map((row) => {
      const one = ai.parseElements(`ELEMENTS:\n${row}`);
      return one.length === 1 && one[0].x === 120 && one[0].y === 340;
    });
    log('readerRowShapes', `${rows.filter(Boolean).length}/${ROWS.length}`);
    // A row that came back in view pixels is rescaled when the view size is
    // known, and dropped when it is not.
    const pixelRow = 'ELEMENTS:\n- Save button | 1920, 1080';
    log(
      'readerPixelRow',
      `${(ai.parseElements(pixelRow, { width: 2560, height: 1440 })[0] || {}).x}:${
        ai.parseElements(pixelRow).length ? 'kept' : 'dropped'
      }`
    );
    // Two rows matching a name equally well is not a coin flip.
    const ambiguous = ai.parseElements(
      'ELEMENTS:\n- Terminal tab | 480,905\n- Problems tab | 600,905'
    );
    log(
      'pointAmbiguous',
      `${ai.matchElement('tab', ambiguous) ? 'guessed' : 'dropped'}:${
        (ai.matchElement('Problems tab', ambiguous) || {}).label
      }`
    );
    const named = ai.createPointExtractor(() => {});
    named.push('Open the [[point Terminal ');
    named.push('tab]] and read it.');
    named.flush();
    log(
      'pointNamedExtract',
      `${named.points.length}:${named.points[0] ? named.points[0].label : 'none'}:${
        named.text.includes('[[') ? 'leaked' : 'clean'
      }`
    );
    showModelPoints(
      [{ x: null, y: null, label: 'terminal tab' }, { x: null, y: null, label: 'nothing here' }],
      { width: 1600, height: 900, display: { x: 0, y: 0, width: 1600, height: 900 } },
      els
    );
    log('pointResolved', cursorBuddy.isActive());
    cursorBuddy.cancel();
    // 500,500 is the middle of the screen on the 0-1000 grid and nowhere near
    // it if the pair is read as pixels of the captured view. This is the
    // regression that put every pointer up and to the left.
    log('pointGridMapping', JSON.stringify(pointFraction({ x: 500, y: 500 }, SHOT_1600)));
    // Over 1000 the pair can only be view pixels, and used to be dropped.
    log('pointPixelRescue', JSON.stringify(pointFraction({ x: 1500, y: 875 }, SHOT_1600)));
    // The bottom edge of the grid is on screen, not out of range.
    log('pointBottomEdge', JSON.stringify(pointFraction({ x: 990, y: 990 }, SHOT_1600)));
    log('pointOffScreen', JSON.stringify(pointFraction({ x: 4000, y: 10 }, SHOT_1600)));

    // ---------------------------------------------------------------- area
    // The whole point of choosing an area is that the same pixel budget buys
    // more of the pixels that matter. This measures it rather than asserting
    // it: pixels-per-point for the whole screen against the same for a crop.
    const disp = screen.getPrimaryDisplay();
    const box = { x: disp.bounds.x + 40, y: disp.bounds.y + 40, width: 320, height: 200 };
    const areaShot = await captureScreen({ region: box });
    const fullShot = await captureScreen();
    log(
      'regionCapture',
      areaShot
        ? `${areaShot.width}x${areaShot.height}:${areaShot.region ? 'tagged' : 'untagged'}`
        : 'failed'
    );
    if (areaShot && fullShot) {
      const fullPpd = fullShot.width / disp.size.width;
      const areaPpd = areaShot.width / box.width;
      log('regionSharper', `${fullPpd.toFixed(2)}x->${areaPpd.toFixed(2)}x`);
    }
    // A point in an area is a fraction of the area, not of the screen. The
    // centre of a box at 40,40 sized 320x200 is 200,140 -- nowhere near the
    // centre of the display.
    const areaCentre = areaShot
      ? {
          x: Math.round(areaShot.region.x + 0.5 * areaShot.region.width),
          y: Math.round(areaShot.region.y + 0.5 * areaShot.region.height),
        }
      : null;
    log('regionPointTarget', JSON.stringify(areaCentre));
    // The selector opens and cancels without stranding a full-screen scrim.
    const pick = region.select(disp);
    region.cancel();
    log('regionSelectorCancels', (await pick) === null && !region.isActive());
    // Escape during a selection must dismiss the scrim, not the panel behind
    // it. The global shortcut is consumed before any window sees the key, so
    // the selector's own handler never runs while the panel is open.
    const escPick = region.select(disp);
    const wasPanel = overlayMode;
    onEscapePressed();
    log(
      'regionEscape',
      `${(await escPick) === null ? 'cancelled' : 'stuck'}:${
        overlayMode === wasPanel ? 'panel-kept' : 'panel-lost'
      }`
    );
    log(
      'regionPrompt',
      ai.makeSystemPrompt({ imageWidth: 800, imageHeight: 400, viaRegion: true }).includes('one area')
        ? 'ok'
        : 'missing'
    );

    // ------------------------------------------------------------ failover
    // A free model that is rate limited is handed on to the next free one; a
    // rejected key is not, because no other model would accept it either.
    let seen = [];
    const rode = await withFailover({
      kind: 'vision',
      model: ai.DEFAULT_MODEL,
      run: (m) => {
        seen.push(m);
        if (seen.length < 2) return Promise.reject(Object.assign(new Error('busy'), { status: 429 }));
        return Promise.resolve('answered');
      },
    });
    log('failoverRateLimit', `${seen.length}:${rode.model !== ai.DEFAULT_MODEL ? 'switched' : 'same'}`);
    let held = 'no-throw';
    try {
      await withFailover({
        kind: 'vision',
        model: ai.DEFAULT_MODEL,
        run: () => Promise.reject(Object.assign(new Error('bad key'), { status: 401 })),
      });
    } catch (err) {
      held = `threw:${err.status}`;
    }
    log('failoverKeepsAuthError', held);
    let capped = 0;
    try {
      await withFailover({
        kind: 'vision',
        model: ai.DEFAULT_MODEL,
        run: () => {
          capped += 1;
          return Promise.reject(Object.assign(new Error('busy'), { status: 429 }));
        },
      });
    } catch (err) {
      // Expected: every attempt failed.
    }
    log('failoverCapped', `${capped}<=${MAX_MODEL_ATTEMPTS}:${capped <= MAX_MODEL_ATTEMPTS}`);
    // The stand-in has to exist. Failover reading its next id straight out of
    // the curated array is how the machinery built to survive a retired model
    // could hand the next attempt another retired one; the catalogue has been
    // warmed by listModels above, so every id it offers is one the live list
    // has. An invariant, because a 404 chasing a 404 is invisible from the
    // outside: the app recovers on the third try and nobody sees the waste.
    const catalogue = new Set((modelList.groups.find((g) => g.tier === 'free') || { models: [] }).models.map((m) => m.id));
    const standIn = ai.nextFreeModel('vision', [ai.DEFAULT_MODEL]);
    must(
      'failoverPicksLiveModel',
      !!standIn && catalogue.has(standIn),
      `${standIn} is not in the live free vision list`
    );
    const brainStandIn = ai.nextFreeModel('text', [ai.DEFAULT_BRAIN]);
    const brainCatalogue = new Set((brainFree || { models: [] }).models.map((m) => m.id));
    must(
      'failoverPicksLiveBrain',
      !!brainStandIn && brainCatalogue.has(brainStandIn),
      `${brainStandIn} is not in the live free text list`
    );
    // The error copy may not assert a stand-in that was never tried. One
    // attempt means one model, and the sentence about the others has to be
    // absent rather than merely wrong.
    const oneTry = ai.friendlyError(Object.assign(new Error('busy'), { status: 429 }), 'x/y:free', 1);
    const threeTries = ai.friendlyError(Object.assign(new Error('busy'), { status: 429 }), 'x/y:free', 3);
    must(
      'errorCopyCountsAttempts',
      !/stand-in/.test(oneTry) && /2 free stand-ins/.test(threeTries),
      'the rate-limit copy does not follow the attempt count'
    );

    // ------------------------------------------------------------- history
    history.setEnabled(true);
    history.clear();
    history.save({
      id: 'smoke-1',
      model: 'test',
      messages: [
        { role: 'user', text: 'why is the build red' },
        { role: 'assistant', text: 'A peer dependency is missing.' },
      ],
    });
    history.save({
      id: 'smoke-2',
      model: 'test',
      messages: [{ role: 'user', text: 'reword this paragraph' }],
    });
    // The same id twice is an edit of one conversation, not a second row.
    history.save({
      id: 'smoke-1',
      model: 'test',
      messages: [
        { role: 'user', text: 'why is the build red' },
        { role: 'assistant', text: 'A peer dependency is missing.' },
        { role: 'user', text: 'and the warning under it?' },
      ],
    });
    log('historySaved', `${history.count()}:${(history.get('smoke-1') || {}).messages.length}`);
    log(
      'historyTitle',
      (history.list()[0] || {}).title === 'reword this paragraph' ||
        (history.list()[0] || {}).title === 'why is the build red'
        ? 'ok'
        : 'wrong'
    );
    // Search reaches the bodies, not only the titles.
    log(
      'historySearch',
      `${history.list('peer dependency').length}:${history.list('nothing here at all').length}`
    );
    history.remove('smoke-2');
    log('historyRemove', history.count());
    // Off means no new writes, and does not delete what is already there.
    history.setEnabled(false);
    history.save({ id: 'smoke-3', messages: [{ role: 'user', text: 'nope' }] });
    log('historyRespectsSwitch', `${history.count()}:${history.get('smoke-3') ? 'wrote' : 'skipped'}`);
    history.setEnabled(store.getSaveHistory());
    history.clear();
    log('historyCleared', history.count());
    // The byte cap, and the cost of enforcing it. trim() used to JSON.stringify
    // the whole history after every answer to find out whether anything needed
    // dropping; it now sums estimated sizes. An estimate that read low would
    // let the file grow past its cap, so the check measures the real
    // serialised length rather than trusting the estimator that produced it.
    const bulk = 'x'.repeat(60 * 1024);
    const trimStart = Date.now();
    for (let i = 0; i < 60; i += 1) {
      history.save({
        id: `budget-${i}`,
        model: 'test',
        messages: [
          { role: 'user', text: bulk },
          { role: 'assistant', text: bulk },
        ],
      });
    }
    const trimMs = Date.now() - trimStart;
    const measured = history.measuredBytes();
    log('historyTrimMs', trimMs);
    log('historyBytes', `${measured}<=${history.MAX_BYTES}`);
    must(
      'historyRespectsByteCap',
      measured <= history.MAX_BYTES && history.count() > 0,
      `${measured} bytes against a ${history.MAX_BYTES} cap, ${history.count()} entries`
    );
    history.clear();
    log('historyRetryGuard', lastQuestion === '' ? 'nothing-to-retry' : 'has-question');
    const submitResult = await handleQuerySubmit('smoke question');
    log('queryWithoutKey', `${submitResult.status}:chat${chat.length}`);
    enterHudEditor();
    log('hudMode', overlayMode);
    log('hudDockedDefault', hudOutputPending ? hudOutputPending.docked : 'none');
    setHudDock(false);
    const detached = hudOutputPending && hudOutputPending.docked === false;
    setHudDock(true);
    const redocked = hudOutputPending && hudOutputPending.docked === true;
    log('hudDockToggle', `${detached}:${redocked}`);
    const snap = findSnap(
      boundsForSize(hudInputSize(), { h: 'left', v: 'top', offX: 30, offY: 30 }),
      hudInputSize()
    );
    log('hudSnapFound', snap ? `${snap.pos.h}-${snap.pos.v}` : 'none');
    exitHudEditor();
    log('hudExitMode', overlayMode);
    log('storedPos', JSON.stringify(store.getOverlayPos()));
    log('storedOutputPos', JSON.stringify(store.getOutputPos()));
    cursorBuddy.showPoints([{ x: 80, y: 80, label: 'smoke point' }]);
    log('buddyPoints', cursorBuddy.isActive());
    cursorBuddy.advance();
    log('buddyPointsDone', !cursorBuddy.isActive());
    startWalkthrough();
    log('walkthrough', cursorBuddy.isActive());
    // The overlay has no taskbar button to go dark, so an unhandled error
    // has to leave a trace somewhere the user can be pointed at.
    const marker = 'smoke: deliberate unhandled rejection';
    process.emit('unhandledRejection', new Error(marker));
    let logged = false;
    try {
      logged = require('fs').readFileSync(crash.file(), 'utf8').includes(marker);
    } catch (err) {
      logged = false;
    }
    must('crashLogged', logged, 'an unhandled rejection never reached errors.log');
    setTimeout(() => {
      if (failures.length) {
        console.error(`[SMOKE] failed: ${failures.join(', ')}`);
        app.exit(1);
        return;
      }
      console.log('[SMOKE] done');
      app.exit(0);
    }, 1200);
  } catch (err) {
    console.error('[SMOKE] error', err);
    app.exit(1);
  }
}
