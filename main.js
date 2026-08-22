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

const store = require('./lib/store');
const ai = require('./lib/openrouter');
const skills = require('./lib/skills');
const motion = require('./lib/motion');
const cursorBuddy = require('./lib/cursor-buddy');

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
let lastCaptureOk = true;
let currentAbort = null;
let queryGen = 0;
let queryBusy = false;
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
const lastRegisterResults = { captureQuery: false, toggleVisibility: false };

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

function workArea() {
  return screen.getPrimaryDisplay().workArea;
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

async function captureScreen() {
  try {
    const primary = screen.getPrimaryDisplay();
    const { width, height } = primary.size;
    const scale = Math.min(1, SHOT_LONG_EDGE / Math.max(width, height));
    const sources = await desktopCapturer.getSources({
      types: ['screen'],
      thumbnailSize: {
        width: Math.round(width * scale),
        height: Math.round(height * scale),
      },
    });
    if (!sources.length) return null;
    const source =
      sources.find((s) => s.display_id === String(primary.id)) || sources[0];
    if (source.thumbnail.isEmpty()) return null;
    // Step quality down only as far as still-readable text allows. A slower
    // upload beats an unreadable screen.
    let quality = SHOT_QUALITY;
    let buf = source.thumbnail.toJPEG(quality);
    while (buf.length > SHOT_BUDGET && quality > SHOT_QUALITY_FLOOR) {
      quality -= 8;
      buf = source.thumbnail.toJPEG(quality);
    }
    // The image size and display bounds let point directives map back from
    // image pixels to screen coordinates.
    const size = source.thumbnail.getSize();
    return {
      base64: buf.toString('base64'),
      bytes: buf.length,
      width: size.width,
      height: size.height,
      display: { ...primary.bounds },
    };
  } catch (err) {
    // Capture can fail on locked desktops or exotic display drivers; the
    // query flow degrades to text-only instead of crashing.
    return null;
  }
}

async function triggerCapture() {
  if (!overlayWin || overlayMode === 'hud') return;
  overlayHidden = false;
  pendingShot = await captureScreen();
  lastCaptureOk = !!pendingShot;
  expandPanel();
  // An existing conversation comes back with the panel.
  if (chat.length) showOutput();
}

// ------------------------------------------------------------------- hotkeys

function onCaptureHotkey() {
  triggerCapture();
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
  applyRegistrations();
}

function onEscapePressed() {
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
  lastRegisterResults.toggleVisibility = globalShortcut.register(
    shortcuts.toggleVisibility,
    onToggleHotkey
  );
  if (overlayMode !== 'pill' || cursorBuddy.isActive()) {
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

function abortCurrentQuery() {
  if (currentAbort) {
    currentAbort.abort();
    currentAbort = null;
  }
}

async function handleQuerySubmit(question) {
  const q = String(question || '').trim();
  if (/^help me[.!]?$/i.test(q)) {
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
  const history = chat.slice(-CHAT_HISTORY_LIMIT);
  chat.push({ role: 'user', text });
  const model = store.getModel();
  const useBrain = store.getBrainEnabled();
  const send = (channel, payload) => {
    if (outputWin && !outputWin.isDestroyed()) {
      outputWin.webContents.send(channel, { gen, ...payload });
    }
  };
  // Echo the message and the thinking state before the capture so sending
  // feels instant; the capture happens under the thinking dots.
  send('chat:user', { text });
  send('query:start', {});
  queryBusy = true;
  sendOverlayState();
  showOutput();
  // Every message captures the screen fresh, so follow-ups talk about what
  // is on screen now, not what was there at the first hotkey press.
  pendingShot = await captureScreen();
  lastCaptureOk = !!pendingShot;
  sendOverlayState();
  const shot = pendingShot;
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
        screenReport = await ai.readScreen({
          apiKey: store.getApiKey(),
          model,
          imageBase64: shot.base64,
          imageWidth: shot.width,
          imageHeight: shot.height,
          question: text,
          signal,
        });
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
      history,
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
        await ai.streamQuery({ ...base, model: answering, screenReport });
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
      await ai.streamQuery({
        ...base,
        model,
        imageBase64: shot ? shot.base64 : null,
      });
    }
    extractor.flush();
    chat.push({ role: 'assistant', text: extractor.text });
    send('query:done', {});
    showModelPoints(extractor.points, shot, ai.parseElements(screenReport));
    return { status: 'done' };
  } catch (err) {
    extractor.flush();
    if (extractor.text) chat.push({ role: 'assistant', text: extractor.text });
    const message = ai.friendlyError(err, err.stage === 'reader' ? model : answering);
    if (message) {
      send('query:error', { message });
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

// Turns directives into screen coordinates. Two paths, because two kinds of
// model produce them.
//
// A model that saw the screen gives pixels in the captured view, which map
// across by ratio; using ratios rather than the scale factor keeps it right
// on a display at any DPI.
//
// A brain working from a reading gives a name, which is looked up in the
// reader's ELEMENTS table and mapped from the 0-1000 grid. A name that
// matches nothing is dropped: a pointer at the wrong place is worse than no
// pointer, and it was the blind model's invented digits that put it there.
function showModelPoints(points, shot, elements) {
  if (!points || !points.length || !shot || !shot.width || !shot.height) return;
  if (overlayMode === 'hud') return;
  const b = shot.display;
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
    if (p.x < 0 || p.y < 0 || p.x > shot.width || p.y > shot.height) continue;
    mapped.push(toScreen(p.x / shot.width, p.y / shot.height, p.label));
  }
  if (!mapped.length) return;
  cursorBuddy.showPoints(mapped.slice(0, 3));
  applyRegistrations();
}

function resetChat() {
  abortCurrentQuery();
  chat = [];
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
  ipcMain.handle('chat:new', () => resetChat());

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
    createOverlay();
    createOutput();
    applyRegistrations();
    if (!lastRegisterResults.captureQuery || !lastRegisterResults.toggleVisibility) {
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
    if (!process.env.AEVAL_SMOKE && !process.env.AEVAL_SHOT) {
      setInterval(keepOnTop, 4000);
    }
  });

  app.on('window-all-closed', () => {
    app.quit();
  });

  app.on('will-quit', () => {
    globalShortcut.unregisterAll();
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
        seedChat: (messages) => {
          chat = messages.slice();
          showOutput();
          outputWin.webContents.send('chat:sync', { messages });
        },
        seedThinking: () => {
          outputWin.webContents.send('query:start', { gen: ++queryGen });
        },
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
  try {
    log('windows', overlayWin && outputWin ? 'overlay+output-ok' : 'missing');
    log('version', app.getVersion());
    log('shortcut.captureQuery', lastRegisterResults.captureQuery);
    log('shortcut.toggleVisibility', lastRegisterResults.toggleVisibility);
    log('model', store.getModel());
    const modelList = await ai.listModels();
    const tiers = modelList.groups.map((g) => `${g.tier}${g.models.length}`).join(',');
    log('modelList', `${tiers}:${modelList.live ? 'live' : 'fallback'}`);
    // Nothing paid may appear until it is asked for, and the default has to
    // be one of the free ones.
    log('modelListPaidHidden', modelList.groups.every((g) => g.tier === 'free'));
    const freeGroup = modelList.groups.find((g) => g.tier === 'free');
    log(
      'modelDefaultOffered',
      !!(freeGroup && freeGroup.models.some((m) => m.id === ai.DEFAULT_MODEL))
    );
    const paidList = await ai.listModels({ includePaid: true });
    log('modelListWithPaid', paidList.groups.map((g) => `${g.tier}${g.models.length}`).join(','));
    const brainList = await ai.listModels({ kind: 'text' });
    const brainFree = brainList.groups.find((g) => g.tier === 'free');
    log(
      'brainListDefault',
      !!(brainFree && brainFree.models.some((m) => m.id === ai.DEFAULT_BRAIN))
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
    // Every skill file parses and the chosen one reaches the system prompt.
    const skillList = skills.list();
    log('skills', `${skillList.length}:${skillList.map((s) => s.id).join('+')}`);
    const prompt = ai.makeSystemPrompt({
      imageWidth: 1280,
      imageHeight: 800,
      skill: skills.get(skills.route({ question: 'why did this fail' }).id),
    });
    log(
      'skillInPrompt',
      prompt.includes('Screen assistant') && prompt.includes('1280x800') ? 'ok' : 'missing'
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

    // Point directives split across deltas are stripped and collected.
    const ex = ai.createPointExtractor(() => {});
    ex.push('Press Save [[point x=1');
    ex.push('20 y=340 | Save button]] to finish.');
    ex.flush();
    log(
      'pointExtract',
      `${ex.points.length}:${ex.text.includes('[[') ? 'leaked' : 'clean'}`
    );
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
    setTimeout(() => {
      console.log('[SMOKE] done');
      app.exit(0);
    }, 1200);
  } catch (err) {
    console.error('[SMOKE] error', err);
    app.exit(1);
  }
}
