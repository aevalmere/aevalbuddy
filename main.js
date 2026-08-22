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

const PILL = { width: 72, height: 44 };
const PANEL_WIDTH = 420;
const INPUT_MIN_HEIGHT = 62;
const INPUT_MAX_HEIGHT = 280;
const DEFAULT_INPUT_HEIGHT = 76;
const OUTPUT_MIN_HEIGHT = 90;
const OUTPUT_MAX_HEIGHT = 620;
const DEFAULT_OUTPUT_HEIGHT = 120;
const HUD_INPUT_SIZE = { width: PANEL_WIDTH, height: 140 };
const HUD_OUTPUT_SIZE = { width: PANEL_WIDTH, height: 200 };
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
  return boundsForSize({ width: PANEL_WIDTH, height: h }, store.getOverlayPos());
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
  const h = clampNum(Math.round(height), OUTPUT_MIN_HEIGHT, OUTPUT_MAX_HEIGHT);
  const size = { width: PANEL_WIDTH, height: h };
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

// Windows 11 can put real acrylic behind a window. That matters because CSS
// backdrop-filter cannot see past a window boundary: a transparent overlay
// has nothing to blur, so whatever is underneath shows through sharp, which
// is what made these surfaces read as stickers rather than glass. Acrylic
// needs transparent:false, and in exchange DWM supplies the rounded corner
// the CSS radius used to provide. Windows 10 ignores backgroundMaterial and
// would just get an opaque square, so it keeps the transparent recipe.
const WIN11_BUILD = 22000;

function supportsAcrylic() {
  if (process.platform !== 'win32') return false;
  const build = Number(String(process.getSystemVersion() || '').split('.')[2]);
  return Number.isFinite(build) && build >= WIN11_BUILD;
}

const ACRYLIC = supportsAcrylic();

function surfaceOptions() {
  return ACRYLIC
    ? {
        transparent: false,
        backgroundColor: '#00000000',
        backgroundMaterial: 'acrylic',
        // An opaque window can take the real system drop shadow, which
        // separates it from the desktop better than any CSS one.
        hasShadow: true,
      }
    : { transparent: true, hasShadow: false };
}

function baseWindowOptions() {
  return {
    frame: false,
    ...surfaceOptions(),
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    show: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  };
}

function createOverlay() {
  overlayWin = new BrowserWindow({ ...pillBounds(), ...baseWindowOptions(), focusable: false });
  overlayWin.setAlwaysOnTop(true, 'screen-saver');
  // Core Cluely-defining feature: exclude the overlay from screen capture.
  // On Windows this maps to WDA_EXCLUDEFROMCAPTURE and is reliable on
  // Windows 10 2004+. Some capture backends (older OS builds, some hardware
  // capture cards) may still see the window; the app keeps working either
  // way, the overlay is just visible in those recordings.
  overlayWin.setContentProtection(true);
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
  overlayWin.on('closed', () => {
    overlayWin = null;
  });
}

