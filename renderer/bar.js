'use strict';

// The single input. One field takes a question, a word to find, or a task,
// and the token at its left says which one Enter will do. The guess comes
// from renderer/intent.js; a prefix (/ ? > //), Ctrl+1/2/3 or the token's menu
// (Alt+Down) settles it outright. A guessed task never runs on Enter: it
// shows a card, and running it takes a click or Alt+Enter.
//
// Loaded after overlay.js, which owns the window state, the notice and the
// resize; both are classic scripts sharing one global scope.
//
// The bar is the app's only text input (REQUESTS.md 9). A follow-up goes to
// the focused or most recent output window (Rust routes query:submit), and
// when a task asks a question or waits on its plan, Enter here answers it.

const Intent = window.AevalIntent;
const token = document.getElementById('intent');
const tokenName = token.querySelector('.intent-name');
const intentDesc = document.getElementById('intent-desc');
const intentMenu = document.getElementById('intent-menu');
const guess = document.getElementById('guess');
const guessText = document.getElementById('guess-text');
const guessAnswer = document.getElementById('guess-answer');
const guessRun = document.getElementById('guess-run');
const guessFind = document.getElementById('guess-find');
const examplesEl = document.getElementById('examples');
const announcer = document.getElementById('announce');

const PREFIX = { find: '/', ask: '?', do: '>' };
const ENTER_SAYS = { ask: 'Enter asks', find: 'Enter goes to the next match', do: 'Enter starts the task' };
// Short enough to sit beside the sigil legend in a 420px capsule at 18px;
// the field's accessible name keeps the full sentence.
const PLACEHOLDER = 'Ask, find, or do';

// lock: null while guessing, else the intent the person chose.
// literal: // was typed, so Find stays on exact words.
const bar = {
  lock: null,
  literal: false,
  guess: Intent.classify('', {}),
  screen: '',
  screenFolded: '',
  history: [],
  recall: -1,
  draft: '',
  announced: '',
  findOn: false,
  findSwitch: null,
  announceTimer: null,
};
let findState = null;
let barHint = '';

// ------------------------------------------------------------- the guess

function barIntent() {
  return bar.lock || bar.guess.intent;
}

function classifyNow() {
  bar.guess = Intent.score(input.value, { screenFolded: bar.screenFolded });
  renderToken();
}

// The token's width follows its word: measured before and after the change
// and animated between the two (FLIP), so the capsule morphs instead of
// snapping. The field beside it never moves; the token has room reserved.
let tokenFlip = null;
function flipToken(change) {
  const w0 = token.getBoundingClientRect().width;
  change();
  const w1 = token.getBoundingClientRect().width;
  if (document.documentElement.dataset.motion === 'reduced' || !w0 || Math.abs(w1 - w0) < 1) return;
  clearTimeout(tokenFlip);
  token.style.transition = 'none';
  token.style.width = `${w0}px`;
  void token.offsetWidth;
  token.style.transition = 'width 240ms var(--ease-morph), background 160ms ease, box-shadow 160ms ease';
  token.style.width = `${w1}px`;
  tokenFlip = setTimeout(() => {
    token.style.width = '';
    token.style.transition = '';
  }, 260);
}

function renderToken() {
  const now = barIntent();
  const wasLocked = token.dataset.locked === 'true';
  const name = Intent.NAME[now];
  if (tokenName.textContent !== name || token.dataset.intent !== now) {
    flipToken(() => {
      token.dataset.intent = now;
      tokenName.textContent = name;
    });
    replayAnim(token, 'swap');
  }
  token.dataset.locked = bar.lock ? 'true' : 'false';
  token.dataset.confidence = bar.lock || bar.guess.margin >= 0.25 ? 'high' : 'low';
  if (bar.lock && !wasLocked) replayAnim(token, 'lock');
  const how = bar.lock ? 'chosen' : 'guessed';
  token.setAttribute('aria-label', `${Intent.NAME[now]}, ${how}. Choose what Enter does.`);
  intentDesc.textContent = `${ENTER_SAYS[now]}${bar.literal ? ', exact words only' : ''}. Ctrl+1 asks, Ctrl+2 finds, Ctrl+3 does it as a task.`;
  // Said once the typing settles, and only when it changed; announcing every
  // keystroke's guess would talk over the person typing.
  clearTimeout(bar.announceTimer);
  bar.announceTimer = setTimeout(() => {
    const say = `${Intent.NAME[barIntent()]}${bar.lock ? '' : ', guessed'}`;
    if (say !== bar.announced && input.value.trim()) {
      bar.announced = say;
      announce(say);
    }
  }, 700);
  renderGuess();
  syncFind();
  renderStatus();
  renderNotice();
}

