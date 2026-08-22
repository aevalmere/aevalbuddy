'use strict';

const {
  app,
  BrowserWindow,
  globalShortcut,
  ipcMain,
  screen,
  desktopCapturer,
  clipboard,
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
const cursorBuddy = require('./lib/cursor-buddy');

const PILL = { width: 72, height: 44 };
const PANEL_WIDTH = 420;
const PANEL_MIN_HEIGHT = 150;
const PANEL_MAX_HEIGHT = 600;
const DEFAULT_PANEL_HEIGHT = 170;
const HUD_SIZE = { width: PANEL_WIDTH, height: 230 };
const SNAP_MARGIN = 16;
const SNAP_DISTANCE = 110;
const DEFAULT_PROMPT = "What's on my screen and what should I do?";

const ACTION_LABELS = {
  captureQuery: 'Capture and ask',
  toggleVisibility: 'Show or hide overlay',
};

let overlayWin = null;
let settingsWin = null;
let overlayMode = 'pill'; // 'pill' | 'panel' | 'hud'
let overlayHidden = false;
let currentPanelHeight = DEFAULT_PANEL_HEIGHT;
let pendingShot = null;
let lastCaptureOk = true;
let currentAbort = null;
let queryGen = 0;
let captureModeActive = false;
let escapeRegistered = false;
let hudPendingPos = null;
let hudSnapInProgress = false;
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

// Every overlay window size is placed from the same stored anchor, so the
// pill, the panel, and the HUD ghost all hug the same corner or edge.
function boundsForSize(size, pos) {
  const wa = workArea();
  const p = pos || store.getOverlayPos();
  let x;
  if (p.h === 'left') x = wa.x + p.offX;
  else if (p.h === 'right') x = wa.x + wa.width - p.offX - size.width;
  else x = wa.x + Math.round((wa.width - size.width) / 2);
  let y;
  if (p.v === 'top') y = wa.y + p.offY;
  else if (p.v === 'bottom') y = wa.y + wa.height - p.offY - size.height;
  else y = wa.y + Math.round((wa.height - size.height) / 2);
  x = Math.max(wa.x, Math.min(x, wa.x + wa.width - size.width));
  y = Math.max(wa.y, Math.min(y, wa.y + wa.height - size.height));
  return { x, y, width: size.width, height: size.height };
}

function pillBounds() {
  return boundsForSize(PILL);
}

function panelBounds(height) {
  const h = Math.max(PANEL_MIN_HEIGHT, Math.min(PANEL_MAX_HEIGHT, Math.round(height)));
  return boundsForSize({ width: PANEL_WIDTH, height: h });
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

function findSnap(rect) {
  let best = null;
  for (const pos of snapAnchors()) {
    const target = boundsForSize(HUD_SIZE, pos);
    const dx = target.x + target.width / 2 - (rect.x + rect.width / 2);
    const dy = target.y + target.height / 2 - (rect.y + rect.height / 2);
    const dist = Math.hypot(dx, dy);
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

function createOverlay() {
  overlayWin = new BrowserWindow({
    ...pillBounds(),
    frame: false,
    transparent: true,
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    focusable: false,
    hasShadow: false,
    show: false,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });
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
  overlayWin.webContents.on('did-finish-load', sendOverlayState);
  attachContextMenu(overlayWin);
  overlayWin.on('moved', onOverlayMoved);
  overlayWin.on('closed', () => {
    overlayWin = null;
  });
}

function openSettings() {
  if (settingsWin && !settingsWin.isDestroyed()) {
    settingsWin.show();
    settingsWin.focus();
    cursorBuddy.notify('settings-opened');
    return;
  }
  settingsWin = new BrowserWindow({
    width: 470,
    height: 760,
    frame: false,
    transparent: true,
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

function expandPanel() {
  if (!overlayWin || overlayMode === 'hud') return;
  overlayMode = 'panel';
  overlayHidden = false;
  overlayWin.setIgnoreMouseEvents(false);
  overlayWin.setFocusable(true);
  overlayWin.setBounds(panelBounds(currentPanelHeight));
  // setFocusable can drop the z-order flag on Windows; reassert it.
  overlayWin.setAlwaysOnTop(true, 'screen-saver');
  overlayWin.show();
  overlayWin.focus();
  applyRegistrations();
  sendOverlayState();
}

function collapsePanel() {
  if (!overlayWin) return;
  abortCurrentQuery();
  overlayMode = 'pill';
  currentPanelHeight = DEFAULT_PANEL_HEIGHT;
  overlayWin.blur();
  overlayWin.setBounds(pillBounds());
  overlayWin.setFocusable(false);
  overlayWin.setAlwaysOnTop(true, 'screen-saver');
  overlayWin.setIgnoreMouseEvents(true, { forward: true });
  overlayWin.showInactive();
  applyRegistrations();
  sendOverlayState();
}

function sendOverlayState() {
  if (!overlayWin || overlayWin.isDestroyed()) return;
  overlayWin.webContents.send('overlay:state', {
    mode: overlayMode,
    hasKey: store.hasApiKey(),
    captureOk: lastCaptureOk,
  });
}

// ------------------------------------------------------------------ HUD edit

function enterHudEditor() {
  if (!overlayWin || overlayMode === 'hud') return;
  if (cursorBuddy.isActive()) cursorBuddy.cancel();
  abortCurrentQuery();
  overlayMode = 'hud';
  overlayHidden = false;
  hudPendingPos = store.getOverlayPos();
  overlayWin.setIgnoreMouseEvents(false);
  overlayWin.setFocusable(true);
  overlayWin.setMovable(true);
  overlayWin.setBounds(boundsForSize(HUD_SIZE));
  overlayWin.setAlwaysOnTop(true, 'screen-saver');
  overlayWin.show();
  overlayWin.focus();
  applyRegistrations();
  sendOverlayState();
}

function exitHudEditor() {
  if (!overlayWin || overlayMode !== 'hud') return;
  const pos = hudPendingPos || posFromRect(overlayWin.getBounds());
  store.setOverlayPos(pos);
  hudPendingPos = null;
  overlayWin.setMovable(false);
  collapsePanel();
  if (settingsWin && !settingsWin.isDestroyed()) settingsWin.focus();
}

// Fires when a drag of the HUD ghost ends (and after programmatic setBounds,
// hence the reentry guard). Snaps to the nearest of the nine anchors when
// close enough, otherwise keeps the free position.
function onOverlayMoved() {
  if (overlayMode !== 'hud' || hudSnapInProgress || !overlayWin) return;
  const rect = overlayWin.getBounds();
  const snap = findSnap(rect);
  if (snap) {
    hudSnapInProgress = true;
    overlayWin.setBounds(snap.rect);
    setImmediate(() => {
      hudSnapInProgress = false;
    });
    hudPendingPos = snap.pos;
    if (!overlayWin.isDestroyed()) {
      overlayWin.webContents.send('hud:snapped', {});
    }
  } else {
    hudPendingPos = posFromRect(rect);
  }
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
    return { base64: buf.toString('base64'), bytes: buf.length };
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
    overlayWin.hide();
  } else if (overlayMode === 'panel') {
    overlayWin.show();
    overlayWin.focus();
  } else {
    overlayWin.showInactive();
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
  const model = store.getModel();
  currentAbort = new AbortController();
  const signal = currentAbort.signal;
  const send = (channel, payload) => {
    if (overlayWin && !overlayWin.isDestroyed()) {
      overlayWin.webContents.send(channel, { gen, ...payload });
    }
  };
  try {
    await ai.streamQuery({
      apiKey: store.getApiKey(),
      model,
      question: q || DEFAULT_PROMPT,
      imageBase64: pendingShot ? pendingShot.base64 : null,
      signal,
      onDelta: (text) => send('query:delta', { text }),
    });
    send('query:done', {});
    return { status: 'done' };
  } catch (err) {
    const message = ai.friendlyError(err, model);
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
    currentPanelHeight = height;
    overlayWin.setBounds(panelBounds(height));
  });
  ipcMain.handle('overlay:set-ignore', (_e, { ignore }) => {
    if (!overlayWin || overlayMode !== 'pill') return;
    overlayWin.setIgnoreMouseEvents(ignore, { forward: true });
  });
  ipcMain.handle('overlay:open-settings', () => openSettings());

  ipcMain.handle('query:submit', (_e, { question }) => handleQuerySubmit(question));
  ipcMain.handle('query:cancel', () => abortCurrentQuery());

  ipcMain.handle('hud:enter', () => enterHudEditor());
  ipcMain.handle('hud:exit', () => exitHudEditor());

  ipcMain.handle('settings:get', () => ({
    hasKey: store.hasApiKey(),
    model: store.getModel(),
    defaultModel: ai.DEFAULT_MODEL,
    shortcuts: store.getShortcuts(),
    active: { ...lastRegisterResults },
    defaults: { ...store.DEFAULT_SHORTCUTS },
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

  ipcMain.handle('settings:save-model', (_e, { model }) => {
    const saved = store.setModel(model);
    return { ok: true, model: saved };
  });

  ipcMain.handle('models:list', async () => {
    const result = await ai.listFreeVisionModels();
    return { ...result, current: store.getModel(), defaultModel: ai.DEFAULT_MODEL };
  });

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
      !process.env.AEVAL_SMOKE
    ) {
      store.setWalkthroughAutoShown();
      setTimeout(startWalkthrough, 1000);
    }
    if (process.env.AEVAL_SMOKE) {
      setTimeout(runSmoke, 800);
    }
  });

  app.on('window-all-closed', () => {
    app.quit();
  });

  app.on('will-quit', () => {
    globalShortcut.unregisterAll();
  });
}

// Headless self-check used by automated verification: boots the real app,
// exercises capture, shortcut registration, panel expansion, and the HUD
// editor round trip, prints machine-readable lines, then exits.
async function runSmoke() {
  const log = (label, value) => console.log(`[SMOKE] ${label}=${value}`);
  try {
    log('windows', overlayWin && !overlayWin.isDestroyed() ? 'overlay-ok' : 'overlay-missing');
    log('shortcut.captureQuery', lastRegisterResults.captureQuery);
    log('shortcut.toggleVisibility', lastRegisterResults.toggleVisibility);
    log('model', store.getModel());
    const modelList = await ai.listFreeVisionModels();
    log('modelList', `${modelList.models.length}:${modelList.live ? 'live' : 'fallback'}`);
    const shot = await captureScreen();
    log('capture', shot ? `ok:${shot.bytes}bytes` : 'failed');
    await triggerCapture();
    log('mode', overlayMode);
    log('hasKey', store.hasApiKey());
    const submitResult = await handleQuerySubmit('smoke question');
    log('queryWithoutKey', submitResult.status);
    enterHudEditor();
    log('hudMode', overlayMode);
    const snap = findSnap(boundsForSize(HUD_SIZE, { h: 'left', v: 'top', offX: 30, offY: 30 }));
    log('hudSnapFound', snap ? `${snap.pos.h}-${snap.pos.v}` : 'none');
    exitHudEditor();
    log('hudExitMode', overlayMode);
    log('storedPos', JSON.stringify(store.getOverlayPos()));
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