function createOutput() {
  outputWin = new BrowserWindow({
    ...boundsForSize({ width: PANEL_WIDTH, height: DEFAULT_OUTPUT_HEIGHT }, store.getOverlayPos()),
    ...baseWindowOptions(),
    focusable: true,
  });
  outputWin.setAlwaysOnTop(true, 'screen-saver');
  outputWin.setContentProtection(true);
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
  outputWin.on('closed', () => {
    outputWin = null;
  });
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
  settingsWin = new BrowserWindow({
    width: 460,
    height: clampNum(700, 420, wa.height - 40),
    frame: false,
    ...surfaceOptions(),
    resizable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    show: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
  // The key-entry screen stays out of screen shares as well.
  settingsWin.setContentProtection(true);
  attachContextMenu(settingsWin);
  settingsWin.loadFile(path.join(__dirname, 'settings', 'settings.html'));
  settingsWin.once('ready-to-show', () => {
    settingsWin.show();
    cursorBuddy.notify('settings-opened');
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
  overlayWin.setAlwaysOnTop(true, 'screen-saver');
  sendOverlayState();
  const to = inputBounds(currentInputHeight);
  if (wasPill) {
    // The renderer measures itself while the glide is running, so its height
    // report is applied once the glide lands rather than fighting it.
    motion.animate(overlayWin, { from, to, duration: T_EXPAND, onDone: syncInputBounds });
  } else {
    motion.stop(overlayWin);
    overlayWin.setBounds(to);
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
  overlayWin.setAlwaysOnTop(true, 'screen-saver');
  overlayWin.setIgnoreMouseEvents(true, { forward: true });
  overlayWin.showInactive();
  const to = pillBounds();
  if (wasPanel) {
    // The renderer keeps showing the panel until the frame is nearly pill
    // sized, so the contents shrink away instead of vanishing at frame one.
    motion.animate(overlayWin, {
      to,
      duration: T_COLLAPSE,
      onDone: sendOverlayState,
    });
  } else {
    motion.stop(overlayWin);
    overlayWin.setBounds(to);
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
  outputWin.setAlwaysOnTop(true, 'screen-saver');
  const to = outputBounds(currentOutputHeight);
  if (!reopening) {
    resizeOutput(to);
    return;
  }
  const op = store.getOutputPos();
  const anchorV = op.docked ? store.getOverlayPos().v : op.v;
  const anchorH = op.docked ? store.getOverlayPos().h : op.h;
  outputWin.setOpacity(0);
  outputWin.setBounds(motion.shrunkFrom(to, anchorV, anchorH));
  outputWin.showInactive();
  motion.animate(outputWin, {
    to,
    duration: T_OUT_IN,
    opacityFrom: 0,
    opacityTo: 1,
    onDone: syncOutputBounds,
  });
}

// Applies a height the renderer reported while a glide was in flight.
function syncInputBounds() {
  if (overlayMode !== 'panel' || !overlayWin || overlayWin.isDestroyed()) return;
  const want = inputBounds(currentInputHeight);
  const have = overlayWin.getBounds();
  if (have.height !== want.height || have.y !== want.y) overlayWin.setBounds(want);
  repositionOutputIfDocked();
}

function syncOutputBounds() {
  if (!outputActive || !outputWin || outputWin.isDestroyed()) return;
  const want = outputBounds(currentOutputHeight);
  const have = outputWin.getBounds();
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
  const rect = outputWin.getBounds();
  const op = store.getOutputPos();
  const anchorV = op.docked ? store.getOverlayPos().v : op.v;
  const anchorH = op.docked ? store.getOverlayPos().h : op.h;
  motion.animate(outputWin, {
    to: motion.shrunkFrom(rect, anchorV, anchorH),
    duration: T_OUT_OUT,
    opacityFrom: 1,
    opacityTo: 0,
    onDone: () => {
      if (outputWin && !outputWin.isDestroyed()) {
        outputWin.hide();
        outputWin.setBounds(rect);
        outputWin.setOpacity(1);
      }
    },
  });
}

// Growing during a conversation glides too, so a long answer does not make
// the window edge jump.
function resizeOutput(to) {
  if (!outputWin || outputWin.isDestroyed()) return;
  motion.animate(outputWin, { to, duration: T_OUT_GROW });
}

function repositionOutputIfDocked() {
  if (!outputActive || !store.getOutputPos().docked) return;
  if (!outputWin || outputWin.isDestroyed()) return;
  // An answer window that is still gliding open must not be cut short here:
  // its own landing calls syncOutputBounds, which recomputes the docked
  // position against wherever the input bar ended up.
  if (motion.isAnimating(outputWin)) return;
  // Otherwise following the input bar is immediate; a glide would lag.
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
    acrylic: ACRYLIC,
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
  if (cursorBuddy.isActive()) cursorBuddy.cancel();
  abortCurrentQuery();
  // Layout editing is a direct-manipulation mode; no glide may be left
  // running to move a ghost the user is dragging.
  motion.stop(overlayWin);
  motion.stop(outputWin);
  overlayWin.setOpacity(1);
  outputWin.setOpacity(1);
  overlayMode = 'hud';
  overlayHidden = false;
  outputActive = false;
  hudInputPending = store.getOverlayPos();
  hudOutputPending = store.getOutputPos();

  overlayWin.setIgnoreMouseEvents(false);
  overlayWin.setFocusable(true);
  overlayWin.setMovable(true);
  overlayWin.setBounds(boundsForSize(HUD_INPUT_SIZE, hudInputPending));
  overlayWin.setAlwaysOnTop(true, 'screen-saver');
  overlayWin.show();
  overlayWin.focus();

  outputWin.setMovable(true);
  outputWin.setBounds(hudOutputGhostBounds());
  outputWin.setAlwaysOnTop(true, 'screen-saver');
  outputWin.showInactive();

  applyRegistrations();
  sendOverlayState();
  outputWin.webContents.send('hud:docked', { docked: hudOutputPending.docked });
}

function hudOutputGhostBounds() {
  if (hudOutputPending.docked && overlayWin && !overlayWin.isDestroyed()) {
    return dockedBounds(overlayWin.getBounds(), HUD_OUTPUT_SIZE, hudInputPending.v);
  }
  return boundsForSize(HUD_OUTPUT_SIZE, hudOutputPending);
}

function exitHudEditor() {
  if (!overlayWin || overlayMode !== 'hud') return;
  store.setOverlayPos(hudInputPending || posFromRect(overlayWin.getBounds()));
  store.setOutputPos(hudOutputPending || store.getOutputPos());
  hudInputPending = null;
  hudOutputPending = null;
  overlayWin.setMovable(false);
  if (outputWin && !outputWin.isDestroyed()) {
    outputWin.setMovable(false);
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
  const snap = findSnap(rect, HUD_INPUT_SIZE);
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
  const dockSlot = dockedBounds(overlayWin.getBounds(), HUD_OUTPUT_SIZE, hudInputPending.v);
  if (centerDistance(dockSlot, rect) <= DOCK_DISTANCE) {
    withSnapGuard(() => outputWin.setBounds(dockSlot));
    hudOutputPending = { ...hudOutputPending, docked: true };
    outputWin.webContents.send('hud:docked', { docked: true });
    outputWin.webContents.send('hud:snapped', {});
    return;
  }
  const snap = findSnap(rect, HUD_OUTPUT_SIZE);
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
    withSnapGuard(() => outputWin.setBounds(boundsForSize(HUD_OUTPUT_SIZE, hudOutputPending)));
  }
  outputWin.webContents.send('hud:docked', { docked: hudOutputPending.docked });
  outputWin.webContents.send('hud:snapped', {});
}

// ------------------------------------------------------------- screen capture

async function captureScreen() {
  try {
    const primary = screen.getPrimaryDisplay();
    const { width, height } = primary.size;
    const scale = Math.min(1, 1280 / width);
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
    // Step quality down until the payload is comfortably under 1MB so the
    // API round trip stays fast.
    let quality = 70;
    let buf = source.thumbnail.toJPEG(quality);
    while (buf.length > 950 * 1024 && quality > 35) {
      quality -= 15;
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
    // streamed, so the user watches the thinking dots through it; saying so
    // is better than an unexplained pause.
    let screenReport = null;
    if (useBrain && shot) {
      send('query:stage', { label: 'Reading your screen' });
      screenReport = await ai.readScreen({
        apiKey: store.getApiKey(),
        model,
        imageBase64: shot.base64,
        imageWidth: shot.width,
        imageHeight: shot.height,
        question: text,
        signal,
      });
      answering = store.getBrain();
      send('query:stage', { label: 'Thinking' });
    }
    await ai.streamQuery({
      apiKey: store.getApiKey(),
      model: answering,
      question: text,
      screenReport,
      imageBase64: screenReport || !shot ? null : shot.base64,
      imageWidth: shot ? shot.width : 0,
      imageHeight: shot ? shot.height : 0,
      history,
      // The answering procedure travels with every request, which is what
      // holds a small fast model to a useful answer shape.
      skill: skills.get(store.getSkillId()),
      signal,
      onDelta: (t) => extractor.push(t),
    });
    extractor.flush();
    chat.push({ role: 'assistant', text: extractor.text });
    send('query:done', {});
    showModelPoints(extractor.points, shot);
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
  }
}

// Maps directive coordinates from image pixels to screen coordinates and
// hands them to the pointer. Ratio mapping sidesteps DPI scale factors.
function showModelPoints(points, shot) {
  if (!points || !points.length || !shot || !shot.width || !shot.height) return;
  if (overlayMode === 'hud') return;
  const b = shot.display;
  const mapped = points
    .filter((p) => p.x >= 0 && p.y >= 0 && p.x <= shot.width && p.y <= shot.height)
    .map((p) => ({
      x: Math.round(b.x + (p.x / shot.width) * b.width),
      y: Math.round(b.y + (p.y / shot.height) * b.height),
      label: p.label || 'Here',
    }));
  if (!mapped.length) return;
  cursorBuddy.showPoints(mapped);
  applyRegistrations();
}

function resetChat() {
  abortCurrentQuery();
  chat = [];
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
    overlayWin.setBounds(inputBounds(height));
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
    acrylic: ACRYLIC,
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
          evalIn(settingsWin, `document.getElementById('model-combo-btn').click(); true`),
        enableBrain: () =>
          evalIn(
            settingsWin,
            `(() => { const c = document.getElementById('brain-enabled');` +
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
      skill: skills.get(store.getSkillId()),
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
      skill: skills.get(store.getSkillId()),
    });
    log(
      'skillInPrompt',
      prompt.includes('Screen assistant') && prompt.includes('1280x800') ? 'ok' : 'missing'
    );
    log('skillFallback', skills.get('no-such-skill').id);
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
    log('acrylic', ACRYLIC);
    log(
      'markdown',
      mdHtml.includes('<li>') && mdHtml.includes('<strong>') && mdHtml.includes('md-h')
        ? 'ok'
        : `unexpected:${mdHtml.slice(0, 80)}`
    );
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
      boundsForSize(HUD_INPUT_SIZE, { h: 'left', v: 'top', offX: 30, offY: 30 }),
      HUD_INPUT_SIZE
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
