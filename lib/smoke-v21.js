'use strict';

// v2.1 smoke checks that need no desktop fixture: the input's guess, the
// agent's gate against every bypass in the trust-safety review, and the
// keys the app must never take from other apps. Called from runSmoke.

const intent = require('../renderer/intent');
const agent = require('./agent');

// What a person types, what is on screen, and what Enter should do.
// "screen" stands for the window text the bar reads when it opens.
const SCREEN = 'Invoice 2291 Total due $4,120.00 Please settle by Friday Submit Cancel refund policy 0x1F3A';
const TABLE = [
  // Questions
  ['why is the build red', 'ask'],
  ['how do I save this as pdf', 'ask'],
  ['what does this error mean?', 'ask'],
  ['explain this function', 'ask'],
  ['summarize this page', 'ask'],
  ['is this email a scam?', 'ask'],
  ['write me a haiku', 'ask'],
  ['translate this paragraph to French', 'ask'],
  ['find the bug in this function', 'ask'],
  ['what is the total of this column', 'ask'],
  ['which button saves the file?', 'ask'],
  ['save', 'ask'],
  ['/etc/hosts', 'ask'],
  ['can you open notepad?', 'ask'],
  ['', 'ask'],
  ['hello', 'ask'],
  ['refund policy', 'find'],
  ['?invoice', 'ask'],
  ['?rename these files', 'ask'],
  // Finding
  ['invoice', 'find'],
  ['2291', 'find'],
  ['total due', 'find'],
  ['0x1F3A', 'find'],
  ['Please settle by Friday', 'find'],
  ['where is the submit button', 'find'],
  ['/invoice', 'find'],
  ['//total', 'find'],
  ['/ deadline', 'find'],
  ['find invoice', 'find'],
  ['where is the cancel button', 'find'],
  // Tasks
  ['rename these screenshots by date', 'do'],
  ['open Notepad and write a haiku', 'do'],
  ['send the report to Sam', 'do'],
  ['delete the old ones', 'do'],
  ['do it', 'do'],
  ['can you open notepad', 'do'],
  ['>save', 'do'],
  ['>what time is it', 'do'],
  ['close all the tabs except this one', 'do'],
  ['sort this sheet by date', 'do'],
  ['open a new tab and search for the weather', 'do'],
  ['move these files into a folder called old', 'do'],
];

// Literal checks on parse: the prefix is consumed, the rest is the body.
const PARSE = [
  ['/invoice', 'find', 'invoice', false],
  ['//total', 'find', 'total', true],
  ['?why', 'ask', 'why', false],
  ['> open notepad', 'do', 'open notepad', false],
  ['/etc/hosts', null, '/etc/hosts', false],
];

// ----------------------------------------------------------------- gate

// A small desktop: the windows the bypasses happen in, and what each one's
// tree holds. Only what the gate reads.
const WIN = {
  notepad: { handle: 11, process: 'notepad.exe', title: 'notes.txt - Notepad', className: 'Notepad' },
  slack: { handle: 12, process: 'slack.exe', title: 'general - Slack', className: 'Chrome_WidgetWin_1' },
  chrome: { handle: 13, process: 'chrome.exe', title: 'New Tab - Google Chrome', className: 'Chrome_WidgetWin_1' },
  cmd: { handle: 14, process: 'WindowsTerminal.exe', title: 'Windows PowerShell', className: 'CASCADIA_HOSTING_WINDOW_CLASS' },
  run: { handle: 15, process: 'explorer.exe', title: 'Run', className: '#32770' },
  outlook: { handle: 16, process: 'OUTLOOK.EXE', title: 'Inbox - Outlook', className: 'rctrl_renwnd32' },
  delDialog: { handle: 17, process: 'explorer.exe', title: 'Confirm', className: '#32770' },
  bank: { handle: 18, process: 'msedge.exe', title: 'Checkout - Pay now', className: 'Chrome_WidgetWin_1' },
  login: { handle: 19, process: 'chrome.exe', title: 'Sign in', className: 'Chrome_WidgetWin_1' },
};
const focused = (n) => ({ ...n, states: [...(n.states || []), 'focused'] });
const TREES = {
  11: [focused({ ref: 'e1', role: 'document', name: 'Text editor' })],
  12: [focused({ ref: 'e1', role: 'edit', name: 'Message #general', automationId: 'msg_input' })],
  13: [focused({ ref: 'e1', role: 'edit', name: 'Address and search bar', automationId: 'omnibox' })],
  14: [focused({ ref: 'e1', role: 'document', name: 'Terminal input' })],
  15: [focused({ ref: 'e1', role: 'edit', name: 'Open:' }), { ref: 'e2', role: 'button', name: 'OK' }],
  16: [focused({ ref: 'e1', role: 'list', name: 'Message list' })],
  17: [{ ref: 'e1', role: 'text', name: 'Are you sure you want to permanently delete this file?' }, { ref: 'e2', role: 'button', name: 'Yes' }],
  18: [{ ref: 'e1', role: 'button', name: 'Continue' }],
  19: [{ ref: 'e1', role: 'edit', name: 'Password', states: ['password'] }, { ref: 'e2', role: 'edit', name: 'Email' }],
};
const stubDesktop = {
  listWindows: async () => Object.values(WIN),
  readTree: async ({ windowHandle }) => ({ nodes: TREES[windowHandle] || [], window: Object.values(WIN).find((w) => w.handle === windowHandle) }),
};
const env = (target, extra) => ({ desktop: stubDesktop, settings: { permission: 'risky' }, target, planGate: false, planApproved: false, ...extra });

