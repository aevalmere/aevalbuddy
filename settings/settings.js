'use strict';

// Settings. One column of pages (Providers, Models, Answers, Find, Tasks,
// Privacy, Shortcuts, General), each a title and grouped rows. A row is one
// line, name and value, and the whole line is the button that opens it.
//
// Channels other lanes are still adding (provider order, failover, text
// size, the OpenRouter connect flow) may be missing from an older build:
// `call` turns a refused or blocked channel into a value, and `listen`
// survives a channel the bridge does not allow yet, so a missing route
// costs one feature, never the page.

const $ = (id) => document.getElementById(id);
const reduceMotion = matchMedia('(prefers-reduced-motion: reduce)');

let data = { shortcuts: {}, active: {} };
let capturing = null; // { action } while recording a new combo
let watchSelector = null;

// ---------------------------------------------------------------- helpers

const FAILED = Symbol('failed');
const TIMED_OUT = Symbol('timed out');

// An invoke that never throws: the reply, FAILED when the channel is
// refused, or TIMED_OUT after `ms`.
function call(channel, payload, ms) {
  let p;
  try {
    p = Promise.resolve(window.aeval.invoke(channel, payload)).catch(() => FAILED);
  } catch {
    return Promise.resolve(FAILED);
  }
  if (!ms) return p;
  let timer = null;
  const late = new Promise((resolve) => {
    timer = setTimeout(() => resolve(TIMED_OUT), ms);
  });
  return Promise.race([p, late]).finally(() => clearTimeout(timer));
}

const failed = (r) => r === FAILED || r === TIMED_OUT;

function listen(channel, cb) {
  try {
    window.aeval.on(channel, cb);
  } catch {
    // Not allowed by this build's bridge yet.
  }
}

function el(tag, attrs, text) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs || {})) {
    if (v === null || v === undefined || v === false) continue;
    if (k === 'class') node.className = v;
    else node.setAttribute(k, v === true ? '' : String(v));
  }
  if (text !== undefined) node.textContent = text;
  return node;
}

function svg(markup, size) {
  const s = size || 16;
  const span = document.createElement('span');
  span.className = 'ico';
  span.setAttribute('aria-hidden', 'true');
  span.innerHTML = `<svg viewBox="0 0 20 20" width="${s}" height="${s}" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">${markup}</svg>`;
  return span;
}

const NS = 'vector-effect="non-scaling-stroke"';
const ICON = {
  chevron: `<path d="M8 5l5 5-5 5" ${NS}/>`,
  ring: `<circle cx="10" cy="10" r="6" ${NS}/>`,
  check: `<circle cx="10" cy="10" r="7" ${NS}/><path d="M7 10.2l2 2 4-4.4" ${NS}/>`,
  cross: `<circle cx="10" cy="10" r="7" ${NS}/><path d="M7.5 7.5l5 5M12.5 7.5l-5 5" ${NS}/>`,
  alert: `<circle cx="10" cy="10" r="7" ${NS}/><path d="M10 6.5v4.5M10 13.5v.01" ${NS}/>`,
  clock: `<circle cx="10" cy="10" r="7" ${NS}/><path d="M10 6.5V10l2.5 1.5" ${NS}/>`,
  dash: `<circle cx="10" cy="10" r="7" ${NS}/><path d="M7 10h6" ${NS}/>`,
  spin: `<circle cx="10" cy="10" r="6.5" opacity=".28" ${NS}/><path d="M10 3.5a6.5 6.5 0 0 1 6.5 6.5" ${NS}/>`,
  grip: `<path d="M7.5 5v.01M12.5 5v.01M7.5 10v.01M12.5 10v.01M7.5 15v.01M12.5 15v.01" stroke-width="2.4" ${NS}/>`,
  pick: `<path d="M5 10.5l3.2 3.2L15 6.5" ${NS}/>`,
};

// Status lines share one helper so none can leave stale text behind or
// reserve space while empty.
function makeStatus(node) {
  let timer = null;
  return (text, sticky) => {
    clearTimeout(timer);
    node.textContent = text || '';
    node.hidden = !text;
    if (text && !sticky) {
      timer = setTimeout(() => {
        node.textContent = '';
        node.hidden = true;
      }, 4000);
    }
  };
}

function setSummary(id, text) {
  const node = $(id);
  if (node) node.textContent = text;
}

// "just now", "5 min ago", "at 14:05", "on 12 Oct".
function ago(ms) {
  if (!ms) return '';
  const min = Math.round((Date.now() - ms) / 60000);
  if (min < 1) return 'just now';
  if (min < 60) return `${min} min ago`;
  const d = new Date(ms);
  const time = d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  if (d.toDateString() === new Date().toDateString()) return `at ${time}`;
  return `on ${d.toLocaleDateString([], { month: 'short', day: 'numeric' })}`;
}

