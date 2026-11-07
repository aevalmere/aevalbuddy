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
const pillLabel = document.querySelector('.pill-label');
const barStatus = document.getElementById('bar-status');
const barStatusText = document.getElementById('bar-status-text');
const barStatusAction = document.getElementById('bar-status-action');
const findScopeBtn = document.getElementById('find-scope');

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
    pillLabel.textContent = 'Ask';
  }
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
      barOpened();
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
  const was = notice.hidden ? '' : noticeText.textContent;
  const now = barIntent();
  if (state.hotkeyFailed) {
    noticeText.textContent = `${state.hotkeyFailed} is taken by another app, so it does nothing. Pick another shortcut in Settings.`;
    noticeSettings.hidden = false;
    notice.hidden = false;
  } else if (!state.hasKey && now !== 'find') {
    noticeText.textContent =
      now === 'do' ? 'Doing tasks needs an OpenRouter API key.' : 'No OpenRouter API key saved yet. Finding words on screen works without one.';
    noticeSettings.hidden = false;
    notice.hidden = false;
  } else if (state.offline && now !== 'find') {
    noticeText.textContent = 'No connection. Finding words on screen still works; answers need the internet.';
    noticeSettings.hidden = true;
    notice.hidden = false;
  } else if (!state.captureOk && now === 'ask') {
    noticeText.textContent =
      'Screen capture failed. Your question will be sent without a screenshot.';
    noticeSettings.hidden = true;
    notice.hidden = false;
  } else {
    notice.hidden = true;
  }
  if (!notice.hidden && noticeText.textContent !== was) announce(noticeText.textContent);
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

// Arrows move the bar between the snap points while the editor is open;
// Enter or Done saves.
const HUD_KEYS = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
document.addEventListener('keydown', (e) => {
  if (state.mode !== 'hud') return;
  const d = HUD_KEYS[e.key];
  if (d) {
    e.preventDefault();
    window.aeval.invoke('hud:nudge', { dx: d[0], dy: d[1] });
  } else if (e.key === 'Enter') {
    e.preventDefault();
    window.aeval.invoke('hud:exit');
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
