'use strict';

// The drag gesture. Positions are written straight to style.transform and
// width/height rather than to left/top, so the four scrim panels and the
// selection frame move on the compositor and a drag across a 4K desktop stays
// at frame rate.

const scrimTop = document.getElementById('scrim-top');
const scrimBottom = document.getElementById('scrim-bottom');
const scrimLeft = document.getElementById('scrim-left');
const scrimRight = document.getElementById('scrim-right');
const sel = document.getElementById('sel');
const sizeChip = document.getElementById('size');
const live = document.getElementById('live');

// Must match MIN_EDGE in lib/region.js: the chip turns red below it, and the
// main process discards it, so the two numbers have to agree.
const MIN_EDGE = 12;
const CHIP_GAP = 10;

let start = null;
let rect = { x: 0, y: 0, width: 0, height: 0 };

// Screen readers hear how to drive the box once, on the first key, and then
// its size as it changes, at most every 500 ms (the last size always lands).
const SAY_MS = 500;
let told = false;
let saidAt = 0;
let sayTimer = null;
let said = '';
function sizeText() {
  return `Area ${rect.width} by ${rect.height}.`;
}
function say(text) {
  said = text;
  saidAt = performance.now();
  live.textContent = text;
}
function announceSize() {
  if (!told) return;
  const text = sizeText();
  if (text === said || sayTimer) return;
  const wait = SAY_MS - (performance.now() - saidAt);
  if (wait <= 0) {
    say(text);
    return;
  }
  sayTimer = setTimeout(() => {
    sayTimer = null;
    if (sizeText() !== said) say(sizeText());
  }, wait);
}
function announceKeys() {
  if (told) return;
  told = true;
  say(`${sizeText()} Arrows move it, Alt+arrows resize, Enter asks, Esc cancels.`);
  // What it said includes the size, so the size is not said again until it changes.
  said = sizeText();
}

function place(el, x, y, w, h) {
  el.style.transform = `translate3d(${x}px, ${y}px, 0)`;
  el.style.width = `${Math.max(0, w)}px`;
  el.style.height = `${Math.max(0, h)}px`;
}

function draw() {
  const { x, y, width, height } = rect;
  const vw = window.innerWidth;
  const vh = window.innerHeight;

  place(scrimTop, 0, 0, vw, y);
  place(scrimBottom, 0, y + height, vw, vh - (y + height));
  place(scrimLeft, 0, y, x, height);
  place(scrimRight, x + width, y, vw - (x + width), height);

  sel.hidden = false;
  place(sel, x, y, width, height);

  const ok = width >= MIN_EDGE && height >= MIN_EDGE;
  sizeChip.hidden = false;
  sizeChip.textContent = ok
    ? `${width} × ${height}`
    : `${width} × ${height}, too small`;
  sizeChip.classList.toggle('too-small', !ok);
  announceSize();

  // Under the selection by default, above it when there is no room below, and
  // never off the side.
  const cw = sizeChip.offsetWidth;
  const ch = sizeChip.offsetHeight;
  let cx = x;
  let cy = y + height + CHIP_GAP;
  if (cy + ch > vh) cy = Math.max(0, y - ch - CHIP_GAP);
  cx = Math.max(0, Math.min(cx, vw - cw));
  sizeChip.style.transform = `translate3d(${Math.round(cx)}px, ${Math.round(cy)}px, 0)`;
}

function fromPointer(e) {
  const x1 = Math.max(0, Math.min(start.x, e.clientX));
  const y1 = Math.max(0, Math.min(start.y, e.clientY));
  const x2 = Math.min(window.innerWidth, Math.max(start.x, e.clientX));
  const y2 = Math.min(window.innerHeight, Math.max(start.y, e.clientY));
  rect = { x: Math.round(x1), y: Math.round(y1), width: Math.round(x2 - x1), height: Math.round(y2 - y1) };
}

document.addEventListener('mousedown', (e) => {
  // Anything but the left button leaves without a selection, which is the
  // convention every other screen-capture tool on Windows follows.
  if (e.button !== 0) {
    window.aeval.invoke('region:cancel');
    return;
  }
  start = { x: e.clientX, y: e.clientY };
  rect = { x: e.clientX, y: e.clientY, width: 0, height: 0 };
  document.body.dataset.dragging = '1';
  draw();
  e.preventDefault();
});

document.addEventListener('mousemove', (e) => {
  if (!start) return;
  fromPointer(e);
  draw();
});

document.addEventListener('mouseup', (e) => {
  if (!start) return;
  fromPointer(e);
  start = null;
  window.aeval.invoke('region:done', rect);
});

// Without a mouse: the first arrow or Enter puts a box in the middle of the
// screen. Arrows move it (Shift for larger steps), Alt+arrows change its
// size, Enter asks about it.
let keyed = false;
function keyBox() {
  if (keyed) return;
  keyed = true;
  const w = Math.round(window.innerWidth / 2);
  const h = Math.round(window.innerHeight / 2);
  rect = { x: Math.round((window.innerWidth - w) / 2), y: Math.round((window.innerHeight - h) / 2), width: w, height: h };
}

const ARROWS = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    e.preventDefault();
    window.aeval.invoke('region:cancel');
    return;
  }
  if (start) return;
  if (e.key === 'Enter') {
    e.preventDefault();
    if (!keyed) {
      keyBox();
      announceKeys();
      draw();
      return;
    }
    window.aeval.invoke('region:done', rect);
    return;
  }
  const d = ARROWS[e.key];
  if (!d) return;
  e.preventDefault();
  keyBox();
  announceKeys();
  const step = e.shiftKey ? 64 : 16;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  if (e.altKey) {
    rect.width = Math.max(MIN_EDGE, Math.min(vw - rect.x, rect.width + d[0] * step));
    rect.height = Math.max(MIN_EDGE, Math.min(vh - rect.y, rect.height + d[1] * step));
  } else {
    rect.x = Math.max(0, Math.min(vw - rect.width, rect.x + d[0] * step));
    rect.y = Math.max(0, Math.min(vh - rect.height, rect.y + d[1] * step));
  }
  draw();
});

// A drag that leaves the window and releases outside it would otherwise strand
// the scrim over the desktop with no way to dismiss it.
window.addEventListener('blur', () => {
  if (start) start = null;
});
