// What the person means by what they typed: Ask, Find, or Do.
//
// Pure and local: no Electron, no network, no model. The bar calls it on
// every pause in typing, so it has to finish in well under 5 ms; the smoke
// run times it and checks it against a table of phrases.
//
// The costs are not equal, and the rules follow from that. A wrong Ask or
// Find wastes a second and changes nothing on the computer. A wrong Do can
// click things. So ties go to Ask, and Do is only ever offered by a guess,
// never started by one: the bar shows an inferred Do as a card that Enter
// cannot confirm (see renderer/overlay.js). The executor's safety gate in
// lib/agent.js does not read anything here; the guess decides what the
// person is offered, never what the gate asks.
//
// The one signal a keyword list cannot have is the screen: "invoice" typed
// over an invoice is a search, the same word over a terminal is a question.
// The bar passes the window-in-front's own text (UI Automation, prefetched
// while the bar opens) as ctx.screen.
//
// Loaded two ways: require() in main and the smoke run, and a plain <script>
// in the bar (window.AevalIntent), hence the wrapper.

(function (root) {
'use strict';

const INTENTS = ['ask', 'find', 'do'];
const NAME = { ask: 'Ask', find: 'Find', do: 'Do' };

// Words that start a question.
const ASK_START =
  /^(what|what's|whats|why|how|who|whom|whose|when|which|is|are|was|were|does|did|can|could|should|would|will|has|have|explain|summari[sz]e|translate|tell me|meaning|define|describe|compare|any idea|i don'?t|i'?m|im|is there|are there|whats? up with)\b/i;
// "how do I save this" is a question about a verb, not an order to do it.
const HOW_TO = /^(how (do|can|should|would) (i|you|we)|how to|is it possible to|what'?s the (best )?way to|can i|should i)\b/i;
// Asking where something is, or for it to be shown, is Find.
const FIND_START =
  /^(find|where is|where's|wheres|where are|where did|locate|show me where|highlight|search for|search|look for|jump to|ctrl\+f)\b/i;
// The orders the agent can carry out. "copy" and "write" are left out on
// purpose: "copy this paragraph" and "write me a haiku" are mostly asks for
// text back, not for clicks.
const DO_VERBS =
  'open|launch|start|close|quit|click|double-click|right-click|press|type|rename|move|paste|save|send|reply|forward|delete|remove|erase|create|make a new|new folder|fill in|fill out|fill|download|upload|install|uninstall|scroll|select all|select|check|uncheck|tick|untick|enable|disable|turn on|turn off|switch to|switch|go to|navigate to|log in|login|sign in|sign out|log out|attach|book|schedule|sort|drag|minimi[sz]e|maximi[sz]e|resize|set|change|add|insert|empty|archive|submit|post|print|run|play|pause|mute|unmute|zip|unzip|extract|convert|export|import|share|pin|unpin|bookmark|refresh|reload|maximise|restart|shut down|lock|order|buy|pay|transfer|accept|decline|approve|merge|deploy|format|reset|sign';
const DO_START = new RegExp(`^(please |pls |can you |could you |would you |go ahead and |now |then )?(${DO_VERBS})\\b`, 'i');
const DO_FOLLOW = /^(do it|do that|do this|go ahead|yes,? do (it|that)|carry (it|that) out|make it so)\b/i;
const FOR_ME = /\b(for me|on my computer|on my behalf)\b/i;
const CHAIN = /\b(and then|then)\b|->/i;
// A request that only needs words back.
const TEXT_ONLY = /^(write|draft|compose|rewrite|reword|proofread|translate|summari[sz]e|explain|list|give me|suggest|generate)\b/i;
// The verbs that never infer a run without a card (trust-safety rule 3).
const RISKY =
  /\b(send|delete|remove|erase|pay|buy|order|purchase|submit|post|share|install|uninstall|format|reset|transfer|sign|reply|forward|merge|deploy|empty|wipe|publish)\b/i;
const VISUAL = /\b(which (button|icon|one|tab|option)|where (is|are|do)|colou?r|icon|button|look(s)? like|see|picture|image|photo|chart|graph|layout|design|logo|font|screenshot|highlighted|red|green|blue)\b/i;

// Shapes people search for verbatim.
const SHAPES = [
  /^"[^"]+"$|^'[^']+'$/, // quoted
  /^[a-z]+[A-Z][A-Za-z0-9]*$/, // camelCase
  /^[A-Za-z0-9]+(_[A-Za-z0-9]+)+$/, // snake_case
  /^0x[0-9a-f]+$/i, // hex
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i, // uuid
  /^[#$€£]?\d[\d,.]*%?$/, // number, money, percent
  /^\d{1,4}[-/.]\d{1,2}[-/.]\d{1,4}$/, // date
  /^v?\d+\.\d+(\.\d+)?$/, // version
  /^\d{1,3}(\.\d{1,3}){3}$/, // ip
  /^[^\s@]+@[^\s@]+\.[a-z]{2,}$/i, // email
  /^[A-Z]{2,}[-#]?\d+$/, // ticket ids like ABC-123
];

function fold(s) {
  return String(s || '')
    .toLowerCase()
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '');
}

function words(s) {
  return String(s || '').trim().split(/\s+/).filter(Boolean);
}

/**
 * Splits a typed line into its prefix lock and the text after it.
 *
 *   ?   Ask        /   Find        //   Find, exact matches only        >   Do
 *
 * A slash followed by a path ("/etc/hosts", "/usr/bin") is text, not a
 * prefix: a second slash inside the first word means a path. "/ etc" and
 * '/"etc"' still search.
 *
 * @param {string} raw
 * @returns {{lock: 'ask'|'find'|'do'|null, literal: boolean, prefix: string, body: string}}
 */
function parse(raw) {
  const s = String(raw || '');
  if (s.startsWith('//')) return { lock: 'find', literal: true, prefix: '//', body: s.slice(2).replace(/^\s+/, '') };
  if (s.startsWith('/')) {
    const first = s.slice(1).split(/\s/)[0];
    if (first.includes('/') && !first.startsWith('"')) return { lock: null, literal: false, prefix: '', body: s };
    return { lock: 'find', literal: false, prefix: '/', body: s.slice(1).replace(/^\s+/, '') };
  }
  if (s.startsWith('?')) return { lock: 'ask', literal: false, prefix: '?', body: s.slice(1).replace(/^\s+/, '') };
  if (s.startsWith('>')) return { lock: 'do', literal: false, prefix: '>', body: s.replace(/^>+/, '').replace(/^\s+/, '') };
  return { lock: null, literal: false, prefix: '', body: s };
}

/**
 * Scores what an unlocked line most likely means.
 *
 * @param {string} text the line without a prefix
 * @param {object} [ctx]
 * @param {string} [ctx.screen] text of the window in front, any case
 * @param {string} [ctx.screenFolded] the same, already folded (the bar folds once per window)
 * @returns {{intent: 'ask'|'find'|'do', scores: object, margin: number, runnerUp: string, why: string[], risky: boolean, visual: boolean}}
 */
function score(text, ctx) {
  const c = ctx || {};
  const t = String(text || '').trim();
  const why = [];
  const s = { ask: 0.3, find: 0, do: 0 };
  if (!t) return finish(s, why, t);
  const w = words(t);
  const n = w.length;

  const howTo = HOW_TO.test(t);
  if (/\?\s*$/.test(t)) {
    s.ask += 0.6;
    why.push('ends with a question mark');
  }
  const polite = /^(can|could|would) you\b/i.test(t) && DO_START.test(t);
  if (ASK_START.test(t) && !FIND_START.test(t) && !polite) {
    s.ask += 0.5;
    why.push('starts like a question');
  }
  if (howTo) {
    s.ask += 0.3;
    why.push('asks how to do something');
  }

  if (FIND_START.test(t)) {
    s.find += 0.65;
    why.push('asks where something is');
    const rest = words(t.replace(FIND_START, '').replace(/^\s*(the|a|an|my)\s+/i, ''));
    // "find the bug in this function" is a question that starts with find.
    if (rest.length > 5 || /\b(bug|error|mistake|problem|issue|wrong|answer|reason)\b/i.test(rest.join(' '))) {
      s.find -= 0.4;
      s.ask += 0.35;
      why.push('what it looks for is a judgement, not a word');
    }
  }

  const shaped = SHAPES.some((re) => re.test(t));
  if (shaped && n <= 3) {
    s.find += 0.45;
    why.push('looks like something written on screen');
  }

  const doStart = DO_START.test(t);
  const follow = DO_FOLLOW.test(t);
  if (doStart && !howTo) {
    // A lone verb ("save") is ambiguous; an order has an object.
    const bonus = n >= 2 ? 0.65 : 0.2;
    s.do += bonus;
    why.push(n >= 2 ? 'starts with something to do' : 'a single verb');
    if (/\?\s*$/.test(t)) s.do -= 0.3;
  }
  if (follow) {
    s.do += 0.75;
    why.push('asks to carry something out');
  }
  if (s.do > 0 && FOR_ME.test(t)) s.do += 0.15;
  if (s.do > 0 && CHAIN.test(t)) s.do += 0.1;
  if (TEXT_ONLY.test(t) && !doStart) {
    s.ask += 0.3;
    why.push('wants words back');
  }

  // Short and verbless: a search candidate, settled by the screen.
  const verbless = !doStart && !follow && !ASK_START.test(t) && !FIND_START.test(t);
  if (verbless && n <= 3) {
    s.find += 0.2;
  }

  const screen = c.screenFolded !== undefined ? c.screenFolded : c.screen ? fold(c.screen) : '';
  if (screen) {
    const body = fold(t.replace(/^["']|["']$/g, '').replace(FIND_START, '').trim());
    if (body.length >= 2 && n <= 5) {
      if (screen.includes(body)) {
        s.find += verbless || FIND_START.test(t) || shaped ? 0.4 : 0.1;
        why.push('it is on the screen');
      } else if (verbless && n <= 3) {
        const toks = words(body).filter((x) => x.length >= 3);
        if (toks.length && toks.every((x) => screen.includes(x))) {
          s.find += 0.2;
          why.push('its words are on the screen');
        } else {
          s.ask += 0.15;
          why.push('it is not on the screen');
        }
      }
    }
  } else if (verbless && n <= 3 && !shaped) {
    // Without the screen there is no evidence for a search either way.
    s.find -= 0.05;
  }

  if (n >= 6 && !doStart && !follow) {
    s.ask += 0.15;
    why.push('a long sentence');
  }
  return finish(s, why, t);
}

function finish(s, why, t) {
  const ranked = INTENTS.map((k) => [k, Math.round(s[k] * 100) / 100]).sort((a, b) => b[1] - a[1] || INTENTS.indexOf(a[0]) - INTENTS.indexOf(b[0]));
  let [intent, top] = ranked[0];
  let [runnerUp, second] = ranked[1];
  // Ties go to Ask, the cheap and harmless reading.
  if (intent !== 'ask' && top - (s.ask || 0) < 0.001) {
    runnerUp = intent;
    second = top;
    intent = 'ask';
    top = s.ask;
  }
  // Find is never guessed. Like a browser's find it starts when asked for
  // (its shortcut, a leading /, Ctrl+2); typed words that happen to be on
  // screen are a question until then. It stays the runner-up, so the bar
  // can still offer it.
  if (intent === 'find') {
    runnerUp = 'find';
    second = top;
    intent = 'ask';
    top = s.ask;
  }
  // Do has to win clearly; anything less is offered as the runner-up.
  if (intent === 'do' && (top < 0.6 || top - second < 0.15)) {
    runnerUp = 'do';
    intent = ranked.find(([k]) => k !== 'do')[0];
    second = top;
    top = s[intent];
  }
  return {
    intent,
    scores: { ask: s.ask, find: s.find, do: s.do },
    margin: Math.max(0, Math.round((top - second) * 100) / 100),
    runnerUp,
    why,
    risky: RISKY.test(t),
    visual: VISUAL.test(t),
  };
}

/**
 * parse + score: the whole decision for one typed line.
 *
 * @param {string} raw what is in the field, prefix included
 * @param {object} [ctx] see score()
 */
function classify(raw, ctx) {
  const p = parse(raw);
  if (p.lock) {
    const sc = score(p.body, ctx);
    return { ...sc, intent: p.lock, locked: true, literal: p.literal, prefix: p.prefix, body: p.body, inferred: false };
  }
  const sc = score(p.body, ctx);
  return { ...sc, locked: false, literal: false, prefix: '', body: p.body, inferred: true };
}

// Starting points under an empty bar, picked from the app in front. Each one
// fills the field, so the chip also teaches what the guess will be.
const EXAMPLES = {
  browser: ['Summarize this page', 'Find "price"', 'Open a new tab and search for the weather'],
  excel: ['What is the total of this column?', 'Find "total"', 'Sort this sheet by date'],
  mail: ['Draft a reply to this email', 'Find "invoice"', 'What does this email want from me?'],
  editor: ['What does this error mean?', 'Find "TODO"', 'Explain this function'],
  terminal: ['Why did this command fail?', 'Find "error"', 'What does this output mean?'],
  explorer: ['Which of these files is the newest?', 'Find "report"', 'Rename these files by date'],
  document: ['Summarize this document', 'Find "deadline"', 'Fix the grammar in this paragraph'],
  other: ["What's on my screen?", 'Find "total"', 'Open Notepad and write a haiku'],
};

function appKind(processName, title) {
  const p = String(processName || '').toLowerCase().replace(/\.exe$/, '');
  const t = String(title || '').toLowerCase();
  if (/^(chrome|msedge|firefox|brave|opera|vivaldi|arc)$/.test(p)) return 'browser';
  if (p === 'excel') return 'excel';
  if (/^(outlook|olk|thunderbird|hxoutlook)$/.test(p) || /\b(inbox|mail)\b/.test(t)) return 'mail';
  if (/^(code|devenv|idea64|pycharm64|sublime_text|notepad\+\+|cursor|rider64|webstorm64)$/.test(p)) return 'editor';
  if (/^(windowsterminal|cmd|powershell|pwsh|conhost|wt|mintty|alacritty)$/.test(p)) return 'terminal';
  if (p === 'explorer') return 'explorer';
  if (/^(winword|notepad|wordpad|acrord32|acrobat|sumatrapdf|onenote|obsidian|notion)$/.test(p)) return 'document';
  return 'other';
}

function examples(processName, title) {
  return EXAMPLES[appKind(processName, title)].slice();
}

const api = { INTENTS, NAME, parse, score, classify, examples, appKind, fold, RISKY, VISUAL };
if (typeof module !== 'undefined' && module.exports) module.exports = api;
else root.AevalIntent = api;
})(typeof window !== 'undefined' ? window : globalThis);