function clock(ms) {
  return new Date(ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

// ------------------------------------------------------------- disclosure

// A row shows one line until it is opened. One row is open at a time: two
// open rows is most of the way back to the wall of controls this replaced.
// The body grows from 0fr to 1fr; once it has landed it stops clipping, so a
// model list can drop below it.
function disclosures() {
  return document.querySelectorAll('main .row:not(.static):not(.action):not(.line)');
}

function headOf(row) {
  return row.querySelector(':scope > .row-head > .edit, :scope > .prov-head');
}

function setOpen(row, open) {
  const was = row.dataset.open === '1';
  const head = headOf(row);
  if (head) head.setAttribute('aria-expanded', String(open));
  if (was === open) return;
  row.dataset.open = open ? '1' : '0';
  if (!open) {
    delete row.dataset.settled;
    return;
  }
  const body = row.querySelector(':scope > .body');
  if (reduceMotion.matches || !body) {
    row.dataset.settled = '1';
    return;
  }
  let landed = false;
  const land = () => {
    if (landed) return;
    landed = true;
    body.removeEventListener('transitionend', onEnd);
    if (row.dataset.open === '1') row.dataset.settled = '1';
  };
  const onEnd = (e) => {
    if (e.target === body) land();
  };
  body.addEventListener('transitionend', onEnd);
  setTimeout(land, 420);
}

function openSection(id) {
  closeCombos(false);
  for (const row of disclosures()) setOpen(row, row.id === id);
}

function closeSections() {
  openSection(null);
}

function toggleRow(row) {
  openSection(row.dataset.open === '1' ? null : row.id);
}

document.addEventListener('click', (e) => {
  const btn = e.target.closest('.row-head > .edit');
  if (!btn) return;
  toggleRow(btn.closest('.row'));
});

// ---------------------------------------------------------- radio groups

// Choice groups follow the ARIA radio pattern: one tab stop (the checked
// option), arrows move and choose.
function syncRoving(group) {
  const radios = [...group.querySelectorAll('[role="radio"]')];
  const on = radios.find((r) => r.getAttribute('aria-checked') === 'true') || radios[0];
  for (const r of radios) r.tabIndex = r === on ? 0 : -1;
}

function checkRadios(selector, isOn) {
  const groups = new Set();
  for (const r of document.querySelectorAll(selector)) {
    r.setAttribute('aria-checked', String(!!isOn(r)));
    const g = r.closest('[role="radiogroup"]');
    if (g) groups.add(g);
  }
  for (const g of groups) syncRoving(g);
}

document.addEventListener('keydown', (e) => {
  const radio = e.target.closest && e.target.closest('[role="radio"]');
  if (!radio || e.altKey || e.ctrlKey || e.metaKey) return;
  const group = radio.closest('[role="radiogroup"]');
  if (!group) return;
  const radios = [...group.querySelectorAll('[role="radio"]')].filter((r) => !r.disabled && !r.closest('[hidden]'));
  const i = radios.indexOf(radio);
  let j = -1;
  if (e.key === 'ArrowDown' || e.key === 'ArrowRight') j = (i + 1) % radios.length;
  else if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') j = (i - 1 + radios.length) % radios.length;
  else if (e.key === 'Home') j = 0;
  else if (e.key === 'End') j = radios.length - 1;
  if (j < 0 || j === i) return;
  e.preventDefault();
  radios[j].focus();
  radios[j].click();
});

// -------------------------------------------------------------- providers

const PROV_INFO = {
  openrouter: {
    name: 'OpenRouter',
    offer: 'Free and paid models',
    placeholder: 'sk-or-v1-…',
    keyUrl: 'https://openrouter.ai/keys',
    keyHost: 'openrouter.ai/keys',
    creditUrl: 'https://openrouter.ai/settings/credits',
  },
  anthropic: {
    name: 'Anthropic',
    offer: 'Claude models',
    placeholder: 'sk-ant-…',
    keyUrl: 'https://console.anthropic.com/settings/keys',
    keyHost: 'console.anthropic.com',
    creditUrl: 'https://console.anthropic.com/settings/billing',
  },
  openai: {
    name: 'OpenAI',
    offer: 'GPT models',
    placeholder: 'sk-…',
    keyUrl: 'https://platform.openai.com/api-keys',
    keyHost: 'platform.openai.com/api-keys',
    creditUrl: 'https://platform.openai.com/settings/organization/billing/overview',
  },
  gemini: {
    name: 'Google Gemini',
    offer: 'Gemini models',
    placeholder: 'AIza…',
    keyUrl: 'https://aistudio.google.com/apikey',
    keyHost: 'aistudio.google.com/apikey',
    creditUrl: 'https://aistudio.google.com/apikey',
  },
};
const DEFAULT_ORDER = ['openrouter', 'anthropic', 'openai', 'gemini'];
const PRIVACY_URL = 'https://openrouter.ai/settings/privacy';
const ORDINAL = ['first', 'second', 'third', 'fourth', 'fifth', 'sixth'];

// Every state is a word plus an icon; the icon is decoration.
const STATUS = {
  'not-added': { word: 'Not added', icon: 'ring' },
  checking: { word: 'Checking…', icon: 'spin' },
  ready: { word: 'Ready', icon: 'check', tone: 'ok' },
  rejected: { word: 'Key rejected', icon: 'cross', tone: 'danger' },
  'no-credit': { word: 'No credit', icon: 'alert', tone: 'warn' },
  busy: { word: 'Busy', icon: 'clock' },
  blocked: { word: 'Free models off', icon: 'alert', tone: 'warn' },
  unchecked: { word: 'Not checked', icon: 'dash' },
  // Before Rust reports statuses: a key is stored, nothing more is known.
  saved: { word: 'Saved', icon: 'check' },
};

const prov = {
  list: [],
  ui: {},
  rows: new Map(),
  listEl: $('providers'),
  live: $('prov-live'),
  ordered: false, // Rust takes an order (settings:set-provider-order)
  suppressClickUntil: 0,
};

const infoOf = (id) => PROV_INFO[id] || { name: id, offer: '', placeholder: '', keyUrl: '', keyHost: '', creditUrl: '' };
const nameOf = (id) => infoOf(id).name;
const article = (name) => (/^[aeiou]/i.test(name) ? 'an' : 'a');
const uiOf = (id) => prov.ui[id] || (prov.ui[id] = {});
const provById = (id) => prov.list.find((p) => p.id === id);

// The provider a pasted key belongs to, by its prefix.
function detectProvider(key) {
  const k = key.trim();
  if (/^sk-or-/.test(k)) return 'openrouter';
  if (/^sk-ant-/.test(k)) return 'anthropic';
  if (/^AIza/.test(k)) return 'gemini';
  if (/^sk-/.test(k)) return 'openai';
  return '';
}

function providersFrom(d) {
  const hints = d.keyHints || {};
  if (Array.isArray(d.providers) && d.providers.length) {
    prov.ordered = true;
    return d.providers
      .filter((p) => p && PROV_INFO[p.id])
      .slice()
      .sort((a, b) => (a.order || 0) - (b.order || 0))
      .map((p) => ({
        id: p.id,
        name: nameOf(p.id),
        hasKey: !!p.hasKey,
        keyHint: p.keyHint || hints[p.id] || '',
        status: p.hasKey ? p.status || 'saved' : 'not-added',
        checkedAt: p.checkedAt || 0,
        busyUntil: p.busyUntil || 0,
        credit: p.credit,
      }));
  }
  // An older build: the presence map, or hasKey alone for one OpenRouter key.
  const keys = d.keys || { openrouter: !!d.hasKey };
  return DEFAULT_ORDER.map((id) => ({
    id,
    name: nameOf(id),
    hasKey: !!keys[id],
    keyHint: hints[id] || '',
    status: keys[id] ? 'saved' : 'not-added',
    checkedAt: 0,
    busyUntil: 0,
    credit: undefined,
  }));
}

const answers = (p) => p.status === 'ready' || p.status === 'saved';
const defaultProvider = () => prov.list.find(answers) || null;

function say(text) {
  // One status line under the list speaks every change; clearing first lets
  // the same sentence be heard twice.
  prov.live.textContent = '';
  requestAnimationFrame(() => {
    prov.live.textContent = text || '';
  });
}

function creditText(p) {
  if (p.id !== 'openrouter') return '';
  const c = p.credit && typeof p.credit === 'object' ? p.credit.remaining : p.credit;
  if (typeof c !== 'number') return '';
  if (c <= 0) return ' Free models only.';
  return ` About $${c.toFixed(2)} of credit left.`;
}

function busyText(p) {
  const until = p.busyUntil ? ` until about ${clock(p.busyUntil)}` : ' for a minute';
  const head = p.id === 'openrouter' ? "OpenRouter's free models are busy." : `${p.name} is limiting requests right now.`;
  const next = prov.list.find((q) => q.id !== p.id && answers(q));
  return next
    ? `${head} Your key is fine. Questions go to ${next.name}${until}.`
    : `${head} Your key is fine. Questions wait and retry${until}.`;
}

// What an expanded row says and offers, from its status and what is going
// on in it right now (design-ux §3.2).
function viewOf(p) {
  const u = uiOf(p.id);
  const n = p.name;
  const info = infoOf(p.id);
  const v = { status: p.status, msg: '', field: false, fieldLabel: `${n} API key`, actions: [], order: true, busy: false };
  const getKey = { kind: 'link', label: `Get a key at ${info.keyHost}`, href: info.keyUrl };

  if (u.testing || p.status === 'checking') {
    v.status = 'checking';
    v.msg = `Checking the key with ${n}…`;
    v.busy = true;
    // The field stays, disabled, while the key in it is checked.
    v.field = p.hasKey ? !!u.mode : p.id !== 'openrouter' || u.mode === 'paste';
    return v;
  }
  if (u.removing) {
    v.msg = `Remove the ${n} key from this PC? This doesn't revoke it at ${n}.`;
    v.actions = [
      { id: 'remove-yes', label: 'Remove', kind: 'danger' },
      { id: 'remove-no', label: 'Keep' },
    ];
    v.order = false;
    return v;
  }
  if (u.detected) {
    const dn = nameOf(u.detected);
    v.msg = `This is ${article(dn)} ${dn} key.`;
    v.field = true;
    v.actions = [{ id: 'save-under', label: `Save it under ${dn}`, kind: 'prominent' }];
    return v;
  }
  if (u.error) {
    v.msg = u.error;
    v.field = !p.hasKey || !!u.mode;
    v.actions = p.hasKey ? [{ id: 'cancel', label: 'Cancel' }] : p.id === 'openrouter' ? [{ id: 'connect', label: 'Connect instead', kind: 'text' }] : [getKey];
    if (!p.hasKey && p.id === 'openrouter') v.actions.unshift(getKey);
    return v;
  }
  if (u.mode === 'replace' && p.hasKey) {
    v.msg = `Paste the new key. The one saved now stays until the new one works.`;
    v.field = true;
    v.fieldLabel = `New ${n} API key`;
    v.actions = [{ id: 'cancel', label: 'Cancel' }, getKey];
    return v;
  }

  switch (p.status) {
    case 'not-added':
      if (p.id === 'openrouter') {
        v.msg = 'Free models, no card needed. Connecting takes about a minute.';
        if (u.connecting) {
          v.msg = 'Approve AevalBuddy in the OpenRouter tab that opened in your browser. This page carries on when you have.';
          v.actions = [
            { id: 'connect', label: 'Waiting for OpenRouter…', kind: 'prominent', disabled: true },
            { id: 'paste', label: 'Paste a key instead', kind: 'text' },
          ];
        } else if (u.mode === 'paste') {
          v.field = true;
          v.actions = [getKey, { id: 'connect', label: 'Connect instead', kind: 'text' }];
        } else {
          v.actions = [
            { id: 'connect', label: 'Connect OpenRouter', kind: 'prominent' },
            { id: 'paste', label: 'Paste a key instead', kind: 'text' },
          ];
        }
      } else {
        v.msg = `Use your own ${n} account. Each question is billed to it.`;
        v.field = true;
        v.actions = [getKey];
      }
      break;
    case 'ready':
      v.msg = `Works. Checked ${ago(p.checkedAt) || 'just now'}.${creditText(p)}`;
      v.actions = [
        { id: 'test', label: 'Test again' },
        { id: 'replace', label: 'Replace key' },
        { id: 'remove', label: 'Remove' },
      ];
      break;
    case 'saved':
      v.msg = 'Saved on this PC.';
      v.actions = [
        { id: 'test', label: 'Test' },
        { id: 'replace', label: 'Replace key' },
        { id: 'remove', label: 'Remove' },
      ];
      break;
    case 'rejected':
      v.msg = `${n} rejected this key. It may have been revoked or mistyped. AevalBuddy is skipping ${n} until you replace it.`;
      v.actions = [
        { id: 'replace', label: 'Replace key', kind: 'prominent' },
        { id: 'remove', label: 'Remove' },
      ];
      break;
    case 'no-credit':
      v.msg =
        p.id === 'openrouter'
          ? 'The key works, but the account has no credit, so only free models answer.'
          : `The key works, but the ${n} account has no credit. AevalBuddy is skipping ${n}.`;
      v.actions = [
        { kind: 'link', label: 'Add credit', href: info.creditUrl, button: true },
        { id: 'test', label: 'Test again' },
        { id: 'remove', label: 'Remove', kind: 'text' },
      ];
      break;
    case 'busy':
      v.msg = busyText(p);
      v.actions = [{ id: 'test', label: 'Try now' }];
      break;
    case 'blocked':
      v.msg =
        "Your OpenRouter privacy setting blocks free models. They're offered on the condition that their providers may keep and train on what they receive, which includes your screen. Turn on \"Free model publication\" to use them, or add credit and pick a paid model.";
      v.actions = [
        { kind: 'link', label: 'Open OpenRouter privacy settings', href: PRIVACY_URL, button: true },
        { id: 'test', label: 'Test again' },
      ];
      break;
    case 'unchecked':
    default:
      v.msg = `Can't reach ${n} right now. The key is saved; AevalBuddy checks again when you're online.`;
      v.actions = [
        { id: 'test', label: 'Test again' },
        { id: 'remove', label: 'Remove', kind: 'text' },
      ];
  }
  return v;
}

function buildRow(id) {
  const info = infoOf(id);
  const li = el('li', { class: 'row prov', id: `prov-${id}`, 'data-provider': id });

  const head = el('button', {
    type: 'button',
    class: 'prov-head',
    id: `prov-${id}-btn`,
    'aria-expanded': 'false',
    'aria-controls': `prov-${id}-body`,
    'aria-describedby': `prov-${id}-sub`,
    'aria-keyshortcuts': 'Alt+ArrowUp Alt+ArrowDown',
  });
  const grip = svg(ICON.grip, 16);
  grip.classList.add('grip');
  head.appendChild(grip);
  head.appendChild(el('span', { class: 'pos' }));
  const who = el('span', { class: 'who' });
  who.appendChild(el('span', { class: 'name' }, info.name));
  who.appendChild(el('span', { class: 'sub', id: `prov-${id}-sub` }));
  head.appendChild(who);
  const state = el('span', { class: 'state' });
  head.appendChild(state);
  head.appendChild(el('span', { class: 'default-cap' }, 'Default'));
  const chev = svg(ICON.chevron, 14);
  chev.classList.add('chev');
  head.appendChild(chev);
  li.appendChild(head);

  const body = el('div', { class: 'body', id: `prov-${id}-body` });
  const inner = el('div', { class: 'body-in' });
  inner.appendChild(el('p', { class: 'prov-msg', id: `prov-${id}-msg` }));

  const clip = el('div', { class: 'prov-clip', hidden: true });
  clip.appendChild(el('span', { class: 'clip-text' }));
  clip.appendChild(el('button', { type: 'button', class: 'g-btn prominent', 'data-act': 'clip' }, 'Use this key'));
  inner.appendChild(clip);

  // The key field stays in the page for the row's life: re-rendering the
  // row never moves it, so focus and a half-pasted key survive.
  const field = el('div', { class: 'prov-field', hidden: true });
  const inputId = id === 'openrouter' ? 'api-key-input' : `key-${id}`;
  field.appendChild(el('label', { class: 'field-label', for: inputId }, `${info.name} API key`));
  const fieldRow = el('div', { class: 'field-row' });
  const input = el('input', {
    id: inputId,
    class: 'field key-input',
    type: 'password',
    placeholder: info.placeholder,
    autocomplete: 'off',
    spellcheck: 'false',
    'aria-describedby': `prov-${id}-msg`,
    'data-provider': id,
  });
  fieldRow.appendChild(input);
  fieldRow.appendChild(el('button', { type: 'button', class: 'g-btn', 'data-act': 'show', 'aria-pressed': 'false', 'aria-controls': inputId }, 'Show'));
  fieldRow.appendChild(el('button', { type: 'button', class: 'g-btn prominent', 'data-act': 'save' }, 'Save'));
  field.appendChild(fieldRow);
  inner.appendChild(field);

  inner.appendChild(el('div', { class: 'actions prov-actions' }));
  const order = el('div', { class: 'actions prov-order', role: 'group', 'aria-label': `Order of ${info.name}` });
  order.appendChild(el('button', { type: 'button', class: 'text-btn', 'data-act': 'up' }, 'Move up'));
  order.appendChild(el('button', { type: 'button', class: 'text-btn', 'data-act': 'down' }, 'Move down'));
  order.appendChild(el('button', { type: 'button', class: 'text-btn', 'data-act': 'first' }, 'Use first'));
  inner.appendChild(order);

  body.appendChild(inner);
  li.appendChild(body);
  return li;
}

function rowFor(id) {
  if (!prov.rows.has(id)) prov.rows.set(id, buildRow(id));
  return prov.rows.get(id);
}

function actionNode(a) {
  if (a.kind === 'link') {
    return el('a', { class: a.button ? 'g-btn' : 'text-link', href: '#', 'data-external': a.href }, a.label);
  }
  const cls = a.kind === 'prominent' ? 'g-btn prominent' : a.kind === 'danger' ? 'g-btn danger' : a.kind === 'text' ? 'text-btn' : 'g-btn';
  return el('button', { type: 'button', class: cls, 'data-act': a.id, disabled: a.disabled }, a.label);
}

function renderProviders() {
  const focused = document.activeElement;
  // An action button is rebuilt below; remember which one had focus.
  const inActs = focused && focused.closest ? focused.closest('.prov-actions') : null;
  const keep = inActs ? { id: inActs.closest('.prov').dataset.provider, act: focused.dataset.act, href: focused.dataset.external } : null;
  const n = prov.list.length;
  const def = defaultProvider();
  const ul = prov.listEl;

  prov.list.forEach((p, i) => {
    const li = rowFor(p.id);
    if (ul.children[i] !== li) ul.insertBefore(li, ul.children[i] || null);
    const v = viewOf(p);
    const st = STATUS[v.status] || STATUS.unchecked;
    const isDefault = def && def.id === p.id;

    const head = li.querySelector('.prov-head');
    head.querySelector('.pos').textContent = String(i + 1);
    const sub = head.querySelector('.sub');
    sub.textContent = p.keyHint || infoOf(p.id).offer;
    sub.classList.toggle('mono', !!p.keyHint);
    const state = head.querySelector('.state');
    state.textContent = '';
    state.appendChild(svg(ICON[st.icon], 14));
    state.appendChild(el('span', { class: 'word' }, st.word));
    state.dataset.tone = st.tone || '';
    state.dataset.status = v.status;
    head.querySelector('.default-cap').hidden = !isDefault;
    const label = [p.name, `${i + 1} of ${n}`, st.word.replace('…', '')];
    if (isDefault) label.push('default');
    head.setAttribute('aria-label', label.join(', '));
    li.classList.toggle('orderable', prov.ordered);

    const msg = li.querySelector('.prov-msg');
    msg.textContent = v.msg;
    msg.dataset.tone = p.status === 'rejected' && !uiOf(p.id).mode ? 'danger' : '';

    const u = uiOf(p.id);
    const clip = li.querySelector('.prov-clip');
    clip.hidden = !(u.clip && !p.hasKey && !v.busy && !u.removing);
    if (u.clip) clip.querySelector('.clip-text').textContent = `Key on the clipboard: ${u.clip}`;

    const field = li.querySelector('.prov-field');
    field.hidden = !v.field;
    field.querySelector('.field-label').textContent = v.fieldLabel;
    const input = field.querySelector('input');
    input.disabled = v.busy;
    for (const b of field.querySelectorAll('button')) b.disabled = v.busy;

    const acts = li.querySelector('.prov-actions');
    acts.textContent = '';
    for (const a of v.actions) acts.appendChild(actionNode(a));
    acts.hidden = !v.actions.length;

    const order = li.querySelector('.prov-order');
    order.hidden = !(v.order && prov.ordered && n > 1);
    order.querySelector('[data-act="up"]').disabled = i === 0 || v.busy;
    order.querySelector('[data-act="down"]').disabled = i === n - 1 || v.busy;
    order.querySelector('[data-act="first"]').hidden = i === 0;
    order.querySelector('[data-act="first"]').disabled = v.busy;
  });

  // Focus goes where the last action said, else stays where it was.
  const want = prov.focusAfter;
  prov.focusAfter = null;
  let target = null;
  if (want) {
    const li = rowFor(want.id);
    if (want.to === 'head') target = li.querySelector('.prov-head');
    else if (want.to === 'field') target = li.querySelector('.prov-field input');
    else if (want.to) target = li.querySelector(`[data-act="${want.to}"]`);
  }
  if (!target && keep && !focused.isConnected) {
    const li = rowFor(keep.id);
    const sel = keep.act ? `[data-act="${keep.act}"]` : `[data-external="${CSS.escape(keep.href || '')}"]`;
    target = li.querySelector(`.prov-actions ${sel}`) || li.querySelector('.prov-head');
  }
  if (target && !target.disabled && !target.closest('[hidden]')) target.focus();
  else if (focused && focused.isConnected && document.activeElement !== focused) focused.focus();

  renderFailover();
  renderRecipients();
  renderModelProviders();
}

function setUi(id, patch) {
  prov.ui[id] = Object.assign(uiOf(id), patch);
}

function clearField(id) {
  const li = rowFor(id);
  const input = li.querySelector('.prov-field input');
  input.value = '';
  input.type = 'password';
  const show = li.querySelector('[data-act="show"]');
  show.textContent = 'Show';
  show.setAttribute('aria-pressed', 'false');
}

function statusWord(id) {
  const p = provById(id);
  return p ? (STATUS[p.status] || STATUS.unchecked).word : '';
}

function announceStatus(id) {
  const p = provById(id);
  if (!p) return;
  const n = p.name;
  const text = {
    ready: `${n} is ready.`,
    saved: `${n} key saved.`,
    rejected: `${n} rejected the key.`,
    'no-credit': `${n} has no credit.`,
    busy: `${n} is busy.`,
    blocked: "OpenRouter's free models are off.",
    unchecked: `Couldn't check ${n}.`,
    'not-added': `${n} key removed.`,
    checking: `Checking ${n}…`,
  }[p.status];
  if (text) say(text);
}

// The status a test result means: Rust's word when it gives one, else read
// from the sentence an older build returns.
function statusOfTest(r) {
  if (failed(r) || !r) return 'unchecked';
  if (r.status) return r.status;
  if (r.ok) return 'ready';
  const m = String(r.message || r.error || '');
  if (/reject|invalid|not valid/i.test(m)) return 'rejected';
  if (/\b402\b|credit|quota|billing/i.test(m)) return 'no-credit';
  if (/\b429\b|rate limit|busy/i.test(m)) return 'busy';
  if (/privacy|data policy|publication/i.test(m)) return 'blocked';
  return 'unchecked';
}

// The credit an older build only writes into its sentence: "About $7.50 of
// credit left." or "the credit is used up".
function creditOfMessage(m) {
  const left = /\$([\d.]+) of credit left/.exec(m || '');
  if (left) return Number(left[1]);
  if (/credit is used up/.test(m || '')) return 0;
  return undefined;
}

function applyTest(id, r) {
  const p = provById(id);
  if (!p) return;
  p.status = p.hasKey ? statusOfTest(r) : 'not-added';
  if (p.status === 'ready') p.checkedAt = Date.now();
  if (r && !failed(r)) {
    const fromText = r.credit === undefined ? creditOfMessage(r.message) : undefined;
    if (fromText !== undefined) p.credit = fromText;
    if (r.credit !== undefined) p.credit = r.credit;
    if (r.busyUntil) p.busyUntil = r.busyUntil;
    if (r.keyHint) p.keyHint = r.keyHint;
  }
}

async function testKey(id) {
  setUi(id, { testing: true, error: '' });
  renderProviders();
  const r = await call('settings:test-key', { provider: id }, 15000);
  setUi(id, { testing: false });
  applyTest(id, r);
  renderProviders();
  announceStatus(id);
}

async function saveKey(id, as) {
  const li = rowFor(id);
  const input = li.querySelector('.prov-field input');
  const key = input.value.trim();
  if (!key) {
    setUi(id, { error: 'Paste a key first.' });
    prov.focusAfter = { id, to: 'field' };
    renderProviders();
    return;
  }
  const detected = detectProvider(key);
  if (!as && detected && detected !== id) {
    setUi(id, { detected, error: '' });
    prov.focusAfter = { id, to: 'save-under' };
    renderProviders();
    say(`This is ${article(nameOf(detected))} ${nameOf(detected)} key.`);
    return;
  }
  const target = as || id;
  if (target !== id) {
    // The key moves to its own provider's row, which opens; this one closes
    // as it was.
    const other = provById(target);
    rowFor(target).querySelector('.prov-field input').value = key;
    clearField(id);
    setUi(id, { detected: '', error: '', mode: null });
    setUi(target, { mode: other && other.hasKey ? 'replace' : target === 'openrouter' ? 'paste' : null });
    openSection(`prov-${target}`);
    return saveKey(target, target);
  }
  setUi(target, { testing: true, error: '', detected: '' });
  renderProviders();
  say(`Checking the key with ${nameOf(target)}…`);

  const r = await call('settings:save-key', { provider: target, key }, 15000);
  setUi(target, { testing: false });
  const p = provById(target);

  if (r === TIMED_OUT) {
    // Rust keeps a key it could not test; this page stops waiting.
    clearField(id);
    setUi(target, { mode: null });
    p.hasKey = true;
    p.status = 'unchecked';
    prov.focusAfter = { id: target, to: 'head' };
    renderProviders();
    announceStatus(target);
    refreshFromRust();
    return;
  }
  if (r === FAILED || !r) {
    setUi(target, { error: "AevalBuddy couldn't save the key. Try again." });
    prov.focusAfter = { id, to: 'field' };
    renderProviders();
    say("AevalBuddy couldn't save the key.");
    return;
  }
  if (!r.ok) {
    if (r.detected && r.detected !== target) {
      setUi(id, { detected: r.detected });
      prov.focusAfter = { id, to: 'save-under' };
      renderProviders();
      say(`This is ${article(nameOf(r.detected))} ${nameOf(r.detected)} key.`);
      return;
    }
    const rejected = r.status === 'rejected' || /reject|invalid|not valid/i.test(String(r.error || ''));
    setUi(target, {
      error: rejected ? `${p.name} rejected this key, so it wasn't saved. Check it and paste it again.` : r.error || "AevalBuddy couldn't save the key. Try again.",
    });
    prov.focusAfter = { id, to: 'field' };
    renderProviders();
    say(rejected ? `${p.name} rejected this key, so it wasn't saved.` : r.error || "Couldn't save the key.");
    return;
  }

  clearField(id);
  setUi(target, { mode: null, error: '', clip: '' });
  p.hasKey = true;
  if (r.keyHint) p.keyHint = r.keyHint;
  prov.focusAfter = { id: target, to: 'head' };
  if (r.status) {
    applyTest(target, r);
    renderProviders();
    announceStatus(target);
  } else {
    // An older build stores without testing; test it now.
    await testKey(target);
    prov.focusAfter = { id: target, to: 'head' };
  }
  refreshFromRust();
}

async function useClipKey(id) {
  setUi(id, { testing: true });
  renderProviders();
  const r = await call('settings:use-clip-key', { provider: id }, 15000);
  setUi(id, { testing: false, clip: '' });
  if (failed(r) || !r || !r.ok) {
    setUi(id, { error: (r && !failed(r) && r.error) || 'The clipboard no longer holds a key.' });
    renderProviders();
    return;
  }
  const target = r.provider && PROV_INFO[r.provider] ? r.provider : id;
  const p = provById(target);
  p.hasKey = true;
  if (r.keyHint) p.keyHint = r.keyHint;
  prov.focusAfter = { id: target, to: 'head' };
  if (r.status) {
    applyTest(target, r);
    renderProviders();
    announceStatus(target);
  } else {
    await testKey(target);
  }
  refreshFromRust();
}

async function connectOpenRouter() {
  setUi('openrouter', { connecting: true, error: '', mode: null });
  renderProviders();
  say('Your browser opened OpenRouter. Approve AevalBuddy there.');
  const r = await call('auth:openrouter-connect', {}, 6 * 60 * 1000);
  setUi('openrouter', { connecting: false });
  const p = provById('openrouter');
  if (failed(r) || !r || !r.ok) {
    const why =
      r === TIMED_OUT
        ? "Didn't hear back from OpenRouter. Try again, or paste a key instead."
        : (r && !failed(r) && r.error) || "Couldn't connect to OpenRouter. Try again, or paste a key instead.";
    setUi('openrouter', { error: why });
    prov.focusAfter = { id: 'openrouter', to: 'connect' };
    renderProviders();
    say(why);
    return;
  }
  p.hasKey = true;
  if (r.keyHint) p.keyHint = r.keyHint;
  prov.focusAfter = { id: 'openrouter', to: 'head' };
  if (r.status) {
    applyTest('openrouter', r);
    renderProviders();
    announceStatus('openrouter');
  } else {
    await testKey('openrouter');
  }
  refreshFromRust();
}

async function removeKey(id) {
  const p = provById(id);
  setUi(id, { removing: false });
  const r = await call('settings:clear-key', { provider: id });
  if (failed(r) || (r && r.ok === false)) {
    prov.focusAfter = { id, to: 'head' };
    renderProviders();
    say(`Couldn't remove the ${p.name} key.`);
    return;
  }
  p.hasKey = false;
  p.keyHint = '';
  p.status = 'not-added';
  p.credit = undefined;
  setUi(id, { mode: null, error: '' });
  prov.focusAfter = { id, to: 'head' };
  renderProviders();
  say(`${p.name} key removed.`);
  refreshFromRust();
}

function announceMove(id, to) {
  const p = provById(id);
  if (to > 0) {
    say(`${p.name} is now ${ORDINAL[to] || `number ${to + 1}`}.`);
    return;
  }
  const def = defaultProvider();
  if (def && def.id === id) say(`${p.name} is now first. It answers by default.`);
  else if (def) say(`${p.name} is now first. ${def.name} answers until ${p.name} is ready.`);
  else say(`${p.name} is now first.`);
}

async function moveProvider(id, to, focusTo) {
  const ids = prov.list.map((p) => p.id);
  const from = ids.indexOf(id);
  to = Math.max(0, Math.min(ids.length - 1, to));
  if (from < 0 || from === to) return;
  const before = prov.list.slice();
  ids.splice(from, 1);
  ids.splice(to, 0, id);
  prov.list = ids.map((x) => before.find((p) => p.id === x));
  prov.focusAfter = { id, to: focusTo || 'head' };
  renderProviders();
  announceMove(id, to);
  const r = await call('settings:set-provider-order', { order: ids });
  if (failed(r) || (r && r.ok === false)) {
    prov.list = before;
    prov.focusAfter = { id, to: 'head' };
    renderProviders();
    say("Couldn't change the order.");
    return;
  }
  refreshFromRust();
}

// Clicks inside the list: the row heads, and every button in a body.
prov.listEl.addEventListener('click', (e) => {
  const head = e.target.closest('.prov-head');
  if (head) {
    if (performance.now() < prov.suppressClickUntil) return;
    toggleRow(head.closest('.prov'));
    return;
  }
  const btn = e.target.closest('[data-act]');
  if (!btn || btn.disabled) return;
  const id = btn.closest('.prov').dataset.provider;
  const p = provById(id);
  const act = btn.dataset.act;
  const i = prov.list.indexOf(p);
  switch (act) {
    case 'show': {
      const input = btn.closest('.field-row').querySelector('input');
      const show = input.type === 'password';
      input.type = show ? 'text' : 'password';
      btn.textContent = show ? 'Hide' : 'Show';
      btn.setAttribute('aria-pressed', String(show));
      input.focus();
      break;
    }
    case 'save':
      saveKey(id);
      break;
    case 'save-under':
      saveKey(id, uiOf(id).detected);
      break;
    case 'clip':
      useClipKey(id);
      break;
    case 'connect':
      connectOpenRouter();
      break;
    case 'paste':
      setUi(id, { mode: 'paste', error: '' });
      prov.focusAfter = { id, to: 'field' };
      renderProviders();
      break;
    case 'test':
      prov.focusAfter = { id, to: 'test' };
      testKey(id);
      break;
    case 'replace':
      setUi(id, { mode: 'replace', error: '', detected: '' });
      prov.focusAfter = { id, to: 'field' };
      renderProviders();
      break;
    case 'cancel':
      clearField(id);
      setUi(id, { mode: null, error: '', detected: '' });
      prov.focusAfter = { id, to: 'head' };
      renderProviders();
      break;
    case 'remove':
      setUi(id, { removing: true });
      prov.focusAfter = { id, to: 'remove-no' };
      renderProviders();
      say(`Remove the ${p.name} key from this PC?`);
      break;
    case 'remove-no':
      setUi(id, { removing: false });
      prov.focusAfter = { id, to: 'head' };
      renderProviders();
      break;
    case 'remove-yes':
      removeKey(id);
      break;
    case 'up':
      moveProvider(id, i - 1, i - 1 === 0 ? 'down' : 'up');
      break;
    case 'down':
      moveProvider(id, i + 1, i + 1 === prov.list.length - 1 ? 'up' : 'down');
      break;
    case 'first':
      moveProvider(id, 0, 'head');
      break;
    default:
  }
});

prov.listEl.addEventListener('keydown', (e) => {
  const input = e.target.closest('.key-input');
  if (input && e.key === 'Enter') {
    e.preventDefault();
    const id = input.dataset.provider;
    if (uiOf(id).detected) saveKey(id, uiOf(id).detected);
    else saveKey(id);
    return;
  }
  if (input && uiOf(input.dataset.provider).detected && e.key !== 'Enter' && e.key !== 'Tab') {
    // Typing again means a new key; the wrong-provider note no longer holds.
    setUi(input.dataset.provider, { detected: '' });
    requestAnimationFrame(renderProviders);
  }
  const head = e.target.closest('.prov-head');
  if (head && e.altKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown') && prov.ordered) {
    e.preventDefault();
    const id = head.closest('.prov').dataset.provider;
    const i = prov.list.findIndex((p) => p.id === id);
    moveProvider(id, i + (e.key === 'ArrowUp' ? -1 : 1), 'head');
  }
});

prov.listEl.addEventListener('input', (e) => {
  const input = e.target.closest('.key-input');
  if (!input) return;
  const u = uiOf(input.dataset.provider);
  if (u.error && u.error !== '') {
    setUi(input.dataset.provider, { error: '' });
    renderProviders();
  }
});

// Drag to reorder: press a row and move it more than 6 px. The others slide
// out of the way; on release the row settles into its slot from where it was
// dropped. Esc puts it back.
prov.listEl.addEventListener('pointerdown', (e) => {
  if (e.button !== 0 || !prov.ordered) return;
  const head = e.target.closest('.prov-head');
  if (!head) return;
  const li = head.closest('.prov');
  const items = [...prov.listEl.children];
  const from = items.indexOf(li);
  const rects = items.map((x) => x.getBoundingClientRect());
  const drag = { y0: e.clientY, active: false, to: from };

  const reset = () => {
    prov.listEl.classList.remove('reordering');
    li.classList.remove('dragging');
    for (const x of items) x.style.transform = '';
  };
  const move = (ev) => {
    const dy = ev.clientY - drag.y0;
    if (!drag.active) {
      if (Math.abs(dy) < 6) return;
      drag.active = true;
      try {
        head.setPointerCapture(e.pointerId);
      } catch {
        // The pointer is already gone.
      }
      closeCombos(false);
      prov.listEl.classList.add('reordering');
      li.classList.add('dragging');
    }
    const lo = rects[0].top - rects[from].top;
    const hi = rects[items.length - 1].bottom - rects[from].bottom;
    const y = Math.max(lo, Math.min(hi, dy));
    li.style.transform = `translateY(${y}px)`;
    const mid = (rects[from].top + rects[from].bottom) / 2 + y;
    let to = from;
    items.forEach((x, i) => {
      const c = (rects[i].top + rects[i].bottom) / 2;
      if (i < from && mid < c) to = Math.min(to, i);
      if (i > from && mid > c) to = Math.max(to, i);
    });
    drag.to = to;
    const h = rects[from].height;
    items.forEach((x, i) => {
      if (i === from) return;
      let s = 0;
      if (from < to && i > from && i <= to) s = -h;
      if (to < from && i >= to && i < from) s = h;
      x.style.transform = s ? `translateY(${s}px)` : '';
    });
  };
  const end = (ev, cancelled) => {
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', up);
    window.removeEventListener('pointercancel', cancel);
    window.removeEventListener('keydown', onKey, true);
    if (!drag.active) return;
    prov.suppressClickUntil = performance.now() + 350;
    const dropTop = li.getBoundingClientRect().top;
    reset();
    if (cancelled || drag.to === from) {
      settle(li, dropTop);
      return;
    }
    moveProvider(li.dataset.provider, drag.to, 'head');
    settle(li, dropTop);
  };
  const up = (ev) => end(ev, false);
  const cancel = (ev) => end(ev, true);
  const onKey = (ev) => {
    if (ev.key !== 'Escape' || !drag.active) return;
    ev.preventDefault();
    ev.stopPropagation();
    end(ev, true);
  };
  window.addEventListener('pointermove', move);
  window.addEventListener('pointerup', up);
  window.addEventListener('pointercancel', cancel);
  window.addEventListener('keydown', onKey, true);
});

// A dropped row glides from where the pointer left it into its slot.
function settle(li, fromTop) {
  if (reduceMotion.matches) return;
  const dy = fromTop - li.getBoundingClientRect().top;
  if (Math.abs(dy) < 1) return;
  li.style.transition = 'none';
  li.style.transform = `translateY(${dy}px)`;
  li.getBoundingClientRect();
  li.style.transition = '';
  li.classList.add('settling');
  li.style.transform = '';
  setTimeout(() => li.classList.remove('settling'), 320);
}

// A key copied from a provider's site is offered when the window comes
// back. Rust reads the clipboard; only a masked form ever reaches this page.
async function checkClipboardKey() {
  const r = await call('settings:clip-key');
  for (const p of prov.list) setUi(p.id, { clip: '' });
  if (!failed(r) && r && r.found) {
    const id = r.provider && PROV_INFO[r.provider] ? r.provider : 'openrouter';
    const p = provById(id);
    if (p && !p.hasKey) {
      setUi(id, { clip: r.masked });
      const anyOpen = document.querySelector('main .row[data-open="1"]');
      if (!anyOpen) openSection(`prov-${id}`);
    }
  }
  renderProviders();
}
window.addEventListener('focus', checkClipboardKey);

listen('provider:status', (s) => {
  if (!s || !s.id) return;
  const p = provById(s.id);
  if (!p) return;
  const before = p.status;
  if (s.status) p.status = p.hasKey || s.status !== 'not-added' ? s.status : 'not-added';
  if (s.status === 'ready') p.checkedAt = s.checkedAt || Date.now();
  if (s.busyUntil !== undefined) p.busyUntil = s.busyUntil || 0;
  if (s.credit !== undefined) p.credit = s.credit;
  if (s.keyHint) p.keyHint = s.keyHint;
  renderProviders();
  if (p.status !== before && !uiOf(p.id).testing) announceStatus(p.id);
});

// Failover: two switches, the second only matters when the first is on.
const failoverAcross = $('failover-across');
const failoverAsk = $('failover-ask');

function renderFailover() {
  const f = data.failover;
  $('failover-group').hidden = !f;
  if (!f) return;
  failoverAcross.checked = f.acrossProviders !== false;
  failoverAsk.checked = f.askBeforePaid !== false;
  failoverAsk.disabled = !failoverAcross.checked;
  failoverAsk.closest('.row').classList.toggle('off', !failoverAcross.checked);
}

async function setFailover(patch) {
  const next = Object.assign({ acrossProviders: true, askBeforePaid: true }, data.failover, patch);
  const before = data.failover;
  data.failover = next;
  renderFailover();
  const r = await call('settings:set-failover', next);
  if (failed(r) || (r && r.ok === false)) {
    data.failover = before;
    renderFailover();
    say("Couldn't change that setting.");
    return;
  }
  if (r && r.failover) data.failover = r.failover;
  renderFailover();
}

failoverAcross.addEventListener('change', () => setFailover({ acrossProviders: failoverAcross.checked }));
failoverAsk.addEventListener('change', () => setFailover({ askBeforePaid: failoverAsk.checked }));

function renderKeyStorage() {
  const node = $('key-storage');
  const text = {
    encrypted: 'Keys are encrypted with your Windows account and only sent to their own provider.',
    plain:
      "Windows couldn't provide a key store, so keys are saved unencrypted in your AevalBuddy settings file. Anyone signed in to this Windows account can read them.",
  }[data.keyStorage];
  node.textContent = text || '';
  node.hidden = !text;
  node.dataset.tone = data.keyStorage === 'plain' ? 'warn' : '';
}

// ------------------------------------------------------------------ models

const modelsMode = $('models-mode');
const modelsCustom = $('models-custom');
const modelsFor = $('models-for');
const showPaid = $('show-paid');
const brainEnabled = $('brain-enabled');
const brainComboEl = $('brain-combo');
const brainNoteEl = $('brain-note');
const MODE_KEY = 'aeval.settings.modelsMode';
const models = { mode: 'auto', provider: 'openrouter' };

function storedMode() {
  try {
    return localStorage.getItem(MODE_KEY);
  } catch {
    return null;
  }
}

function storeMode(m) {
  try {
    localStorage.setItem(MODE_KEY, m);
  } catch {
    // A private profile; the choice lasts this window.
  }
}

function picksAreDefaults(d) {
  return (
    (!d.defaultModel || d.model === d.defaultModel) &&
    !d.brainEnabled &&
    (!d.defaultFindModel || d.findModel === d.defaultFindModel) &&
    (!d.defaultAgentModel || d.agentModel === d.defaultAgentModel)
  );
}

function renderModelsMode() {
  checkRadios('[data-models-mode]', (r) => r.dataset.modelsMode === models.mode);
  modelsCustom.hidden = models.mode !== 'custom';
}

// One segment per provider with a key; the default provider first chosen.
function renderModelProviders() {
  const keyed = prov.list.filter((p) => p.hasKey);
  if (!keyed.some((p) => p.id === models.provider)) {
    const def = defaultProvider();
    models.provider = def ? def.id : keyed[0] ? keyed[0].id : 'openrouter';
  }
  const seg = modelsFor.querySelector('.seg');
  const showSeg = prov.ordered && keyed.length > 1;
  modelsFor.hidden = !showSeg;
  if (showSeg) {
    const have = [...seg.children].map((b) => b.dataset.provider).join();
    const want = keyed.map((p) => p.id).join();
    if (have !== want) {
      seg.textContent = '';
      for (const p of keyed) {
        seg.appendChild(el('button', { type: 'button', role: 'radio', class: 'seg-option', 'data-provider': p.id }, p.name));
      }
    }
    checkRadios('.seg-option', (b) => b.dataset.provider === models.provider);
  }
  const native = models.provider !== 'openrouter';
  $('paid-row').hidden = native;
  $('billed-note').hidden = !native;
}

modelsFor.addEventListener('click', (e) => {
  const b = e.target.closest('.seg-option');
  if (!b || b.dataset.provider === models.provider) return;
  models.provider = b.dataset.provider;
  renderModelProviders();
  reloadCombos();
});

modelsMode.addEventListener('click', (e) => {
  const b = e.target.closest('[data-models-mode]');
  if (!b) return;
  const m = b.dataset.modelsMode;
  if (m === models.mode) return;
  models.mode = m;
  storeMode(m);
  renderModelsMode();
  if (m === 'auto') useRecommendedModels();
});

// Automatic: every job back to the model AevalBuddy recommends for it.
async function useRecommendedModels() {
  const d = data;
  const saves = [];
  if (d.defaultModel) saves.push(call('settings:save-model', { model: d.defaultModel }));
  if (d.defaultFindModel) saves.push(call('settings:save-find-model', { model: d.defaultFindModel }));
  if (d.defaultAgentModel) saves.push(call('settings:save-agent-model', { model: d.defaultAgentModel }));
  if (d.brainEnabled) saves.push(call('settings:set-brain-enabled', { on: false }));
  if (prov.ordered) {
    for (const p of prov.list.filter((q) => q.hasKey && q.id !== 'openrouter')) {
      for (const ch of ['settings:save-model', 'settings:save-brain', 'settings:save-find-model', 'settings:save-agent-model']) {
        saves.push(call(ch, { provider: p.id, model: '' }));
      }
    }
  }
  await Promise.all(saves);
  data.model = d.defaultModel || data.model;
  data.findModel = d.defaultFindModel || data.findModel;
  data.agentModel = d.defaultAgentModel || data.agentModel;
  data.brainEnabled = false;
  brainEnabled.checked = false;
  showBrainPicker(false);
  reloadCombos();
}

// ----------------------------------------------------------------- combos

// Four pickers with identical behaviour (the screen model, the optional
// answer model, the Find model and the task model): written once. The list
// is a solid menu inside the page (glass on glass is never drawn), grouped
// by tier, with type-ahead.
const combos = [];

function makeCombo(config) {
  const { rootId, button, label, detail, list, note, kind, saveChannel, summaryId } = config;
  const state = { index: new Map(), offline: '', current: '', provider: 'openrouter' };
  let typed = '';
  let typedAt = 0;

  function optionDetail(m) {
    return [m.tag ? `${m.tag}.` : '', m.note, m.detail].filter(Boolean).join(' ');
  }

  function sync() {
    for (const opt of list.querySelectorAll('[role="option"]')) {
      const selected = opt.dataset.id === state.current;
      opt.setAttribute('aria-selected', String(selected));
    }
    const known = state.index.get(state.current);
    label.textContent = known ? known.name : state.current || 'Choose…';
    if (summaryId) setSummary(summaryId, known ? known.name : state.current || 'Not set');
    // The closed control shows price and context only: it is one line, and a
    // description worth reading gets cut off there. The description goes
    // underneath, where it has room to wrap.
    detail.textContent = known ? [known.tag, known.detail].filter(Boolean).join(' · ') : state.current ? 'Custom model id' : '';
    if (note) note.textContent = [known ? known.note : '', state.offline].filter(Boolean).join(' ');
    if (config.onSync) config.onSync();
  }

  async function load(provider) {
    if (provider) state.provider = provider;
    const payload = { kind };
    if (state.provider !== 'openrouter') payload.provider = state.provider;
    const res = await call('models:list', payload);
    if (failed(res) || !res) return;
    list.textContent = '';
    state.index = new Map();
    if (res.current) state.current = res.current;
    let g = 0;
    for (const group of res.groups || []) {
      g += 1;
      const wrap = el('div', { class: 'menu-group', role: 'group', 'aria-label': group.label });
      wrap.appendChild(el('div', { class: 'menu-head', 'aria-hidden': 'true' }, group.label));
      for (const m of group.models) {
        state.index.set(m.id, m);
        const opt = el('div', { class: 'menu-row', role: 'option', tabindex: '-1', 'data-id': m.id, title: m.id, id: `${rootId}-o-${g}-${state.index.size}` });
        opt.appendChild(el('span', { class: 'opt-name' }, m.name));
        const d = optionDetail(m);
        if (d) opt.appendChild(el('span', { class: 'opt-detail' }, d));
        const tick = svg(ICON.pick, 16);
        tick.classList.add('tick');
        opt.appendChild(tick);
        wrap.appendChild(opt);
      }
      list.appendChild(wrap);
    }
    const where = state.provider === 'openrouter' ? 'OpenRouter' : nameOf(state.provider);
    state.offline = res.live === false ? `Could not reach ${where}, so this is the built-in list.` : '';
    sync();
  }

  function options() {
    return [...list.querySelectorAll('[role="option"]')];
  }

  function open() {
    closeCombos(false);
    list.hidden = false;
    button.setAttribute('aria-expanded', 'true');
    const target = list.querySelector('[aria-selected="true"]') || options()[0];
    if (target) {
      target.focus();
      target.scrollIntoView({ block: 'nearest' });
    }
  }

  function close(focusButton) {
    if (list.hidden) return;
    list.hidden = true;
    button.setAttribute('aria-expanded', 'false');
    if (focusButton) button.focus();
  }

  function choose(id) {
    state.current = id;
    const payload = { model: id };
    if (state.provider !== 'openrouter') payload.provider = state.provider;
    call(saveChannel, payload).then(() => sync());
    sync();
    close(true);
  }

  button.addEventListener('click', () => {
    if (list.hidden) open();
    else close(false);
  });

  button.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      open();
    }
  });

  list.addEventListener('click', (e) => {
    const opt = e.target.closest('[role="option"]');
    if (opt) choose(opt.dataset.id);
  });

  list.addEventListener('keydown', (e) => {
    const opts = options();
    const idx = opts.indexOf(document.activeElement);
    const go = (o) => {
      if (!o) return;
      o.focus();
      o.scrollIntoView({ block: 'nearest' });
    };
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      go(opts[idx + 1] || opts[0]);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      go(opts[idx - 1] || opts[opts.length - 1]);
    } else if (e.key === 'Home') {
      e.preventDefault();
      go(opts[0]);
    } else if (e.key === 'End') {
      e.preventDefault();
      go(opts[opts.length - 1]);
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      if (idx >= 0) choose(opts[idx].dataset.id);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      close(true);
    } else if (e.key === 'Tab') {
      close(false);
    } else if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
      // Type-ahead: the first model whose name starts with what was typed.
      const now = performance.now();
      typed = now - typedAt > 700 ? e.key.toLowerCase() : typed + e.key.toLowerCase();
      typedAt = now;
      const hit = opts.find((o) => o.querySelector('.opt-name').textContent.toLowerCase().startsWith(typed));
      if (hit) go(hit);
    }
  });

  document.addEventListener('click', (e) => {
    if (!list.hidden && !e.target.closest(`#${rootId}`)) close(false);
  });

  const api = {
    load,
    close,
    isOpen: () => !list.hidden,
    provider: () => state.provider,
    setCurrent: (id) => {
      state.current = id;
      sync();
    },
  };
  combos.push(api);
  return api;
}

