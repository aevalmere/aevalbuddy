'use strict';

const buddy = document.getElementById('buddy');
const ring = buddy.querySelector('.ring');
const callout = document.getElementById('callout');

let visible = false;

function clamp(value, min, max) {
  return Math.max(min, Math.min(max, value));
}

window.aeval.on('buddy:show', ({ x, y, targetW, targetH, message }) => {
  const ringSize = clamp(Math.max(targetW, targetH) + 16, 30, 96);
  ring.style.width = `${ringSize}px`;
  ring.style.height = `${ringSize}px`;

  const firstShow = !visible;
  if (firstShow) {
    // Appear in place; only later moves slide.
    buddy.classList.add('no-anim');
    callout.classList.add('no-anim');
  }

  buddy.style.transform = `translate3d(${x}px, ${y}px, 0)`;

  callout.textContent = message;
  const cw = callout.offsetWidth;
  const ch = callout.offsetHeight;
  let left = x + ringSize / 2 + 14;
  if (left + cw > window.innerWidth - 12) {
    left = x - ringSize / 2 - 14 - cw;
  }
  const top = clamp(y - ch / 2, 12, window.innerHeight - ch - 12);
  callout.style.transform = `translate3d(${Math.round(left)}px, ${Math.round(top)}px, 0)`;

  if (firstShow) {
    // Flush styles so the jump lands before transitions come back.
    void buddy.offsetWidth;
    buddy.classList.remove('no-anim');
    callout.classList.remove('no-anim');
  }

  buddy.classList.remove('done');
  callout.classList.remove('done');
  buddy.classList.add('visible');
  callout.classList.add('visible');
  visible = true;
});

window.aeval.on('buddy:hide', () => {
  buddy.classList.remove('visible');
  callout.classList.remove('visible');
  visible = false;
});

window.aeval.on('buddy:done', () => {
  buddy.classList.add('done');
  callout.classList.add('done');
  visible = false;
});
