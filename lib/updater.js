'use strict';

// Auto-update, by way of GitHub releases.
//
// The installer in dist/ was a frozen snapshot: whoever installed 0.7.0 kept
// 0.7.0 until they went looking for a newer file. This module makes the
// installed copy look for itself. electron-updater reads latest.yml from the
// newest GitHub release, compares it with app.getVersion(), downloads the
// NSIS installer in the background, and swaps it in on the next quit.
//
// Nothing here interrupts. A check runs shortly after launch and then every
// few hours; when a download has finished the tray grows a "Restart to
// update" item and one notification says so. The user restarts when they
// like, or never, in which case the update is applied the next time the app
// quits anyway.
//
// Unpackaged (npm start), there is no installer to replace and the updater
// would only throw, so every call becomes a no-op that reports 'dev'.

const { app, Notification } = require('electron');

const FIRST_CHECK_MS = 20 * 1000;
const RECHECK_MS = 4 * 60 * 60 * 1000;

let updater = null;
let deps = null;
let state = { status: 'idle', version: '', error: '' };
let timer = null;
let notified = false;

function set(next) {
  state = { ...state, ...next };
  if (deps && deps.onChange) deps.onChange(get());
}

/** @returns {{status: string, version: string, error: string, current: string}} */
function get() {
  return { ...state, current: app.getVersion() };
}

function load() {
  if (updater) return updater;
  // electron-updater is only reachable in a packaged build; requiring it
  // under npm start is fine but pointless, so the require stays lazy.
  const { autoUpdater } = require('electron-updater');
  autoUpdater.autoDownload = true;
  autoUpdater.autoInstallOnAppQuit = true;
  // Logs land beside errors.log, so a failed update can be read after the fact.
  autoUpdater.logger = {
    info: () => {},
    warn: (m) => console.warn('[aevalbuddy] update:', m),
    error: (m) => console.error('[aevalbuddy] update:', m),
    debug: () => {},
  };
  autoUpdater.on('checking-for-update', () => set({ status: 'checking', error: '' }));
  autoUpdater.on('update-not-available', () => set({ status: 'current', error: '' }));
  autoUpdater.on('update-available', (info) =>
    set({ status: 'downloading', version: info.version, error: '' })
  );
  autoUpdater.on('update-downloaded', (info) => {
    set({ status: 'ready', version: info.version, error: '' });
    announce(info.version);
  });
  autoUpdater.on('error', (err) =>
    set({ status: 'error', error: (err && err.message) || String(err) })
  );
  updater = autoUpdater;
  return updater;
}

function announce(version) {
  if (notified) return;
  notified = true;
  try {
    if (!Notification.isSupported()) return;
    const n = new Notification({
      title: `AevalBuddy ${version} is ready`,
      body: 'Restart from the tray icon to update. It also applies on the next quit.',
      silent: true,
    });
    n.on('click', () => deps && deps.restart && deps.restart());
    n.show();
  } catch (err) {
    // A notification is a courtesy; the tray item carries the same message.
  }
}

/**
 * @param {object} d
 * @param {(state: object) => void} d.onChange  tray and settings refresh from this
 * @param {() => void} [d.restart]
 */
function init(d) {
  deps = d;
  if (!app.isPackaged) {
    set({ status: 'dev' });
    return;
  }
  timer = setTimeout(() => {
    check();
    timer = setInterval(check, RECHECK_MS);
  }, FIRST_CHECK_MS);
}

/** One check, now. Resolves to the state once the check has settled. */
async function check() {
  if (!app.isPackaged) return get();
  if (state.status === 'ready' || state.status === 'downloading') return get();
  try {
    await load().checkForUpdates();
  } catch (err) {
    set({ status: 'error', error: (err && err.message) || String(err) });
  }
  return get();
}

/** Quit and run the downloaded installer. Only meaningful when status is 'ready'. */
function restart() {
  if (state.status !== 'ready' || !updater) return false;
  // isSilent, isForceRunAfter: the one-click installer runs without a dialog
  // and relaunches the app when it is done.
  setImmediate(() => updater.quitAndInstall(true, true));
  return true;
}

function isReady() {
  return state.status === 'ready';
}

function destroy() {
  if (timer) {
    clearTimeout(timer);
    clearInterval(timer);
    timer = null;
  }
}

module.exports = { init, check, restart, get, isReady, destroy };
