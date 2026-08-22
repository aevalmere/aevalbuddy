'use strict';

const Store = require('electron-store');
const { safeStorage } = require('electron');

const DEFAULT_SHORTCUTS = {
  captureQuery: 'Ctrl+Shift+Space',
  toggleVisibility: 'Ctrl+Shift+H',
};

const store = new Store({
  name: 'aevalbuddy',
  defaults: {
    // apiKeyEncrypted holds a base64 DPAPI blob; apiKeyPlain is the fallback
    // when the OS keystore is unavailable (rare on Windows 10/11).
    apiKeyEncrypted: '',
    apiKeyPlain: '',
    shortcuts: { ...DEFAULT_SHORTCUTS },
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

function hasApiKey() {
  return getApiKey().length > 0;
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

module.exports = {
  DEFAULT_SHORTCUTS,
  getApiKey,
  setApiKey,
  hasApiKey,
  getShortcuts,
  setShortcut,
  resetShortcuts,
};