function setLock(next, opts) {
  const o = opts || {};
  bar.lock = next && Intent.INTENTS.includes(next) ? next : null;
  bar.literal = bar.lock === 'find' && !!o.literal;
  renderToken();
  if (o.say) announce(bar.lock ? `${Intent.NAME[bar.lock]}, chosen` : 'Guessing again');
}

// A prefix typed at the start becomes the lock and leaves the text.
// "/etc/hosts" is a path, not a Find: a / whose first word has another /
// in it stays text, and a lock taken too early is handed back.
function takePrefix() {
  const v = input.value;
  if (bar.lock === 'find' && bar.prefixed === '/' && /^\S*\//.test(v)) {
    const at = input.selectionStart;
    input.value = `/${v}`;
    input.setSelectionRange(at + 1, at + 1);
    bar.prefixed = '';
    setLock(null);
    return;
  }
  if (bar.lock) return;
  let p = null;
  if (v.startsWith('//')) p = { lock: 'find', literal: true, n: 2 };
  else if (/^\/[^/]/.test(v) && Intent.parse(v).lock === 'find') p = { lock: 'find', n: 1 };
  else if (v.startsWith('?')) p = { lock: 'ask', n: 1 };
  else if (v.startsWith('>')) p = { lock: 'do', n: 1 };
  if (!p) return;
  const at = Math.max(0, input.selectionStart - p.n);
  input.value = v.slice(p.n).replace(/^\s/, '');
  input.setSelectionRange(at, at);
  bar.prefixed = p.n === 2 ? '//' : v[0];
  setLock(p.lock, { literal: p.literal, say: true });
}

// ------------------------------------------------------------- the menu

function openMenu() {
  intentMenu.hidden = false;
  token.setAttribute('aria-expanded', 'true');
  const opts = [...intentMenu.querySelectorAll('[role="option"]')];
  for (const o of opts) o.setAttribute('aria-selected', o.dataset.intent === barIntent() ? 'true' : 'false');
  const cur = opts.find((o) => o.dataset.intent === barIntent()) || opts[0];
  cur.focus();
  requestResize();
}

function closeMenu(refocus) {
  if (intentMenu.hidden) return;
  intentMenu.hidden = true;
  token.setAttribute('aria-expanded', 'false');
  if (refocus) input.focus();
  requestResize();
}

token.addEventListener('click', () => (intentMenu.hidden ? openMenu() : closeMenu(true)));
intentMenu.addEventListener('click', (e) => {
  const o = e.target.closest('[role="option"]');
  if (!o) return;
  setLock(o.dataset.intent, { say: true });
  closeMenu(true);
});
intentMenu.addEventListener('keydown', (e) => {
  const opts = [...intentMenu.querySelectorAll('[role="option"]')];
  const i = opts.indexOf(document.activeElement);
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    opts[(i + (e.key === 'ArrowDown' ? 1 : -1) + opts.length) % opts.length].focus();
  } else if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault();
    if (i >= 0) setLock(opts[i].dataset.intent, { say: true });
    closeMenu(true);
  } else if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    closeMenu(true);
  } else if (e.key === 'Tab') {
    closeMenu(false);
  }
});

// ------------------------------------------------------------ guess card

// A task that was only guessed. Enter answers it as a question; doing it on
// the computer is a click, or Alt+Enter while the card is showing.
function renderGuess() {
  const show = !bar.lock && bar.guess.intent === 'do' && !!input.value.trim();
  if (guess.hidden === !show) return;
  guess.hidden = !show;
  if (show) {
    guessText.textContent = 'This reads like a task. Enter answers it; AevalBuddy can also do it on your computer.';
    announce('This reads like a task. Enter answers it. Alt+Enter runs it.');
  }
  requestResize();
}

guessAnswer.addEventListener('click', () => send('ask'));
guessRun.addEventListener('click', () => send('do'));
guessFind.addEventListener('click', () => {
  setLock('find', { say: true });
  input.focus();
});

// ------------------------------------------------------------ live Find

