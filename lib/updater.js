'use strict';

// Auto-update, by way of GitHub releases.
//
// electron-updater reads latest.yml from the newest published GitHub release
// (the atom feed and /releases/latest on github.com, not api.github.com, so
// the 60-an-hour API limit does not apply), compares it with
// app.getVersion(), downloads the NSIS installer in the background, checks
// its sha512 against latest.yml, and swaps it in on the next quit.
//
// What this module adds on top is the behaviour a person notices:
//
// - When it checks: 20 seconds after launch, every four hours after that,
//   shortly after the machine wakes, and the moment the network comes back
//   if a check found it gone. A check never starts while a question is being
//   answered; it waits a minute and tries again.
// - What failure looks like: nothing alarming. Offline, rate limited, or a
//   server having a bad minute are all ordinary, and each one is retried on
//   a widening backoff (2, 5, 15, 30, 60, 120 minutes, then the normal four
//   hours). Settings says so in one plain sentence; the tray says nothing.
//   The raw error goes to updater.log beside the settings, never to a dialog.
// - What success looks like: a "Restart to update" item at the top of the
//   tray menu and in Settings > About, and one silent notification, held back
//   until no answer is streaming. Restart is the user's call. If they never
//   make it, the installer runs when the app quits anyway.
//
// Unpackaged (npm start), there is no installer to replace, so every call is
// a no-op that reports 'dev'.
//
// Two environment variables exist for the cloud test in
// .github/workflows/update-check.yml and do nothing unless set:
// AEVAL_UPDATE_FEED points the updater at a generic feed instead of GitHub,
// and AEVAL_UPDATE_QUIT=1 quits as soon as a download is ready, which is the
// install-on-quit path with no human to choose Quit.

const { app, Notification, net, powerMonitor, shell } = require('electron');
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const FIRST_CHECK_MS = 20 * 1000;
const RECHECK_MS = 4 * 60 * 60 * 1000;
// Wi-Fi takes a few seconds to come back after the lid opens; a check fired
// at the instant of resume would only ever find the machine offline.
const RESUME_DELAY_MS = 15 * 1000;
// A wake within this long of the last good check does not need another.
const RESUME_STALE_MS = 30 * 60 * 1000;
const BUSY_DEFER_MS = 60 * 1000;
const ONLINE_POLL_MS = 30 * 1000;
const BACKOFF_MS = [2, 5, 15, 30, 60, 120].map((m) => m * 60 * 1000);
const LOG_MAX_BYTES = 256 * 1024;

let updater = null;
let deps = {};
let timer = null;
let onlineTimer = null;
let inFlight = null;
let failures = 0;
let lastError = null;
let announcePending = false;
let announcedVersion = '';
let lastProgressEmit = 0;
let releaseBase = '';
let lastRunVersion = '';

let state = {
  // dev | idle | checking | current | downloading | ready | offline | retrying
  status: 'idle',
  version: '',
  notes: '',
  releaseUrl: '',
  progress: 0,
  transferred: 0,
  total: 0,
  bytesPerSecond: 0,
  lastChecked: 0,
  nextCheck: 0,
  problem: '',
  restartPending: false,
  // The version this copy ran before an update replaced it, for the first
  // session after that update only.
  updatedFrom: '',
};

// ------------------------------------------------------------------ log

function userFile(name) {
  return path.join(app.getPath('userData'), name);
}

function log(level, message) {
  const line = `[${new Date().toISOString()}] ${level} ${message}\n`;
  try {
    const p = userFile('updater.log');
    if (fs.existsSync(p) && fs.statSync(p).size > LOG_MAX_BYTES) {
      fs.writeFileSync(p, fs.readFileSync(p, 'utf8').slice(-Math.floor(LOG_MAX_BYTES / 2)));
    }
    fs.appendFileSync(p, line);
  } catch (err) {
    // The log is for reading after the fact; it must not become the failure.
  }
  if (level !== 'info') console.warn(`[aevalbuddy] update ${level}:`, message);
}

