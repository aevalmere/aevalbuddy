'use strict';

// Guided-walkthrough pointer. Draws an animated dot in its own transparent,
// click-through BrowserWindow layered above the app windows, and walks through
// a list of steps: { win: 'overlay'|'settings', targetSelector, message,
// advanceOn }. It never moves the real OS cursor. Steps advance on named app
// events (cursorBuddy.notify('key-saved')), and the whole walkthrough can be
// dismissed at any time with Escape or a click away from the target.

const { BrowserWindow, screen } = require('electron');
const path = require('path');

const POLL_MS = 150;

let deps = null;
let buddyWin = null;
let steps = [];
let index = -1;
let pollTimer = null;
let active = false;
let lastSent = null;
// 'steps' walks selectors inside app windows; 'points' shows fixed screen
// coordinates handed over by the model, advanced with the callout button.
let mode = 'steps';
let pointList = [];
let pointIndex = 0;

/**
 * @param {object} d
 * @param {(name: string) => BrowserWindow|null} d.getWindowFor
 * @param {(step: object) => void} d.ensureStepPrereq puts the app in a state
 *        where the step target can exist (e.g. expands the overlay panel)
 * @param {() => void} [d.onDone]
 * @param {() => void} [d.onCancel]
 */
function init(d) {
  deps = d;
}

function isActive() {
  return active;
}

function start(walkthroughSteps) {
  if (!deps) return;
  if (active) cancel();
  mode = 'steps';
  steps = walkthroughSteps;
  active = true;
  createWindow();
  goTo(0);
}

/**
 * Shows model-directed points, one at a time: { x, y, label } in screen
 * coordinates. The callout advances through them and dismisses like the
 * walkthrough (close button, Escape, or finishing the sequence).
 */
function showPoints(points) {
  if (!deps || !Array.isArray(points) || !points.length) return;
  if (active) cancel();
  mode = 'points';
  pointList = points.slice(0, 3);
  pointIndex = 0;
  active = true;
  lastSent = null;
  createWindow();
  // A freshly created window has not loaded its renderer yet; sending
  // buddy:show before that would vanish.
  if (buddyWin.webContents.isLoading()) {
    buddyWin.webContents.once('did-finish-load', showCurrentPoint);
  } else {
    showCurrentPoint();
  }
}

function showCurrentPoint() {
  if (!active || mode !== 'points' || !buddyWin || buddyWin.isDestroyed()) return;
  const p = pointList[pointIndex];
  const bounds = buddyWin.getBounds();
  buddyWin.webContents.send('buddy:show', {
    kind: 'point',
    x: Math.round(p.x - bounds.x),
    y: Math.round(p.y - bounds.y),
    targetW: 28,
    targetH: 28,
    message: p.label || 'Here',
    stepIndex: pointIndex,
    stepCount: pointList.length,
  });
  buddyWin.moveTop();
}

// The callout's action button: next point, or done after the last one.
function advance() {
  if (!active || mode !== 'points') return;
  if (pointIndex + 1 < pointList.length) {
    pointIndex += 1;
    showCurrentPoint();
  } else {
    complete();
  }
}

function goTo(i) {
  index = i;
  lastSent = null;
  const step = steps[index];
  deps.ensureStepPrereq(step);
  broadcastWatch();
  clearInterval(pollTimer);
  pollTimer = setInterval(resolveTarget, POLL_MS);
  resolveTarget();
}

// Tells each app window which selector counts as "on the walkthrough path",
// so its renderer can dismiss on click-away without cancelling when the user
// clicks the control the step is asking them to use. clickScope may be wider
// than the pointer target (e.g. the whole API-key section).
function broadcastWatch() {
  const step = active && index >= 0 ? steps[index] : null;
  for (const name of ['overlay', 'settings']) {
    const win = deps.getWindowFor(name);
    if (!win || win.isDestroyed()) continue;
    const selector =
      step && step.win === name ? step.clickScope || step.targetSelector : null;
    win.webContents.send('buddy:watch', { selector });
  }
}

function resolveTarget() {
  if (!active) return;
  const step = steps[index];
  const win = deps.getWindowFor(step.win);
  if (!win || win.isDestroyed()) {
    hidePointer();
    return;
  }
  win.webContents.send('buddy:resolve', { selector: step.targetSelector });
}