function closeCombos(focusButton) {
  for (const c of combos) if (c.isOpen()) c.close(focusButton);
}

const modelCombo = makeCombo({
  rootId: 'model-combo',
  button: $('model-combo-btn'),
  label: $('model-combo-label'),
  detail: $('model-combo-detail'),
  list: $('model-combo-list'),
  note: $('model-note'),
  kind: 'vision',
  summaryId: 'model-summary',
  saveChannel: 'settings:save-model',
});

const brainCombo = makeCombo({
  rootId: 'brain-combo',
  button: $('brain-combo-btn'),
  label: $('brain-combo-label'),
  detail: $('brain-combo-detail'),
  list: $('brain-combo-list'),
  note: brainNoteEl,
  kind: 'text',
  saveChannel: 'settings:save-brain',
  onSync: () => {
    if (brainEnabled.checked) setSummary('brain-summary', brainName());
  },
});

const findCombo = makeCombo({
  rootId: 'find-combo',
  button: $('find-combo-btn'),
  label: $('find-combo-label'),
  detail: $('find-combo-detail'),
  list: $('find-combo-list'),
  note: $('find-model-note'),
  kind: 'find',
  summaryId: 'find-model-summary',
  saveChannel: 'settings:save-find-model',
});

const agentCombo = makeCombo({
  rootId: 'agent-combo',
  button: $('agent-combo-btn'),
  label: $('agent-combo-label'),
  detail: $('agent-combo-detail'),
  list: $('agent-combo-list'),
  note: $('agent-model-note'),
  kind: 'agent',
  summaryId: 'agent-model-summary',
  saveChannel: 'settings:save-agent-model',
});

