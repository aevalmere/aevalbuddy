'use strict';

const keyInput = document.getElementById('api-key-input');
const toggleKeyBtn = document.getElementById('toggle-key');
const saveKeyBtn = document.getElementById('save-key');
const testKeyBtn = document.getElementById('test-key');
const removeKeyBtn = document.getElementById('remove-key');
const keyStatus = document.getElementById('key-status');
const appVersion = document.getElementById('app-version');
const showPaid = document.getElementById('show-paid');
const brainEnabled = document.getElementById('brain-enabled');
const brainComboEl = document.getElementById('brain-combo');
const brainNoteEl = document.getElementById('brain-note');
const skillList = document.getElementById('skill-list');
const editPositionBtn = document.getElementById('edit-position');
const shortcutList = document.getElementById('shortcut-list');
const shortcutError = document.getElementById('shortcut-error');
const resetBtn = document.getElementById('reset-shortcuts');
const closeBtn = document.getElementById('close-settings');
const quitBtn = document.getElementById('quit-app');

let data = { shortcuts: {}, active: {} };
let capturing = null; // { action } while recording a new combo
let watchSelector = null;

// ------------------------------------------------------------- collapsing

// A group shows one line until Edit is pressed. Only one is open at a time,
// because two open groups is most of the way back to the wall of controls
// this replaced.
function openSection(id) {
  for (const sec of document.querySelectorAll('main section')) {
    const open = sec.id === id;
    sec.dataset.open = open ? '1' : '0';
    const edit = sec.querySelector('.edit[data-edit]');
    if (edit) edit.textContent = open ? 'Done' : 'Edit';
  }
}

function closeSections() {
  openSection(null);
}

document.addEventListener('click', (e) => {
  const btn = e.target.closest('.edit[data-edit]');
  if (!btn) return;
  const sec = document.getElementById(btn.dataset.edit);
  openSection(sec && sec.dataset.open === '1' ? null : btn.dataset.edit);
});

function setSummary(id, text) {
  const el = document.getElementById(id);
  if (el) el.textContent = text;
}

// Status lines share one helper so none can leave stale text behind or
// reserve space while empty.
function makeStatus(el) {
  let timer = null;
  return (text, sticky) => {
    clearTimeout(timer);
    el.textContent = text || '';
    el.hidden = !text;
    if (text && !sticky) {
      timer = setTimeout(() => {
        el.textContent = '';
        el.hidden = true;
      }, 3000);
    }
  };
}

const setKeyStatus = makeStatus(keyStatus);
const setShortcutError = makeStatus(shortcutError);

// --------------------------------------------------------------- API key

function renderKeyState(saved) {
  setSummary('key-summary', saved ? 'Saved' : 'Not set');
  removeKeyBtn.hidden = !saved;
  testKeyBtn.hidden = !saved;
}

toggleKeyBtn.addEventListener('click', () => {
  const show = keyInput.type === 'password';
  keyInput.type = show ? 'text' : 'password';
  toggleKeyBtn.textContent = show ? 'Hide' : 'Show';
  toggleKeyBtn.setAttribute('aria-pressed', String(show));
  keyInput.focus();
});

function saveKey() {
  window.aeval.invoke('settings:save-key', { key: keyInput.value }).then((r) => {
    if (!r.ok) {
      setKeyStatus(r.error, true);
      return;
    }
    keyInput.value = '';
    keyInput.type = 'password';
    toggleKeyBtn.textContent = 'Show';
    toggleKeyBtn.setAttribute('aria-pressed', 'false');
    renderKeyState(true);
    setKeyStatus('Saved.');
  });
}

saveKeyBtn.addEventListener('click', saveKey);

testKeyBtn.addEventListener('click', () => {
  setKeyStatus('Checking...', true);
  window.aeval.invoke('settings:test-key').then((r) => setKeyStatus(r.message, !r.ok));
});

removeKeyBtn.addEventListener('click', () => {
  window.aeval.invoke('settings:clear-key').then(() => {
    renderKeyState(false);
    setKeyStatus('Removed.');
  });
});

keyInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    e.preventDefault();
    saveKey();
  }
});

// ----------------------------------------------------------------- combos

