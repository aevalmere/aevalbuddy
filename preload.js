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
]);

const LISTEN_CHANNELS = new Set([
  'overlay:state',
  'hud:snapped',
  'hud:docked',
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
  'buddy:move',
  'settings:data',
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