function reloadCombos() {
  const p = models.provider;
  if (p === 'openrouter') {
    modelCombo.setCurrent(data.model || '');
    brainCombo.setCurrent(data.brain || '');
    findCombo.setCurrent(data.findModel || '');
    agentCombo.setCurrent(data.agentModel || '');
  }
  return Promise.all([modelCombo.load(p), brainCombo.load(p), findCombo.load(p), agentCombo.load(p)]);
}

function showBrainPicker(on) {
  brainComboEl.hidden = !on;
  brainNoteEl.hidden = !on;
  setSummary('brain-summary', on ? brainName() : 'Off');
}

function brainName() {
  const name = $('brain-combo-label').textContent.trim();
  return name && name !== 'Loading…' ? name : 'On';
}

showPaid.addEventListener('change', () => {
  call('settings:set-show-paid', { on: showPaid.checked }).then(reloadCombos);
});

brainEnabled.addEventListener('change', () => {
  call('settings:set-brain-enabled', { on: brainEnabled.checked }).then((r) => {
    const on = !failed(r) && r ? !!r.brainEnabled : brainEnabled.checked;
    data.brainEnabled = on;
    showBrainPicker(on);
  });
});

// ----------------------------------------------------------------- skills

const skillList = $('skill-list');
const skillsStatus = makeStatus($('skills-status'));
const AUTO = { id: 'auto', name: 'Automatic', summary: 'Picks from what is on screen and what you asked. Recommended.' };

