// The agents lane's ui-shots scenes, mocks, checks and CSS lint list: the
// card that grows out of the bar when a coding agent finishes or needs you,
// and Settings' Agents page. The shape is documented at the top of
// tools/ui-shots/shots.mjs.

const PREFS = {
  enabled: true,
  agentsOff: ['cursor'],
  chats: false,
  keepAwake: true,
  displayOn: false,
  lidAwake: false,
  lidSaved: '',
  notifyDone: true,
  notifyBackgroundDone: false,
  notifyNeedsYou: true,
  notifyBackgroundNeedsYou: true,
  sound: true,
  expandOnHover: true,
  altTab: true,
  cardTime: 1,
  showChildren: true,
  offered: true,
};

const SNAPSHOT = {
  prefs: PREFS,
  agents: [
    { id: 'claude', name: 'Claude Code', found: true, on: true, connected: true, note: 'Restart open sessions once.', error: null },
    { id: 'codex', name: 'Codex', found: true, on: true, connected: false, note: 'Run /hooks once in Codex to trust the new hooks.', error: 'hooks.json is not a JSON object' },
    { id: 'gemini', name: 'Gemini CLI', found: false, on: true, connected: false, note: '', error: null },
    { id: 'cursor', name: 'Cursor', found: true, on: false, connected: false, note: '', error: null },
  ],
  aevalsistant: false,
  chatProblem: null,
  hasLid: true,
  working: 2,
  awake: true,
  screenOn: false,
};

export const mocks = {
  'agents:get': SNAPSHOT,
  'agents:set': SNAPSHOT,
  'agents:ready': { ok: true },
  'agents:size': { from: 'top' },
  'agents:hover': { ok: true },
  'agents:test': { ok: true },
};

export const cssLintEnforce = ['renderer/agents.css'];

