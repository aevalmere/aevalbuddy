// A stand-in for window.aeval in the renderer screenshot run (shots.mjs). It
// runs in every page the scene frames, before the page's own scripts, the way
// the Tauri bridge does. Each frame is one AevalBuddy window: its offset in
// the scene is its screen position (window.screenX/Y), the scene is one
// display, and Rust's answers come from window.top.__MOCK__[frame name].
(() => {
  'use strict';
  const fe = window.frameElement;
  if (!fe) return;
  const off = () => fe.getBoundingClientRect();
  Object.defineProperty(window, 'screenX', { get: () => Math.round(off().left) });
  Object.defineProperty(window, 'screenY', { get: () => Math.round(off().top) });
  Object.defineProperty(window, 'screenLeft', { get: () => Math.round(off().left) });
  Object.defineProperty(window, 'screenTop', { get: () => Math.round(off().top) });

  const top = window.top;
  const cfg = (top.__MOCK__ && top.__MOCK__[fe.name]) || {};
  const common = top.__MOCK_COMMON__ || {};
  const answers = Object.assign({}, common, cfg.responses || {});
  const listeners = new Map();
  const calls = [];

  const api = {
    invoke(channel, payload) {
      calls.push({ channel, payload });
      if (channel === 'output:bar') return Promise.resolve(window.__barRect || null);
      // Rust sizes the window to the sheet plus the 20 px margin on each side.
      if ((channel === 'output:resize' || channel === 'overlay:resize') && payload && payload.height > 0 && !fe.dataset.fixed) {
        fe.style.height = `${Math.min(Math.round(payload.height) + 40, Number(fe.dataset.max || 900))}px`;
        return Promise.resolve(null);
      }
      const r = answers[channel];
      if (r === '__reject__') return Promise.reject(new Error(`${channel} is not ported to Rust yet`));
      const v = typeof r === 'function' ? r(payload, window) : r;
      return Promise.resolve(v === undefined ? null : JSON.parse(JSON.stringify(v)));
    },
    on(channel, cb) {
      if (!listeners.has(channel)) listeners.set(channel, []);
      listeners.get(channel).push(cb);
      return () => {
        const l = listeners.get(channel) || [];
        const i = l.indexOf(cb);
        if (i >= 0) l.splice(i, 1);
      };
    },
  };
  Object.freeze(api);
  Object.defineProperty(window, 'aeval', { value: api, writable: false, configurable: false });
  window.__emit = (channel, payload) => {
    for (const cb of (listeners.get(channel) || []).slice()) cb(payload);
  };
  window.__calls = calls;
})();
