// Generated from lib/contrast.js PROBE (do not edit; regenerate when that changes).
(() => {
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
      if (/^inset\(\s*50%/.test(cs.clipPath || '')) return null;
      const b = e.getBoundingClientRect();
      const m = /^rect\(([^)]*)\)/.exec(cs.clip || '');
      if (m && (cs.position === 'absolute' || cs.position === 'fixed')) {
        const v = m[1].split(/[\s,]+/).map(parseFloat);
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
})()
