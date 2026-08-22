'use strict';

const keyInput = document.getElementById('api-key-input');
const toggleKeyBtn = document.getElementById('toggle-key');
const saveKeyBtn = document.getElementById('save-key');
const keyStatus = document.getElementById('key-status');
const modelInput = document.getElementById('model-input');
const saveModelBtn = document.getElementById('save-model');
const modelStatus = document.getElementById('model-status');
const editPositionBtn = document.getElementById('edit-position');
const shortcutList = document.getElementById('shortcut-list');
const shortcutError = document.getElementById('shortcut-error');
const shortcutErrorText = document.getElementById('shortcut-error-text');
const resetBtn = document.getElementById('reset-shortcuts');
const closeBtn = document.getElementById('close-settings');
const quitBtn = document.getElementById('quit-app');

let data = { shortcuts: {}, active: {} };
let capturing = null; // { action, row, button } while recording a new combo
let watchSelector = null;
let statusTimer = null;

// ------------------------------------------------------------------ load

window.aeval.invoke('settings:get').then((d) => {
  data = d;
  modelInput.value = d.model || '';
  modelInput.placeholder = d.defaultModel || '';
  renderShortcuts();
});

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
      statusTimer = setTimeout(() => {
        keyStatus.textContent = '';
      }, 2500);
    } else {
      keyStatus.textContent = r.error;
    }
  });
}

saveKeyBtn.addEventListener('click', saveKey);
keyInput.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') {
    e.preventDefault();
    saveKey();
  }
});

// ----------------------------------------------------------------- model

let modelStatusTimer = null;

function saveModel() {
  window.aeval.invoke('settings:save-model', { model: modelInput.value }).then((r) => {
    if (!r.ok) return;
    modelInput.value = r.model;
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
  if (e.key === 'Escape' && !capturing) {
    window.aeval.invoke('settings:close');
  }
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