// Each of these must ask (or refuse). From trust-safety.md C1 and the HIGH
// findings: newlines and tabs in type, Enter under other names, sequences
// and repeats, Alt chords, Run, terminals, the address bar, Delete outside
// Explorer, dialogs, drags, unnamed targets, risky pages, password fields.
const MUST_ASK = [
  ['type newline in a chat compose box', { name: 'type', args: { text: 'hi\n' } }, {}, WIN.slack.handle],
  ['type carriage return', { name: 'type', args: { text: 'hi\r' } }, {}, WIN.slack.handle],
  ['type tab moves fields', { name: 'type', args: { text: 'a\tb' } }, {}, WIN.chrome.handle],
  ['key return', { name: 'key', args: { text: 'return' } }, {}, WIN.notepad.handle],
  ['key Enter', { name: 'key', args: { text: 'Enter' } }, {}, WIN.notepad.handle],
  ['key ctrl+enter', { name: 'key', args: { text: 'ctrl+enter' } }, {}, WIN.slack.handle],
  ['key ctrl+shift+enter', { name: 'key', args: { text: 'ctrl+shift+enter' } }, {}, WIN.slack.handle],
  ['key sequence ending in enter', { name: 'key', args: { text: 'tab enter' } }, {}, WIN.chrome.handle],
  ['key numpad enter alias', { name: 'key', args: { text: 'kp_enter' } }, {}, WIN.notepad.handle],
  ['key alt+s sends in Outlook', { name: 'key', args: { text: 'alt+s' } }, {}, WIN.outlook.handle],
  ['key ctrl+d', { name: 'key', args: { text: 'ctrl+d' } }, {}, WIN.outlook.handle],
  ['key delete in Outlook', { name: 'key', args: { text: 'Delete' } }, {}, WIN.outlook.handle],
  ['key shift+delete', { name: 'key', args: { text: 'shift+delete' } }, {}, WIN.notepad.handle],
  ['key ctrl+shift+delete', { name: 'key', args: { text: 'ctrl+shift+delete' } }, {}, WIN.chrome.handle],
  ['key win+r', { name: 'key', args: { text: 'win+r' } }, {}, WIN.notepad.handle],
  ['key win+x', { name: 'key', args: { text: 'super+x' } }, {}, WIN.notepad.handle],
  ['key alt+f4', { name: 'key', args: { text: 'alt+f4' } }, {}, WIN.notepad.handle],
  ['key ctrl+w', { name: 'key', args: { text: 'ctrl+w' } }, {}, WIN.chrome.handle],
  ['key f5 repeated', { name: 'key', args: { text: 'f5', repeat: 50 } }, {}, WIN.chrome.handle],
  ['type into the Run box', { name: 'type', args: { text: 'powershell -c calc' } }, {}, WIN.run.handle],
  ['type into a terminal', { name: 'type', args: { text: 'rm -rf build' } }, {}, WIN.cmd.handle],
  ['key enter in a terminal', { name: 'key', args: { text: 'enter' } }, {}, WIN.cmd.handle],
  ['type a URL into the address bar', { name: 'type', args: { text: 'evil.example' } }, {}, WIN.chrome.handle],
  ['click Yes in a delete dialog', { name: 'left_click', args: { ref: 'e2' } }, { node: { role: 'button', name: 'Yes' } }, WIN.delDialog.handle],
  ['click a button in a mail app', { name: 'left_click', args: { ref: 'e9' } }, { node: { role: 'button', name: 'Reply all' } }, WIN.outlook.handle],
  ['click on a checkout page', { name: 'left_click', args: { ref: 'e1' } }, { node: { role: 'button', name: 'Continue' } }, WIN.bank.handle],
  ['click Pay now', { name: 'left_click', args: { ref: 'e1' } }, { node: { role: 'button', name: 'Pay now' } }, WIN.notepad.handle],
  ['click Authorize', { name: 'left_click', args: { ref: 'e1' } }, { node: { role: 'button', name: 'Authorize app' } }, WIN.notepad.handle],
  ['click by automationId send', { name: 'left_click', args: { ref: 'e1' } }, { node: { role: 'button', name: '', automationId: 'sendButton' } }, WIN.notepad.handle],
  ['click an unnamed target', { name: 'left_click', args: { x: 10, y: 10 } }, { node: { role: 'pane', name: '' } }, WIN.notepad.handle],
  ['click where nothing could be read', { name: 'left_click', args: { x: 10, y: 10 } }, { node: null }, WIN.notepad.handle],
  ['drag anything', { name: 'left_click_drag', args: { start_x: 1, start_y: 1, x: 300, y: 300 } }, { node: { role: 'listitem', name: 'a.txt' } }, WIN.notepad.handle],
  ['type into a password field', { name: 'type', args: { text: 'hunter2' } }, { node: { role: 'edit', name: 'Password', states: ['password'] } }, WIN.login.handle],
  ['type with unknown focus beside a password field', { name: 'type', args: { text: 'x' } }, {}, WIN.login.handle],
  ['read the clipboard', { name: 'clipboard_read', args: {} }, {}, WIN.notepad.handle],
  ['close a window', { name: 'close_window', args: { handle: 11 } }, { windowHandle: 11 }, WIN.notepad.handle],
  ['open a link', { name: 'open', args: { target: 'https://example.com/a' } }, {}, WIN.notepad.handle],
];