function renderSkills(list, current) {
  skillList.textContent = '';
  for (const s of [AUTO].concat(list)) {
    const btn = el('button', { type: 'button', class: 'choice-option skill-option', role: 'radio', 'aria-checked': String(s.id === current), 'data-id': s.id });
    btn.appendChild(el('span', { class: 'skill-name' }, s.name));
    const lines = [s.summary, s.truncated ? 'Only the first 8 KB is used.' : ''].filter(Boolean);
    if (lines.length) btn.appendChild(el('span', { class: 'skill-summary' }, lines.join(' ')));
    skillList.appendChild(btn);
  }
  syncRoving(skillList);
  const active = [AUTO].concat(list).find((s) => s.id === current) || AUTO;
  setSummary('skill-summary', active.name);
}

skillList.addEventListener('click', (e) => {
  const btn = e.target.closest('.skill-option');
  if (!btn) return;
  call('settings:save-skill', { skill: btn.dataset.id }).then((r) => {
    if (failed(r) || !r) return;
    checkRadios('.skill-option', (o) => o.dataset.id === r.skill);
    const on = skillList.querySelector(`[data-id="${CSS.escape(r.skill)}"] .skill-name`);
    if (on) setSummary('skill-summary', on.textContent);
  });
});

$('open-skills').addEventListener('click', () => {
  call('skills:open-folder').then((r) => {
    if (failed(r) || (r && r.ok === false)) skillsStatus("Couldn't open the styles folder.");
  });
});

