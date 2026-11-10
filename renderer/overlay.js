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
const barEl = document.getElementById('bar');
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

// The orb's name carries what its rim and badge show (a11y N9).
const ORB_SAYS = {
  idle: 'Open AevalBuddy',
  chat: 'Open AevalBuddy, a conversation is waiting',
  busy: 'AevalBuddy is answering',
};

function render(prevMode) {
  // A fold that is still running is finished by whatever comes next.
  if (state.mode !== 'pill' || prevMode !== 'panel') endFold();
  document.body.dataset.mode = state.mode;
  // idle, chat (a conversation is waiting behind it), busy (an answer is on
  // its way) or agent (it has the computer): the orb shows which.
  const ps = state.pillState || 'idle';
  document.body.dataset.pill = ps;
  document.body.dataset.pillError = state.pillError || state.offline ? '1' : '0';
  pill.title = state.offline ? 'Offline' : state.pillError ? 'The last task did not finish' : '';
  pill.classList.toggle('orbit', ps === 'busy' || (ps === 'agent' && state.agent && state.agent.state !== 'paused'));
  let say = ORB_SAYS[ps] || ORB_SAYS.idle;
  if (state.agent && state.agent.running) {
    const n = Math.max(1, state.agent.step || 1);
    pillLabel.textContent = state.agent.state === 'paused' ? '||' : String(n);
    say = state.agent.state === 'paused' ? 'Task paused' : `Task running, step ${n}`;
  } else {
    pillLabel.textContent = '';
  }
  if (state.offline) say += ', offline';
  else if (state.pillError) say += ', the last task did not finish';
  pill.setAttribute('aria-label', say);
  if (state.inputWidth) {
    document.documentElement.style.setProperty('--panel-w', state.inputWidth + 'px');
  }
  applyAnchor();
  renderRegion();
  syncBarFromState();
  const folding = state.mode === 'pill' && prevMode === 'panel' && foldIntoOrb();
  pill.hidden = state.mode !== 'pill';
  panel.hidden = state.mode !== 'panel' && !folding;
  hud.hidden = state.mode !== 'hud';
  if (state.mode === 'pill' && prevMode !== 'pill') {
    // Main made the whole window click-through again on the way to the pill.
    // Without this the cache still says "taking clicks" from the bar, so the
    // first move over the orb never asks main to take them, and the orb
    // cannot be clicked until the pointer leaves and comes back.
    ignoreState = true;
  }
  if (state.mode === 'panel' && prevMode !== 'panel') {
    // Main turned hit-testing on for the whole window; the margin goes back
    // to click-through on the next mouse move.
    ignoreState = false;
    if (prevMode === 'pill') growFromOrb();
  }
  if (state.mode === 'panel') {
    renderNotice();
    if (prevMode !== 'panel') {
      readyPending = true;
      input.focus();
      input.select();
      typeEarlyKeys();
      barOpened();
      if (document.activeElement === input && document.hasFocus()) sendReady();
    }
    requestResize();
  }
}

// Rust times hotkey to typing: bar:ready goes once the field has focus
// after an open (U25). If the window is not focused yet, the field's focus
// event sends it when it is.
let readyPending = false;
function sendReady() {
  if (!readyPending) return;
  readyPending = false;
  window.aeval.invoke('bar:ready').catch(() => {});
}
input.addEventListener('focus', sendReady);

// Keys typed right after the hotkey belong to the field, even if they land
// before it has focus: a printable key without Ctrl or Alt anywhere in the
// open bar moves focus to the field first and then goes through (U25).
// Space still presses a focused button, and the menus keep their keys.
document.addEventListener(
  'keydown',
  (e) => {
    if (state.mode !== 'panel' || e.ctrlKey || e.altKey || e.metaKey || e.key.length !== 1) return;
    const a = document.activeElement;
    if (a === input || (a && a.closest && a.closest('#intent-menu, #model-menu, input, textarea'))) return;
    if (e.key === ' ' && a && a.closest && a.closest('button, [role="option"], [role="menuitem"]')) return;
    input.focus();
  },
  true
);

