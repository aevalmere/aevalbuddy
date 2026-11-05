'use strict';

// Draws what lib/lens.js sends: hit boxes, the current-hit ring with its why
// chip, and the agent's control frame. Coordinates are relative to this
// display's top-left corner.

const hitsEl = document.getElementById('hits');
const ring = document.getElementById('ring');
const why = document.getElementById('why');
const frameEl = document.getElementById('frame');
const chip = document.getElementById('chip');

const STAGGER_MS = 12;
const STAGGER_CAP = 300;
const PAD = 3;
let drawnKey = '';
let lastDisplayed = -1;

function placeRing(h) {
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
    const below = y + h.h + PAD * 2 + 8;
    const top = below + wr.height > window.innerHeight - 8 ? y - wr.height - 8 : below;
    const left = Math.max(8, Math.min(window.innerWidth - wr.width - 8, x));
    why.style.transform = `translate(${Math.round(left)}px, ${Math.round(top)}px)`;
  } else {
    why.classList.remove('on');
  }
}

window.aeval.on('lens:draw', ({ hits, current, frame }) => {
  const key = hits.map((h) => `${h.i}:${h.kind}:${h.x},${h.y},${h.w},${h.h}`).join('|');
  if (key !== drawnKey) {
    drawnKey = key;
    hitsEl.innerHTML = '';
    hits.forEach((h, n) => {
      const el = document.createElement('div');
      el.className = `hit ${h.kind}`;
      el.dataset.i = String(h.i);
      el.style.transform = `translate(${h.x}px, ${h.y}px)`;
      el.style.width = `${h.w}px`;
      el.style.height = `${h.h}px`;
      el.style.animationDelay = `${Math.min(STAGGER_CAP, n * STAGGER_MS)}ms`;
      hitsEl.appendChild(el);
    });
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
    lastDisplayed = -1;
  }
  frameEl.classList.toggle('on', !!frame);
  chip.classList.toggle('on', !!(frame && frame.chip));
  chip.textContent = frame && frame.chip ? frame.chip : '';
  document.body.dataset.hits = String(hits.length);
});
