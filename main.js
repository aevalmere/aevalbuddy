'use strict';

const {
  app,
  BrowserWindow,
  globalShortcut,
  ipcMain,
  screen,
  desktopCapturer,
} = require('electron');
const path = require('path');

const store = require('./lib/store');
const claude = require('./lib/claude');
const cursorBuddy = require('./lib/cursor-buddy');

const MARGIN = 16;
const PILL = { width: 72, height: 44 };
const PANEL_WIDTH = 420;
const PANEL_MIN_HEIGHT = 150;
const PANEL_MAX_HEIGHT = 600;
const DEFAULT_PANEL_HEIGHT = 170;
const DEFAULT_PROMPT = "What's on my screen and what should I do?";

const ACTION_LABELS = {
  captureQuery: 'Capture and ask',
  toggleVisibility: 'Show or hide overlay',
};

let overlayWin = null;
let settingsWin = null;
let overlayMode = 'pill';
let overlayHidden = false;
let currentPanelHeight = DEFAULT_PANEL_HEIGHT;
let pendingShot = null;
let lastCaptureOk = true;
let currentAbort = null;
let queryGen = 0;
let captureModeActive = false;
let escapeRegistered = false;
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
    message: 'Paste your Anthropic API key, then press Save key',
    advanceOn: 'key-saved',
  },
];

// ---------------------------------------------------------------- positioning

function workArea() {
  return screen.getPrimaryDisplay().workArea;
}

function pillBounds() {
  const wa = workArea();
  return {
    x: wa.x + wa.width - PILL.width - MARGIN,
    y: wa.y + MARGIN,
    width: PILL.width,
    height: PILL.height,
  };
}

function panelBounds(height) {
  const wa = workArea();
  const h = Math.max(PANEL_MIN_HEIGHT, Math.min(PANEL_MAX_HEIGHT, Math.round(height)));
  return {
    x: wa.x + wa.width - PANEL_WIDTH - MARGIN,
    y: wa.y + MARGIN,
    width: PANEL_WIDTH,
    height: h,
  };
}

// -------------------------------------------------------------------- windows

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
    height: 640,
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
  if (!overlayWin) return;
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
    shortcuts: store.getShortcuts(),
    active: { ...lastRegisterResults },
  });
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
  if (!overlayWin) return;
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
  if (!overlayWin) return;
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
  if (settingsWin && !settingsWin.isDestroyed() && settingsWin.isFocused()) {
    settingsWin.close();
    return;
  }
  if (overlayMode === 'panel') {
    collapsePanel();
  }
}

// Re-derives every global shortcut from current state. Escape is registered
// globally only while the panel or walkthrough is open, so the app never
// swallows Escape system-wide while idle. Everything is suspended while the
// settings panel is recording a new combo.
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
  if (overlayMode === 'panel' || cursorBuddy.isActive()) {
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
  currentAbort = new AbortController();
  const signal = currentAbort.signal;
  const send = (channel, payload) => {
    if (overlayWin && !overlayWin.isDestroyed()) {
      overlayWin.webContents.send(channel, { gen, ...payload });
    }
  };
  try {
    await claude.streamQuery({
      apiKey: store.getApiKey(),
      question: q || DEFAULT_PROMPT,
      imageBase64: pendingShot ? pendingShot.base64 : null,
      signal,
      onDelta: (text) => send('query:delta', { text }),
    });
    send('query:done', {});
    return { status: 'done' };
  } catch (err) {
    const message = claude.friendlyError(err);
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
  ipcMain.handle('overlay:collapse', () => collapsePanel());
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

  ipcMain.handle('settings:get', () => ({
    hasKey: store.hasApiKey(),
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
    sendOverlayState();
    return {
      ok: true,
      shortcuts: store.getShortcuts(),
      active: { ...lastRegisterResults },
    };
  });

  ipcMain.handle('shortcuts:reset', () => {
    const shortcuts = store.resetShortcuts();
    applyRegistrations();
    sendOverlayState();
    return { ok: true, shortcuts, active: { ...lastRegisterResults } };
  });

  ipcMain.handle('buddy:resolve-result', (_e, payload) => {
    cursorBuddy.handleResolveResult(payload);
  });
  ipcMain.handle('buddy:dismiss', () => cursorBuddy.notify('dismiss'));

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
    if (!store.hasApiKey() && !process.env.AEVAL_SMOKE) {
      // First launch with no key: the cursor buddy walks the user to settings.
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
// exercises capture, shortcut registration, and the query-panel expansion,
// prints machine-readable lines, then exits.
async function runSmoke() {
  const log = (label, value) => console.log(`[SMOKE] ${label}=${value}`);
  try {
    log('windows', overlayWin && !overlayWin.isDestroyed() ? 'overlay-ok' : 'overlay-missing');
    log('shortcut.captureQuery', lastRegisterResults.captureQuery);
    log('shortcut.toggleVisibility', lastRegisterResults.toggleVisibility);
    const shot = await captureScreen();
    log('capture', shot ? `ok:${shot.bytes}bytes` : 'failed');
    await triggerCapture();
    log('mode', overlayMode);
    log('hasKey', store.hasApiKey());
    const submitResult = await handleQuerySubmit('smoke question');
    log('queryWithoutKey', submitResult.status);
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
