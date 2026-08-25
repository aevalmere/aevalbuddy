'use strict';

// The pointer glides on a requestAnimationFrame interpolation loop rather
// than stepped CSS transitions: every frame it moves a fraction of the
// remaining distance toward the target, so updates arriving from the main
// process at any cadence still render as one continuous motion.
//
// Positioning is deliberately literal. The transform IS the target pixel;
// there is no centring offset anywhere in this file. The one offset that has
// to exist, between the arrow path's apex and its element origin, lives in
// the stylesheet next to the path it belongs to.

const buddy = document.getElementById('buddy');
const callout = document.getElementById('callout');
const calloutMessage = document.getElementById('callout-message');
const calloutClose = document.getElementById('callout-close');
const calloutCount = document.getElementById('callout-count');
const calloutHint = document.getElementById('callout-hint');

const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
const EASE = reduceMotion ? 1 : 0.16;
const TYPE_MS = 16; // per character
const ARROW_W = 26;
const GAP = 8;
const EDGE = 12;

let target = null; // { x, y } tip position in window coordinates
let cur = { x: 0, y: 0 };
let shown = false;
let rafId = null;
let message = '';
let calloutSize = { w: 0, h: 0 };
let ignoreState = true;
// Where the user has dragged the bubble relative to its default placement.
// Reset whenever a new point arrives, so the next one is not left offscreen.
let dragBy = { x: 0, y: 0 };
let dragging = null;
let typeTimer = null;
// Point mode advances by clicking the bubble; walkthrough steps advance on
// the app action they describe, so a click there only dismisses.
let advanceOnClick = false;
let lastOfSequence = false;

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

// Beside the arrow, tail pointing back at it, flipped to the other side when
// there is no room. The tail sits 18px down the bubble, so the bubble starts
// slightly above the tip to put the tail level with the arrow's shaft.
function positionCallout() {
  const w = calloutSize.w || callout.offsetWidth;
  const h = calloutSize.h || callout.offsetHeight;
  let side = 'right';
  let left = cur.x + ARROW_W + GAP;
  if (left + w > window.innerWidth - EDGE) {
    side = 'left';
    left = cur.x - GAP - w;
  }
  let top = cur.y - 6;
  left = clamp(left + dragBy.x, EDGE, Math.max(EDGE, window.innerWidth - w - EDGE));
  top = clamp(top + dragBy.y, EDGE, Math.max(EDGE, window.innerHeight - h - EDGE));
  callout.dataset.side = side;
  callout.style.transform = `translate3d(${Math.round(left)}px, ${Math.round(top)}px, 0)`;
}

// Types the message out. Feels like the buddy is talking rather than a label
// blinking into existence; it finishes instantly under reduced motion, and
// clicking the bubble skips the rest.
function typeOut(text) {
  clearInterval(typeTimer);
  if (reduceMotion) {
    calloutMessage.textContent = text;
    calloutMessage.classList.remove('typing');
    measure();
    return;
  }
  calloutMessage.textContent = '';
  calloutMessage.classList.add('typing');
  // Reserve the finished size first so the bubble does not grow line by line
  // and shove the tail around while it types.
  calloutMessage.style.minHeight = '';
  const probe = document.createElement('span');
  probe.textContent = text;
  calloutMessage.appendChild(probe);
  measure();
  calloutMessage.style.minHeight = `${calloutMessage.offsetHeight}px`;
  calloutMessage.textContent = '';
  let i = 0;
  typeTimer = setInterval(() => {
    i += 1;
    calloutMessage.textContent = text.slice(0, i);
    if (i >= text.length) {
      clearInterval(typeTimer);
      typeTimer = null;
      calloutMessage.classList.remove('typing');
    }
  }, TYPE_MS);
}

function finishTyping() {
  if (!typeTimer) return;
  clearInterval(typeTimer);
  typeTimer = null;
  calloutMessage.textContent = message;
  calloutMessage.classList.remove('typing');
}

function measure() {
  calloutSize = { w: callout.offsetWidth, h: callout.offsetHeight };
}

