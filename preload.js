'use strict';

const { contextBridge, ipcRenderer } = require('electron');

// Narrow, allowlisted bridge. Renderers never see Node, the API key, or raw
// ipcRenderer.

const INVOKE_CHANNELS = new Set([
  'overlay:trigger',
  'overlay:collapse',
  'overlay:resize',
  'overlay:set-ignore',
  'overlay:open-settings',
  'output:resize',
  'open-external',
  'query:submit',
  'query:cancel',
  'query:retry',
  'chat:new',
  'region:pick',
  'region:clear',
  'region:done',
  'region:cancel',
  'overlay:open-history',
  'history:list',
  'history:open',
  'history:remove',
  'history:clear',
  'history:close',
  'settings:get',
  'settings:save-key',
  'settings:clear-key',
  'settings:save-model',
  'settings:save-skill',
  'settings:save-brain',
  'settings:set-brain-enabled',
  'settings:set-show-paid',
  'settings:set-save-history',
  'settings:clear-history',
  'settings:set-launch-at-login',
  'settings:test-key',
  'update:check',
  'update:restart',
  'settings:close',
  'models:list',
  'clipboard:write',
  'hud:enter',
  'hud:exit',
  'hud:set-dock',
  'shortcuts:set',
  'shortcuts:reset',
  'shortcuts:capture-mode',
  'buddy:resolve-result',
  'buddy:dismiss',
  'buddy:advance',
  'buddy:set-ignore',
  'app:quit',
  // v2: the command bar's modes, Find, the agent, and the tour.
  'bar:mode',
  'find:open',
  'find:query',
  'find:step',
  'find:scope',
  'find:to-ask',
  'find:close',
  'agent:start',
  'agent:stop',
  'agent:confirm',
  'agent:continue',
  'agent:consent',
  'agent:do-it',
  'ui:context-menu',
  'settings:save-find-model',
  'settings:save-agent-model',
  'settings:set-find',
  'settings:set-agent',
  'tour:replay',
  'buddy:card',
  'buddy:skip',
  // v2.1: the single input, keyboard reach between windows, help.
  'find:related',
  'bar:history',
  'bar:history-push',
  'ui:focus-answer',
  'ui:focus-bar',
  'app:help',
  'settings:clip-key',
  'settings:use-clip-key',
  'hud:nudge',
  'buddy:focus',
  // Phase B: the glass backdrop, click-through on the shadow margin, and
  // the solid-surface setting.
  'glass:hello',
  'glass:passthrough',
  'settings:set-surface',
  // Tauri rewrite, split (d): floating output windows and the glass slider
  // (.claude/rust-rewrite/ARCHITECTURE.md, section 10). Electron's main
  // process never answers them.
  'output:bar',
  'output:close',
  'settings:set-glass',
  // ---- lane: windows (do not edit outside your lane) ----
  'settings:set-text-scale',
  'output:blur',
  'output:cycle-target',
  'output:close-all',
  'output:last-answer',
  'output:undo-reset',
  'output:glass',
  'window:nudge',
  'bar:dirty',
  'bar:ready',
  'bar:prefill-open',
  'a11y:announce',
  // ---- end: windows ----
  // ---- lane: capture ----
  // ---- end: capture ----
  // ---- lane: llm ----
  // ---- end: llm ----
  // ---- lane: agent ----
  // ---- end: agent ----
]);

const LISTEN_CHANNELS = new Set([
  'overlay:state',
  'hud:snapped',
  'query:start',
  'query:stage',
  'query:delta',
  'query:done',
  'query:error',
  'chat:user',
  'chat:reset',
  'chat:sync',
  'history:changed',
  'buddy:resolve',
  'buddy:watch',
  'buddy:show',
  'buddy:hide',
  'buddy:done',
  'update:state',
  'find:hits',
  'find:state',
  'agent:begin',
  'agent:step',
  'agent:state',
  'agent:confirm-request',
  'agent:done',
  'agent:consent-request',
  'lens:draw',
  'view:set',
  'bar:prefill',
  'settings:section',
  'bar:open',
  'bar:context',
  'glass:frame',
  'glass:tier',
  'lens:loupe',
  // ---- lane: windows (do not edit outside your lane) ----
  'output:meta',
  'ui:text-scale',
  'bar:clear',
  'bar:note',
  'a11y:say',
  // ---- end: windows ----
  // ---- lane: capture ----
  // ---- end: capture ----
  // ---- lane: llm ----
  // ---- end: llm ----
  // ---- lane: agent ----
  // ---- end: agent ----
]);

contextBridge.exposeInMainWorld('aeval', {
  invoke(channel, payload) {
    if (!INVOKE_CHANNELS.has(channel)) {
      return Promise.reject(new Error(`Blocked IPC channel: ${channel}`));
    }
    return ipcRenderer.invoke(channel, payload);
  },
  on(channel, callback) {
    if (!LISTEN_CHANNELS.has(channel)) {
      throw new Error(`Blocked IPC channel: ${channel}`);
    }
    const listener = (_event, payload) => callback(payload);
    ipcRenderer.on(channel, listener);
    return () => ipcRenderer.removeListener(channel, listener);
  },
});
