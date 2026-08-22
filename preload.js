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
  'query:submit',
  'query:cancel',
  'settings:get',
  'settings:save-key',
  'settings:save-model',
  'settings:close',
  'hud:enter',
  'hud:exit',
  'shortcuts:set',
  'shortcuts:reset',
  'shortcuts:capture-mode',
  'buddy:resolve-result',
  'buddy:dismiss',
  'buddy:set-ignore',
  'app:quit',
]);

const LISTEN_CHANNELS = new Set([
  'overlay:state',
  'hud:snapped',
  'query:delta',
  'query:done',
  'query:error',
  'buddy:resolve',
  'buddy:watch',
  'buddy:show',
  'buddy:hide',
  'buddy:done',
  'settings:data',
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
