// Shared fixtures and helpers for the ui-shots lane files
// (tools/ui-shots/lanes/<lane>.mjs). Owned by the harness lane: a lane that
// needs another fixture defines it in its own file, or asks harness to add it
// here. Nothing in this file may import playwright: bridge.test.mjs imports
// the lane files under plain node.

// The desktop every scene is laid on, in CSS pixels.
export const W = 1600;
export const H = 1000;

// lib/backdrop-fixture.html kinds a scene can be laid on.
export const BACKDROPS = ['wallpaper', 'white', 'black', 'dark-editor', 'photo-busy'];

// The bar window: a 640 px panel in a window 40 wider and taller.
export const BAR = { x: 480, y: 24, w: 680, h: 240 };
// The same window at the shipped default width (D-16: 600 px panel).
export const BAR_DEFAULT = { x: 500, y: 24, w: 640, h: 240 };
// One output window under the bar.
export const OUT1 = { x: 560, y: 92, w: 520, h: 520 };

const now = Date.now();

// What Rust answers every page unless a lane's `mocks` or a window's
// `responses` say otherwise.
export const COMMON = {
  'glass:hello': null,
  // Rust says whether the text reached the clipboard; pages say "Copied"
  // only then.
  'clipboard:write': { ok: true },
  'bar:history': [],
  'bar:history-push': [],
  'settings:get': {
    hasKey: true,
    model: 'google/gemma-3-27b-it:free',
    defaultModel: 'google/gemma-3-27b-it:free',
    brain: 'deepseek/deepseek-chat-v3-0324:free',
    brainEnabled: false,
    defaultBrain: 'deepseek/deepseek-chat-v3-0324:free',
    findModel: 'google/gemma-3-27b-it:free',
    defaultFindModel: 'google/gemma-3-27b-it:free',
    agentModel: 'google/gemma-3-27b-it:free',
    defaultAgentModel: 'google/gemma-3-27b-it:free',
    find: { scope: 'window', related: 'ask', ocr: true },
    agent: { permission: 'risky', maxSteps: 25, screenshots: true, returnCursor: true, consent: true },
    winBuild: 26100,
    showPaid: false,
    skills: [],
    skill: 'auto',
    shortcuts: { captureQuery: 'Ctrl+Alt+Space', captureRegion: '', findOnScreen: '', agentTask: '', toggleVisibility: '' },
    active: { captureQuery: true, captureRegion: null, findOnScreen: null, agentTask: null, toggleVisibility: null },
    defaults: { captureQuery: 'Ctrl+Alt+Space', captureRegion: '', findOnScreen: '', agentTask: '', toggleVisibility: '' },
    version: '2.0.0',
    update: { state: 'idle', version: '2.0.0', checkedAt: now - 3600e3 },
    canHide: true,
    saveHistory: true,
    historyCount: 4,
    launchAtLogin: true,
    launchAtLoginLive: true,
    surface: 'auto',
    glass: { tier: 'live', reasons: [] },
  },
  'models:list': {
    live: true,
    current: 'google/gemma-3-27b-it:free',
    groups: [
      {
        label: 'Free',
        models: [
          { id: 'google/gemma-3-27b-it:free', name: 'Gemma 3 27B', free: true },
          { id: 'mistralai/mistral-small-3.1-24b-instruct:free', name: 'Mistral Small 3.1', free: true },
        ],
      },
    ],
  },
  'history:list': {
    rows: [
      { id: 'a', title: 'What does this error in the build log mean?', reply: 'The linker cannot find vcruntime140.dll because the redistributable is missing…', asked: 3, at: now - 20 * 60e3 },
      { id: 'b', title: 'Summarise the quarterly numbers on this slide', reply: 'Revenue grew 12% quarter on quarter, driven by the services line…', asked: 1, at: now - 3 * 3600e3 },
      { id: 'c', title: 'Rename these files by date', reply: 'Renamed 14 files in Downloads to YYYY-MM-DD names.', asked: 2, at: now - 30 * 3600e3 },
      { id: 'd', title: 'Which of these flights is cheapest?', reply: 'The 07:45 KLM flight at €212 is the cheapest direct option.', asked: 1, at: now - 4 * 86400e3 },
    ],
  },
};

export const ANSWER = [
  'The build fails because **vcruntime140.dll** is not on this machine. The linker log shows it three times, each after `LINK : fatal error LNK1104`.',
  '',
  '## Fix it',
  '1. Install the *Microsoft Visual C++ 2015-2022 Redistributable (x64)*.',
  '2. Restart the terminal so `PATH` is read again.',
  '3. Run the build again:',
  '',
  '```',
  'cargo build --release',
  '```',
  '',
  'If it still fails, the project may target x86; check `rust-toolchain.toml`.',
].join('\n');

export const barState = (extra) => ({
  mode: 'panel',
  hasKey: true,
  captureOk: true,
  inputAnchor: { h: 'center', v: 'top' },
  inputWidth: 640,
  pillState: 'idle',
  agent: { running: false },
  ...extra,
});

export const barContext = {
  screen: 'Build log LINK fatal error LNK1104 cannot open file vcruntime140.dll',
  title: 'Developer PowerShell',
  process: 'WindowsTerminal',
  chars: 2400,
};

// The helpers below take a Playwright Frame (t.frame(name)) or Page (t.page).

export async function openBar(f, extra) {
  await f.evaluate(
    ({ st, cx }) => {
      window.__emit('overlay:state', st);
      window.__emit('bar:context', cx);
    },
    { st: barState(extra), cx: barContext }
  );
}

// The bar's glass rect in screen coordinates: what Rust answers for
// output:bar.
export async function barRect(f) {
  return f.evaluate(() => {
    const r = document.getElementById('bar').getBoundingClientRect();
    return { x: window.screenX + r.left, y: window.screenY + r.top, width: r.width, height: r.height };
  });
}

export async function streamAnswer(f, question, answer) {
  await f.evaluate(
    ({ q, a }) => {
      window.__emit('overlay:state', { mode: 'panel', chatMax: 640 });
      window.__emit('chat:user', { gen: 1, text: q });
      window.__emit('query:start', { gen: 1 });
      window.__emit('query:delta', { gen: 1, text: a });
      window.__emit('query:done', { gen: 1, usage: { model: 'google/gemma-3-27b-it:free', ttftMs: 900, ms: 3200, prompt: 1100, completion: 240, cost: 0 }, question: q });
    },
    { q: question, a: answer }
  );
}

// Bottom edge of a window's frame as the mock last sized it, so a clip holds
// the whole window after output:resize grew it.
export const bottomOf = (page, name) =>
  page.evaluate((n) => {
    const f = document.querySelector(`iframe[name="${n}"]`);
    const r = f.getBoundingClientRect();
    return Math.min(Math.ceil(r.bottom) + 20, innerHeight);
  }, name);

// Plain objects merge key by key; anything else replaces.
export function deepMerge(base, over) {
  const plain = (v) => v && typeof v === 'object' && !Array.isArray(v);
  if (!plain(base) || !plain(over)) return over === undefined ? base : over;
  const out = { ...base };
  for (const [k, v] of Object.entries(over)) out[k] = deepMerge(base[k], v);
  return out;
}