// And these must not ask, or the gate is just a wall.
const MUST_PASS = [
  ['type plain text in Notepad', { name: 'type', args: { text: 'hello' } }, {}, WIN.notepad.handle],
  ['type a newline into Notepad\'s document', { name: 'type', args: { text: 'line one\nline two' } }, {}, WIN.notepad.handle],
  ['key ctrl+c', { name: 'key', args: { text: 'ctrl+c' } }, {}, WIN.notepad.handle],
  ['key down arrow', { name: 'key', args: { text: 'down' } }, {}, WIN.notepad.handle],
  ['click a named tab', { name: 'left_click', args: { ref: 'e1' } }, { node: { role: 'tabitem', name: 'View' } }, WIN.notepad.handle],
];

async function gateRows(c) {
  const { must, log } = c;
  const missed = [];
  for (const [label, call, info, target] of MUST_ASK) {
    let r;
    try {
      r = await agent.gate(call, { ...info, windowHandle: info.windowHandle || target }, env(target));
    } catch (err) {
      r = { cls: 'threw', why: err.message };
    }
    if (!r || (r.cls !== 'consequential' && r.cls !== 'refused')) missed.push(`${label}=${r && r.cls}`);
  }
  log('gateBypasses', `${MUST_ASK.length - missed.length}/${MUST_ASK.length}`);
  must('gateBypasses', missed.length === 0, missed.join(' | '));

  const walled = [];
  for (const [label, call, info, target] of MUST_PASS) {
    const r = await agent.gate(call, { ...info, windowHandle: target }, env(target));
    if (!r || r.cls !== 'input') walled.push(`${label}=${r && r.cls}:${r && r.why}`);
  }
  must('gatePassesPlainInput', walled.length === 0, walled.join(' | '));

  // An exception inside the gate asks; it never allows.
  const broken = {
    listWindows: async () => {
      throw new Error('engine gone');
    },
    readTree: async () => {
      throw new Error('engine gone');
    },
  };
  const closed = await agent.gate({ name: 'type', args: { text: 'x' } }, null, { desktop: broken, settings: {}, target: 1 });
  must('gateFailsClosed', closed && closed.cls !== 'input' && closed.cls !== 'read', JSON.stringify(closed));

  // Handed-over text waits for the plan before any action.
  const gated = await agent.gate({ name: 'left_click', args: { ref: 'e1' } }, { node: { role: 'tabitem', name: 'View' }, windowHandle: 11 }, env(11, { planGate: true }));
  const reads = await agent.gate({ name: 'read_page', args: {} }, {}, env(11, { planGate: true }));
  must('gatePlanFirst', gated.cls === 'refused' && reads.cls === 'read', `${gated.cls}/${reads.cls}`);

  // Chords: aliases canonicalised, every chord in a sequence seen.
  const ch = agent.chords('Return ctrl+Shift+Enter tab');
  must('gateChords', ch.length === 3 && ch[0].key === 'enter' && ch[1].key === 'enter' && ch[1].mods.includes('ctrl'), JSON.stringify(ch));
}