// Find runs while Find is what Enter will do: every keystroke for exact
// words, and the model stages after a pause (or after a yes, by setting).
// A guess that flickers between Find and Ask while typing waits a moment
// before the lens is opened or closed, so it does not blink.
function syncFind() {
  const want = barIntent() === 'find' && !!input.value.trim();
  clearTimeout(bar.findSwitch);
  if (want === bar.findOn) {
    if (want) queryFind();
    return;
  }
  const go = () => {
    bar.findOn = want;
    window.aeval.invoke('bar:mode', { mode: want ? 'find' : 'ask' });
    if (want) queryFind();
  };
  if (bar.lock) go();
  else bar.findSwitch = setTimeout(go, 250);
}

function queryFind() {
  window.aeval.invoke('find:query', { query: input.value, literal: bar.literal });
}

// One line under the bar: Find's count and what it is doing, or a hint.
function renderStatus() {
  let text = '';
  let action = null;
  if (bar.findOn && barIntent() === 'find' && findState && findState.open) {
    text = [findState.status, ...(findState.lines || [])].filter(Boolean).join('\n');
    if (findState.action === 'related') {
      action = { label: 'Look for related', run: () => window.aeval.invoke('find:related') };
    } else if (findState.action === 'ask' || (findState.hits === 0 && input.value.trim())) {
      action = { label: 'Ask instead', run: () => send('ask') };
    }
    findScopeBtn.hidden = false;
    findScopeBtn.textContent = findState.scope === 'screen' ? 'Whole screen' : 'This window';
  } else {
    findScopeBtn.hidden = true;
  }
  if (barHint) text = barHint;
  if (barStatusText.textContent !== text && text) announceSoon(text);
  barStatusText.textContent = text;
  barStatusAction.hidden = !action;
  if (action) {
    barStatusAction.textContent = action.label;
    barStatusAction.onclick = action.run;
  }
  const show = !!text || !findScopeBtn.hidden;
  if (barStatus.hidden === show) barStatus.hidden = !show;
  requestResize();
}

findScopeBtn.addEventListener('click', () => window.aeval.invoke('find:scope', {}));

window.aeval.on('find:state', (s) => {
  // hits: how many there are once Find has read the window and every stage
  // has settled; -1 while that is still unknown.
  const c = (s && s.counts) || { exact: 0, related: 0 };
  const busy = !s || s.reading || (s.stage && (s.stage.related === 'running' || s.stage.concept === 'running'));
  findState = s && s.open ? { ...s, hits: c.exact + c.related || (busy ? -1 : 0) } : s;
  if (s && s.open === false) bar.findOn = false;
  renderStatus();
});

// --------------------------------------------------------------- examples

// Starting points from the app in front, under an empty bar. Each one fills
// the field, so it also shows what the guess will make of it.
function renderExamples(list) {
  examplesEl.textContent = '';
  for (const ex of list.slice(0, 3)) {
    const b = document.createElement('button');
    b.type = 'button';
    b.className = 'example-chip glass thin interactive';
    b.textContent = ex;
    b.addEventListener('click', () => {
      const m = /^Find "(.+)"$/.exec(ex);
      if (m) {
        input.value = m[1];
        setLock('find');
      } else {
        input.value = ex;
        setLock(null);
      }
      input.focus();
      onInput();
    });
    examplesEl.appendChild(b);
  }
  renderExamplesShown();
}

// Suggestions are for someone who has not used the bar yet: shown until the
// first request is sent, then never again (REQUESTS.md 3). Rust keeps the
// flag in the settings file (hasSentFirstRequest, sent in overlay:state);
// localStorage holds it too, so the chips go at once, before Rust answers.
const FIRST_KEY = 'aeval.hasSentFirstRequest';
function sentBefore() {
  if (state.hasSentFirstRequest) return true;
  try {
    return localStorage.getItem(FIRST_KEY) === '1';
  } catch (err) {
    return false;
  }
}

function markSent() {
  if (sentBefore()) return;
  try {
    localStorage.setItem(FIRST_KEY, '1');
  } catch (err) {
    // Rust's copy of the flag still lands with the request.
  }
  state.hasSentFirstRequest = true;
  examplesEl.textContent = '';
  renderExamplesShown();
}

function renderExamplesShown() {
  const show = !sentBefore() && !input.value && examplesEl.childElementCount > 0;
  if (examplesEl.hidden === !show) return;
  examplesEl.hidden = !show;
  requestResize();
}