const logger = {
  info: (m) => log('info', String(m)),
  warn: (m) => log('warn', String(m)),
  error: (m) => log('error', String(m)),
  debug: () => {},
};

// The last good check survives a restart, so "Checked 2 hours ago" is true
// the first time Settings opens after launch.
function loadPersisted() {
  try {
    const saved = JSON.parse(fs.readFileSync(userFile('updater.json'), 'utf8'));
    if (Number.isFinite(saved.lastChecked)) state.lastChecked = saved.lastChecked;
    if (typeof saved.lastRunVersion === 'string') lastRunVersion = saved.lastRunVersion;
  } catch (err) {
    // First run, or a file nobody needs to recover.
  }
}

function persist() {
  try {
    fs.writeFileSync(
      userFile('updater.json'),
      JSON.stringify({ lastChecked: state.lastChecked, lastRunVersion: app.getVersion() })
    );
  } catch (err) {
    // Losing "last checked" costs one line of text in Settings.
  }
}

// ------------------------------------------------------------------ state

function set(next) {
  state = { ...state, ...next };
  if (deps.onChange) deps.onChange(get());
}

/** A copy of the update state, for the tray, Settings, and the smoke test. */
function get() {
  return { ...state, current: app.getVersion() };
}

function isBusy() {
  return !!(deps.isBusy && deps.isBusy());
}

function feedOverride() {
  const url = process.env.AEVAL_UPDATE_FEED || '';
  return /^https?:\/\//i.test(url) ? url.replace(/\/?$/, '/') : '';
}

// Where the release page for a version lives, read from the same
// app-update.yml the updater reads so the two cannot disagree.
function readReleaseBase() {
  try {
    const yml = fs.readFileSync(path.join(process.resourcesPath, 'app-update.yml'), 'utf8');
    const owner = (yml.match(/^owner:\s*(\S+)/m) || [])[1];
    const repo = (yml.match(/^repo:\s*(\S+)/m) || [])[1];
    if (/^provider:\s*github/m.test(yml) && owner && repo) {
      return `https://github.com/${owner}/${repo}/releases/tag/v`;
    }
  } catch (err) {
    // No release link is shown; nothing else depends on it.
  }
  return '';
}

// ------------------------------------------------------------- formatting

