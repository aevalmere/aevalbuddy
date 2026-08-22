'use strict';

// The pointer glides with a requestAnimationFrame interpolation loop rather
// than stepped CSS transitions: every frame it moves a fraction of the
// remaining distance toward the target, so updates from the main process at
// any cadence still render as one continuous motion.

const buddy = document.getElementById('buddy');
const ring = buddy.querySelector('.ring');
const callout = document.getElementById('callout');
const calloutMessage = document.getElementById('callout-message');
const calloutClose = document.getElementById('callout-close');

const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
const EASE = reduceMotion ? 1 : 0.16;

let target = null; // { x, y, ringSize }
let cur = { x: 0, y: 0 };
let shown = false;
let rafId = null;
let message = '';
let calloutSize = { w: 0, h: 0 };
let ignoreState = true;

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

function tick() {
  rafId = null;
  if (!target) return;
  cur.x += (target.x - cur.x) * EASE;
  cur.y += (target.y - cur.y) * EASE;
  if (Math.abs(target.x - cur.x) < 0.5 && Math.abs(target.y - cur.y) < 0.5) {
    cur.x = target.x;
    cur.y = target.y;
  }
  buddy.style.transform = `translate3d(${cur.x}px, ${cur.y}px, 0)`;
  positionCallout();
  if (cur.x !== target.x || cur.y !== target.y) {
    rafId = requestAnimationFrame(tick);
  }
}

function schedule() {
  if (!rafId) rafId = requestAnimationFrame(tick);
}

function positionCallout() {
  const rs = target ? target.ringSize : 40;
  let left = cur.x + rs / 2 + 14;
  if (left + calloutSize.w > window.innerWidth - 12) {
    left = cur.x - rs / 2 - 14 - calloutSize.w;
  }
  const top = clamp(cur.y - calloutSize.h / 2, 12, window.innerHeight - calloutSize.h - 12);
  callout.style.transform = `translate3d(${Math.round(left)}px, ${Math.round(top)}px, 0)`;
}

window.aeval.on('buddy:show', ({ x, y, targetW, targetH, message: msg }) => {
  const ringSize = clamp(Math.max(targetW, targetH) + 16, 30, 96);
  ring.style.width = `${ringSize}px`;
  ring.style.height = `${ringSize}px`;

  if (msg !== message) {
    message = msg;
    calloutMessage.textContent = msg;
    calloutSize = { w: callout.offsetWidth, h: callout.offsetHeight };
  }

  target = { x, y, ringSize };
  if (!shown) {
    // First appearance lands in place; every later update glides.
    cur = { x, y };
    shown = true;
  }
  buddy.classList.remove('done');
  callout.classList.remove('done');
  buddy.classList.add('visible');
  callout.classList.add('visible');
  schedule();
});

window.aeval.on('buddy:hide', () => {
  // Fade only; position is kept so a re-show glides from where it was.
  buddy.classList.remove('visible');
  callout.classList.remove('visible');
});

window.aeval.on('buddy:done', () => {
  buddy.classList.add('done');
  callout.classList.add('done');
});

// The window is click-through by default. While the cursor is over the
// visible callout, hit-testing turns on so its close button works; anywhere
// else, clicks keep passing through to whatever is underneath.
function setIgnore(ignore) {
  if (ignore === ignoreState) return;
  ignoreState = ignore;
  window.aeval.invoke('buddy:set-ignore', { ignore });
}

document.addEventListener('mousemove', (e) => {
  const el = document.elementFromPoint(e.clientX, e.clientY);
  const overCallout =
    !!(el && el.closest('#callout')) && callout.classList.contains('visible');
  setIgnore(!overCallout);
});

document.addEventListener('mouseleave', () => setIgnore(true));

calloutClose.addEventListener('click', () => {
  window.aeval.invoke('buddy:dismiss');
});
