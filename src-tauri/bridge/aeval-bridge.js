// window.aeval for Tauri: the same narrow surface preload.js gave the pages
// under Electron (ARCHITECTURE.md section 4.2). Injected into every webview as
// an initialization script, wrapped by ipc::bridge_script(), which appends
// the call with the two allowlists so they have one source in Rust.
//
// Tauri's own globals are looked up when a call is made, not when this runs,
// so the order of initialization scripts does not matter.
function aevalBridge(invokeList, listenList) {
  'use strict';
  if (window.aeval) return;
  const INVOKE = new Set(invokeList);
  const LISTEN = new Set(listenList);
  const tauri = () => window.__TAURI__;
  const asError = (e) => (e instanceof Error ? e : new Error(String(e)));

  const api = {
    invoke(channel, payload) {
      if (!INVOKE.has(channel)) {
        return Promise.reject(new Error(`Blocked IPC channel: ${channel}`));
      }
      return tauri()
        .core.invoke('aeval_invoke', { channel, payload: payload === undefined ? null : payload })
        .catch((e) => {
          throw asError(e);
        });
    },
    on(channel, callback) {
      if (!LISTEN.has(channel)) {
        throw new Error(`Blocked IPC channel: ${channel}`);
      }
      // Listen on this window only: an event sent to the answer window must
      // never reach the bar, as with Electron's per-webContents send.
      let off = null;
      let gone = false;
      tauri()
        .webviewWindow.getCurrentWebviewWindow()
        .listen(channel, (event) => callback(event.payload))
        .then((unlisten) => {
          if (gone) unlisten();
          else off = unlisten;
        });
      return () => {
        gone = true;
        if (off) off();
      };
    },
  };
  Object.freeze(api);
  Object.defineProperty(window, 'aeval', { value: api, writable: false, configurable: false });
}