// Two pickers with identical behaviour (the model that sees the screen, and
// the optional brain that answers from its reading), so the behaviour is
// written once and instantiated twice rather than copied.
function makeCombo(config) {
  const { rootId, button, label, detail, list, note, kind, save } = config;
  const state = { index: new Map(), offline: '', current: '' };

  function optionDetail(m) {
    return [m.note, m.detail].filter(Boolean).join(' ');
  }

  function sync() {
    for (const li of list.querySelectorAll('li[data-id]')) {
      const selected = li.dataset.id === state.current;
      li.classList.toggle('selected', selected);
      li.setAttribute('aria-selected', String(selected));
    }
    const known = state.index.get(state.current);
    label.textContent = known ? known.name : state.current || 'Choose...';
    if (config.summaryId) {
      setSummary(config.summaryId, known ? known.name : state.current || 'Not set');
    }
    // The collapsed control shows price and context only: it is one line, and
    // a description worth reading gets cut off there. The description goes
    // underneath, where it has room to wrap.
    detail.textContent = known ? known.detail : state.current ? 'Custom model id' : '';
    if (note) {
      note.textContent = [known ? known.note : '', state.offline].filter(Boolean).join(' ');
    }
  }

  function load() {
    return window.aeval.invoke('models:list', { kind }).then((res) => {
      list.innerHTML = '';
      state.index = new Map();
      if (res.current) state.current = res.current;
      for (const group of res.groups || []) {
        const head = document.createElement('li');
        head.className = 'combo-group';
        head.setAttribute('aria-hidden', 'true');
        head.textContent = group.label;
        list.appendChild(head);
        for (const m of group.models) {
          state.index.set(m.id, m);
          const li = document.createElement('li');
          li.className = 'combo-option';
          li.setAttribute('role', 'option');
          li.tabIndex = -1;
          li.dataset.id = m.id;
          li.title = m.id;
          const name = document.createElement('span');
          name.className = 'opt-name';
          name.textContent = m.name;
          li.appendChild(name);
          const d = optionDetail(m);
          if (d) {
            const el = document.createElement('span');
            el.className = 'opt-detail';
            el.textContent = d;
            li.appendChild(el);
          }
          list.appendChild(li);
        }
      }
      state.offline = res.live
        ? ''
        : 'Could not reach OpenRouter, so this is the built-in list.';
      sync();
    });
  }

  function open() {
    list.hidden = false;
    button.setAttribute('aria-expanded', 'true');
    const target = list.querySelector('li.selected') || list.querySelector('li[data-id]');
    if (target) target.focus();
  }

  function close(focusButton) {
    list.hidden = true;
    button.setAttribute('aria-expanded', 'false');
    if (focusButton) button.focus();
  }

  function choose(id) {
    state.current = id;
    save(id).then(() => sync());
    close(true);
  }

  button.addEventListener('click', () => {
    if (list.hidden) open();
    else close(false);
  });

  button.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      open();
    }
  });

  list.addEventListener('click', (e) => {
    const li = e.target.closest('li[data-id]');
    if (li) choose(li.dataset.id);
  });

  list.addEventListener('keydown', (e) => {
    const options = [...list.querySelectorAll('li[data-id]')];
    const idx = options.indexOf(document.activeElement);
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      (options[idx + 1] || options[0]).focus();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      (options[idx - 1] || options[options.length - 1]).focus();
    } else if (e.key === 'Home') {
      e.preventDefault();
      if (options[0]) options[0].focus();
    } else if (e.key === 'End') {
      e.preventDefault();
      if (options.length) options[options.length - 1].focus();
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      if (idx >= 0) choose(options[idx].dataset.id);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close(true);
    } else if (e.key === 'Tab') {
      close(false);
    }
  });

  document.addEventListener('click', (e) => {
    if (!list.hidden && !e.target.closest('#' + rootId)) close(false);
  });

  return {
    load,
    close,
    isOpen: () => !list.hidden,
    setCurrent: (id) => {
      state.current = id;
      sync();
    },
  };
}

const modelCombo = makeCombo({
  rootId: 'model-combo',
  button: document.getElementById('model-combo-btn'),
  label: document.getElementById('model-combo-label'),
  detail: document.getElementById('model-combo-detail'),
  list: document.getElementById('model-combo-list'),
  note: document.getElementById('model-note'),
  kind: 'vision',
  summaryId: 'model-summary',
  save: (id) => window.aeval.invoke('settings:save-model', { model: id }),
});

