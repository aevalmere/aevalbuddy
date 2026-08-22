'use strict';

const keyInput = document.getElementById('api-key-input');
const toggleKeyBtn = document.getElementById('toggle-key');
const saveKeyBtn = document.getElementById('save-key');
const removeKeyBtn = document.getElementById('remove-key');
const keyState = document.getElementById('key-state');
const keyStateText = document.getElementById('key-state-text');
const appVersion = document.getElementById('app-version');
const keyStatus = document.getElementById('key-status');
const modelInput = document.getElementById('model-input');
const saveModelBtn = document.getElementById('save-model');
const modelStatus = document.getElementById('model-status');
const comboBtn = document.getElementById('model-combo-btn');
const comboLabel = document.getElementById('model-combo-label');
const comboList = document.getElementById('model-combo-list');
const editPositionBtn = document.getElementById('edit-position');
const shortcutList = document.getElementById('shortcut-list');
const shortcutError = document.getElementById('shortcut-error');
const shortcutErrorText = document.getElementById('shortcut-error-text');
const resetBtn = document.getElementById('reset-shortcuts');
const closeBtn = document.getElementById('close-settings');
const quitBtn = document.getElementById('quit-app');

let data = { shortcuts: {}, active: {} };
let models = [];
let capturing = null; // { action, row, button } while recording a new combo
let watchSelector = null;
let statusTimer = null;
let modelStatusTimer = null;

// ------------------------------------------------------------------ load

window.aeval.invoke('settings:get').then((d) => {
  data = d;
  modelInput.value = d.model || '';
  modelInput.placeholder = d.defaultModel || '';
  appVersion.textContent = d.version || '';
  renderKeyState(!!d.hasKey);
  renderShortcuts();
  loadModels();
});

function renderKeyState(saved) {
  keyState.classList.toggle('on', saved);
  keyStateText.textContent = saved ? 'Key saved' : 'No key yet';
  removeKeyBtn.hidden = !saved;
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
    clearTimeout(statusTimer);
    if (r.ok) {
      keyStatus.textContent = 'Saved.';
      keyInput.value = '';
      keyInput.type = 'password';
      toggleKeyBtn.textContent = 'Show';
      toggleKeyBtn.setAttribute('aria-pressed', 'false');
      renderKeyState(true);
      statusTimer = setTimeout(() => {
        keyStatus.textContent = '';
      }, 2500);
    } else {
      keyStatus.textContent = r.error;
    }
  });
}

saveKeyBtn.addEventListener('click', saveKey);

removeKeyBtn.addEventListener('click', () => {
  window.aeval.invoke('settings:clear-key').then(() => {
    clearTimeout(statusTimer);
    renderKeyState(false);
    keyStatus.textContent = 'Key removed.';
    statusTimer = setTimeout(() => {
      keyStatus.textContent = '';
    }, 2500);
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
  window.aeval.invoke('models:list').then(({ models: list, live }) => {
    models = list;
    comboList.innerHTML = '';
    if (!live) {
      const note = document.createElement('li');
      note.className = 'muted';
      note.textContent = 'Live list unavailable; showing known models';
      note.setAttribute('aria-disabled', 'true');
      comboList.appendChild(note);
    }
    for (const m of models) {
      const li = document.createElement('li');
      li.setAttribute('role', 'option');
      li.tabIndex = -1;
      li.dataset.id = m.id;
      li.textContent = m.name;
      li.title = m.id;
      comboList.appendChild(li);
    }
    syncCombo();
  });
}

function syncCombo() {
  const current = modelInput.value.trim();
  let currentName = null;
  for (const li of comboList.querySelectorAll('li[data-id]')) {
    const selected = li.dataset.id === current;
    li.classList.toggle('selected', selected);
    li.setAttribute('aria-selected', String(selected));
    if (selected) currentName = li.textContent;
  }
  comboLabel.textContent = currentName || current || 'Pick a free vision model...';
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
    clearTimeout(modelStatusTimer);
    modelStatus.textContent = 'Saved.';
    modelStatusTimer = setTimeout(() => {
      modelStatus.textContent = '';
    }, 2500);
  });
}

saveModelBtn.addEventListener('click', saveModel);
modelInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    e.preventDefault();
    saveModel();
  }
});

// -------------------------------------------------------------- position

editPositionBtn.addEventListener('click', () => {
  window.aeval.invoke('hud:enter');
});

// -------------------------------------------------------------- shortcuts

function renderShortcuts() {
  for (const row of shortcutList.querySelectorAll('.shortcut-row[data-action]')) {
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

function showShortcutError(message) {
  shortcutErrorText.textContent = message;
  shortcutError.hidden = false;
}

function clearShortcutError() {
  shortcutError.hidden = true;
  shortcutErrorText.textContent = '';
}

function startCapture(action, row, button) {
  if (capturing) endCapture();
  capturing = { action, row, button };
  clearShortcutError();
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
  const row = button.closest('.shortcut-row');
  startCapture(row.dataset.action, row, button);
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
      clearShortcutError();
      endCapture();
      return;
    }
    const key = normalizeKey(e);
    if (!key) return; // a bare modifier press; keep listening
    if (!e.ctrlKey && !e.altKey && !e.metaKey) {
      showShortcutError('Include Ctrl, Alt, or Win in the combo, then try again.');
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
        clearShortcutError();
        endCapture();
      } else {
        showShortcutError(r.error);
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
    clearShortcutError();
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
