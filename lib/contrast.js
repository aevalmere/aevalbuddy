'use strict';

// The contrast gate of the visual self-check (PLAN-v2.1 Phase B). Glass is
// see-through, so a colour pair in a stylesheet says nothing about what a
// person reads: the background of a label is whatever the desktop is under
// it, blurred and scrimmed. This measures text against the pixels it is
// actually drawn on.
//
// Two photographs of the same frame: the page as it is, and the page with
// every glyph made transparent. For each run of visible text, the second
// photograph gives the real background under it. The text's own colour is
// read from the computed style and laid over each background pixel, so
// alpha text is measured as it renders. The worst background in the run is
// the 95th percentile toward the text's own lightness: a single stray pixel
// does not fail a label, a bright stripe behind one does.
//
// WCAG 2.2: 4.5:1 for body text, 3:1 for large text (24px, or 18.66px at
// 700) and for UI labels drawn that large.

// Underlines go too: a link's decoration has its own colour, and left in
// place it would be read as the background of its own text. Selected text
// is painted by ::selection, which * does not reach.
const HIDE_TEXT =
  '*, *::before, *::after { color: transparent !important; -webkit-text-fill-color: transparent !important;' +
  ' text-shadow: none !important; caret-color: transparent !important;' +
  ' text-decoration-color: transparent !important; }' +
  ' ::placeholder { color: transparent !important; }' +
  ' ::selection { color: transparent !important; -webkit-text-fill-color: transparent !important; }';

// Runs in the page: every visible run of text, with its rect, colour and
// size. Text inside something at zero opacity, hidden, or clipped out of
// the viewport is not read by anyone and is not measured. Neither is the
// part of a run cut off by an ancestor: overflow other than visible, a
// clip rect, or clip-path inset(50%). That is how a screen-reader-only
// live region (.sr-only) is hidden, and how a scrolled list hides the rows
// above its top edge. What is left of a run after clipping is measured in
// full, so a label half under a scroll edge is still held to the floor.
const PROBE = `(() => {
  const out = [];
  const W = innerWidth, H = innerHeight;
  const seen = (el) => {
    if (!el.checkVisibility || !el.checkVisibility({ opacityProperty: true, visibilityProperty: true })) return false;
    for (let e = el; e && e.nodeType === 1; e = e.parentElement) {
      if (parseFloat(getComputedStyle(e).opacity) < 0.6) return false;
      if (e.getAttribute('aria-hidden') === 'true' && e.classList.contains('gl')) return false;
    }
    return true;
  };
  // The part of r that ancestors of el (and el) let paint, or null.
  // Overflow clips only along the containing-block chain, the way the
  // engine paints: an absolute box escapes the overflow of static
  // ancestors up to its positioned one, and a fixed box escapes them all.
  // (The overlay's body is zero-height with overflow hidden and everything
  // in it positioned; clipping by it measured nothing at all.)
  const clipTo = (el, r) => {
    let L = r.left, T = r.top, R = r.right, B = r.bottom;
    let escape = 'none'; // 'abs' until a positioned ancestor, 'fixed' for good
    for (let e = el; e && e.nodeType === 1 && e !== document.documentElement; e = e.parentElement) {
      const cs = getComputedStyle(e);
      let clips = escape === 'none';
      if (escape === 'abs' && cs.position !== 'static') {
        clips = true;
        escape = 'none';
      }
      if (/^inset\\(\\s*50%/.test(cs.clipPath || '')) return null;
      const b = e.getBoundingClientRect();
      const m = /^rect\\(([^)]*)\\)/.exec(cs.clip || '');
      if (m && (cs.position === 'absolute' || cs.position === 'fixed')) {
        const v = m[1].split(/[\\s,]+/).map(parseFloat);
        if (!isNaN(v[0])) T = Math.max(T, b.top + v[0]);
        if (!isNaN(v[1])) R = Math.min(R, b.left + v[1]);
        if (!isNaN(v[2])) B = Math.min(B, b.top + v[2]);
        if (!isNaN(v[3])) L = Math.max(L, b.left + v[3]);
      }
      if (clips && cs.overflowX !== 'visible') { L = Math.max(L, b.left); R = Math.min(R, b.right); }
      if (clips && cs.overflowY !== 'visible') { T = Math.max(T, b.top); B = Math.min(B, b.bottom); }
      if (R - L < 1 || B - T < 1) return null;
      if (cs.position === 'fixed') escape = 'fixed';
      else if (cs.position === 'absolute' && escape !== 'fixed') escape = 'abs';
    }
    return { left: L, top: T, right: R, bottom: B };
  };
  const push = (el, r0, color, text) => {
    const r = clipTo(el, r0);
    if (!r) return;
    const x = Math.max(0, r.left), y = Math.max(0, r.top);
    const w = Math.min(W, r.right) - x, h = Math.min(H, r.bottom) - y;
    if (w < 3 || h < 6) return;
    const cs = getComputedStyle(el);
    out.push({ x, y, w, h, color, size: parseFloat(cs.fontSize), weight: parseInt(cs.fontWeight, 10) || 400,
      text: text.trim().slice(0, 40), sel: (el.id ? '#' + el.id : el.tagName.toLowerCase() + (el.classList[0] ? '.' + el.classList[0] : '')) });
  };
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    if (!n.nodeValue.trim()) continue;
    const el = n.parentElement;
    if (!el || /^(SCRIPT|STYLE|NOSCRIPT|TEXTAREA|OPTION)$/.test(el.tagName) || !seen(el)) continue;
    const range = document.createRange();
    range.selectNodeContents(n);
    for (const r of range.getClientRects()) push(el, r, getComputedStyle(el).color, n.nodeValue);
  }
  for (const el of document.querySelectorAll('input[type=text], input[type=password], input:not([type]), textarea')) {
    if (!seen(el)) continue;
    const r = el.getBoundingClientRect();
    const cs = getComputedStyle(el);
    const padL = parseFloat(cs.paddingLeft), padR = parseFloat(cs.paddingRight);
    const lh = Math.min(r.height, parseFloat(cs.fontSize) * 1.3);
    const box = { left: r.left + padL, right: r.right - padR, top: r.top + (r.height - lh) / 2, bottom: r.top + (r.height + lh) / 2 };
    // Selected text is painted in the ::selection colour, not the field's.
    const sel = el === document.activeElement && el.selectionStart !== el.selectionEnd;
    const vc = sel ? getComputedStyle(el, '::selection').color || cs.color : cs.color;
    if (el.value) push(el, box, vc, el.type === 'password' ? 'password' : el.value);
    else if (el.placeholder) push(el, box, getComputedStyle(el, '::placeholder').color, el.placeholder);
  }
  return out;
})()`;