// ------------------------------------------------------------- find, agent

const findOcr = $('find-ocr');
const agentSteps = $('agent-steps');
const agentStepsValue = $('agent-steps-value');
const agentShots = $('agent-screenshots');
const agentCursor = $('agent-cursor');
const agentStatus = makeStatus($('agent-status'));

function renderFindSettings(f) {
  data.find = f;
  checkRadios('[data-find-scope]', (b) => b.dataset.findScope === f.scope);
  checkRadios('[data-find-related]', (b) => b.dataset.findRelated === f.related);
  findOcr.checked = !!f.ocr;
  const rel = { ask: ' · related on request', auto: ' · related automatic', off: '' }[f.related] || '';
  setSummary('find-summary', `${f.scope === 'screen' ? 'Whole screen' : 'This window'}${rel}`);
}

function renderAgentSettings(a) {
  data.agent = a;
  checkRadios('[data-permission]', (b) => b.dataset.permission === a.permission);
  agentSteps.value = String(a.maxSteps);
  agentStepsValue.textContent = String(a.maxSteps);
  agentSteps.style.setProperty('--k', String((a.maxSteps - 10) / 40));
  agentShots.checked = !!a.screenshots;
  agentCursor.checked = !!a.returnCursor;
  $('agent-revoke').hidden = !a.consent;
  setSummary('agent-summary', `${a.permission === 'every' ? 'Asks before every action' : 'Asks for risky steps'} · up to ${a.maxSteps} steps`);
}

const setFind = (patch) =>
  call('settings:set-find', patch).then((r) => {
    if (!failed(r) && r && r.find) renderFindSettings(r.find);
  });
const setAgent = (patch) =>
  call('settings:set-agent', patch).then((r) => {
    if (!failed(r) && r && r.agent) renderAgentSettings(r.agent);
  });

document.addEventListener('click', (e) => {
  const scope = e.target.closest('[data-find-scope]');
  if (scope) setFind({ scope: scope.dataset.findScope });
  const rel = e.target.closest('[data-find-related]');
  if (rel) setFind({ related: rel.dataset.findRelated });
  const perm = e.target.closest('[data-permission]');
  if (perm) setAgent({ permission: perm.dataset.permission });
});
findOcr.addEventListener('change', () => setFind({ ocr: findOcr.checked }));
agentSteps.addEventListener('input', () => {
  agentStepsValue.textContent = agentSteps.value;
  agentSteps.style.setProperty('--k', String((Number(agentSteps.value) - 10) / 40));
});
agentSteps.addEventListener('change', () => setAgent({ maxSteps: Number(agentSteps.value) }));
agentShots.addEventListener('change', () => setAgent({ screenshots: agentShots.checked }));
agentCursor.addEventListener('change', () => setAgent({ returnCursor: agentCursor.checked }));
$('agent-revoke').addEventListener('click', () => {
  setAgent({ consent: false }).then(() => agentStatus('A task will ask again before it next starts.'));
});

// ---------------------------------------------------------------- privacy

// Who receives the screen, from the ready providers in the order they are
// tried (Rust's recipients, else this page's own list).
function renderRecipients() {
  const rec = Array.isArray(data.recipients) ? data.recipients : prov.list.filter(answers);
  const names = rec.map((r) => (PROV_INFO[r.id] ? PROV_INFO[r.id].name : r.name)).filter(Boolean);
  let first;
  if (names.length === 1) first = `Asking sends your question and a screenshot, or the window's text, to ${names[0]}.`;
  else if (names.length) first = `Asking sends your question and a screenshot, or the window's text, to the first ready provider: ${names.join(', then ')}.`;
  else first = "Nothing is sent until a provider is ready. Then asking sends your question and a screenshot, or the window's text, to it.";
  $('recipients-text').textContent = `${first} Tasks send one each step. Free models are offered on the condition that their providers may keep and train on what they receive. Paid models don't. Exact Find never leaves this computer; related Find sends the window's text to the Find model.`;
}

const saveHistory = $('save-history');
const historyStatus = makeStatus($('history-status'));
const historyConfirm = $('history-confirm');
const historyActions = $('history-actions');

function renderHistory() {
  const n = data.historyCount || 0;
  setSummary('history-summary', data.saveHistory ? (n ? `${n} saved` : 'On') : 'Off');
  $('clear-history').hidden = !n;
}

saveHistory.addEventListener('change', () => {
  call('settings:set-save-history', { on: saveHistory.checked }).then((r) => {
    if (failed(r) || !r) return;
    data.saveHistory = r.saveHistory;
    data.historyCount = r.historyCount;
    renderHistory();
    historyStatus(r.saveHistory ? 'New conversations will be kept.' : 'Nothing new will be saved.');
  });
});

$('open-history').addEventListener('click', () => call('overlay:open-history'));

function closeHistoryConfirm(focusDelete) {
  historyConfirm.hidden = true;
  historyActions.hidden = false;
  if (focusDelete) $('clear-history').focus();
}

// Delete all asks inline, with the number, and Keep has focus.
$('clear-history').addEventListener('click', () => {
  const n = data.historyCount || 0;
  $('history-confirm-text').textContent = n === 1 ? 'Delete the 1 conversation?' : `Delete all ${n} conversations?`;
  historyActions.hidden = true;
  historyConfirm.hidden = false;
  $('history-confirm-no').focus();
});

$('history-confirm-no').addEventListener('click', () => closeHistoryConfirm(true));

$('history-confirm-yes').addEventListener('click', () => {
  const n = data.historyCount || 0;
  call('settings:clear-history').then((r) => {
    closeHistoryConfirm(false);
    if (failed(r) || !r) {
      historyStatus("Couldn't delete the history.", true);
      $('clear-history').focus();
      return;
    }
    data.historyCount = r.historyCount;
    renderHistory();
    historyStatus(n === 1 ? 'Deleted 1 conversation.' : `Deleted ${n} conversations.`);
    $('open-history').focus();
  });
});

// -------------------------------------------------------------- shortcuts

const shortcutList = $('shortcut-list');
const shortcutError = $('shortcut-error');
const shortcutFree = $('shortcut-free');
const setShortcutError = makeStatus(shortcutError);

function renderShortcuts() {
  setSummary('shortcut-summary', [data.shortcuts.captureQuery, data.shortcuts.findOnScreen, data.shortcuts.agentTask].filter(Boolean).join(' · '));
  for (const row of shortcutList.querySelectorAll('.sc-row[data-action]')) {
    const action = row.dataset.action;
    const kbd = row.querySelector('.sc-current');
    const warn = row.querySelector('.sc-warn');
    const button = row.querySelector('.sc-change');
    const isCapturing = capturing && capturing.action === action;
    kbd.textContent = isCapturing ? 'Press keys' : data.shortcuts[action] || 'Not set';
    kbd.classList.toggle('empty', !isCapturing && !data.shortcuts[action]);
    const taken = data.active[action] === false;
    warn.hidden = !taken;
    warn.textContent = taken ? 'Another app already uses this shortcut. Choose another.' : '';
    row.classList.toggle('capturing', !!isCapturing);
    button.textContent = isCapturing ? 'Cancel' : 'Change';
  }
}