function replay(el, cls) {
  el.classList.remove(cls);
  void el.offsetWidth; // restart the animation
  el.classList.add(cls);
}

window.aeval.on('buddy:show', ({ x, y, message: msg, kind, stepIndex, stepCount }) => {
  // Model-directed points are stepped through by clicking the bubble;
  // walkthrough steps advance on the app action they describe.
  const isPoint = kind === 'point';
  advanceOnClick = isPoint;
  lastOfSequence = isPoint && stepIndex + 1 >= stepCount;
  calloutHint.hidden = !isPoint;
  if (isPoint) {
    calloutHint.textContent = lastOfSequence ? 'click to close' : 'click for the next one';
    calloutCount.textContent = stepCount > 1 ? `${stepIndex + 1} of ${stepCount}` : '';
  } else {
    calloutCount.textContent = '';
  }

  const newMessage = msg !== message || isPoint;
  if (newMessage) {
    message = msg;
    dragBy = { x: 0, y: 0 };
    typeOut(msg);
    replay(callout, 'in');
  }

  target = { x, y };
  if (!shown) {
    // First appearance lands in place; every later update glides.
    cur = { x, y };
    shown = true;
  }
  if (newMessage) replay(buddy, 'land');
  buddy.classList.remove('done');
  callout.classList.remove('done');
  buddy.classList.add('visible');
  callout.classList.add('visible');
  schedule();
  positionCallout();
});

window.aeval.on('buddy:hide', () => {
  // Fade only; position is kept so a re-show glides from where it was.
  buddy.classList.remove('visible');
  callout.classList.remove('visible');
  // The main process hands the mouse back at the same moment; this keeps the
  // local flag in step so the next mousemove does not think it already did.
  dragging = null;
  ignoreState = true;
});

window.aeval.on('buddy:done', () => {
  finishTyping();
  buddy.classList.add('done');
  callout.classList.add('done');
});

// The window is click-through by default. While the cursor is over the
// visible bubble, hit-testing turns on so its buttons work; anywhere else,
// clicks keep passing through to whatever is underneath. Hit-testing also
// stays on for the whole of a drag, otherwise letting the cursor slip off
// the bubble would drop it mid-gesture.
function setIgnore(ignore) {
  if (ignore === ignoreState) return;
  ignoreState = ignore;
  window.aeval.invoke('buddy:set-ignore', { ignore });
}

document.addEventListener('mousemove', (e) => {
  if (dragging) {
    dragBy = { x: dragging.baseX + (e.clientX - dragging.fromX), y: dragging.baseY + (e.clientY - dragging.fromY) };
    positionCallout();
    return;
  }
  const el = document.elementFromPoint(e.clientX, e.clientY);
  const overCallout =
    !!(el && el.closest('#callout')) && callout.classList.contains('visible');
  setIgnore(!overCallout);
});

document.addEventListener('mouseleave', () => {
  if (!dragging) setIgnore(true);
});

// Drag anywhere on the bubble except its buttons; a click that never moved
// just skips the typing.
callout.addEventListener('mousedown', (e) => {
  if (e.button !== 0 || e.target.closest('button')) return;
  dragging = { fromX: e.clientX, fromY: e.clientY, baseX: dragBy.x, baseY: dragBy.y, moved: false };
  callout.classList.add('dragging');
  e.preventDefault();
});

// A click that never moved is navigation, not a drag. The first one finishes
// the typing (the way a dialogue box in a game does), the next one moves on.
document.addEventListener('mouseup', (e) => {
  if (!dragging) return;
  const moved =
    Math.abs(e.clientX - dragging.fromX) > 2 || Math.abs(e.clientY - dragging.fromY) > 2;
  dragging = null;
  callout.classList.remove('dragging');
  if (moved) return;
  replay(callout, 'press');
  if (typeTimer) {
    finishTyping();
    return;
  }
  if (advanceOnClick) window.aeval.invoke('buddy:advance');
});

window.addEventListener('resize', positionCallout);

calloutClose.addEventListener('click', () => {
  window.aeval.invoke('buddy:dismiss');
});
