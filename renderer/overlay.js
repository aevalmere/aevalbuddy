'use strict';

const pill = document.getElementById('pill');
const panel = document.getElementById('panel');
const input = document.getElementById('query-input');
const settingsBtn = document.getElementById('settings-btn');
const closeBtn = document.getElementById('close-btn');
const notice = document.getElementById('notice');
const noticeText = document.getElementById('notice-text');
const noticeSettings = document.getElementById('notice-settings');
const hud = document.getElementById('hud');
const hudDone = document.getElementById('hud-done');
const regionBtn = document.getElementById('region-btn');
const regionChip = document.getElementById('region-chip');
const regionSize = document.getElementById('region-size');
const regionClear = document.getElementById('region-clear');
const modeLabel = document.getElementById('mode-label');
const pillLabel = document.querySelector('.pill-label');
const barStatus = document.getElementById('bar-status');
const barStatusText = document.getElementById('bar-status-text');
const barStatusAction = document.getElementById('bar-status-action');
const findScopeBtn = document.getElementById('find-scope');

// ------------------------------------------------------------------ modes

// Ask, Find, Agent: one bar, three verbs. The text in the bar belongs to the
// person, not to a mode, so it survives every switch.
const MODES = ['ask', 'find', 'agent'];
const MODE_NAME = { ask: 'Ask', find: 'Find', agent: 'Agent' };
const PLACEHOLDER = {
  ask: 'Ask about your screen',
  find: 'Find on screen',
  agent: 'Tell it what to do',
};
let barMode = 'ask';
let findState = null;
let agentHint = '';
let findTimer = null;

function applyMode(mode, tell) {
  const next = MODES.includes(mode) ? mode : 'ask';
  const changed = next !== barMode;
  barMode = next;
  document.body.dataset.barMode = next;
  modeLabel.textContent = MODE_NAME[next];
  modeLabel.setAttribute('aria-label', `Mode: ${MODE_NAME[next]}. Click to switch.`);
  input.placeholder = PLACEHOLDER[next];
  input.setAttribute('aria-label', `${PLACEHOLDER[next]}. Press Enter to ${next === 'find' ? 'go to the next match' : next === 'agent' ? 'start' : 'send'}.`);
  if (changed) {
    replayAnim(modeLabel, 'swap');
    replayAnim(input, 'swap');
    agentHint = '';
  }
  if (tell && changed) {
    window.aeval.invoke('bar:mode', { mode: next });
    if (next === 'find' && input.value.trim()) queryFind();
  }
  renderStatus();
}

function cycleMode(by) {
  const i = MODES.indexOf(barMode);
  applyMode(MODES[(i + by + MODES.length) % MODES.length], true);
}

function queryFind() {
  clearTimeout(findTimer);
  window.aeval.invoke('find:query', { query: input.value });
}

// One line under the bar: Find's count and what the model stages are doing,
// or the Agent's hint. It takes no space while it has nothing to say.
function renderStatus() {
  let text = '';
  let action = null;
  if (barMode === 'find' && findState && findState.open) {
    text = [findState.status, ...(findState.lines || [])].filter(Boolean).join('\n');
    if (findState.action === 'ask') action = { label: 'Ask instead', run: () => applyMode('ask', true) };
    findScopeBtn.hidden = false;
    findScopeBtn.textContent = findState.scope === 'screen' ? 'Whole screen' : 'This window';
  } else {
    findScopeBtn.hidden = true;
  }
  if (barMode === 'agent') text = agentHint;
  barStatusText.textContent = text;
  barStatusAction.hidden = !action;
  if (action) {
    barStatusAction.textContent = action.label;
    barStatusAction.onclick = action.run;
  }
  const show = !!text || !findScopeBtn.hidden;
  if (barStatus.hidden === show) {
    barStatus.hidden = !show;
    requestResize();
  } else if (show) {
    requestResize();
  }
}

modeLabel.addEventListener('click', () => cycleMode(1));
findScopeBtn.addEventListener('click', () => window.aeval.invoke('find:scope', {}));

window.aeval.on('find:state', (s) => {
  findState = s;
  renderStatus();
});

window.aeval.on('bar:prefill', ({ text, mode }) => {
  if (mode) applyMode(mode, false);
  input.value = text || '';
  input.focus();
  if (barMode === 'find' && input.value) queryFind();
});

let state = { mode: 'pill', hasKey: false, captureOk: true };
let watchSelector = null;
let ignoreState = true;
let resizeQueued = false;

// ----------------------------------------------------------------- render

