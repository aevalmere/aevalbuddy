// window.aeval for Tauri: the same narrow surface preload.js gave the pages
// under Electron (ARCHITECTURE.md section 4.2). Injected into every webview as
// an initialization script, wrapped by ipc::bridge_script(), which appends
// the call with the two allowlists so they have one source in Rust.
//
// Tauri's own globals are looked up when a call is made, not when this runs,
// so the order of initialization scripts does not matter.
//
// Besides the API, this file takes back the browser behaviour Electron never
// had and WebView2 does (ARCHITECTURE.md, split d): the default context
// menu, browser accelerator keys (reload, find, print, zoom, back), Ctrl+wheel
// zoom, and navigation by a dropped file or link. An overlay that reloads on
// F5 or opens a find bar on Ctrl+F has lost its state, which is lost
// functionality.
function aevalBridge(invokeList, listenList) {
  'use strict';
  if (window.aeval) return;
  const INVOKE = new Set(invokeList);
  const LISTEN = new Set(listenList);
  const tauri = () => window.__TAURI__;
  const asError = (e) => (e instanceof Error ? e : new Error(String(e)));

  // Electron's ipcRenderer.on registered synchronously; Tauri's listen is a
  // round trip. A page registers its listeners and then says hello (an
  // invoke), and Rust answers the hello with the page's first state. So an
  // invoke waits for every listen still in flight: the hello can never
  // overtake the listener it is meant to feed.
  let gate = null;
  function hold(p) {
    const settled = p.then(
      () => {},
      () => {}
    );
    const g = Promise.all([gate, settled]).then(() => {
      if (gate === g) gate = null;
    });
    gate = g;
  }

  function send(channel, payload) {
    return tauri()
      .core.invoke('aeval_invoke', { channel, payload: payload === undefined ? null : payload })
      .catch((e) => {
        throw asError(e);
      });
  }

  const api = {
    invoke(channel, payload) {
      if (!INVOKE.has(channel)) {
        return Promise.reject(new Error(`Blocked IPC channel: ${channel}`));
      }
      if (!gate) return send(channel, payload);
      return gate.then(() => send(channel, payload));
    },
    on(channel, callback) {
      if (!LISTEN.has(channel)) {
        throw new Error(`Blocked IPC channel: ${channel}`);
      }
      // Listen on this window only: an event sent to the answer window must
      // never reach the bar, as with Electron's per-webContents send.
      let off = null;
      let gone = false;
      const p = tauri()
        .webviewWindow.getCurrentWebviewWindow()
        .listen(channel, (event) => callback(event.payload))
        .then((unlisten) => {
          if (gone) unlisten();
          else off = unlisten;
        });
      hold(p);
      return () => {
        gone = true;
        if (off) off();
      };
    },
  };
  Object.freeze(api);
  Object.defineProperty(window, 'aeval', { value: api, writable: false, configurable: false });

  // ------------------------------------------------- browser behaviour off

  const editable = (t) =>
    !!(t && t.closest && t.closest('input:not([type="checkbox"]):not([type="radio"]):not([type="range"]), textarea, [contenteditable=""], [contenteditable="true"]'));

  // The page's own menus (answer, hit, row) call preventDefault themselves;
  // anything they leave would be WebView2's Back / Reload / Save as / Print.
  // A text field keeps the edit menu (cut, copy, paste).
  window.addEventListener('contextmenu', (e) => {
    if (!e.defaultPrevented && !editable(e.target)) e.preventDefault();
  });

  // Keys WebView2 acts on by itself. Pages see the key first (this runs in
  // the bubble phase on window, after every page handler), so the bar's
  // Ctrl+1..3, F6 and the shortcut recorder are untouched.
  const CTRL_KEYS = new Set(['r', 'f', 'g', 'p', 's', 'u', 'j', 'h', 'o', 'n', 't', 'w', 'l', 'd', 'e', '+', '-', '=', '0', '_']);
  window.addEventListener('keydown', (e) => {
    const k = (e.key || '').toLowerCase();
    let block = false;
    if (k === 'f5' || k === 'f3' || k === 'f7' || k === 'f11' || k === 'f12' || k === 'browserback' || k === 'browserforward' || k === 'browserrefresh') block = true;
    else if (e.ctrlKey && !e.altKey && CTRL_KEYS.has(k)) block = true;
    else if (e.altKey && (k === 'arrowleft' || k === 'arrowright' || k === 'home')) block = true;
    else if (e.ctrlKey && e.shiftKey && (k === 'i' || k === 'c' || k === 'delete')) block = true;
    // Editing chords (Ctrl+A, C, V, X, Z, Y) are not in the list, so text
    // fields keep them.
    if (block) e.preventDefault();
  });

  // Ctrl+wheel and pinch zoom the whole page in WebView2.
  window.addEventListener(
    'wheel',
    (e) => {
      if (e.ctrlKey) e.preventDefault();
    },
    { passive: false }
  );

  // A file or link dropped on a page would navigate the window away from it.
  const noDrop = (e) => {
    if (e.dataTransfer && [...(e.dataTransfer.types || [])].some((t) => t === 'Files' || t === 'text/uri-list')) {
      if (!editable(e.target)) e.preventDefault();
    }
  };
  window.addEventListener('dragover', noDrop);
  window.addEventListener('drop', noDrop);
}