function showFree(action, free) {
  shortcutFree.textContent = '';
  const list = Array.isArray(free) ? free.filter(Boolean) : [];
  shortcutFree.hidden = !list.length;
  if (!list.length) return;
  shortcutFree.appendChild(el('span', { class: 'note' }, 'Free:'));
  for (const chord of list) {
    shortcutFree.appendChild(el('button', { type: 'button', class: 'g-btn', 'data-free': chord, 'data-action': action, 'aria-label': `Use ${chord}` }, chord));
  }
}

function startCapture(action, button) {
  if (capturing) endCapture();
  capturing = { action };
  setShortcutError('');
  showFree(action, []);
  renderShortcuts();
  call('shortcuts:capture-mode', { on: true });
  button.focus();
  say('Press the new shortcut, or Esc to cancel.');
}

function endCapture() {
  capturing = null;
  renderShortcuts();
  call('shortcuts:capture-mode', { on: false });
}

function applyShortcut(action, accelerator) {
  return call('shortcuts:set', { action, accelerator }).then((r) => {
    if (failed(r) || !r) return false;
    if (r.ok) {
      data.shortcuts = r.shortcuts;
      data.active = r.active;
      setShortcutError('');
      showFree(action, []);
      if (capturing) endCapture();
      else renderShortcuts();
      return true;
    }
    setShortcutError(r.error, true);
    showFree(action, r.free);
    return false;
  });
}

shortcutList.addEventListener('click', (e) => {
  const button = e.target.closest('.sc-change');
  if (!button) return;
  const action = button.closest('.sc-row').dataset.action;
  // The same button cancels, so a recording started by accident is not a
  // trap that only Escape gets out of.
  if (capturing && capturing.action === action) {
    endCapture();
    return;
  }
  startCapture(action, button);
});

shortcutFree.addEventListener('click', (e) => {
  const b = e.target.closest('[data-free]');
  if (!b) return;
  applyShortcut(b.dataset.action, b.dataset.free).then((ok) => {
    if (ok) {
      const change = shortcutList.querySelector(`.sc-row[data-action="${b.dataset.action}"] .sc-change`);
      if (change) change.focus();
      say(`Shortcut set to ${b.dataset.free}.`);
    }
  });
});

const KEY_ALIASES = {
  ' ': 'Space',
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  '+': 'Plus',
  Enter: 'Enter',
  Tab: 'Tab',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  Insert: 'Insert',
};

function normalizeKey(e) {
  const k = e.key;
  if (['Control', 'Alt', 'AltGraph', 'Shift', 'Meta'].includes(k)) return null;
  if (KEY_ALIASES[k]) return KEY_ALIASES[k];
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(k)) return k;
  if (k.length === 1) return k.toUpperCase();
  return null;
}

window.addEventListener(
  'keydown',
  (e) => {
    if (!capturing) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === 'Escape') {
      setShortcutError('');
      endCapture();
      return;
    }
    const key = normalizeKey(e);
    if (!key) return; // a bare modifier press; keep listening
    if (!e.ctrlKey && !e.altKey && !e.metaKey) {
      setShortcutError('Add Ctrl, Alt or Win to the shortcut.', true);
      return;
    }
    const mods = [];
    if (e.ctrlKey) mods.push('Ctrl');
    if (e.altKey) mods.push('Alt');
    if (e.metaKey) mods.push('Super');
    if (e.shiftKey) mods.push('Shift');
    // A failed set stays in capture mode, so another combo can be tried at
    // once or a free one picked below.
    applyShortcut(capturing.action, [...mods, key].join('+'));
  },
  true
);

$('reset-shortcuts').addEventListener('click', () => {
  call('shortcuts:reset').then((r) => {
    if (failed(r) || !r) return;
    data.shortcuts = r.shortcuts;
    data.active = r.active;
    setShortcutError('');
    showFree('', []);
    renderShortcuts();
  });
});

// ---------------------------------------------------------------- general

// Text size: the person's own factor on top of Windows' text size.
const textScale = $('text-scale');
const textScaleValue = $('text-scale-value');
let textSaveTimer = null;

function renderTextScale() {
  const pct = Math.round((Number(data.textScale) || 1) * 100);
  textScale.value = String(pct);
  textScale.setAttribute('aria-valuetext', `${pct} percent`);
  textScale.style.setProperty('--k', String((pct - 100) / 100));
  textScaleValue.textContent = `${pct}%`;
  setSummary('text-size-summary', `${pct}%`);
  const os = Number(data.osTextScale) || 1;
  $('text-size-note').textContent =
    Math.abs(os - 1) > 0.001 ? `Starts from your Windows text size. Windows is set to ${Math.round(os * 100)}%.` : 'Starts from your Windows text size.';
}

textScale.addEventListener('input', () => {
  data.textScale = Number(textScale.value) / 100;
  renderTextScale();
  clearTimeout(textSaveTimer);
  textSaveTimer = setTimeout(() => call('settings:set-text-scale', { scale: data.textScale }), 150);
});

listen('ui:text-scale', (s) => {
  if (s && typeof s.textScale === 'number') {
    data.textScale = s.textScale;
    renderTextScale();
    return;
  }
  // Ctrl+= or Ctrl+- in some window: read the stored factor back.
  call('settings:get').then((d) => {
    if (failed(d) || !d || typeof d.textScale !== 'number') return;
    data.textScale = d.textScale;
    data.osTextScale = d.osTextScale;
    renderTextScale();
  });
});

const launchAtLogin = $('launch-at-login');

function renderStartup() {
  setSummary('startup-summary', data.launchAtLogin ? 'On' : 'Off');
  const note = $('startup-note');
  // Unpackaged, the Run key would point at a development binary rather than
  // at AevalBuddy, so the preference is remembered and not applied. Saying
  // so beats a switch that silently does nothing.
  note.textContent = data.launchAtLoginLive ? '' : 'Remembered, but only applied in an installed build.';
  note.hidden = !note.textContent;
}

launchAtLogin.addEventListener('change', () => {
  call('settings:set-launch-at-login', { on: launchAtLogin.checked }).then((r) => {
    if (failed(r) || !r) return;
    data.launchAtLogin = r.launchAtLogin;
    renderStartup();
  });
});

// Liquid glass. Stop labels sit on their stops, the track has a tick at each,
// and a drag that ends near a stop lands on it. Arrow keys go stop to stop.
const SURFACE_WHY = {
  'high-contrast': 'A contrast theme is on, so surfaces are solid.',
  'capture-visible': 'This Windows build cannot hide AevalBuddy from capture, so glass, which needs that, is off.',
  'capture-failed': 'The screen could not be read for the glass, so surfaces are solid for now.',
};
const TRANSPARENCY_NOTE = 'Windows transparency effects are off. AevalBuddy keeps its glass; choose None for solid panels.';
const GLASS_NAMES = ['None', 'Subtle', 'Liquid', 'Very', 'Very very'];
const glassRange = $('glass-range');
const clamp01 = (k) => Math.min(1, Math.max(0, k));
const nearestStop = (k) => Math.round(clamp01(k) * 4) / 4;
const glassName = (k) => GLASS_NAMES[Math.round(clamp01(k) * 4)];
const snapGlass = (k) => (Math.abs(k - nearestStop(k)) <= 0.04 ? nearestStop(k) : k);

function currentGlass() {
  if (typeof data.glassIntensity === 'number') return data.glassIntensity;
  // A profile from before the slider: Solid meant None.
  if (data.surface === 'solid') return 0;
  return window.AevalGlass ? window.AevalGlass.intensity() : 0.5;
}

function renderSurface() {
  const k = currentGlass();
  const g = data.glass || { tier: 'live', reasons: [] };
  const reasons = g.reasons || [];
  const why = reasons.map((r) => SURFACE_WHY[r]).filter(Boolean);
  const name = glassName(k);
  setSummary('surface-summary', k > 0 && why.length ? `${name}, solid for now` : name);
  glassRange.value = String(k);
  glassRange.setAttribute('aria-valuetext', name);
  glassRange.style.setProperty('--k', String(k));
  const on = nearestStop(k);
  for (const s of document.querySelectorAll('.glass-stops span')) {
    s.classList.toggle('on', Math.abs(Number(s.dataset.k) - on) < 0.001);
  }
  const notes = k > 0 ? why.slice() : [];
  if (k > 0 && reasons.includes('transparency-off')) notes.push(TRANSPARENCY_NOTE);
  const note = $('surface-note');
  note.textContent = notes.join(' ');
  note.hidden = !note.textContent;
}

// Live while dragging, in this window and (through glass.js) every other;
// saved once the hand stops.
let glassSaveTimer = null;
function applyGlass(k, now) {
  data.glassIntensity = k;
  if (window.AevalGlass) window.AevalGlass.setIntensity(k);
  renderSurface();
  clearTimeout(glassSaveTimer);
  glassSaveTimer = setTimeout(
    () => {
      call('settings:set-glass', { glassIntensity: k }).then((r) => {
        if (!failed(r) && r && r.glass) data.glass = r.glass;
        renderSurface();
      });
    },
    now ? 0 : 200
  );
}

glassRange.addEventListener('input', () => {
  const raw = Number(glassRange.value);
  const k = snapGlass(raw);
  if (k !== raw) glassRange.value = String(k);
  applyGlass(k);
});

glassRange.addEventListener('change', () => {
  const k = snapGlass(Number(glassRange.value));
  glassRange.value = String(k);
  applyGlass(k, true);
});

glassRange.addEventListener('keydown', (e) => {
  const up = e.key === 'ArrowRight' || e.key === 'ArrowUp' || e.key === 'PageUp';
  const down = e.key === 'ArrowLeft' || e.key === 'ArrowDown' || e.key === 'PageDown';
  if (!up && !down) return;
  e.preventDefault();
  const k = Number(glassRange.value);
  const next = up ? Math.ceil(k * 4 + 0.001) / 4 : Math.floor(k * 4 - 0.001) / 4;
  applyGlass(clamp01(next), true);
});

document.querySelector('.glass-stops').addEventListener('click', (e) => {
  const s = e.target.closest('span[data-k]');
  if (!s) return;
  applyGlass(Number(s.dataset.k), true);
  glassRange.focus();
});

// Updates. A failed check never reads as an error here; it says when the
// next try is. The footer only speaks up when a restart is waiting.
const updateBtn = $('update-btn');
const aboutSummary = $('about-summary');
const updateLine = $('update-line');
const updateProgress = $('update-progress');
const updateNotes = $('update-notes');
const updateRestart = $('update-restart');
const updateCheck = $('update-check');
const updateRelease = $('update-release');

function megabytes(n) {
  return `${(n / 1048576).toFixed(n > 10485760 ? 0 : 1)} MB`;
}