// The window grows away from the anchored corner (lib/motion.js); the
// stylesheet pins the panel to those same edges so the growth uncovers it.
function applyAnchor() {
  const a = state.inputAnchor || { h: 'right', v: 'top' };
  document.body.dataset.anchorH = a.h;
  document.body.dataset.anchorV = a.v;
}

function replayAnim(el, cls) {
  el.classList.remove(cls);
  void el.offsetWidth;
  el.classList.add(cls);
}

function render(prevMode) {
  document.body.dataset.mode = state.mode;
  // idle, chat (a conversation is waiting behind it), busy (an answer is on
  // its way) or agent (it has the computer): the collapsed pill shows which.
  document.body.dataset.pill = state.pillState || 'idle';
  document.body.dataset.pillError = state.pillError || state.offline ? '1' : '0';
  pill.title = state.offline ? 'Offline' : state.pillError ? 'The last task did not finish' : '';
  if (state.agent && state.agent.running) {
    pillLabel.textContent = state.agent.state === 'paused' ? 'Paused' : `Step ${Math.max(1, state.agent.step || 1)}`;
  } else {
    pillLabel.textContent = MODE_NAME[state.barMode] || 'Ask';
  }
  if (state.barMode && state.barMode !== barMode) applyMode(state.barMode, false);
  if (state.inputWidth) {
    document.documentElement.style.setProperty('--panel-w', state.inputWidth + 'px');
  }
  applyAnchor();
  renderRegion();
  pill.hidden = state.mode !== 'pill';
  panel.hidden = state.mode !== 'panel';
  hud.hidden = state.mode !== 'hud';
  if (state.mode === 'panel') {
    if (prevMode !== 'panel') replayAnim(panel, 'in');
    renderNotice();
    if (prevMode !== 'panel') {
      input.focus();
      input.select();
    }
    requestResize();
  }
  if (state.mode === 'pill' && prevMode !== 'pill') {
    replayAnim(pill, 'in');
  }
}

// The chip is the only thing telling the user that "what about now?" is
// about a rectangle rather than the desktop.
function renderRegion() {
  const r = state.region;
  document.body.dataset.region = r ? '1' : '0';
  regionChip.hidden = !r;
  if (r) regionSize.textContent = `Asking about a ${r.width} × ${r.height} area`;
}

function renderNotice() {
  if (!state.hasKey && barMode !== 'find') {
    noticeText.textContent =
      barMode === 'agent' ? 'Agent needs an OpenRouter API key.' : 'No OpenRouter API key saved yet.';
    noticeSettings.hidden = false;
    notice.hidden = false;
  } else if (state.offline && barMode === 'ask') {
    noticeText.textContent = 'No connection. Ask works again when you are back online.';
    noticeSettings.hidden = true;
    notice.hidden = false;
  } else if (!state.captureOk && barMode === 'ask') {
    noticeText.textContent =
      'Screen capture failed. Your question will be sent without a screenshot.';
    noticeSettings.hidden = true;
    notice.hidden = false;
  } else {
    notice.hidden = true;
  }
}

function requestResize() {
  if (state.mode !== 'panel' || resizeQueued) return;
  resizeQueued = true;
  requestAnimationFrame(() => {
    resizeQueued = false;
    // getBoundingClientRect is border-box, so this is the whole panel
    // including its 1px border; padding it out again leaves a dead strip.
    const height = Math.ceil(panel.getBoundingClientRect().height);
    window.aeval.invoke('overlay:resize', { height });
  });
}

new ResizeObserver(requestResize).observe(panel);

// ------------------------------------------------------------------ query

function submit() {
  const question = input.value;
  // Clear immediately, like a messaging app; the transcript echoes the
  // message. Without a key the text stays put so nothing typed is lost.
  if (state.hasKey) input.value = '';
  window.aeval.invoke('query:submit', { question }).then((result) => {
    if (result && result.status === 'walkthrough') {
      input.value = '';
    }
  });
}

// ------------------------------------------------------------------ state

window.aeval.on('overlay:state', (next) => {
  const prevMode = state.mode;
  state = { ...state, ...next };
  render(prevMode);
});

let snapFlashTimer = null;
window.aeval.on('hud:snapped', () => {
  hud.classList.add('snapped');
  clearTimeout(snapFlashTimer);
  snapFlashTimer = setTimeout(() => hud.classList.remove('snapped'), 350);
});

// ------------------------------------------------------------ interactions

pill.addEventListener('click', () => {
  window.aeval.invoke('overlay:trigger');
});
pill.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault();
    window.aeval.invoke('overlay:trigger');
  }
});