async function basics(c) {
  const { must, log } = c;

  // The guess.
  const wrong = [];
  for (const [text, want] of TABLE) {
    const got = intent.classify(text, { screen: SCREEN, screenFolded: intent.fold(SCREEN) }).intent;
    if (got !== want) wrong.push(`"${text}" ${got}!=${want}`);
  }
  log('intentTable', `${TABLE.length - wrong.length}/${TABLE.length}`);
  must('intentTable', TABLE.length >= 40 && wrong.length === 0, wrong.join(' | '));

  const badParse = [];
  for (const [raw, lock, body, literal] of PARSE) {
    const p = intent.parse(raw);
    if (p.lock !== lock || p.body !== body || p.literal !== literal) badParse.push(`${raw}:${JSON.stringify(p)}`);
  }
  must('intentPrefixes', badParse.length === 0, badParse.join(' | '));

  // Fast enough to run on every keystroke: under 5 ms each, against a
  // full window of screen text.
  const big = intent.fold(SCREEN.repeat(150));
  intent.score('warm up', { screenFolded: big });
  let worst = 0;
  for (const [text] of TABLE) {
    const t0 = process.hrtime.bigint();
    intent.score(text, { screenFolded: big });
    worst = Math.max(worst, Number(process.hrtime.bigint() - t0) / 1e6);
  }
  log('intentWorstMs', worst.toFixed(2));
  must('intentFast', worst < 5, `${worst.toFixed(2)}ms`);

  // A guessed task is never a run: the margin rule leaves weak ones as Ask.
  const weak = intent.classify('fix it', {});
  must('intentDoNeedsMargin', weak.intent !== 'do' || weak.margin >= 0.15, JSON.stringify({ i: weak.intent, m: weak.margin }));

  await gateRows(c);

  // Keys never taken from other apps: no global Enter ever, and no global
  // Escape while only the bar is open.
  await c.openBar(null);
  await new Promise((r) => setTimeout(r, 300));
  const enterHeld = c.globalShortcut.isRegistered('Enter');
  const escHeld = c.globalShortcut.isRegistered('Escape');
  c.collapse();
  await new Promise((r) => setTimeout(r, 200));
  const escIdle = c.globalShortcut.isRegistered('Escape');
  log('keySwallow', `enter=${enterHeld} escBar=${escHeld} escIdle=${escIdle}`);
  must('noGlobalEnter', enterHeld === false, 'Enter is registered globally');
  must('noGlobalEscWithBar', escHeld === false && escIdle === false, `bar=${escHeld} idle=${escIdle}`);

  // The bar opens guessing, never in the last mode used.
  await c.openBar('find');
  await new Promise((r) => setTimeout(r, 200));
  c.collapse();
  await new Promise((r) => setTimeout(r, 300));
  await c.openBar(null);
  await new Promise((r) => setTimeout(r, 300));
  const tok = await c
    .overlayWin()
    .webContents.executeJavaScript(`(() => { const t = document.getElementById('intent'); return t.dataset.locked + ':' + t.dataset.intent; })()`);
  c.collapse();
  must('barOpensUnlocked', tok === 'false:ask', tok);

  // The update feed override is for tests only.
  const updater = require('./updater');
  if (typeof updater.feedOverride === 'function') {
    const keep = { ...process.env };
    process.env.AEVAL_UPDATE_FEED = 'http://evil.example/feed';
    const remote = updater.feedOverride();
    process.env.AEVAL_UPDATE_FEED = 'https://user:pw@example.com/feed';
    const creds = updater.feedOverride();
    process.env.AEVAL_UPDATE_FEED = keep.AEVAL_UPDATE_FEED || '';
    if (!keep.AEVAL_UPDATE_FEED) delete process.env.AEVAL_UPDATE_FEED;
    must('feedOverrideStrict', !remote && !creds, `${remote} ${creds}`);
  }
}

module.exports = { basics, TABLE, MUST_ASK };