// The character itself, for the two cases keydown cannot handle. Typed while
// the window still shows the orb: the hotkey fired and the bar's state is on
// its way, or the bar just folded and the window kept the focus. Those keys
// are held for a moment and typed into the field when the bar opens. And a
// key whose keydown did not name its character (injected text arrives as
// VK_PACKET): it goes into the field here.
const EARLY_MS = 600;
let earlyKeys = [];
document.addEventListener(
  'keypress',
  (e) => {
    if (e.ctrlKey || e.altKey || e.metaKey || !e.key || e.key.length !== 1) return;
    if (state.mode === 'pill') {
      earlyKeys.push({ key: e.key, at: performance.now() });
      e.preventDefault();
      return;
    }
    if (state.mode !== 'panel') return;
    const a = document.activeElement;
    if (a === input || (a && a.closest && a.closest('#intent-menu, #model-menu, input, textarea'))) return;
    if (e.key === ' ' && a && a.closest && a.closest('button, [role="option"], [role="menuitem"]')) return;
    e.preventDefault();
    input.focus();
    typeIntoField(e.key);
  },
  true
);

function typeIntoField(text) {
  input.setRangeText(text, input.selectionStart, input.selectionEnd, 'end');
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

function typeEarlyKeys() {
  const now = performance.now();
  const text = earlyKeys
    .filter((k) => now - k.at < EARLY_MS)
    .map((k) => k.key)
    .join('');
  earlyKeys = [];
  if (text) typeIntoField(text);
}

// ----------------------------------------------------------------- morphs

// The orb and the bar are one piece of glass. Growing, the bar's glass
// layer starts as the orb's circle at the orb's place and its edges travel
// out to the capsule's; the contents fade in a beat behind. Folding is the
// same in reverse, then the orb takes over. Only the layer's insets and
// radius move, so nothing reflows, and it runs on the compositor.
function motionOn() {
  return document.documentElement.dataset.motion !== 'reduced' && !state.reducedMotion;
}

function orbInsets() {
  const g = barEl.querySelector(':scope > .gl');
  const o = state.orbAt;
  if (!g || !o) return null;
  const br = barEl.getBoundingClientRect();
  // The orb's rect inside this window: the window's origin is the bar's
  // glass rect minus the stage margin, and the panel sits in that margin.
  const pr = panel.getBoundingClientRect();
  const ox = pr.left + o.x;
  const oy = pr.top + o.y;
  return {
    g,
    t: oy - br.top,
    l: ox - br.left,
    r: br.right - (ox + o.width),
    b: br.bottom - (oy + o.height),
  };
}

function setInsets(g, v, radius) {
  g.style.setProperty('--gi-t', `${v ? v.t : 0}px`);
  g.style.setProperty('--gi-l', `${v ? v.l : 0}px`);
  g.style.setProperty('--gi-r', `${v ? v.r : 0}px`);
  g.style.setProperty('--gi-b', `${v ? Math.max(0, v.b) : 0}px`);
  if (radius) g.style.setProperty('--gr', radius);
  else g.style.removeProperty('--gr');
}

let morphTimer = null;

function growFromOrb() {
  clearTimeout(morphTimer);
  panel.dataset.morph = 'start';
  if (!motionOn()) {
    panel.dataset.morph = 'in';
    morphTimer = setTimeout(() => delete panel.dataset.morph, 300);
    return;
  }
  const v = orbInsets();
  if (!v) {
    delete panel.dataset.morph;
    return;
  }
  const g = v.g;
  g.style.transition = 'none';
  setInsets(g, v, '22px');
  void g.offsetWidth;
  if (window.AevalGlass) window.AevalGlass.morph(320);
  g.style.transition = 'inset 320ms var(--ease-morph), border-radius 320ms var(--ease-morph)';
  setInsets(g, null, null);
  panel.dataset.morph = 'in';
  morphTimer = setTimeout(() => {
    g.style.transition = '';
    delete panel.dataset.morph;
  }, 520);
}

let folding = false;

function foldIntoOrb() {
  if (!motionOn() || panel.hidden) return false;
  const v = orbInsets();
  if (!v) return false;
  clearTimeout(morphTimer);
  folding = true;
  document.body.dataset.folding = '1';
  panel.dataset.morph = 'out';
  const g = v.g;
  if (window.AevalGlass) window.AevalGlass.morph(200);
  g.style.transition = 'inset 200ms var(--ease-glide), border-radius 200ms var(--ease-glide)';
  setInsets(g, v, '22px');
  morphTimer = setTimeout(endFold, 200);
  return true;
}

function endFold() {
  if (!folding) return;
  folding = false;
  clearTimeout(morphTimer);
  const g = barEl.querySelector(':scope > .gl');
  if (g) {
    g.style.transition = 'none';
    setInsets(g, null, null);
    void g.offsetWidth;
    g.style.transition = '';
  }
  delete panel.dataset.morph;
  delete document.body.dataset.folding;
  if (state.mode !== 'panel') panel.hidden = true;
}

// The chip is the only thing telling the user that "what about now?" is
// about a rectangle rather than the desktop.
function renderRegion() {
  const r = state.region;
  document.body.dataset.region = r ? '1' : '0';
  regionChip.hidden = !r;
  if (r) regionSize.textContent = `Asking about a ${r.width} × ${r.height} area`;
}

// Something that went wrong (the shortcut is taken, capture failed) is said
// at once; a standing state (no provider, offline) waits its turn.
function renderNotice() {
  const was = notice.hidden ? '' : noticeText.textContent;
  const now = barIntent();
  let urgent = false;
  // Notices name what is needed, never one vendor (copy deck C-1..C-5).
  if (state.hotkeyFailed) {
    urgent = true;
    noticeText.textContent = `Another app already uses ${state.hotkeyFailed}, so it can't open AevalBuddy. Choose another shortcut.`;
    noticeSettings.textContent = 'Change shortcut';
    noticeSettings.dataset.section = 'pg-shortcuts';
    noticeSettings.hidden = false;
    notice.hidden = false;
  } else if (!state.hasKey && now !== 'find') {
    noticeText.textContent =
      now === 'do' ? 'Tasks need an AI provider.' : 'Answers need an AI provider. Finding words on screen works without one.';
    noticeSettings.textContent = 'Connect a provider';
    noticeSettings.dataset.section = 'pg-providers';
    noticeSettings.hidden = false;
    notice.hidden = false;
  } else if (state.offline && now !== 'find') {
    noticeText.textContent = 'No connection. Finding words on screen still works; answers need the internet.';
    noticeSettings.hidden = true;
    notice.hidden = false;
  } else if (!state.captureOk && now === 'ask') {
    urgent = true;
    noticeText.textContent =
      'Screen capture failed. Your question will be sent without a screenshot.';
    noticeSettings.hidden = true;
    notice.hidden = false;
  } else {
    notice.hidden = true;
  }
  if (!notice.hidden && noticeText.textContent !== was) announce(noticeText.textContent, urgent ? 'assertive' : 'polite');
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
// The notice's button opens Settings where the fix is: Providers for a
// missing provider, Shortcuts for a taken shortcut.
noticeSettings.addEventListener('click', () => {
  const section = noticeSettings.dataset.section || '';
  window.aeval.invoke('overlay:open-settings', section ? { section } : undefined);
});
closeBtn.addEventListener('click', () => {
  window.aeval.invoke('overlay:minimize');
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

// The open bar works the same way now: the window carries a transparent
// margin for the shadow (lib/stage.js), and a click there belongs to the
// app underneath.
document.addEventListener('mousemove', (e) => {
  if (state.mode === 'pill') {
    const el = document.elementFromPoint(e.clientX, e.clientY);
    setIgnore(!(el && el.closest('#pill')));
  } else if (state.mode === 'hud') {
    setIgnore(false);
  } else if (state.mode === 'panel' && !folding) {
    const el = document.elementFromPoint(e.clientX, e.clientY);
    setIgnore(!(el && el.closest('#panel .glass, #intent-menu, #examples .glass, .frame-handle')));
  }
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