regionBtn.addEventListener('click', () => {
  window.aeval.invoke('region:pick');
});
regionClear.addEventListener('click', () => {
  window.aeval.invoke('region:clear');
});
settingsBtn.addEventListener('click', () => {
  window.aeval.invoke('overlay:open-settings');
});
noticeSettings.addEventListener('click', () => {
  window.aeval.invoke('overlay:open-settings');
});
closeBtn.addEventListener('click', () => {
  window.aeval.invoke('overlay:collapse');
});
hudDone.addEventListener('click', () => {
  window.aeval.invoke('hud:exit');
});

input.addEventListener('keydown', (e) => {
  if (e.ctrlKey && !e.altKey && ['1', '2', '3'].includes(e.key)) {
    e.preventDefault();
    applyMode(MODES[Number(e.key) - 1], true);
    return;
  }
  if (e.key === 'Tab' && !e.ctrlKey && !e.altKey) {
    e.preventDefault();
    // With text in Find, Tab switches what is searched; on an empty bar it
    // moves to the next mode.
    if (barMode === 'find' && input.value.trim()) window.aeval.invoke('find:scope', {});
    else cycleMode(e.shiftKey ? -1 : 1);
    return;
  }
  if (e.key !== 'Enter') return;
  e.preventDefault();
  if (barMode === 'find') {
    if (e.ctrlKey) window.aeval.invoke('find:to-ask', {});
    else window.aeval.invoke('find:step', { by: e.shiftKey ? -1 : 1 });
    return;
  }
  if (barMode === 'agent') {
    const task = input.value.trim();
    if (!task) {
      agentHint = 'Say what to do, for example "rename these files by date".';
      renderStatus();
      return;
    }
    window.aeval.invoke('agent:start', { task }).then((r) => {
      if (r && (r.status === 'started' || r.status === 'consent' || r.status === 'offline')) {
        input.value = '';
        agentHint = '';
      } else if (r && r.status === 'busy') {
        agentHint = 'A task is already running. Esc stops it.';
      } else if (r && r.status === 'unsupported') {
        agentHint = 'The agent needs Windows.';
      }
      renderStatus();
      renderNotice();
    });
    return;
  }
  submit();
});

input.addEventListener('input', () => {
  if (barMode !== 'find') return;
  // Literal hits are local and instant, so every keystroke goes straight
  // through; the model stages wait for a pause on the other side.
  queryFind();
});

// Window-level fallback for Escape. While the panel is open Escape is a
// global shortcut handled in the main process; this covers the rare case
// where that registration failed.
window.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    window.aeval.invoke('overlay:collapse');
  }
});

// Click-through management while idle: the window ignores mouse events with
// forwarding on, so mousemove still arrives here. Hit-testing turns back on
// only while the cursor is over the pill.
function setIgnore(ignore) {
  if (ignore === ignoreState) return;
  ignoreState = ignore;
  window.aeval.invoke('overlay:set-ignore', { ignore });
}

document.addEventListener('mousemove', (e) => {
  if (state.mode !== 'pill') return;
  const el = document.elementFromPoint(e.clientX, e.clientY);
  setIgnore(!(el && el.closest('#pill')));
});

document.addEventListener('mouseleave', () => {
  if (state.mode === 'pill') setIgnore(true);
});

// ---------------------------------------------------------- cursor buddy

window.aeval.on('buddy:watch', ({ selector }) => {
  watchSelector = selector;
});

window.aeval.on('buddy:resolve', ({ selector }) => {
  // A list of selectors means the first one that is showing: the tour points
  // at the pill, or at the bar if it is open.
  const el = [...document.querySelectorAll(selector)].find((x) => !x.hidden && !x.closest('[hidden]'));
  if (!el || el.hidden || el.closest('[hidden]')) {
    window.aeval.invoke('buddy:resolve-result', { found: false });
    return;
  }
  const r = el.getBoundingClientRect();
  window.aeval.invoke('buddy:resolve-result', {
    found: true,
    cx: window.screenX + r.left + r.width / 2,
    cy: window.screenY + r.top + r.height / 2,
    w: r.width,
    h: r.height,
  });
});

// Clicking the highlighted target lets the walkthrough advance naturally;
// clicking anywhere else dismisses it so it never traps the user.
document.addEventListener('click', (e) => {
  if (!watchSelector) return;
  if (!e.target.closest(watchSelector)) {
    window.aeval.invoke('buddy:dismiss');
  }
});
