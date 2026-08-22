'use strict';

const keyInput = document.getElementById('api-key-input');
const toggleKeyBtn = document.getElementById('toggle-key');
const saveKeyBtn = document.getElementById('save-key');
const testKeyBtn = document.getElementById('test-key');
const removeKeyBtn = document.getElementById('remove-key');
const keyState = document.getElementById('key-state');
const keyStateText = document.getElementById('key-state-text');
const keyStatus = document.getElementById('key-status');
const appVersion = document.getElementById('app-version');
const modelInput = document.getElementById('model-input');
const modelCustom = document.getElementById('model-custom');
const saveModelBtn = document.getElementById('save-model');
const modelStatus = document.getElementById('model-status');
const modelNote = document.getElementById('model-note');
const comboBtn = document.getElementById('model-combo-btn');
const comboLabel = document.getElementById('model-combo-label');
const comboDetail = document.getElementById('model-combo-detail');
const comboList = document.getElementById('model-combo-list');
const skillList = document.getElementById('skill-list');
const editPositionBtn = document.getElementById('edit-position');
const shortcutList = document.getElementById('shortcut-list');
const shortcutError = document.getElementById('shortcut-error');
const resetBtn = document.getElementById('reset-shortcuts');
const closeBtn = document.getElementById('close-settings');
const quitBtn = document.getElementById('quit-app');

let data = { shortcuts: {}, active: {} };
let modelIndex = new Map(); // id -> { name, note, detail, tier }
let offlineNote = '';
let capturing = null; // { action } while recording a new combo
let watchSelector = null;

// Status lines share one helper so none of them can leave stale text behind
// or reserve space while empty.
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
const setModelStatus = makeStatus(modelStatus);
const setShortcutError = makeStatus(shortcutError);

// ------------------------------------------------------------------ load

window.aeval.invoke('settings:get').then((d) => {
  data = d;
  modelInput.value = d.model || '';
  modelInput.placeholder = d.defaultModel || '';
  appVersion.textContent = d.version || '';
  renderKeyState(!!d.hasKey);
  renderSkills(d.skills || [], d.skill);
  renderShortcuts();
  loadModels();
});

function renderKeyState(saved) {
  keyState.classList.toggle('on', saved);
  keyStateText.textContent = saved ? 'Key saved' : 'No key yet';
  removeKeyBtn.hidden = !saved;
  testKeyBtn.disabled = !saved;
}

// --------------------------------------------------------------- API key

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
    setKeyStatus('Saved. Press "Test it" to check it works.');
  });
}

saveKeyBtn.addEventListener('click', saveKey);

testKeyBtn.addEventListener('click', () => {
  setKeyStatus('Checking...', true);
  window.aeval.invoke('settings:test-key').then((r) => {
    setKeyStatus(r.message, !r.ok);
  });
});

removeKeyBtn.addEventListener('click', () => {
  window.aeval.invoke('settings:clear-key').then(() => {
    renderKeyState(false);
    setKeyStatus('Key removed.');
  });
});

keyInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    e.preventDefault();
    saveKey();
  }
});

// ----------------------------------------------------------------- model

function loadModels() {
  window.aeval.invoke('models:list').then(({ groups, live }) => {
    comboList.innerHTML = '';
    modelIndex = new Map();
    for (const group of groups || []) {
      const head = document.createElement('li');
      head.className = 'combo-group';
      head.setAttribute('aria-hidden', 'true');
      head.textContent = group.label;
      comboList.appendChild(head);
      for (const m of group.models) {
        modelIndex.set(m.id, m);
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
        const detail = optionDetail(m);
        if (detail) {
          const d = document.createElement('span');
          d.className = 'opt-detail';
          d.textContent = detail;
          li.appendChild(d);
        }
        comboList.appendChild(li);
      }
    }
    offlineNote = live
      ? ''
      : 'Could not reach OpenRouter, so this is the built-in list. Prices and availability may have moved on.';
    syncCombo();
  });
}

function optionDetail(m) {
  return [m.note, m.detail].filter(Boolean).join(' ');
}

