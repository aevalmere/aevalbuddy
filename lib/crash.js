'use strict';

// Why an overlay needs this more than an ordinary application does.
//
// Every window here is a Windows tool window with skipTaskbar set: no taskbar
// button, nothing in Alt-Tab. When an ordinary app throws, its button goes
// dark and its window stays on screen carrying the error. When this one
// throws, there is nothing to notice. The app is simply not there any more,
// and the user is left pressing a shortcut that does nothing and wondering
// whether they ever started it. That is the same failure the tray icon was
// added to fix, arriving by a different route.
//
// So an unhandled error is written to a log beside the settings, announced
// once through a notification, and the app is kept alive. Node is right that
// a process is in an undefined state after an uncaught exception, but for
// this app the alternative is not a clean restart, it is disappearing.
//
// A renderer that dies is reloaded, because an always-on-top blank rectangle
// is worse than no window at all.

const { app, Notification } = require('electron');
const fs = require('fs');
const path = require('path');

const MAX_BYTES = 256 * 1024;

let logPath = null;
let announced = false;

function file() {
  if (!logPath) logPath = path.join(app.getPath('userData'), 'errors.log');
  return logPath;
}

function write(scope, err) {
  const stamp = new Date().toISOString();
  const detail = err && err.stack ? err.stack : String(err);
  try {
    const p = file();
    // Keep the tail rather than the head: the newest crash is the one being
    // read, and an unbounded log on a loop that throws every frame is its own
    // bug report.
    if (fs.existsSync(p) && fs.statSync(p).size > MAX_BYTES) {
      fs.writeFileSync(p, fs.readFileSync(p, 'utf8').slice(-Math.floor(MAX_BYTES / 2)));
    }
    fs.appendFileSync(p, `[${stamp}] ${scope}\n${detail}\n\n`);
  } catch (err2) {
    // The log is a convenience; failing to write it must not become the
    // second unhandled error.
  }
  console.error(`[aevalbuddy] ${scope}:`, detail);
}

// Once per session. Something that throws on every frame must not become
// something that notifies on every frame.
function announce() {
  if (announced) return;
  announced = true;
  try {
    if (!Notification.isSupported()) return;
    new Notification({
      title: 'AevalBuddy hit an error',
      body: 'It is still running. The details were written to errors.log in its settings folder.',
      silent: true,
    }).show();
  } catch (err) {
    // A notification that cannot be shown is not worth crashing over.
  }
}

/** Catch what would otherwise end the process without a trace. */
function install() {
  process.on('uncaughtException', (err) => {
    write('uncaughtException', err);
    announce();
  });
  process.on('unhandledRejection', (reason) => {
    write('unhandledRejection', reason);
    announce();
  });
}

/**
 * Reload a window whose renderer died. Wired through
 * app.on('browser-window-created'), so it covers the windows the lib modules
 * make as well as the ones main.js makes, including any added later.
 */
function guard(win) {
  if (!win || win.isDestroyed()) return;
  win.webContents.on('render-process-gone', (event, details) => {
    const reason = (details && details.reason) || 'unknown';
    write('render-process-gone', new Error(reason));
    // A clean exit is the window being closed on purpose.
    if (reason === 'clean-exit' || win.isDestroyed()) return;
    announce();
    try {
      win.reload();
    } catch (err) {
      write('reload-failed', err);
    }
  });
}

module.exports = { install, guard, file };
