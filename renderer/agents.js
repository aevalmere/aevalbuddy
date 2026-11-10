'use strict';

// The agents card (src-tauri/src/agents/mod.rs). Rust sends `agents:card`
// with everything to draw; the page draws it out of sight, tells Rust how
// tall it is (`agents:size`, which places and shows the window), and only
// then lets it grow out of the bar. `agents:hide` folds it back, and the
// page says `agents:hidden` once the frame without it is on screen.
(() => {
  const card = document.getElementById('card');
  const jump = document.getElementById('jump');
  const dot = document.getElementById('dot');
  const title = document.getElementById('title');
  const place = document.getElementById('place');
  const keys = document.getElementById('keys');
  const close = document.getElementById('close');
  const detail = document.getElementById('detail');
  const actions = document.getElementById('actions');
  const rows = document.getElementById('rows');
  const more = document.getElementById('more');

  // Room round the card for its shadow, both sides.
  const MARGIN = 16;
  const OPEN_AFTER_MS = 150;
  // The fold's length (agents.css, --d-glob-close).
  const FOLD_MS = 260;
  let data = null;
  let shown = '';
  let openTimer = null;
  let hideSeq = 0;

  const send = (channel, payload) => {
    try {
      return Promise.resolve(window.aeval.invoke(channel, payload)).catch(() => null);
    } catch (err) {
      return Promise.resolve(null);
    }
  };

  function el(tag, cls, text) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  }

  function replay(node, cls) {
    node.classList.remove(cls);
    void node.offsetWidth; // restart the animation
    node.classList.add(cls);
  }

  // The card's height with its shadow room, for the window.
  // offsetHeight, which a transform in mid-animation does not change.
  const height = () => card.offsetHeight + MARGIN * 2;

  function size() {
    return send('agents:size', { height: height() }).then((r) => {
      if (r && r.from) document.body.dataset.from = r.from;
      if (window.AevalGlass) window.AevalGlass.track(40);
    });
  }

  function row(r) {
    const li = el('li', `row ${r.state}${r.depth ? ' child' : ''}`);
    const b = el('button', 'row-btn');
    b.type = 'button';
    b.appendChild(el('span', 'dot'));
    b.appendChild(el('span', 'row-name', r.name));
    if (r.place) b.appendChild(el('span', 'row-place', r.place));
    b.appendChild(el('span', 'row-text', r.text));
    if (r.canJump) b.addEventListener('click', () => send('agents:open', { session: r.session }));
    else b.disabled = true;
    li.appendChild(b);
    return li;
  }

  function draw(p) {
    data = p;
    card.dataset.kind = p.kind;
    document.body.dataset.from = p.from || 'top';
    title.textContent = p.title;
    place.textContent = p.place || '';
    place.hidden = !p.place;
    keys.hidden = !p.altTab;
    jump.classList.toggle('plain', !p.canJump);
    detail.textContent = p.detail || '';
    detail.hidden = !p.detail;

    actions.textContent = '';
    for (const a of p.actions || []) {
      const b = el('button', a.id === 'enable' ? 'act primary' : 'act', a.label);
      b.type = 'button';
      b.addEventListener('click', () => send('agents:act', { id: a.id }));
      actions.appendChild(b);
    }
    actions.hidden = !actions.childElementCount;
    // A question from the app is read whole before it is answered.
    card.classList.toggle('asks', actions.childElementCount > 0);

    rows.textContent = '';
    for (const r of p.rows || []) rows.appendChild(row(r));
    rows.hidden = !rows.childElementCount;
    more.hidden = !(p.more > 0);
    more.textContent = p.more > 0 ? `${p.more} more` : '';
  }

  window.aeval.on('agents:card', (p) => {
    if (!p) return;
    const key = [p.session, p.kind, p.title, p.detail].join('\n');
    const fresh = key !== shown || card.hidden || card.classList.contains('out');
    hideSeq += 1;
    draw(p);
    card.hidden = false;
    if (!fresh) {
      // The same card with newer rows: it keeps its place and only resizes.
      size();
      return;
    }
    shown = key;
    card.classList.remove('out', 'open', 'in');
    card.classList.add('measuring');
    size().then(() => {
      card.classList.remove('measuring');
      replay(card, 'in');
    });
  });

  window.aeval.on('agents:hide', () => {
    clearTimeout(openTimer);
    card.classList.remove('in', 'open');
    card.classList.add('out');
    shown = '';
    // The window hides when the fold is over and painted, so it never goes
    // with a card on it and never comes back showing the old one.
    const seq = ++hideSeq;
    const gone = () => {
      if (seq !== hideSeq || card.hidden || !card.classList.contains('out')) return;
      card.hidden = true;
      window.AevalGlass.painted(() => {
        if (seq === hideSeq && card.hidden) send('agents:hidden');
      });
    };
    Promise.all(card.getAnimations().map((a) => a.finished)).then(gone, gone);
    // A page that is not being painted never finishes an animation.
    setTimeout(gone, FOLD_MS + 60);
  });

  // The pointer on the card holds the countdown, and after a moment opens
  // the whole message.
  card.addEventListener('mouseenter', () => {
    send('agents:hover', { on: true });
    clearTimeout(openTimer);
    if (!data || !data.expand) return;
    openTimer = setTimeout(() => {
      card.classList.add('open');
      size();
    }, OPEN_AFTER_MS);
  });

  card.addEventListener('mouseleave', () => {
    send('agents:hover', { on: false });
    clearTimeout(openTimer);
    if (!card.classList.contains('open')) return;
    card.classList.remove('open');
    // The window follows once the card has closed.
    setTimeout(() => {
      if (!card.classList.contains('open') && !card.hidden) size();
    }, 220);
  });

  jump.addEventListener('click', () => {
    if (!data) return;
    if (data.canJump) send('agents:open', data.session ? { session: data.session } : {});
    else if (!(data.actions || []).length) send('agents:dismiss');
  });

  close.addEventListener('click', () => send('agents:dismiss'));

  // Last, so every listener above is live before Rust sends the card.
  send('agents:ready');
})();