function plain(html) {
  return String(html || '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&#x27;/g, "'")
    .replace(/&amp;/g, '&')
    .replace(/\s+/g, ' ')
    .trim();
}

/**
 * A sentence or two of what changed, from GitHub's release notes (HTML from
 * the atom feed, or one entry per version when several are skipped). The
 * notes lead with a "What's new" list, so list items win over paragraphs.
 */
function notesSnippet(notes) {
  const html = Array.isArray(notes)
    ? notes.map((n) => (n && n.note) || '').join('\n')
    : String(notes || '');
  const items = [...html.matchAll(/<li[^>]*>([\s\S]*?)<\/li>/gi)]
    .map((m) => plain(m[1]))
    .filter(Boolean);
  let text = items.length ? items.slice(0, 3).join(' · ') : plain(html.split(/<\/p>/i)[0]);
  if (text.length > 180) text = `${text.slice(0, 177).replace(/\s+\S*$/, '')}…`;
  return text;
}

/**
 * What kind of failure this is. Every kind is retried; the kind only picks
 * the sentence. Messages come from Chromium's net stack (net::ERR_*), from
 * Node, or from builder-util-runtime's HttpError ("429 Too Many Requests"),
 * sometimes wrapped inside an ERR_UPDATER_* error whose message carries the
 * original.
 */
function classify(err) {
  const msg = String((err && (err.stack || err.message)) || err || '');
  const status = err && err.statusCode;
  if (
    /ERR_INTERNET_DISCONNECTED|ERR_NAME_NOT_RESOLVED|ERR_NAME_RESOLUTION_FAILED|ERR_NETWORK_CHANGED|ERR_NETWORK_IO_SUSPENDED|ERR_ADDRESS_UNREACHABLE|ERR_CONNECTION_(REFUSED|RESET|TIMED_OUT|CLOSED|ABORTED|FAILED)|ERR_TIMED_OUT|ERR_PROXY_CONNECTION_FAILED|ERR_NETWORK_ACCESS_DENIED|ENOTFOUND|EAI_AGAIN|ECONNRESET|ECONNREFUSED|ETIMEDOUT|ENETUNREACH|socket hang up/i.test(
      msg
    )
  ) {
    return 'offline';
  }
  if (status === 429 || /\b429\b|rate.?limit|too many requests/i.test(msg)) return 'limited';
  if (/sha512 checksum mismatch|ERR_CHECKSUM_MISMATCH|checksum/i.test(msg)) return 'corrupt';
  return 'server';
}

function clock(ms) {
  return new Date(ms).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
}

function problemText(kind, nextCheck) {
  const at = nextCheck ? ` Trying again at ${clock(nextCheck)}.` : '';
  if (kind === 'offline') return "You're offline. AevalBuddy checks again once you're connected.";
  if (kind === 'limited') return `GitHub asked for a pause.${at}`;
  if (kind === 'corrupt') return `The download did not verify, so it was thrown away.${at}`;
  return `The update server did not answer.${at}`;
}

function backoff(n) {
  const base = BACKOFF_MS[Math.min(n, BACKOFF_MS.length) - 1] || RECHECK_MS;
  // Up to 10% jitter, so a GitHub hiccup does not bring every copy back at
  // the same second.
  return Math.round(base * (1 + Math.random() * 0.1));
}

// ------------------------------------------------------------- scheduling

function schedule(ms) {
  clearTimeout(timer);
  timer = setTimeout(scheduledCheck, ms);
  if (timer.unref) timer.unref();
  state.nextCheck = Date.now() + ms;
}

function scheduledCheck() {
  timer = null;
  // Never compete with an answer that is streaming for bandwidth or for the
  // user's attention.
  if (isBusy()) {
    schedule(BUSY_DEFER_MS);
    return;
  }
  check();
}

function online() {
  try {
    return net.isOnline();
  } catch (err) {
    return true;
  }
}

// Main has no 'online' event, so while offline this polls a flag Chromium
// keeps current anyway. Nothing goes over the network until it flips.
function waitForNetwork() {
  if (onlineTimer) return;
  onlineTimer = setInterval(() => {
    if (!online()) return;
    clearInterval(onlineTimer);
    onlineTimer = null;
    schedule(isBusy() ? BUSY_DEFER_MS : 1000);
  }, ONLINE_POLL_MS);
  if (onlineTimer.unref) onlineTimer.unref();
}

function fail(err) {
  if (err && err === lastError) return;
  lastError = err;
  failures += 1;
  const kind = classify(err);
  log('warn', `${kind} (attempt ${failures}): ${(err && (err.stack || err.message)) || err}`);
  if (kind === 'offline' && !online()) {
    clearTimeout(timer);
    set({ status: 'offline', problem: problemText('offline'), progress: 0, nextCheck: 0 });
    waitForNetwork();
    return;
  }
  // A refused or reset connection with the network up is the server's
  // problem, not the user's, so it gets the server's sentence.
  schedule(backoff(failures));
  set({
    status: 'retrying',
    problem: problemText(kind === 'offline' ? 'server' : kind, state.nextCheck),
    progress: 0,
  });
}

// ------------------------------------------------------------- updater

function load() {
  if (updater) return updater;
  const { autoUpdater } = require('electron-updater');
  autoUpdater.logger = logger;
  autoUpdater.autoDownload = true;
  autoUpdater.autoInstallOnAppQuit = true;
  // A full NSIS installer, never the web stub; also silences a warning
  // electron-updater prints on every download otherwise.
  autoUpdater.disableWebInstaller = true;
  const feed = feedOverride();
  if (feed) {
    log('info', `feed override: ${feed}`);
    autoUpdater.setFeedURL({ provider: 'generic', url: feed, useMultipleRangeRequest: false });
  }
  autoUpdater.on('checking-for-update', () => set({ status: 'checking', problem: '' }));
  autoUpdater.on('update-not-available', () => set({ status: 'current', problem: '' }));
  autoUpdater.on('update-available', (info) =>
    set({
      status: 'downloading',
      version: info.version,
      notes: notesSnippet(info.releaseNotes),
      releaseUrl: releaseBase && !feed ? `${releaseBase}${info.version}` : '',
      progress: 0,
      transferred: 0,
      total: 0,
      problem: '',
    })
  );
  autoUpdater.on('download-progress', (p) => {
    // One menu rebuild per visible change, not one per network chunk.
    const now = Date.now();
    const progress = Math.max(0, Math.min(1, (p.percent || 0) / 100));
    if (Math.floor(progress * 100) === Math.floor(state.progress * 100) && now - lastProgressEmit < 1000) {
      return;
    }
    lastProgressEmit = now;
    set({
      status: 'downloading',
      progress,
      transferred: p.transferred || 0,
      total: p.total || 0,
      bytesPerSecond: p.bytesPerSecond || 0,
    });
  });
  autoUpdater.on('update-downloaded', (info) => {
    failures = 0;
    clearTimeout(timer);
    timer = null;
    set({ status: 'ready', version: info.version, progress: 1, problem: '', nextCheck: 0 });
    const verified = recordVerified(info);
    announcePending = true;
    if (!isBusy()) idle();
    if (process.env.AEVAL_UPDATE_QUIT === '1') {
      verified.then(() => {
        log('info', 'AEVAL_UPDATE_QUIT: quitting so the installer runs on quit');
        app.quit();
      });
    }
  });
  autoUpdater.on('error', (err) => fail(err));
  updater = autoUpdater;
  return updater;
}

// electron-updater has already refused anything whose sha512 differs from
// latest.yml by the time this runs. Hashing the file again here is for the
// log: it is what the cloud test compares against the feed it served.
function recordVerified(info) {
  const file = info && info.downloadedFile;
  const want = (info && info.files && info.files[0] && info.files[0].sha512) || info.sha512 || '';
  if (!file) return Promise.resolve();
  const hash = crypto.createHash('sha512');
  return new Promise((resolve) => {
    fs.createReadStream(file)
      .on('data', (chunk) => hash.update(chunk))
      .on('error', (err) => {
        log('warn', `could not re-hash ${file}: ${err.message}`);
        resolve();
      })
      .on('end', () => {
        const got = hash.digest('base64');
        log('info', `downloaded ${info.version} sha512=${got} expected=${want} match=${got === want}`);
        resolve();
      });
  });
}

function announce(version) {
  if (announcedVersion === version) return;
  announcedVersion = version;
  try {
    if (!Notification.isSupported()) return;
    const n = new Notification({
      title: `AevalBuddy ${version} is ready`,
      body: 'It installs the next time you quit. Click to restart now.',
      silent: true,
    });
    n.on('click', () => restart());
    n.show();
  } catch (err) {
    // The tray item and Settings carry the same message.
  }
}

// A silent update relaunches the app with nothing on screen saying it
// changed. One quiet notice, once, on the first launch of a new version,
// pointing at what is new.
function announceUpdated() {
  const version = app.getVersion();
  try {
    if (!Notification.isSupported()) return;
    const n = new Notification({
      title: `AevalBuddy updated to ${version}`,
      body: releaseBase ? "Click to see what's new." : `It was ${state.updatedFrom}.`,
      silent: true,
    });
    if (releaseBase) n.on('click', () => shell.openExternal(`${releaseBase}${version}`));
    n.show();
  } catch (err) {
    // Settings > About says the same thing for the rest of the session.
  }
}

// ------------------------------------------------------------------ API

/**
 * @param {object} d
 * @param {(state: object) => void} d.onChange  tray and settings refresh from this
 * @param {() => boolean} [d.isBusy]  true while a question is being answered
 */
function init(d) {
  deps = d || {};
  if (!app.isPackaged) {
    set({ status: 'dev' });
    return;
  }
  loadPersisted();
  releaseBase = readReleaseBase();
  const updatedFrom = lastRunVersion && lastRunVersion !== app.getVersion() ? lastRunVersion : '';
  if (updatedFrom) {
    log('info', `running ${app.getVersion()}, updated from ${updatedFrom}`);
    set({ updatedFrom, releaseUrl: releaseBase ? `${releaseBase}${app.getVersion()}` : '' });
  }
  persist();
  // The automated runs check by hand when they mean to (see runSmoke); a
  // background check would only add network traffic to a timed test.
  if (process.env.AEVAL_SMOKE || process.env.AEVAL_SHOT) return;
  if (updatedFrom) setTimeout(announceUpdated, 3000);
  schedule(feedOverride() ? 2000 : FIRST_CHECK_MS);
  try {
    powerMonitor.on('resume', () => {
      if (state.status === 'ready' || state.status === 'downloading' || inFlight) return;
      if (
        state.status === 'offline' ||
        state.status === 'retrying' ||
        Date.now() - state.lastChecked > RESUME_STALE_MS
      ) {
        schedule(RESUME_DELAY_MS);
      }
    });
  } catch (err) {
    log('warn', `no resume events: ${err.message}`);
  }
}

/** One check, now. Resolves to the state once the check has settled. */
function check() {
  if (!app.isPackaged) return Promise.resolve(get());
  if (state.status === 'ready' || state.status === 'downloading') return Promise.resolve(get());
  if (inFlight) return inFlight;
  clearTimeout(timer);
  timer = null;
  if (!online()) {
    set({ status: 'offline', problem: problemText('offline'), nextCheck: 0 });
    waitForNetwork();
    return Promise.resolve(get());
  }
  set({ status: 'checking', problem: '' });
  inFlight = (async () => {
    try {
      const result = await load().checkForUpdates();
      // The download runs on after the check resolves. Its failures arrive
      // through the 'error' event; without this catch the same rejection
      // also surfaced as an unhandled one, which crash.js announces as
      // "AevalBuddy hit an error" for what is only a dropped connection.
      if (result && result.downloadPromise) result.downloadPromise.catch(() => {});
      failures = 0;
      lastError = null;
      state.lastChecked = Date.now();
      persist();
      if (state.status !== 'downloading' && state.status !== 'ready') {
        schedule(RECHECK_MS);
        set({ status: state.status === 'checking' ? 'current' : state.status });
      } else {
        set({});
      }
    } catch (err) {
      fail(err);
    } finally {
      inFlight = null;
    }
    return get();
  })();
  return inFlight;
}

/**
 * Quit and run the downloaded installer. While an answer is streaming the
 * restart waits for it to finish rather than cutting it off.
 */
function restart() {
  if (state.status !== 'ready' || !updater) return false;
  if (isBusy()) {
    set({ restartPending: true });
    return true;
  }
  // isSilent, isForceRunAfter: the one-click installer runs without a dialog
  // and relaunches the app when it is done. quitAndInstall ends in app.quit(),
  // so the update is only as reliable as quitting is: a window 'close'
  // handler that calls preventDefault, or a child process left running from
  // the install folder, holds the installer up. Neither exists today; any
  // that is added must let a quit through, and stop its children in
  // 'before-quit'.
  setImmediate(() => updater.quitAndInstall(true, true));
  return true;
}

/** Main calls this whenever a question finishes; held-back work runs here. */
function idle() {
  if (state.status !== 'ready') return;
  if (state.restartPending) {
    restart();
    return;
  }
  if (announcePending) {
    announcePending = false;
    announce(state.version);
  }
}

function isReady() {
  return state.status === 'ready';
}

function destroy() {
  clearTimeout(timer);
  clearInterval(onlineTimer);
  timer = null;
  onlineTimer = null;
}

module.exports = {
  init,
  check,
  restart,
  idle,
  get,
  isReady,
  destroy,
  // Pure helpers, exported for the smoke test.
  classify,
  notesSnippet,
  problemText,
};