function renderUpdate(u) {
  u = u || {};
  const v = u.current || data.version || '';
  const checked = u.lastChecked ? `Checked ${ago(u.lastChecked)}.` : '';
  let summary = v;
  let line = '';
  switch (u.status) {
    case 'dev':
      summary = `${v} · development build`;
      line = 'Updates are checked by the installed app, not by a development build.';
      break;
    case 'checking':
      summary = `${v} · checking`;
      line = 'Checking for a newer version…';
      break;
    case 'downloading': {
      const pct = Math.floor((u.progress || 0) * 100);
      summary = `${v} · downloading ${u.version}`;
      line = u.total ? `Downloading ${u.version}: ${megabytes(u.transferred)} of ${megabytes(u.total)}, ${pct}%.` : `Downloading ${u.version}…`;
      break;
    }
    case 'ready':
      summary = `${v} · ${u.version} ready`;
      line = u.restartPending ? `${u.version} installs as soon as the current answer finishes.` : `${u.version} is downloaded and verified. It installs when you quit, or now.`;
      break;
    case 'offline':
    case 'retrying':
      line = `${u.problem || ''} ${checked}`.trim();
      break;
    case 'current':
      summary = `${v} · up to date`;
      line = `You have the latest version. ${checked}`.trim();
      break;
    default:
      line = checked || 'Checks for updates in the background.';
  }
  if (u.updatedFrom && u.status !== 'ready' && u.status !== 'downloading') line = `Updated from ${u.updatedFrom}. ${line}`;
  aboutSummary.textContent = summary;
  updateLine.textContent = line;

  updateProgress.hidden = u.status !== 'downloading';
  updateProgress.firstElementChild.style.transform = `scaleX(${u.progress || 0})`;

  const showNotes = !!u.notes && (u.status === 'downloading' || u.status === 'ready');
  updateNotes.hidden = !showNotes;
  updateNotes.textContent = showNotes ? `New in ${u.version}: ${u.notes}` : '';

  updateRelease.hidden = !u.releaseUrl;
  updateRelease.dataset.external = u.releaseUrl || '';
  updateRelease.textContent = u.updatedFrom && u.status !== 'ready' ? "What's new" : 'Release notes';

  updateRestart.hidden = !(u.status === 'ready' && !u.restartPending);
  updateCheck.hidden = u.status === 'dev' || u.status === 'ready' || u.status === 'downloading';
  updateCheck.disabled = u.status === 'checking';

  updateBtn.hidden = !(u.status === 'ready' && !u.restartPending);
}

const restartToUpdate = () => call('update:restart');
updateRestart.addEventListener('click', restartToUpdate);
updateBtn.addEventListener('click', restartToUpdate);

updateCheck.addEventListener('click', () => {
  renderUpdate({ ...(data.update || {}), status: 'checking' });
  call('update:check').then((r) => {
    if (!failed(r) && r && r.status) {
      data.update = r;
      renderUpdate(r);
    }
  });
});

listen('update:state', (u) => {
  data.update = u;
  renderUpdate(u);
});

// "Checked 3 min ago" has to keep being true while the window stays open,
// and so does a provider's "Checked just now".
setInterval(() => {
  renderUpdate(data.update);
  if (document.querySelector('.prov[data-open="1"]') && !document.activeElement.closest('.prov-field')) renderProviders();
}, 30000);

// Bar position: where the bar sits, and Move… to change it.
const H_NAME = { left: 'left', center: 'centre', right: 'right' };
function renderBarPosition() {
  const pos = data.overlayPos || data.inputAnchor;
  let text = '';
  if (pos && pos.v && pos.h) {
    const v = pos.v === 'bottom' ? 'Bottom' : 'Top';
    text = `${v} ${H_NAME[pos.h] || pos.h}`;
  }
  setSummary('layout-summary', text);
}

$('edit-position').addEventListener('click', () => call('hud:enter'));
$('replay-tour').addEventListener('click', () => call('tour:replay'));

// -------------------------------------------------------------------- rail

// The rail scrolls the column to a page, and the page in view lights up its
// name. One column underneath, so narrow windows lose nothing but the rail.
const mainEl = document.querySelector('main');
const railButtons = [...document.querySelectorAll('#rail [data-page]')];

function markRail() {
  const top = mainEl.scrollTop + 24;
  let current = railButtons[0] && railButtons[0].dataset.page;
  for (const b of railButtons) {
    const page = $(b.dataset.page);
    if (page && page.offsetTop - mainEl.offsetTop <= top) current = b.dataset.page;
  }
  if (mainEl.scrollTop + mainEl.clientHeight >= mainEl.scrollHeight - 4 && railButtons.length) {
    current = railButtons[railButtons.length - 1].dataset.page;
  }
  for (const b of railButtons) b.setAttribute('aria-current', String(b.dataset.page === current));
}

function showPage(id, focus) {
  const page = $(id);
  if (!page) return;
  mainEl.scrollTo({ top: page.offsetTop - mainEl.offsetTop, behavior: reduceMotion.matches ? 'auto' : 'smooth' });
  if (focus) {
    const h1 = page.querySelector('h1');
    if (h1) h1.focus({ preventScroll: true });
  }
}

for (const b of railButtons) b.addEventListener('click', () => showPage(b.dataset.page));
mainEl.addEventListener('scroll', markRail, { passive: true });

// A section someone else asked for: a page, a row, a provider (`prov-<id>`,
// from an answer's Fix key), or the names older callers use.
function goToSection(id) {
  if (!id) return;
  if (id === 'providers' || id === 'pg-account' || id === 'sec-key') {
    const target = prov.list.find((p) => !p.hasKey) || prov.list[0];
    showPage('pg-providers');
    if (target && (id === 'sec-key' || !prov.list.some((p) => p.hasKey))) {
      // The first-run tour points at the key field.
      if (id === 'sec-key' && target.id === 'openrouter') setUi('openrouter', { mode: 'paste' });
      openSection(`prov-${target.id}`);
      prov.focusAfter = { id: target.id, to: 'head' };
      renderProviders();
    } else {
      const h1 = $('pg-providers').querySelector('h1');
      h1.focus({ preventScroll: true });
    }
    return;
  }
  const sec = $(id);
  if (!sec) return;
  if (sec.classList.contains('page')) {
    showPage(id, true);
    return;
  }
  if (sec.classList.contains('prov')) {
    const pid = sec.dataset.provider;
    const p = provById(pid);
    if (p && p.status === 'rejected') setUi(pid, { mode: 'replace' });
    prov.focusAfter = { id: pid, to: p && p.status === 'rejected' ? 'field' : 'head' };
  }
  openSection(id);
  const page = sec.closest('.page');
  if (page) showPage(page.id);
  if (sec.classList.contains('prov')) renderProviders();
  else {
    const head = headOf(sec);
    if (head) head.focus({ preventScroll: true });
  }
}

listen('settings:section', (s) => goToSection(s && s.id));

// ----------------------------------------------------------- window chrome

$('close-settings').addEventListener('click', () => call('settings:close'));
$('quit-app').addEventListener('click', () => call('app:quit'));

// Links out open in the real browser, validated by Rust to http/https.
document.addEventListener('click', (e) => {
  const a = e.target.closest('[data-external]');
  if (!a) return;
  e.preventDefault();
  if (a.dataset.external) call('open-external', { url: a.dataset.external });
});

window.addEventListener('keydown', (e) => {
  if (capturing) return;
  if (e.key === 'F1') {
    e.preventDefault();
    showPage('pg-shortcuts', true);
    return;
  }
  if (e.key !== 'Escape') return;
  if (combos.some((c) => c.isOpen())) {
    closeCombos(true);
    return;
  }
  const removing = prov.list.find((p) => uiOf(p.id).removing);
  if (removing) {
    setUi(removing.id, { removing: false });
    prov.focusAfter = { id: removing.id, to: 'head' };
    renderProviders();
    return;
  }
  if (!historyConfirm.hidden) {
    closeHistoryConfirm(true);
    return;
  }
  const open = document.querySelector('main .row[data-open="1"]');
  if (open) {
    const head = headOf(open);
    closeSections();
    if (head) head.focus({ preventScroll: true });
    return;
  }
  call('settings:close');
});

// ------------------------------------------------------------ cursor buddy

listen('buddy:watch', ({ selector }) => {
  watchSelector = selector;
});

listen('buddy:resolve', ({ selector }) => {
  const node = document.querySelector(selector);
  if (!node || node.hidden || node.closest('[hidden]')) {
    call('buddy:resolve-result', { found: false });
    return;
  }
  const r = node.getBoundingClientRect();
  call('buddy:resolve-result', {
    found: true,
    cx: window.screenX + r.left + r.width / 2,
    cy: window.screenY + r.top + r.height / 2,
    w: r.width,
    h: r.height,
  });
});

document.addEventListener('click', (e) => {
  if (!watchSelector) return;
  if (!e.target.closest(watchSelector)) call('buddy:dismiss');
});

// ------------------------------------------------------------------ load

function renderAll(d) {
  $('app-version').textContent = d.version || '';
  renderUpdate(d.update);
  // Below Windows 10 build 19041 the call that hides these windows from a
  // screen share does nothing. Saying so beats letting someone believe it.
  setSummary('share-summary', d.canHide !== false ? 'Hidden' : 'Visible');
  $('share-line').textContent =
    d.canHide !== false
      ? `Every AevalBuddy window, the highlights and the pointer included, is left out of screen shares and recordings (Windows build ${d.winBuild || 'unknown'}).`
      : `Windows build ${d.winBuild} cannot hide windows from a screen share; that needs build 19041 or later. People watching your screen can see AevalBuddy.`;
  renderFindSettings(d.find || { scope: 'window', related: 'ask', ocr: true });
  renderAgentSettings(d.agent || { permission: 'risky', maxSteps: 25, screenshots: true, returnCursor: true });
  showPaid.checked = !!d.showPaid;
  brainEnabled.checked = !!d.brainEnabled;
  saveHistory.checked = d.saveHistory !== false;
  launchAtLogin.checked = !!d.launchAtLogin;
  renderHistory();
  renderStartup();
  renderSurface();
  renderTextScale();
  renderBarPosition();
  renderKeyStorage();
  renderSkills(d.skills || [], d.skill);
  renderShortcuts();
}

// Rust's view after a change: statuses, hints, order and recipients, without
// undoing what a row is in the middle of.
function refreshFromRust() {
  return call('settings:get').then((d) => {
    if (failed(d) || !d) return;
    const fresh = providersFrom(d);
    for (const p of fresh) {
      const old = provById(p.id);
      if (old && uiOf(p.id).testing) p.status = old.status;
      if (old && !p.checkedAt && old.checkedAt) p.checkedAt = old.checkedAt;
      if (old && p.status === 'saved' && old.hasKey && old.status !== 'saved') p.status = old.status;
    }
    data = Object.assign(data, {
      providers: d.providers,
      keys: d.keys,
      keyHints: d.keyHints,
      keyStorage: d.keyStorage,
      failover: d.failover,
      recipients: d.recipients,
      hasKey: d.hasKey,
    });
    prov.list = fresh;
    renderKeyStorage();
    renderProviders();
  });
}

call('settings:get').then((d) => {
  if (failed(d) || !d) return;
  data = Object.assign({ shortcuts: {}, active: {} }, d);
  if (typeof d.glassIntensity === 'number' && window.AevalGlass) window.AevalGlass.setIntensity(d.glassIntensity);
  prov.list = providersFrom(d);
  renderAll(d);
  renderProviders();

  const stored = storedMode();
  models.mode = picksAreDefaults(d) ? (stored === 'custom' ? 'custom' : 'auto') : 'custom';
  renderModelsMode();
  showBrainPicker(!!d.brainEnabled);
  reloadCombos().then(() => showBrainPicker(!!d.brainEnabled));
  markRail();

  // With no provider yet, the OpenRouter row is open: Connect is the first
  // thing on the page.
  if (!prov.list.some((p) => p.hasKey)) {
    const first = prov.list.find((p) => p.id === 'openrouter') || prov.list[0];
    if (first) openSection(`prov-${first.id}`);
  }
  checkClipboardKey();
});