function syncCombo() {
  const current = modelInput.value.trim();
  for (const li of comboList.querySelectorAll('li[data-id]')) {
    const selected = li.dataset.id === current;
    li.classList.toggle('selected', selected);
    li.setAttribute('aria-selected', String(selected));
  }
  const known = modelIndex.get(current);
  comboLabel.textContent = known ? known.name : current || 'Choose a model...';
  // The collapsed control shows only price and context: it is one line, and
  // a description long enough to be useful gets cut off there anyway. The
  // description goes under the control, where it has room to wrap.
  comboDetail.textContent = known ? known.detail : current ? 'Custom model id' : '';
  modelNote.textContent = [known ? known.note : '', offlineNote].filter(Boolean).join(' ');
  // A model that is not in the list came from the custom field, so show that
  // field open rather than leaving the id invisible.
  if (current && !known) modelCustom.open = true;
}

function comboOpen() {
  comboList.hidden = false;
  comboBtn.setAttribute('aria-expanded', 'true');
  const target =
    comboList.querySelector('li.selected') || comboList.querySelector('li[data-id]');
  if (target) target.focus();
}

function comboClose(focusButton) {
  comboList.hidden = true;
  comboBtn.setAttribute('aria-expanded', 'false');
  if (focusButton) comboBtn.focus();
}

function chooseModel(id) {
  modelInput.value = id;
  saveModel();
  comboClose(true);
}

comboBtn.addEventListener('click', () => {
  if (comboList.hidden) comboOpen();
  else comboClose(false);
});

comboBtn.addEventListener('keydown', (e) => {
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    e.preventDefault();
    comboOpen();
  }
});

comboList.addEventListener('click', (e) => {
  const li = e.target.closest('li[data-id]');
  if (li) chooseModel(li.dataset.id);
});

comboList.addEventListener('keydown', (e) => {
  const options = [...comboList.querySelectorAll('li[data-id]')];
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
    if (idx >= 0) chooseModel(options[idx].dataset.id);
  } else if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    comboClose(true);
  } else if (e.key === 'Tab') {
    comboClose(false);
  }
});

document.addEventListener('click', (e) => {
  if (!comboList.hidden && !e.target.closest('#model-combo')) {
    comboClose(false);
  }
});

function saveModel() {
  window.aeval.invoke('settings:save-model', { model: modelInput.value }).then((r) => {
    if (!r.ok) return;
    modelInput.value = r.model;
    syncCombo();
    setModelStatus('Saved.');
  });
}

saveModelBtn.addEventListener('click', saveModel);
modelInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    e.preventDefault();
    saveModel();
  }
});

// ----------------------------------------------------------------- skills

function renderSkills(list, current) {
  skillList.innerHTML = '';
  for (const s of list) {
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
}

skillList.addEventListener('click', (e) => {
  const btn = e.target.closest('.skill-option');
  if (!btn) return;
  window.aeval.invoke('settings:save-skill', { skill: btn.dataset.id }).then((r) => {
    for (const other of skillList.querySelectorAll('.skill-option')) {
      other.setAttribute('aria-checked', String(other.dataset.id === r.skill));
    }
  });
});

// -------------------------------------------------------------- position

editPositionBtn.addEventListener('click', () => {
  window.aeval.invoke('hud:enter');
});

// -------------------------------------------------------------- shortcuts

function renderShortcuts() {
  for (const row of shortcutList.querySelectorAll('.sc-row[data-action]')) {
    const action = row.dataset.action;
    const kbd = row.querySelector('.sc-current');
    const warn = row.querySelector('.sc-warn');
    const button = row.querySelector('.sc-change');
    kbd.textContent = data.shortcuts[action] || '';
    warn.hidden = data.active[action] !== false;
    const isCapturing = capturing && capturing.action === action;
    row.classList.toggle('capturing', !!isCapturing);
    button.textContent = isCapturing ? 'Press keys' : 'Change';
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
  startCapture(button.closest('.sc-row').dataset.action, button);
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
      setShortcutError('Include Ctrl, Alt, or Win in the combo, then try again.', true);
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
        // Stay in capture mode so the user can immediately try another combo.
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

// ----------------------------------------------------------- window chrome

closeBtn.addEventListener('click', () => window.aeval.invoke('settings:close'));
quitBtn.addEventListener('click', () => window.aeval.invoke('app:quit'));

window.addEventListener('keydown', (e) => {
  if (e.key !== 'Escape' || capturing) return;
  if (!comboList.hidden) {
    comboClose(true);
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