window.aeval.on('bar:context', (c) => {
  bar.screen = String((c && c.screen) || '');
  bar.screenFolded = Intent.fold(bar.screen);
  if (!sentBefore()) renderExamples(Intent.examples(c && c.process, c && c.title));
  if (input.value.trim()) classifyNow();
});

// ---------------------------------------------------------------- sending

function pushHistory(text) {
  const t = String(text || '').trim();
  if (!t) return;
  window.aeval.invoke('bar:history-push', { text: t }).then((h) => {
    if (Array.isArray(h)) bar.history = h;
  });
  bar.recall = -1;
}

function clearField() {
  input.value = '';
  bar.prefixed = '';
  setLock(null);
  renderExamplesShown();
}

// what: the intent to send as. A guessed task sent with plain Enter goes
// as a question.
function send(what) {
  const text = input.value;
  const body = text.trim();
  barHint = '';
  if (body) markSent();
  const recallAs = bar.lock && PREFIX[bar.lock] ? `${bar.literal ? '//' : PREFIX[bar.lock]}${body}` : body;
  if (what === 'find') {
    window.aeval.invoke('find:step', { by: 1 });
    pushHistory(recallAs);
    return;
  }
  if (what === 'do') {
    if (!body) {
      barHint = 'Say what to do, for example "rename these files by date".';
      renderStatus();
      return;
    }
    pushHistory(`>${body}`);
    window.aeval.invoke('agent:start', { task: body }).then((r) => {
      // Waiting on consent keeps the words: declining must not cost them.
      if (r && (r.status === 'started' || r.status === 'offline')) clearField();
      else if (r && r.status === 'busy') barHint = 'A task is already running. Esc stops it.';
      else if (r && r.status === 'unsupported') barHint = 'Doing tasks needs Windows.';
      else if (r && r.status === 'needs-key') barHint = '';
      renderStatus();
      renderNotice();
    });
    return;
  }
  pushHistory(recallAs);
  const inferred = !bar.lock;
  // Cleared at once, like a messaging app; the transcript echoes it. With
  // no key the text stays so nothing typed is lost.
  if (state.hasKey) clearField();
  window.aeval.invoke('query:submit', { question: body, inferred }).then((result) => {
    if (result && result.status === 'walkthrough') clearField();
  });
}

// A run that is waiting on the person: a question (asking), its plan
// (plan) or a pause. Rust says so in overlay:state.agent.
function agentWaiting() {
  const a = state.agent;
  if (!a || !a.running) return '';
  return a.state === 'asking' || a.state === 'plan' || a.state === 'paused' ? a.state : '';
}

const WAITING_PLACEHOLDER = {
  asking: 'Answer the task’s question',
  plan: 'Enter starts the task',
  paused: 'Enter continues the task',
};

// Called by overlay.js whenever the state changes.
function syncBarFromState() {
  const w = agentWaiting();
  input.placeholder = WAITING_PLACEHOLDER[w] || PLACEHOLDER;
  input.setAttribute('aria-label', WAITING_PLACEHOLDER[w] || 'Ask, find, or tell me what to do');
  document.body.dataset.waiting = w || '';
  renderExamplesShown();
}

function onEnter(e) {
  const waiting = agentWaiting();
  const typed = input.value.trim();
  if (waiting === 'asking' && typed) {
    pushHistory(typed);
    window.aeval.invoke('agent:continue', { text: typed });
    clearField();
    return;
  }
  if ((waiting === 'plan' || waiting === 'paused') && !typed) {
    window.aeval.invoke('agent:continue', {});
    return;
  }
  const now = barIntent();
  if (now === 'find') {
    if (e.ctrlKey) {
      window.aeval.invoke('find:to-ask', {});
      return;
    }
    // A guessed Find with nothing on screen was a question after all.
    if (!bar.lock && findState && findState.open && findState.hits === 0) {
      send('ask');
      return;
    }
    if (e.shiftKey) window.aeval.invoke('find:step', { by: -1 });
    else send('find');
    return;
  }
  if (now === 'do' && bar.lock) {
    send('do');
    return;
  }
  if (now === 'do' && e.altKey && !guess.hidden) {
    send('do');
    return;
  }
  send('ask');
}

// ---------------------------------------------------------------- keyboard

