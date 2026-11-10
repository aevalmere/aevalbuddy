'use strict';

// Draws what lib/lens.js sends: hit boxes, the current-hit ring with its why
// chip, and the agent's control frame. Coordinates are relative to this
// display's top-left corner.

const hitsEl = document.getElementById('hits');
const ring = document.getElementById('ring');
const why = document.getElementById('why');
const frameEl = document.getElementById('frame');
const chip = document.getElementById('chip');
const loupeEl = ring.querySelector('.loupe');
let loupeUrl = '';

const STAGGER_MS = 12;
const STAGGER_CAP = 300;
const PAD = 3;
let drawnKey = '';
let lastDisplayed = -1;
let shown = []; // the hits on this display, for keeping the why chip off them
let drawnRects = new Set(); // the marks already up, so they do not fade in again

// Where the why chip goes: below the ring, above it, or level with it on
// the right or the left; the first of those that stays on the display and
// covers no other hit. Always going below put it over the next line's hit,
// often the next thing the person wants to read (D21). When every side
// covers something, the side covering the fewest wins.
function whySpot(h, x, y, wr) {
  const W = window.innerWidth;
  const H = window.innerHeight;
  const clampX = (v) => Math.max(8, Math.min(W - wr.width - 8, v));
  const clampY = (v) => Math.max(8, Math.min(H - wr.height - 8, v));
  const ringW = h.w + PAD * 2;
  const ringH = h.h + PAD * 2;
  const spots = [
    { left: clampX(x), top: y + ringH + 8 },
    { left: clampX(x), top: y - wr.height - 8 },
    { left: x + ringW + 8, top: clampY(y + ringH / 2 - wr.height / 2) },
    { left: x - wr.width - 8, top: clampY(y + ringH / 2 - wr.height / 2) },
  ].filter((s) => s.left >= 8 && s.top >= 8 && s.left + wr.width <= W - 8 && s.top + wr.height <= H - 8);
  if (!spots.length) return { left: clampX(x), top: clampY(y + ringH + 8) };
  const covered = (s) =>
    shown.filter(
      (o) =>
        o.i !== h.i &&
        o.x < s.left + wr.width &&
        o.x + o.w > s.left &&
        o.y < s.top + wr.height &&
        o.y + o.h > s.top
    ).length;
  let best = spots[0];
  let least = covered(best);
  for (const s of spots.slice(1)) {
    if (!least) break;
    const n = covered(s);
    if (n < least) {
      best = s;
      least = n;
    }
  }
  return best;
}

// The magnifier drops when the ring moves and comes back, with a small
// settle, once main has cut the new hit's pixels.
function dropLoupe() {
  ring.classList.remove('loupe-on');
  loupeEl.style.backgroundImage = '';
  if (loupeUrl && loupeUrl.startsWith('blob:')) URL.revokeObjectURL(loupeUrl);
  loupeUrl = '';
}

window.aeval.on('lens:loupe', ({ i, img }) => {
  if (i !== lastDisplayed || !img) return;
  // A URL under aeval-frame:// on Tauri (ARCHITECTURE.md D4), JPEG bytes under
  // Electron.
  const url = typeof img === 'string' ? img : URL.createObjectURL(new Blob([img], { type: 'image/jpeg' }));
  const owned = typeof img !== 'string';
  const pic = new Image();
  pic.onload = () => {
    if (i !== lastDisplayed) {
      if (owned) URL.revokeObjectURL(url);
      return;
    }
    dropLoupe();
    loupeUrl = url;
    loupeEl.style.backgroundImage = `url("${url}")`;
    void loupeEl.offsetWidth;
    ring.classList.add('loupe-on');
  };
  pic.src = url;
});

function placeRing(h) {
  if (h.i !== lastDisplayed) dropLoupe();
  const x = h.x - PAD;
  const y = h.y - PAD;
  ring.style.transform = `translate(${x}px, ${y}px)`;
  ring.style.width = `${h.w + PAD * 2}px`;
  ring.style.height = `${h.h + PAD * 2}px`;
  ring.classList.add('on');
  if (h.kind !== 'literal' && h.why) {
    why.textContent = h.why;
    why.classList.add('on');
    const wr = why.getBoundingClientRect();
    // A capsule while it fits on one line, a card once it wraps.
    const cs = getComputedStyle(why);
    const line = parseFloat(cs.lineHeight) || 18;
    const inner = wr.height - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom);
    why.classList.toggle('multi', inner > line * 1.5);
    const spot = whySpot(h, x, y, wr);
    why.style.transform = `translate(${Math.round(spot.left)}px, ${Math.round(spot.top)}px)`;
  } else {
    why.classList.remove('on');
  }
}

window.aeval.on('lens:draw', ({ hits, current, frame }) => {
  shown = hits;
  const key = hits.map((h) => `${h.i}:${h.kind}:${h.x},${h.y},${h.w},${h.h}`).join('|');
  if (key !== drawnKey) {
    drawnKey = key;
    hitsEl.innerHTML = '';
    // A mark that was already there stays put: only new ones fade in, so
    // typing another letter does not make the whole page blink.
    const next = new Set();
    let fresh = 0;
    hits.forEach((h) => {
      const el = document.createElement('div');
      el.className = `hit ${h.kind}`;
      el.dataset.i = String(h.i);
      el.style.transform = `translate(${h.x}px, ${h.y}px)`;
      el.style.width = `${h.w}px`;
      el.style.height = `${h.h}px`;
      const at = `${h.kind}:${h.x},${h.y},${h.w},${h.h}`;
      next.add(at);
      if (drawnRects.has(at)) {
        el.style.animation = 'none';
        el.style.opacity = '1';
      } else {
        el.style.animationDelay = `${Math.min(STAGGER_CAP, fresh * STAGGER_MS)}ms`;
        fresh += 1;
      }
      hitsEl.appendChild(el);
    });
    drawnRects = next;
  }
  const cur = hits.find((h) => h.i === current);
  if (cur) {
    // The first ring of a session appears in place; after that it glides.
    if (lastDisplayed < 0) {
      ring.style.transition = 'none';
      placeRing(cur);
      void ring.offsetWidth;
      ring.style.transition = '';
    } else {
      placeRing(cur);
    }
    lastDisplayed = current;
  } else {
    ring.classList.remove('on');
    why.classList.remove('on');
    dropLoupe();
    lastDisplayed = -1;
  }
  frameEl.classList.toggle('on', !!frame);
  chip.classList.toggle('on', !!(frame && frame.chip));
  chip.textContent = frame && frame.chip ? frame.chip : '';
  document.body.dataset.hits = String(hits.length);
});

// Last, so every listener above is live before main sends the first draw.
window.aeval.invoke('lens:ready');
