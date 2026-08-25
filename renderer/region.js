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

// Must match MIN_EDGE in lib/region.js: the chip turns red below it, and the
// main process discards it, so the two numbers have to agree.
const MIN_EDGE = 12;
const CHIP_GAP = 10;

let start = null;
let rect = { x: 0, y: 0, width: 0, height: 0 };

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
    : `${width} × ${height} — too small`;
  sizeChip.classList.toggle('too-small', !ok);

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

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    e.preventDefault();
    window.aeval.invoke('region:cancel');
  }
});

// A drag that leaves the window and releases outside it would otherwise strand
// the scrim over the desktop with no way to dismiss it.
window.addEventListener('blur', () => {
  if (start) start = null;
});