function parseColor(s) {
  const m = /rgba?\(([^)]+)\)/.exec(s || '');
  if (!m) return null;
  const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number);
  return { r: p[0], g: p[1], b: p[2], a: p.length > 3 ? p[3] : 1 };
}

function lin(c) {
  const v = c / 255;
  return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
}

function lum(r, g, b) {
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

function ratio(a, b) {
  const hi = Math.max(a, b);
  const lo = Math.min(a, b);
  return (hi + 0.05) / (lo + 0.05);
}

/**
 * @param {Electron.BrowserWindow} win
 * @returns {Promise<{runs:number, min:number, fails:Array}>}
 */
async function measure(win) {
  const wc = win.webContents;
  const runs = await wc.executeJavaScript(PROBE);
  if (!runs.length) return { runs: 0, min: Infinity, fails: [] };
  const key = await wc.insertCSS(HIDE_TEXT);
  let bg;
  try {
    await new Promise((r) => setTimeout(r, 160));
    bg = await wc.capturePage();
  } finally {
    await wc.removeInsertedCSS(key).catch(() => {});
  }
  const size = bg.getSize();
  const css = await wc.executeJavaScript('[innerWidth, innerHeight]');
  const k = size.width / css[0];
  const bmp = bg.toBitmap(); // BGRA
  const fails = [];
  let min = Infinity;
  for (const t of runs) {
    const fg = parseColor(t.color);
    if (!fg || fg.a < 0.05) continue;
    const x0 = Math.max(0, Math.floor(t.x * k));
    const y0 = Math.max(0, Math.floor(t.y * k));
    const x1 = Math.min(size.width, Math.ceil((t.x + t.w) * k));
    const y1 = Math.min(size.height, Math.ceil((t.y + t.h) * k));
    const step = Math.max(1, Math.round(Math.sqrt(((x1 - x0) * (y1 - y0)) / 900)));
    const ratios = [];
    for (let y = y0; y < y1; y += step) {
      for (let x = x0; x < x1; x += step) {
        const i = (y * size.width + x) * 4;
        const br = bmp[i + 2];
        const bgc = bmp[i + 1];
        const bb = bmp[i];
        // The glyph as rendered: its colour over this pixel.
        const r = fg.r * fg.a + br * (1 - fg.a);
        const g = fg.g * fg.a + bgc * (1 - fg.a);
        const b = fg.b * fg.a + bb * (1 - fg.a);
        ratios.push(ratio(lum(r, g, b), lum(br, bgc, bb)));
      }
    }
    if (!ratios.length) continue;
    ratios.sort((a, b) => a - b);
    const worst = ratios[Math.floor(ratios.length * 0.05)];
    const large = t.size >= 24 || (t.size >= 18.66 && t.weight >= 700);
    const need = large ? 3 : 4.5;
    if (worst < min) min = worst;
    if (worst < need) fails.push({ sel: t.sel, text: t.text, ratio: Math.round(worst * 100) / 100, need, size: t.size });
  }
  return { runs: runs.length, min: Math.round(min * 100) / 100, fails };
}

module.exports = { measure };