const brainCombo = makeCombo({
  rootId: 'brain-combo',
  button: document.getElementById('brain-combo-btn'),
  label: document.getElementById('brain-combo-label'),
  detail: document.getElementById('brain-combo-detail'),
  list: document.getElementById('brain-combo-list'),
  note: brainNoteEl,
  kind: 'text',
  save: (id) => window.aeval.invoke('settings:save-brain', { model: id }),
});

function showBrainPicker(on) {
  brainComboEl.hidden = !on;
  brainNoteEl.hidden = !on;
  setSummary('brain-summary', on ? brainName() : 'Off');
}

function brainName() {
  const el = document.getElementById('brain-combo-label');
  const name = el ? el.textContent.trim() : '';
  return name && name !== 'Loading...' ? name : 'On';
}

showPaid.addEventListener('change', () => {
  window.aeval
    .invoke('settings:set-show-paid', { on: showPaid.checked })
    .then(() => Promise.all([modelCombo.load(), brainCombo.load()]));
});

brainEnabled.addEventListener('change', () => {
  window.aeval
    .invoke('settings:set-brain-enabled', { on: brainEnabled.checked })
    .then((r) => showBrainPicker(r.brainEnabled));
});

// ----------------------------------------------------------------- skills

const AUTO = {
  id: 'auto',
  name: 'Automatic',
  summary: 'Picks from what is on screen and what you asked. Recommended.',
};

function renderSkills(list, current) {
  skillList.innerHTML = '';
  for (const s of [AUTO].concat(list)) {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'skill-option';
    btn.setAttribute('role', 'radio');
    btn.setAttribute('aria-checked', String(s.id === current));
    btn.dataset.id = s.id;

    const mark = document.createElement('span');
    mark.className = 'skill-mark';
    mark.setAttribute('aria-hidden', 'true');
    btn.appendChild(mark);

    const name = document.createElement('span');
    name.className = 'skill-name';
    name.textContent = s.name;
    btn.appendChild(name);

    if (s.summary) {
      const sum = document.createElement('span');
      sum.className = 'skill-summary';
      sum.textContent = s.summary;
      btn.appendChild(sum);
    }
    skillList.appendChild(btn);
  }
  const active = [AUTO].concat(list).find((s) => s.id === current) || AUTO;
  setSummary('skill-summary', active.name);
}

skillList.addEventListener('click', (e) => {
  const btn = e.target.closest('.skill-option');
  if (!btn) return;
  window.aeval.invoke('settings:save-skill', { skill: btn.dataset.id }).then((r) => {
    for (const other of skillList.querySelectorAll('.skill-option')) {
      const on = other.dataset.id === r.skill;
      other.setAttribute('aria-checked', String(on));
      if (on) setSummary('skill-summary', other.querySelector('.skill-name').textContent);
    }
  });
});

// -------------------------------------------------------------- shortcuts

function renderShortcuts() {
  setSummary(
    'shortcut-summary',
    [data.shortcuts.captureQuery, data.shortcuts.toggleVisibility].filter(Boolean).join(' · ')
  );
  for (const row of shortcutList.querySelectorAll('.sc-row[data-action]')) {
    const action = row.dataset.action;
    const kbd = row.querySelector('.sc-current');
    const warn = row.querySelector('.sc-warn');
    const button = row.querySelector('.sc-change');
    const isCapturing = capturing && capturing.action === action;
    kbd.textContent = isCapturing ? 'Press keys' : data.shortcuts[action] || '';
    warn.hidden = data.active[action] !== false;
    row.classList.toggle('capturing', !!isCapturing);
    button.textContent = isCapturing ? 'Cancel' : 'Change';
  }
}

function startCapture(action, button) {
  if (capturing) endCapture();
  capturing = { action };
  setShortcutError('');
  renderShortcuts();
  window.aeval.invoke('shortcuts:capture-mode', { on: true });
  button.focus();
}

function endCapture() {
  capturing = null;
  renderShortcuts();
  window.aeval.invoke('shortcuts:capture-mode', { on: false });
}