function onInput() {
  barHint = '';
  if (bar.recall >= 0 && input.value !== bar.history[bar.recall]) bar.recall = -1;
  takePrefix();
  classifyNow();
  renderExamplesShown();
}

input.placeholder = PLACEHOLDER;
input.addEventListener('input', onInput);

input.addEventListener('keydown', (e) => {
  // An IME is still composing: Enter picks the candidate, not the action.
  if (e.isComposing || e.keyCode === 229) return;
  if (e.ctrlKey && !e.altKey && !e.shiftKey && ['1', '2', '3'].includes(e.key)) {
    e.preventDefault();
    setLock(Intent.INTENTS[Number(e.key) - 1], { say: true });
    return;
  }
  if (e.altKey && e.key === 'ArrowDown') {
    e.preventDefault();
    openMenu();
    return;
  }
  if (e.key === 'F6') {
    e.preventDefault();
    window.aeval.invoke('ui:focus-answer');
    return;
  }
  // The first Backspace on an empty field gives the lock back.
  if (e.key === 'Backspace' && bar.lock && input.selectionStart === 0 && input.selectionEnd === 0) {
    e.preventDefault();
    bar.prefixed = '';
    setLock(null, { say: true });
    return;
  }
  if ((e.key === 'ArrowUp' || e.key === 'ArrowDown') && !e.altKey && recallKey(e.key === 'ArrowUp' ? -1 : 1)) {
    e.preventDefault();
    return;
  }
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    // Text first, then the bar.
    if (input.value || bar.lock) {
      clearField();
      announce('Cleared');
    } else {
      window.aeval.invoke('overlay:collapse');
    }
    return;
  }
  if (e.key !== 'Enter') return;
  e.preventDefault();
  onEnter(e);
});

// Up recalls older entries while the field is empty or still showing one;
// Down walks back toward what was being typed.
function recallKey(dir) {
  const h = bar.history;
  if (!h.length) return false;
  if (bar.recall < 0 && (dir > 0 || input.value)) return false;
  if (bar.recall < 0) {
    bar.draft = input.value;
    bar.recall = h.length;
  }
  const next = bar.recall + dir;
  if (next >= h.length) {
    bar.recall = -1;
    input.value = bar.draft;
  } else if (next >= 0) {
    bar.recall = next;
    input.value = h[next];
  } else {
    return true;
  }
  setLock(null);
  takePrefix();
  classifyNow();
  input.setSelectionRange(input.value.length, input.value.length);
  renderExamplesShown();
  return true;
}

// Escape anywhere else in the window: the menu, then the bar.
window.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && !e.defaultPrevented) {
    if (!intentMenu.hidden) closeMenu(true);
    else window.aeval.invoke('overlay:collapse');
  }
  if (e.key === 'F6' && !e.defaultPrevented) window.aeval.invoke('ui:focus-answer');
});

// ------------------------------------------------------------- announcer

// The window's one live region. Notices, Find counts and the guess all
// speak through it, so a screen reader hears one voice, not three.
function announce(text) {
  announcer.textContent = '';
  requestAnimationFrame(() => {
    announcer.textContent = String(text || '');
  });
}

let announceSoonTimer = null;
function announceSoon(text) {
  clearTimeout(announceSoonTimer);
  announceSoonTimer = setTimeout(() => announce(text), 500);
}

// ------------------------------------------------------------- from main

// The bar opens unlocked, guessing, unless a flow asked for one action.
window.aeval.on('bar:open', ({ lock }) => {
  const l = lock === 'agent' ? 'do' : lock;
  bar.prefixed = '';
  setLock(l || null);
});

window.aeval.on('bar:prefill', ({ text, mode }) => {
  input.value = text || '';
  bar.prefixed = '';
  setLock(mode === 'agent' ? 'do' : mode === 'find' ? 'find' : mode === 'ask' ? null : bar.lock);
  input.focus();
  renderExamplesShown();
});

// A task that started from the bar leaves it; one waiting on consent keeps
// its words until it really starts.
window.aeval.on('agent:begin', ({ task }) => {
  if (input.value.trim() === String(task || '').trim()) clearField();
});

function barOpened() {
  bar.recall = -1;
  // Closing the bar closed Find with it.
  bar.findOn = false;
  window.aeval.invoke('bar:history').then((h) => {
    if (Array.isArray(h)) bar.history = h;
  });
  renderToken();
  renderExamplesShown();
}

renderToken();