// Called by main with the renderer's answer to buddy:resolve. Coordinates
// arrive in screen space; the buddy window converts to its own space. The
// poll runs several times a second, so identical frames are dropped here
// and only real movement or a new message reaches the renderer.
function handleResolveResult(result) {
  if (!active || !buddyWin || buddyWin.isDestroyed()) return;
  if (!result || !result.found) {
    hidePointer();
    return;
  }
  const bounds = buddyWin.getBounds();
  const payload = {
    kind: 'step',
    x: Math.round(result.cx - bounds.x),
    y: Math.round(result.cy - bounds.y),
    targetW: Math.round(result.w),
    targetH: Math.round(result.h),
    message: steps[index].message,
  };
  const key = `${payload.x}|${payload.y}|${payload.targetW}|${payload.targetH}|${payload.message}`;
  if (key === lastSent) return;
  lastSent = key;
  buddyWin.webContents.send('buddy:show', payload);
  buddyWin.moveTop();
}

function hidePointer() {
  lastSent = null;
  if (buddyWin && !buddyWin.isDestroyed()) {
    buddyWin.webContents.send('buddy:hide');
  }
}

// Lets the callout's close button receive clicks: the window stays
// click-through except while the cursor hovers the callout.
function setClickThrough(ignore) {
  if (buddyWin && !buddyWin.isDestroyed()) {
    buddyWin.setIgnoreMouseEvents(!!ignore, { forward: true });
  }
}

/**
 * App events flow through here: the current step advances when its advanceOn
 * event fires, 'dismiss' cancels, and closing settings mid-walkthrough falls
 * back to the first step instead of stranding the pointer.
 */
function notify(eventName) {
  if (!active) return;
  if (eventName === 'dismiss') {
    cancel();
    return;
  }
  // Point mode only reacts to dismissal and its own advance button.
  if (mode !== 'steps') return;
  const step = steps[index];
  if (eventName === step.advanceOn) {
    if (index + 1 < steps.length) {
      goTo(index + 1);
    } else {
      complete();
    }
    return;
  }
  if (eventName === 'settings-closed' && step.win === 'settings' && index > 0) {
    goTo(0);
  }
}

function complete() {
  if (!active) return;
  active = false;
  clearInterval(pollTimer);
  broadcastWatchCleared();
  if (buddyWin && !buddyWin.isDestroyed()) {
    buddyWin.webContents.send('buddy:done');
  }
  setTimeout(destroyWindow, 700);
  if (deps.onDone) deps.onDone();
}

function cancel() {
  if (!active) return;
  active = false;
  clearInterval(pollTimer);
  broadcastWatchCleared();
  destroyWindow();
  if (deps.onCancel) deps.onCancel();
}

function broadcastWatchCleared() {
  for (const name of ['overlay', 'settings']) {
    const win = deps.getWindowFor(name);
    if (win && !win.isDestroyed()) {
      win.webContents.send('buddy:watch', { selector: null });
    }
  }
}

function createWindow() {
  if (buddyWin && !buddyWin.isDestroyed()) return;
  const display = screen.getPrimaryDisplay().bounds;
  buddyWin = new BrowserWindow({
    x: display.x,
    y: display.y,
    width: display.width,
    height: display.height,
    frame: false,
    transparent: true,
    resizable: false,
    movable: false,
    focusable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    hasShadow: false,
    show: true,
    webPreferences: {
      preload: path.join(__dirname, '..', 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
      // The buddy window is never focused; without this its animations stall.
      backgroundThrottling: false,
    },
  });
  buddyWin.setIgnoreMouseEvents(true, { forward: true });
  buddyWin.setAlwaysOnTop(true, 'screen-saver');
  // Keeps the walkthrough out of screen shares too. Reliable on Windows
  // 10 2004+; other capture backends may still see it, which is harmless here.
  buddyWin.setContentProtection(true);
  buddyWin.loadFile(path.join(__dirname, '..', 'renderer', 'buddy.html'));
  buddyWin.on('closed', () => {
    buddyWin = null;
  });
}

function destroyWindow() {
  if (buddyWin && !buddyWin.isDestroyed()) {
    buddyWin.destroy();
  }
  buddyWin = null;
}

// The visual self-check needs the pointer window to photograph it.
function debugWindow() {
  return buddyWin && !buddyWin.isDestroyed() ? buddyWin : null;
}

module.exports = {
  init,
  start,
  showPoints,
  debugWindow,
  advance,
  cancel,
  notify,
  isActive,
  handleResolveResult,
  setClickThrough,
};
