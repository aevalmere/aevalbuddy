'use strict';

const Store = require('electron-store');
const { safeStorage } = require('electron');
const { DEFAULT_MODEL } = require('./openrouter');

const DEFAULT_SHORTCUTS = {
  captureQuery: 'Ctrl+Shift+Space',
  toggleVisibility: 'Ctrl+Shift+H',
};

// Overlay anchor: which screen edge each axis hugs, plus the offset from
// that edge. 'center' ignores the offset on its axis.
const DEFAULT_OVERLAY_POS = { h: 'right', v: 'top', offX: 16, offY: 16 };

// The answer window either docks to the input bar (follows it around) or
// anchors independently with the same edge semantics.
const DEFAULT_OUTPUT_POS = { docked: true, h: 'right', v: 'top', offX: 16, offY: 240 };

const store = new Store({
  name: 'aevalbuddy',
  defaults: {
    // apiKeyEncrypted holds a base64 DPAPI blob; apiKeyPlain is the fallback
    // when the OS keystore is unavailable (rare on Windows 10/11).
    apiKeyEncrypted: '',
    apiKeyPlain: '',
    model: DEFAULT_MODEL,
    // The stored model only wins once the user has actively saved one;
    // until then the shipping default applies, so default changes reach
    // existing installs (electron-store persists defaults to disk).
    modelChosen: false,
    shortcuts: { ...DEFAULT_SHORTCUTS },
    overlayPos: { ...DEFAULT_OVERLAY_POS },
    outputPos: { ...DEFAULT_OUTPUT_POS },
    walkthroughAutoShown: false,
  },
});

function encryptionReady() {
  try {
    return safeStorage.isEncryptionAvailable();
  } catch (err) {
    // safeStorage throws before app ready; callers only run after ready.
    return false;
  }
}

function getApiKey() {
  const blob = store.get('apiKeyEncrypted');
  if (blob && encryptionReady()) {
    try {
      return safeStorage.decryptString(Buffer.from(blob, 'base64'));
    } catch (err) {
      // Decryption fails if the OS user profile changed; fall through so the
      // user can re-enter the key instead of being stuck.
      return '';
    }
  }
  return store.get('apiKeyPlain') || '';
}

function setApiKey(key) {
  const value = String(key || '').trim();
  if (value && encryptionReady()) {
    store.set('apiKeyEncrypted', safeStorage.encryptString(value).toString('base64'));
    store.set('apiKeyPlain', '');
  } else {
    store.set('apiKeyPlain', value);
    store.set('apiKeyEncrypted', '');
  }
}

function clearApiKey() {
  store.set('apiKeyEncrypted', '');
  store.set('apiKeyPlain', '');
}

function hasApiKey() {
  return getApiKey().length > 0;
}

function getModel() {
  if (!store.get('modelChosen')) return DEFAULT_MODEL;
  const value = String(store.get('model') || '').trim();
  return value || DEFAULT_MODEL;
}

function setModel(model) {
  const value = String(model || '').trim();
  store.set('model', value || DEFAULT_MODEL);
  store.set('modelChosen', true);
  return getModel();
}

function getShortcuts() {
  return { ...DEFAULT_SHORTCUTS, ...store.get('shortcuts') };
}

function setShortcut(action, accelerator) {
  const shortcuts = getShortcuts();
  shortcuts[action] = accelerator;
  store.set('shortcuts', shortcuts);
}

function resetShortcuts() {
  store.set('shortcuts', { ...DEFAULT_SHORTCUTS });
  return getShortcuts();
}

function sanitizeOverlayPos(pos) {
  const p = pos && typeof pos === 'object' ? pos : {};
  const h = ['left', 'center', 'right'].includes(p.h) ? p.h : DEFAULT_OVERLAY_POS.h;
  const v = ['top', 'center', 'bottom'].includes(p.v) ? p.v : DEFAULT_OVERLAY_POS.v;
  const clamp = (n) =>
    Number.isFinite(n) ? Math.max(0, Math.min(8000, Math.round(n))) : 16;
  return { h, v, offX: clamp(p.offX), offY: clamp(p.offY) };
}

function getOverlayPos() {
  return sanitizeOverlayPos(store.get('overlayPos'));
}

function setOverlayPos(pos) {
  const value = sanitizeOverlayPos(pos);
  store.set('overlayPos', value);
  return value;
}

function getOutputPos() {
  const p = store.get('outputPos');
  const base = sanitizeOverlayPos(p);
  return { ...base, docked: !(p && typeof p === 'object') || p.docked !== false };
}

function setOutputPos(pos) {
  const base = sanitizeOverlayPos(pos);
  const value = { ...base, docked: !pos || pos.docked !== false };
  store.set('outputPos', value);
  return value;
}

function getWalkthroughAutoShown() {
  return !!store.get('walkthroughAutoShown');
}

function setWalkthroughAutoShown() {
  store.set('walkthroughAutoShown', true);
}

module.exports = {
  DEFAULT_SHORTCUTS,
  DEFAULT_OVERLAY_POS,
  getApiKey,
  setApiKey,
  clearApiKey,
  hasApiKey,
  getModel,
  setModel,
  getShortcuts,
  setShortcut,
  resetShortcuts,
  getOverlayPos,
  setOverlayPos,
  getOutputPos,
  setOutputPos,
  getWalkthroughAutoShown,
  setWalkthroughAutoShown,
};
