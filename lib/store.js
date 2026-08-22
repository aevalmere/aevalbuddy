'use strict';

const Store = require('electron-store');
const { safeStorage } = require('electron');
const { DEFAULT_MODEL, DEFAULT_BRAIN } = require('./openrouter');
const skills = require('./skills');

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
    // Brain mode: a vision model reads the screen, a stronger text-only
    // model answers from that reading. Off by default because it costs a
    // second round trip.
    brainEnabled: false,
    brain: DEFAULT_BRAIN,
    brainChosen: false,
    // Paid models stay out of the pickers until this is turned on.
    showPaidModels: false,
    // Which answering procedure rides along with each request. 'auto' lets
    // the router pick per request from the screen and the question; any
    // other value pins one.
    skill: 'auto',
    shortcuts: { ...DEFAULT_SHORTCUTS },
    overlayPos: { ...DEFAULT_OVERLAY_POS },
    outputPos: { ...DEFAULT_OUTPUT_POS },
    // User-set window sizes. Width applies to both overlay windows; the
    // answer window also remembers a height once it has been dragged, which
    // switches it from sizing itself to the transcript.
    inputWidth: 420,
    outputWidth: 420,
    outputHeight: 0,
    settingsSize: { width: 460, height: 470 },
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

function getBrain() {
  if (!store.get('brainChosen')) return DEFAULT_BRAIN;
  const value = String(store.get('brain') || '').trim();
  return value || DEFAULT_BRAIN;
}

function setBrain(model) {
  const value = String(model || '').trim();
  store.set('brain', value || DEFAULT_BRAIN);
  store.set('brainChosen', true);
  return getBrain();
}

function getBrainEnabled() {
  return !!store.get('brainEnabled');
}

function setBrainEnabled(on) {
  store.set('brainEnabled', !!on);
  return getBrainEnabled();
}

function getShowPaidModels() {
  return !!store.get('showPaidModels');
}

function setShowPaidModels(on) {
  store.set('showPaidModels', !!on);
  return getShowPaidModels();
}

// 'auto' is a valid value alongside the skill ids. A skill file removed
// between versions falls back to automatic rather than silently sending no
// procedure at all.
function getSkillId() {
  const value = String(store.get('skill') || '').trim();
  if (value === 'auto' || skills.has(value)) return value;
  return 'auto';
}

function setSkillId(id) {
  const value = String(id || '').trim();
  store.set('skill', value === 'auto' || skills.has(value) ? value : 'auto');
  return getSkillId();
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

const MIN_W = 320;
const MAX_W = 900;

function clampW(n, fallback) {
  const v = Math.round(Number(n));
  if (!Number.isFinite(v)) return fallback;
  return Math.max(MIN_W, Math.min(MAX_W, v));
}

function getInputWidth() {
  return clampW(store.get('inputWidth'), 420);
}

function setInputWidth(w) {
  store.set('inputWidth', clampW(w, 420));
  return getInputWidth();
}

function getOutputWidth() {
  return clampW(store.get('outputWidth'), 420);
}

function setOutputWidth(w) {
  store.set('outputWidth', clampW(w, 420));
  return getOutputWidth();
}

// 0 means the answer window still sizes itself to the transcript. Any other
// value is a height the user dragged to, and it wins until New chat.
function getOutputHeight() {
  const v = Math.round(Number(store.get('outputHeight')));
  return Number.isFinite(v) && v > 0 ? v : 0;
}

function setOutputHeight(h) {
  const v = Math.round(Number(h));
  store.set('outputHeight', Number.isFinite(v) && v > 0 ? v : 0);
  return getOutputHeight();
}

function getSettingsSize() {
  const v = store.get('settingsSize') || {};
  return {
    width: Math.max(380, Math.min(900, Math.round(Number(v.width)) || 460)),
    height: Math.max(360, Math.min(1400, Math.round(Number(v.height)) || 470)),
  };
}

function setSettingsSize(size) {
  store.set('settingsSize', getSettingsSizeFrom(size));
  return getSettingsSize();
}

function getSettingsSizeFrom(size) {
  const s2 = size || {};
  return {
    width: Math.max(380, Math.min(900, Math.round(Number(s2.width)) || 460)),
    height: Math.max(360, Math.min(1400, Math.round(Number(s2.height)) || 470)),
  };
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
  getBrain,
  setBrain,
  getBrainEnabled,
  setBrainEnabled,
  getShowPaidModels,
  setShowPaidModels,
  getSkillId,
  setSkillId,
  getShortcuts,
  setShortcut,
  resetShortcuts,
  getOverlayPos,
  setOverlayPos,
  getOutputPos,
  setOutputPos,
  getInputWidth,
  setInputWidth,
  getOutputWidth,
  setOutputWidth,
  getOutputHeight,
  setOutputHeight,
  getSettingsSize,
  setSettingsSize,
  MIN_W,
  MAX_W,
  getWalkthroughAutoShown,
  setWalkthroughAutoShown,
};