shortcutList.addEventListener('click', (e) => {
  const button = e.target.closest('.sc-change');
  if (!button) return;
  const action = button.closest('.sc-row').dataset.action;
  // The same button cancels, so a recording started by accident is not a
  // trap that only Escape gets out of.
  if (capturing && capturing.action === action) {
    endCapture();
    return;
  }
  startCapture(action, button);
});

const KEY_ALIASES = {
  ' ': 'Space',
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  '+': 'Plus',
  Enter: 'Enter',
  Tab: 'Tab',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  Insert: 'Insert',
};

function normalizeKey(e) {
  const k = e.key;
  if (['Control', 'Alt', 'AltGraph', 'Shift', 'Meta'].includes(k)) return null;
  if (KEY_ALIASES[k]) return KEY_ALIASES[k];
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(k)) return k;
  if (k.length === 1) return k.toUpperCase();
  return null;
}

window.addEventListener(
  'keydown',
  (e) => {
    if (!capturing) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape') {
      setShortcutError('');
      endCapture();
      return;
    }
    const key = normalizeKey(e);
    if (!key) return; // a bare modifier press; keep listening
    if (!e.ctrlKey && !e.altKey && !e.metaKey) {
      setShortcutError('Include Ctrl, Alt, or Win in the combo.', true);
      return;
    }
    const mods = [];
    if (e.ctrlKey) mods.push('Ctrl');
    if (e.altKey) mods.push('Alt');
    if (e.metaKey) mods.push('Super');
    if (e.shiftKey) mods.push('Shift');
    const accelerator = [...mods, key].join('+');
    window.aeval.invoke('shortcuts:set', { action: capturing.action, accelerator }).then((r) => {
      if (r.ok) {
        data.shortcuts = r.shortcuts;
        data.active = r.active;
        setShortcutError('');
        endCapture();
      } else {
        setShortcutError(r.error, true);
        // Stay in capture mode so another combo can be tried immediately.
      }
    });
  },
  true
);

resetBtn.addEventListener('click', () => {
  window.aeval.invoke('shortcuts:reset').then((r) => {
    data.shortcuts = r.shortcuts;
    data.active = r.active;
    setShortcutError('');
    renderShortcuts();
  });
});

editPositionBtn.addEventListener('click', () => {
  window.aeval.invoke('hud:enter');
});

// ----------------------------------------------------------- window chrome

closeBtn.addEventListener('click', () => window.aeval.invoke('settings:close'));
quitBtn.addEventListener('click', () => window.aeval.invoke('app:quit'));

// Links out open in the real browser, main-process validated to http/https.
document.addEventListener('click', (e) => {
  const a = e.target.closest('[data-external]');
  if (!a) return;
  e.preventDefault();
  window.aeval.invoke('open-external', { url: a.dataset.external });
});

window.addEventListener('keydown', (e) => {
  if (e.key !== 'Escape' || capturing) return;
  if (modelCombo.isOpen()) {
    modelCombo.close(true);
    return;
  }
  if (brainCombo.isOpen()) {
    brainCombo.close(true);
    return;
  }
  if (document.querySelector('main section[data-open="1"]')) {
    closeSections();
    return;
  }
  window.aeval.invoke('settings:close');
});

// ------------------------------------------------------------ cursor buddy

window.aeval.on('buddy:watch', ({ selector }) => {
  watchSelector = selector;
});

window.aeval.on('buddy:resolve', ({ selector }) => {
  const el = document.querySelector(selector);
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

document.addEventListener('click', (e) => {
  if (!watchSelector) return;
  if (!e.target.closest(watchSelector)) {
    window.aeval.invoke('buddy:dismiss');
  }
});

// ------------------------------------------------------------------ load

window.aeval.invoke('settings:get').then((d) => {
  data = d;
  appVersion.textContent = d.version || '';
  showPaid.checked = !!d.showPaid;
  brainEnabled.checked = !!d.brainEnabled;
  showBrainPicker(!!d.brainEnabled);
  renderKeyState(!!d.hasKey);
  renderSkills(d.skills || [], d.skill);
  renderShortcuts();
  modelCombo.setCurrent(d.model || '');
  brainCombo.setCurrent(d.brain || '');
  modelCombo.load();
  brainCombo.load().then(() => showBrainPicker(!!d.brainEnabled));
  // Nothing saved yet means the key field should be in front of the user
  // rather than one click behind an Edit link.
  openSection(d.hasKey ? null : 'sec-key');
});