export const checks = [
  // The card never takes the keyboard, so nothing in it may depend on focus.
  {
    name: 'watchCardNeedsNoFocus',
    run: (ctx) => !/\.focus\(/.test(ctx.read('renderer/agents.js')) || 'agents.js focuses something',
  },
];

// Under the bar, centred on it: 452 wide (the 420 card and its shadow room).
const CARD = { x: 574, y: 92, w: 452, h: 260 };
const CLIP = { x: CARD.x - 20, y: CARD.y - 20, width: CARD.w + 40, height: CARD.h + 40 };
const S = { x: 120, y: 130, w: 960, h: 700 };

const ROWS = [
  { session: 'a', depth: 0, name: 'site', place: 'Codex · Terminal', state: 'needs-you', text: 'needs you', canJump: true },
  { session: 'b', depth: 0, name: 'aevalrena', place: 'VS Code', state: 'done', text: 'done 3m ago', canJump: true },
  { session: 'c', depth: 0, name: 'rover', place: 'Terminal', state: 'working', text: 'working 12m, 1 subagent', canJump: true },
  { session: 'c', subagent: 'x1', depth: 1, name: 'Explore', place: '', state: 'working', text: 'running 4m', canJump: false },
];

const DONE = {
  kind: 'done',
  title: 'aevalbuddy finished',
  place: 'VS Code',
  detail: 'The tour card is redesigned and the checks pass. Two things are left for you to decide before the release.',
  session: 's1',
  canJump: true,
  altTab: true,
  expand: true,
  rows: ROWS,
  more: 2,
  actions: [],
  from: 'top',
};

const facts = (f) =>
  f.evaluate(() => {
    const card = document.getElementById('card');
    const style = getComputedStyle(card);
    return {
      shown: !card.hidden && card.classList.contains('in'),
      kind: card.dataset.kind,
      title: document.getElementById('title').textContent,
      place: document.getElementById('place').textContent,
      keys: !document.getElementById('keys').hidden,
      rows: [...document.querySelectorAll('#rows .row')].map((r) => r.className),
      disabled: document.querySelectorAll('#rows .row-btn:disabled').length,
      more: document.getElementById('more').textContent,
      buttons: [...document.querySelectorAll('#actions .act')].map((b) => b.textContent),
      radius: style.borderTopLeftRadius,
      oneLine: document.getElementById('detail').getBoundingClientRect().height <= 20,
      wide: document.documentElement.scrollWidth - document.documentElement.clientWidth,
    };
  });

function cardScene(name, payload, expect) {
  return {
    name,
    backdrop: 'wallpaper',
    windows: [{ name: 'card', src: '/renderer/agents.html', ...CARD, z: 1, fixed: true }],
    async run(t) {
      const f = t.frame('card');
      t.check(`${name}.ready`, (await t.calls('card', 'agents:ready')).length === 1, 'agents.js never invoked agents:ready');
      await t.emit('card', 'agents:card', payload);
      await t.settle(900);
      const sized = (await t.calls('card', 'agents:size')).map((c) => c.payload.height);
      t.check(`${name}.sized`, sized.length >= 1 && sized[0] > 60, `agents:size heights ${JSON.stringify(sized)}`);
      const c = await facts(f);
      t.check(`${name}.drawn`, c.shown && c.kind === payload.kind && c.title === payload.title && c.radius === '28px' && c.wide === 0, JSON.stringify(c));
      t.check(`${name}.rows`, c.rows.length === expect.rows && c.disabled === expect.disabled && c.more === expect.more, JSON.stringify([c.rows, c.disabled, c.more]));
      t.check(`${name}.controls`, c.keys === expect.keys && c.buttons.join('|') === expect.buttons, JSON.stringify([c.keys, c.buttons]));
      await t.shot(name, CLIP);
      return c;
    },
  };
}

const done = cardScene('watch-card-done', DONE, { rows: 4, disabled: 1, more: '2 more', keys: true, buttons: '' });

export const scenes = [
  {
    ...done,
    async run(t) {
      const c = await done.run(t);
      t.check('watchDetailOneLineAtRest', c.oneLine, 'the detail shows more than one line before the pointer rests on it');
      const f = t.frame('card');
      // The pointer resting on the card holds the countdown and opens the
      // whole message.
      await t.page.mouse.move(CARD.x + 200, CARD.y + 40);
      await t.settle(500);
      const open = await f.evaluate(() => ({
        open: document.getElementById('card').classList.contains('open'),
        tall: document.getElementById('detail').getBoundingClientRect().height > 20,
      }));
      const hover = (await t.calls('card', 'agents:hover')).map((x) => x.payload.on);
      t.check('watchHoverOpensTheMessage', open.open && open.tall && hover[0] === true, JSON.stringify([open, hover]));
      await t.shot('watch-card-open', CLIP);
      // A row goes to that session's window; a subagent's row goes nowhere.
      await f.click('#rows .row.needs-you .row-btn');
      const opened = (await t.calls('card', 'agents:open')).map((x) => x.payload.session);
      t.check('watchRowOpensItsSession', opened.join() === 'a', JSON.stringify(opened));
      await t.page.mouse.move(0, 0);
      await t.emit('card', 'agents:hide', null);
      await t.settle(400);
      const out = await f.evaluate(() => document.getElementById('card').classList.contains('out'));
      t.check('watchCardFoldsAway', out, 'agents:hide did not fold the card');
    },
  },
  cardScene(
    'watch-card-needs-you',
    { ...DONE, kind: 'needs-you', title: 'site needs you', place: 'Codex · Terminal', detail: 'Codex wants to use shell', session: 'a', rows: ROWS.slice(1, 3), more: 0, altTab: false },
    { rows: 2, disabled: 0, more: '', keys: false, buttons: '' },
  ),
  cardScene(
    'watch-card-offer',
    {
      kind: 'info',
      title: 'Watch your coding agents?',
      place: '',
      detail: 'AevalBuddy can keep this PC awake while Claude Code, Codex and the others work, and tell you when one finishes or needs you.',
      session: null,
      canJump: false,
      altTab: false,
      expand: true,
      rows: [],
      more: 0,
      actions: [{ id: 'enable', label: 'Turn on' }, { id: 'later', label: 'Not now' }],
      from: 'top',
    },
    { rows: 0, disabled: 0, more: '', keys: false, buttons: 'Turn on|Not now' },
  ),

  // Settings, Agents: each agent found on this PC, keep-awake and the cards.
  {
    name: 'watch-settings',
    backdrop: 'wallpaper',
    textScale: true,
    windows: [{ name: 'settings', src: '/settings/settings.html', ...S, z: 1 }],
    async run(t) {
      const st = t.frame('settings');
      await t.settle(300);
      await st.evaluate(() => {
        showPage('pg-watch');
        openSection('sec-watch-agents');
      });
      await t.settle(600);
      await t.shot('watch-settings-agents', { x: S.x - 10, y: S.y - 10, width: S.w + 20, height: S.h + 20 });
      const page = await st.evaluate(() => ({
        enabled: document.getElementById('watch-enabled').checked,
        summary: document.getElementById('watch-agents-summary').textContent,
        agents: [...document.querySelectorAll('#watch-agent-list .switch')].map((l) => l.firstChild.firstChild.textContent),
        says: [...document.querySelectorAll('#watch-agent-list .watch-says')].map((n) => n.textContent),
        status: document.getElementById('watch-status').textContent,
        awake: document.getElementById('watch-awake-summary').textContent,
        cards: document.getElementById('watch-cards-summary').textContent,
        lid: !document.getElementById('watch-lid-row').hidden,
        time: document.querySelector('[data-watch-time][aria-checked="true"]')?.dataset.watchTime,
      }));
      t.check(
        'watchSettingsPage',
        page.enabled && page.summary === '1 of 3 connected' && page.agents.join() === 'Claude Code,Codex,Cursor' && page.says.length === 2 && /Could not connect Codex/.test(page.status) && page.awake === 'On · holding now' && page.cards === 'Finished and needs you' && page.lid && page.time === '1',
        JSON.stringify(page),
      );
      // Chat watching: off until asked for, saved by its switch, and when it
      // stopped by itself the reason shows under the switch.
      const chat = () =>
        st.evaluate(() => {
          const box = document.querySelector('input[data-watch="chats"]');
          const says = document.getElementById('watch-chats-status');
          return { label: box.closest('label').textContent.trim(), shown: !box.closest('label').hidden, on: box.checked, says: says.hidden ? '' : says.textContent };
        });
      const chatOff = await chat();
      await st.click('input[data-watch="chats"]');
      await t.settle(200);
      const chatSet = (await t.calls('settings', 'agents:set')).map((c) => c.payload);
      t.check(
        'watchChatsSwitch',
        chatOff.label === 'Also watch the Claude and ChatGPT apps' && chatOff.shown && !chatOff.on && chatOff.says === '' && chatSet.some((p) => p.chats === true),
        JSON.stringify([chatOff, chatSet]),
      );
      const stopped = 'Chat watching stopped: Windows UI Automation did not load.';
      await t.emit('settings', 'agents:state', { ...SNAPSHOT, prefs: { ...PREFS, chats: true }, chatProblem: stopped });
      await t.settle(200);
      const chatOn = await chat();
      t.check('watchChatsSaysWhyItStopped', chatOn.on && chatOn.says === stopped, JSON.stringify(chatOn));
      await t.shot('watch-settings-chats', { x: S.x - 10, y: S.y - 10, width: S.w + 20, height: S.h + 20 });
      // With agent watching off there is nothing for it to add to.
      await t.emit('settings', 'agents:state', { ...SNAPSHOT, prefs: { ...PREFS, enabled: false, chats: true }, chatProblem: stopped });
      await t.settle(200);
      const chatHidden = await chat();
      t.check('watchChatsNeedsWatching', !chatHidden.shown && chatHidden.says === '', JSON.stringify(chatHidden));
      await t.emit('settings', 'agents:state', SNAPSHOT);
      await t.settle(200);
      await st.evaluate(() => openSection('sec-watch-awake'));
      await t.settle(400);
      await st.click('input[data-watch="displayOn"]');
      await t.settle(200);
      const set = (await t.calls('settings', 'agents:set')).map((c) => c.payload);
      t.check('watchSettingsSaves', set.some((p) => p.displayOn === true), JSON.stringify(set));
      await t.shot('watch-settings-awake', { x: S.x - 10, y: S.y - 10, width: S.w + 20, height: S.h + 20 });
      // With the lid kept working, the note says what happens once the
      // agents stop and the lid is still shut.
      await t.emit('settings', 'agents:state', { ...SNAPSHOT, prefs: { ...PREFS, lidAwake: true } });
      await t.settle(200);
      const lid = await st.evaluate(() => ({
        note: document.getElementById('watch-awake-note').textContent,
        summary: document.getElementById('watch-awake-summary').textContent,
        on: document.querySelector('input[data-watch="lidAwake"]').checked,
      }));
      t.check('watchLidNoteSaysItSleeps', lid.on && /a minute after the last one stops, the PC sleeps or hibernates/.test(lid.note) && lid.summary === 'On, lid closed too · holding now', JSON.stringify(lid));
      await t.emit('settings', 'agents:state', SNAPSHOT);
      await t.settle(200);
    },
  },
];
