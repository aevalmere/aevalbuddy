'use strict';

// OpenRouter chat-completions client. Plain fetch plus SSE parsing; the
// OpenAI-compatible wire format needs no SDK. Runs in the main process only,
// so the API key never reaches a renderer.

const ENDPOINT = 'https://openrouter.ai/api/v1/chat/completions';
const MODELS_ENDPOINT = 'https://openrouter.ai/api/v1/models';
const KEY_ENDPOINT = 'https://openrouter.ai/api/v1/key';

// Free comes first on purpose: the app must be usable with an OpenRouter
// account that has never been topped up. Paid models are one toggle away in
// settings for anyone who wants the speed, and nothing paid is ever selected
// on their behalf.
// Reading accuracy is the whole job, whether this model answers or only
// reads for the brain. The fastest free model held this slot and it was the
// wrong trade: an answer built on a misread variable name is not a fast
// answer, it is a wrong one.
const DEFAULT_MODEL = 'nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free';

// The optional second stage, off by default. A free vision model reads the
// screen, then a much stronger free text model does the thinking over that
// reading. See readScreen below for what that trades away.
const DEFAULT_BRAIN = 'nvidia/nemotron-3-super-120b-a12b:free';

// Hand-picked, in the order they appear in settings. Every id is checked
// against the live list before it is offered, so a retired model disappears
// instead of failing at query time. Notes describe the model; the price and
// context line under each one is built from live data.
const CURATED = [
  {
    id: 'nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free',
    tier: 'free',
    note: 'Built as a perception sub-agent, which is the job here: read the screen accurately, because everything else is built on that reading. 3B active parameters, so it is not slow. The default.',
  },
  {
    id: 'google/gemma-4-26b-a4b-it:free',
    tier: 'free',
    note: 'Only 4B parameters active per token, so it is the quickest of the free ones. Pick it for speed over accuracy.',
  },
  {
    id: 'dots-studio/dots-3-note-preview:free',
    tier: 'free',
    note: 'Half a million tokens of context. Handles very long conversations.',
  },
  {
    id: 'google/gemma-4-31b-it:free',
    tier: 'free',
    note: 'The larger Gemma. Better answers, slower to start.',
  },
  {
    id: 'thinkingmachines/inkling:free',
    tier: 'free',
    note: 'Thinking Machines. Strong reasoning for a free model.',
  },
  {
    id: 'qwen/qwen3.7-flash',
    tier: 'paid',
    note: 'Quick to start talking and reliable at reading a screen. The one to pick if you add credit.',
  },
  {
    id: 'deepseek/deepseek-v4-flash-vision-exp',
    tier: 'paid',
    note: 'DeepSeek, built for vision. Good on dense text and code.',
  },
  { id: 'z-ai/glm-4.6v', tier: 'paid', note: 'GLM. Strong at reading interfaces and dialogs.' },
  {
    id: 'bytedance-seed/seed-1.6-flash',
    tier: 'paid',
    note: 'Cheapest of the quick ones. Short answers.',
  },
  {
    id: 'z-ai/glm-5v-turbo',
    tier: 'paid',
    note: 'The most capable vision model here. Slower to start, better at multi-step problems.',
  },
];

// Text-only models for the brain stage. These never see an image, so the
// vision requirement drops away and the strongest free models on OpenRouter
// become available.
const BRAINS = [
  {
    id: 'nvidia/nemotron-3-super-120b-a12b:free',
    tier: 'free',
    note: 'Most of Ultra at a fraction of the wait. The default.',
  },
  {
    id: 'nvidia/nemotron-3-ultra-550b-a55b:free',
    tier: 'free',
    note: '550B parameters, a million tokens of context. Heavy, and slow to start.',
  },
  {
    id: 'nvidia/nemotron-3.5-lightning:free',
    tier: 'free',
    note: 'Built for speed. Use it when the reading is already doing the hard part.',
  },
  { id: 'z-ai/glm-5.3', tier: 'paid', note: 'GLM 5.3. The sharpest brain here, for a fraction of a cent.' },
  {
    id: 'deepseek/deepseek-v4-flash',
    tier: 'paid',
    note: 'DeepSeek Flash. Answers almost instantly for a fraction of a cent.',
  },
];

const TIER_LABEL = {
  free: 'Free',
  paid: 'Paid, a fraction of a cent per question',
};

// Shown when the live list cannot be fetched (offline, endpoint change), so
// the picker is never empty.
function fallbackFrom(list, includePaid) {
  return list
    .filter((c) => includePaid || c.tier === 'free')
    .map((c) => ({ id: c.id, name: c.id, note: c.note, tier: c.tier, detail: '' }));
}

let modelsCache = { at: 0, data: null };

// OpenRouter has shipped two shapes for model modality over time; accept
// either so a schema shift degrades to the fallback list instead of an
// empty picker.
function acceptsImages(m) {
  const a = m.architecture;
  if (!a) return false;
  if (typeof a.modality === 'string' && a.modality.includes('image')) return true;
  return Array.isArray(a.input_modalities) && a.input_modalities.includes('image');
}

function formatContext(n) {
  if (!n) return '';
  if (n >= 1000000) return `${Math.round(n / 1000000)}M context`;
  return `${Math.round(n / 1024)}K context`;
}

// Price per million tokens, which is the unit OpenRouter publishes and the
// only one that stays true as rates change.
function formatPrice(pricing) {
  const inp = Number(pricing && pricing.prompt);
  const out = Number(pricing && pricing.completion);
  if (!Number.isFinite(inp) || !Number.isFinite(out)) return '';
  if (inp === 0 && out === 0) return 'Free';
  return `$${(inp * 1e6).toFixed(2)} in / $${(out * 1e6).toFixed(2)} out per million tokens`;
}

function describe(m) {
  return [formatPrice(m.pricing), formatContext(m.context_length)].filter(Boolean).join(' · ');
}

/**
 * The model catalogue for a settings picker, as ordered groups.
 *
 * @param {object} [opts]
 * @param {'vision'|'text'} [opts.kind] vision models answer directly; text
 *        models are only ever used as the brain, which never sees an image
 * @param {boolean} [opts.includePaid] paid models stay hidden until asked for
 * @returns {Promise<{groups: Array<{tier, label, models}>, live: boolean}>}
 */
async function listModels(opts) {
  const kind = (opts && opts.kind) === 'text' ? 'text' : 'vision';
  const includePaid = !!(opts && opts.includePaid);
  const curated = kind === 'text' ? BRAINS : CURATED;
  const fresh = modelsCache.data && Date.now() - modelsCache.at < 5 * 60 * 1000;
  const offline = () => ({ groups: groupBy(fallbackFrom(curated, includePaid)), live: false });

  let all = fresh ? modelsCache.data : null;
  if (!all) {
    try {
      const res = await fetch(MODELS_ENDPOINT, { signal: AbortSignal.timeout(8000) });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const body = await res.json();
      all = Array.isArray(body.data) ? body.data : [];
      if (!all.length) return offline();
      modelsCache = { at: Date.now(), data: all };
    } catch (err) {
      return offline();
    }
  }

  const byId = new Map(all.map((m) => [m.id, m]));
  const models = [];
  for (const c of curated) {
    if (!includePaid && c.tier !== 'free') continue;
    const live = byId.get(c.id);
    // A model that has been retired, or that lost image input, is dropped
    // rather than offered and failed at query time.
    if (!live) continue;
    if (kind === 'vision' && !acceptsImages(live)) continue;
    models.push({
      id: c.id,
      name: live.name || c.id,
      note: c.note,
      tier: c.tier,
      detail: describe(live),
    });
  }
  if (!models.length) return offline();
  return { groups: groupBy(models), live: true };
}

function groupBy(models) {
  return ['free', 'paid']
    .map((tier) => ({
      tier,
      label: TIER_LABEL[tier],
      models: models.filter((m) => m.tier === tier),
    }))
    .filter((g) => g.models.length);
}

/**
 * Checks a key against OpenRouter and reports what it can see, so the
 * settings panel can say "this key works" before the first real question
 * rather than failing inside an answer.
 *
 * @param {string} apiKey
 * @returns {Promise<{ok: boolean, message: string}>}
 */
async function testKey(apiKey) {
  const key = String(apiKey || '').trim();
  if (!key) return { ok: false, message: 'No key saved yet.' };
  try {
    const res = await fetch(KEY_ENDPOINT, {
      headers: { Authorization: `Bearer ${key}` },
      signal: AbortSignal.timeout(8000),
    });
    if (res.status === 401) {
      return { ok: false, message: 'OpenRouter rejected this key. Check it and save again.' };
    }
    if (!res.ok) return { ok: false, message: `OpenRouter replied HTTP ${res.status}.` };
    const body = await res.json();
    const d = (body && body.data) || {};
    const limit = Number(d.limit);
    const used = Number(d.usage);
    if (Number.isFinite(limit) && Number.isFinite(used)) {
      const left = Math.max(0, limit - used);
      return {
        ok: true,
        message:
          left > 0
            ? `Key works. About $${left.toFixed(2)} of credit left.`
            : 'Key works, but the credit is used up. Add credit, or pick a Free model.',
      };
    }
    // No limit set means a pay-as-you-go account with a balance OpenRouter
    // does not report here; the key itself is confirmed good.
    return { ok: true, message: 'Key works.' };
  } catch (err) {
    return { ok: false, message: 'Could not reach OpenRouter. Check your connection.' };
  }
}

// The reader stage. A vision model turns the screen into a short structured
// report, and a separate text model answers from that report.
//
// The point of splitting is that OpenRouter's strongest free models are text
// only: Nemotron Super cannot see an image, so the only way to put it behind this
// app is to hand it a reading. What it costs is honest to state. Two calls
// run in sequence, so the wait is the reader's full reply plus the brain's
// first token, and the brain only knows what the reader wrote down, so a
// detail the reader skipped is a detail the brain cannot use. The ELEMENTS
// block exists for exactly that reason: without coordinates in the reading,
// the brain could never point at anything.
// One coordinate convention for the whole app, shared by the reader's ELEMENTS
// table and by a model answering straight from the screen.
//
// Pixels were the old rule on the answering side and they were the wrong one.
// A vision model is never told how large the view it received is: the provider
// resizes and re-tiles the image on the way in, so "the centre in pixels of a
// 1600x900 view" asks it to convert between a space it can see and a number it
// was never given. It answers on the 0-1000 grid it was trained on instead, the
// app divided those numbers by the image size, and the pointer landed hundreds
// of pixels up and to the left -- with anything down the right or bottom edge
// out of range and silently dropped.
const GRID_RULE = [
  'Coordinates are on a 0 to 1000 grid laid over the view, whichever way round',
  'and whatever size it is: 0,0 is the top-left corner, 1000,1000 the bottom',
  'right, 500,500 the middle. x is across, y is down.',
].join('\n');

const READER_PROMPT = [
  'You are the eyes of a desktop assistant. You cannot answer the question.',
  'Another model answers from your notes and nothing else, so a detail you',
  'leave out is a detail it cannot use, and a detail you invent becomes its',
  'wrong answer. Report in this exact shape, with nothing before or after it:',
  '',
  'SCREEN: the application and the window title, one line',
  'STATE: what is happening right now, one line',
  'FOCUS: what is selected, focused or under the cursor, or NONE',
  'TASK: exactly one of debug, explain, rewrite, assist. debug if something has',
  '  failed or is showing an error. explain if the user is looking at something',
  '  unfamiliar. rewrite if the screen is text they are composing. assist otherwise.',
  'TEXT: the text that matters, copied character for character. Include the',
  '  whole error message, file paths, identifiers, numbers, and any code in',
  '  view. Take as many lines as it needs. Write NONE if there is no text.',
  'ELEMENTS:',
  '- label | x,y',
  '  one line per control worth pointing at, at most 8, ordered by how likely',
  '  the user is to want it. Label each one exactly as it is labelled on',
  '  screen, because the answering model can only refer to it by that name.',
  '',
  GRID_RULE,
  'Give the centre of the element.',
  '',
  'Copy text exactly. Never correct a spelling, never paraphrase an error, and',
  'never round a number. Write only what is visible; if something is too small',
  'or too blurred to read, write it as unreadable instead of guessing. No',
  'advice, no preamble, no markdown, no code fences.',
].join('\n');

// One ELEMENTS line. The reader is asked for "label | x,y" and a small model
// drifts off that in a dozen small ways: parentheses instead of a pipe, "x="
// in front of the numbers, a numbered list instead of a dash, a colon after
// the label, markdown emphasis around it, a fraction of one or a percentage
// instead of the grid. Every one of those used to be dropped on the floor,
// and a dropped row is not a cosmetic loss: it is a label the brain names in
// its answer and the app cannot resolve, so the reply says "click here" and
// no pointer appears.
//
// One regex for all of that was unreadable and still missed cases, so the
// line comes apart in steps: strip the bullet, find the coordinate pair
// anchored to the end of the line, and take everything before it as the
// label.
const BULLET_RE = /^\s*(?:[-*\u2022]|\d{1,2}[.)])\s+/;
const COORD_RE =
  /(?:^|[\s|(:,])\s*\(?\s*(?:x\s*[=:]\s*)?(\d{1,5}(?:\.\d+)?%?)\s*(?:,|\s)\s*(?:y\s*[=:]\s*)?(\d{1,5}(?:\.\d+)?%?)\s*\)?\s*$/i;

/**
 * Two raw coordinate tokens onto the 0-1000 grid.
 *
 * Percentages, fractions of one and raw view pixels are the three ways a
 * model leaves the stated grid, and the first two are recoverable without
 * guessing. Pixels need the size of the view the model was looking at; given
 * it, a value over 1000 that still fits inside the view is rescaled, and
 * without it the row is dropped rather than placed wrongly.
 */
function toGrid(rawX, rawY, view) {
  let x = parseFloat(rawX);
  let y = parseFloat(rawY);
  if (!Number.isFinite(x) || !Number.isFinite(y) || x < 0 || y < 0) return null;
  if (/%$/.test(rawX) && /%$/.test(rawY)) {
    x *= 10;
    y *= 10;
  } else if (x <= 1 && y <= 1 && (rawX.includes('.') || rawY.includes('.'))) {
    x *= 1000;
    y *= 1000;
  } else if (x > 1000 || y > 1000) {
    const w = view && view.width;
    const h = view && view.height;
    if (!w || !h || x > w || y > h) return null;
    x = (x / w) * 1000;
    y = (y / h) * 1000;
  }
  if (x > 1000 || y > 1000) return null;
  return { x: Math.round(x), y: Math.round(y) };
}

// Whatever is left in front of the coordinates: the separator that split them
// off, and any markdown the reader wrapped the name in.
function cleanLabel(text) {
  return String(text || '')
    .replace(/[|(:,\s-]+$/, '')
    .replace(/^[*_`"'\s]+/, '')
    .replace(/[*_`"'\s]+$/, '')
    .trim();
}

/**
 * The reader's ELEMENTS table as [{ label, x, y }] on the 0-1000 grid. When a
 * text-only brain is answering, this table is the only place screen positions
 * exist: the brain names a label, and the app looks the position up here.
 *
 * @param {string} report the reader's reply
 * @param {{width: number, height: number}} [view] the size of the image the
 *        reader was given, used only to rescue a row that came back in view
 *        pixels instead of on the grid
 */
function parseElements(report, view) {
  const lines = String(report || '').split(/\r?\n/);
  const out = [];
  let inside = false;
  for (const line of lines) {
    if (/^\s*ELEMENTS\s*:/i.test(line)) {
      inside = true;
      continue;
    }
    if (!inside) continue;
    // Any other field name ends the block.
    if (/^[A-Z][A-Z ]{2,}\s*:/.test(line)) break;
    const body = line.replace(BULLET_RE, '');
    const m = body.match(COORD_RE);
    if (!m) continue;
    const point = toGrid(m[1], m[2], view);
    if (!point) continue;
    const label = cleanLabel(body.slice(0, m.index));
    if (!label) continue;
    out.push({ label, x: point.x, y: point.y });
    if (out.length >= 8) break;
  }
  return out;
}

function normLabel(text) {
  return String(text || '')
    .toLowerCase()
    .replace(/[^a-z0-9 ]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

// Under this, the name did not really match anything. Under the margin, two
// rows matched it about equally well and there is no way to tell which was
// meant.
const MATCH_MIN = 0.5;
const MATCH_MARGIN = 0.08;

/**
 * Finds the element a directive meant.
 *
 * Every row is scored and the best one wins, which is the whole point: the
 * first pass took the first containment hit in list order, so a table holding
 * "Terminal tab" and "Problems tab" resolved the name "tab" to Terminal every
 * time, by position rather than by meaning. Now an ambiguous name scores the
 * same against both, fails the margin, and points at nothing.
 *
 * @returns {{label: string, x: number, y: number}|null}
 */
function matchElement(label, elements) {
  const want = normLabel(label);
  if (!want || !elements || !elements.length) return null;
  const wantWords = want.split(' ').filter(Boolean);
  let best = null;
  let bestScore = 0;
  let runnerUp = 0;
  for (const el of elements) {
    const key = normLabel(el.label);
    if (!key) continue;
    // An exact name is unambiguous by definition and skips the rest.
    if (key === want) return el;
    let score = 0;
    if (key.includes(want) || want.includes(key)) {
      // How much of the longer name the shorter one accounts for: "Save" is a
      // better read of "Save" than "Save As Template" is.
      const shorter = Math.min(key.length, want.length);
      const longer = Math.max(key.length, want.length);
      score = 0.6 + 0.35 * (shorter / longer);
    }
    const words = new Set(key.split(' ').filter(Boolean));
    const hits = wantWords.filter((w) => words.has(w)).length;
    if (hits) {
      // Balanced against both lengths, so a long row does not win on the
      // strength of one shared word.
      const overlap = (2 * hits) / (wantWords.length + words.size);
      score = Math.max(score, Math.min(0.95, overlap));
    }
    if (score > bestScore) {
      runnerUp = bestScore;
      bestScore = score;
      best = el;
    } else if (score > runnerUp) {
      runnerUp = score;
    }
  }
  if (bestScore < MATCH_MIN) return null;
  if (bestScore - runnerUp < MATCH_MARGIN) return null;
  return best;
}

/**
 * Runs the reader stage: one non-streamed call that returns a description of
 * the screen for the brain to work from.
 *
 * @param {object} opts
 * @param {string} opts.apiKey
 * @param {string} opts.model a vision model
 * @param {string} opts.imageBase64 JPEG of the screen, base64 without prefix
 * @param {number} opts.imageWidth
 * @param {number} opts.imageHeight
 * @param {string} opts.question asked so the reader knows what to look at
 * @param {AbortSignal} opts.signal
 * @returns {Promise<string>} the report
 */
// The reader is told to answer in a fixed shape, but it is still a small
// model: a reply that lost the shape is worse than no reply, because the
// brain would treat prose as a reading of the screen. Anything without a
// SCREEN line, or too short to contain one, is rejected so the caller can
// fall back to answering directly.
function validReport(text) {
  const t = String(text || '').trim();
  return t.length >= 40 && /^\s*SCREEN\s*:/im.test(t);
}

// Small models like to wrap structured output in a code fence.
function unfence(text) {
  const t = String(text || '').trim();
  const m = t.match(/^```[a-z]*\s*\n([\s\S]*?)\n?```$/i);
  return (m ? m[1] : t).trim();
}

/** The task word the reader chose, for automatic answer-style selection. */
function extractTask(report) {
  const m = String(report || '').match(/^\s*TASK\s*:\s*([a-z]+)/im);
  return m ? m[1].toLowerCase() : '';
}

// Appended to the reader's brief when the image is an area the user dragged
// rather than the whole desktop. Without it the reader writes "SCREEN: Visual
// Studio Code" for a crop of one error message and the brain answers as though
// it can see the rest of the window.
const REGION_NOTE = [
  '',
  'This image is one area the user selected out of their screen, not the whole',
  'screen. Report only what is inside it. If the area cuts something off, say',
  'so rather than assuming what the rest of it said.',
].join('\n');

async function readScreen({
  apiKey,
  model,
  imageBase64,
  imageWidth,
  imageHeight,
  question,
  region,
  signal,
}) {
  const res = await fetch(ENDPOINT, {
    method: 'POST',
    signal,
    headers: {
      Authorization: `Bearer ${apiKey}`,
      'Content-Type': 'application/json',
      'X-Title': 'AevalBuddy',
    },
    body: JSON.stringify({
      model,
      stream: false,
      // Room for a whole error message and a screen of code. The reading is
      // everything the brain will ever know about the screen, so cutting it
      // short to save a second costs more than the second is worth.
      max_tokens: 1600,
      temperature: 0,
      messages: [
        {
          role: 'system',
          content: region ? READER_PROMPT + REGION_NOTE : READER_PROMPT,
        },
        {
          role: 'user',
          content: [
            { type: 'image_url', image_url: { url: `data:image/jpeg;base64,${imageBase64}` } },
            {
              type: 'text',
              text: `The user asked: ${question}\n\nReport the screen.`,
            },
          ],
        },
      ],
    }),
  });
  if (!res.ok) {
    let message = `HTTP ${res.status}`;
    try {
      const data = await res.json();
      if (data && data.error && data.error.message) message = data.error.message;
    } catch (parseErr) {
      // Status alone is enough to map the failure.
    }
    const err = new ApiError(res.status, message);
    // Tagged so the caller can name the reader in the error rather than the
    // brain, which never ran.
    err.stage = 'reader';
    throw err;
  }
  const body = await res.json();
  const text =
    body && body.choices && body.choices[0] && body.choices[0].message
      ? String(body.choices[0].message.content || '').trim()
      : '';
  const report = unfence(text);
  if (!validReport(report)) {
    const err = new ApiError(204, 'the reader did not describe the screen');
    err.stage = 'reader';
    throw err;
  }
  return report;
}

// The assistant speaks as a helper built into the desktop, never as a bot
// examining an attachment: "on your screen", not "in the screenshot". The
// pointer directive lets it summon the on-screen pointer at a location.
//
// imageWidth and imageHeight are read as a presence flag and nothing more.
// Their values are deliberately not quoted to the model: see GRID_RULE.
function makeSystemPrompt({ imageWidth, imageHeight, skill, viaReader, viaRegion }) {
  const lines = [
    'You are AevalBuddy, an assistant built into the user\'s Windows desktop.',
    'This is an ongoing conversation; the user may follow up on earlier answers.',
  ];
  if (viaReader) {
    // The brain is text only. It gets a reading instead of the screen, and
    // must not pretend to more certainty than the reading supports.
    if (viaRegion) {
      lines.push(
        'The report below covers one area the user drew around, not their whole',
        'screen. Nothing outside that area was looked at, so do not refer to it.'
      );
    }
    lines.push(
      'You cannot see the screen yourself. A reader model looked at it and wrote',
      'the report below. Treat the report as what is on screen and speak to the',
      'user directly about their screen, never mentioning the report, a reader,',
      'a screenshot, or an image. If the report does not cover something you need,',
      'say what you would need to see rather than inventing it.',
      'You do not know where anything is on the screen and must never guess a',
      'position. To make the on-screen pointer indicate something, put a',
      'directive on its own line: [[point Save button]], where the text is a',
      "label copied exactly from the report's ELEMENTS list. Naming it is how",
      'you point at it. A name that is not in that list points at nothing, so',
      'copy it rather than describing it. Use at most 3 per reply, and only',
      'when the location is the answer.'
    );
  } else if (imageWidth && imageHeight) {
    lines.push(
      viaRegion
        ? "With this message you can see one area of the user's screen, which they" +
            ' drew around because it is the part they mean. You cannot see the rest' +
            ' of their screen, so do not describe or assume anything outside it.'
        : "With this message you can see the user's screen.",
      'Talk about it as their screen ("on your screen", "the dialog in the corner").',
      'Never say screenshot, image, picture, or "provided"; you are looking at their screen, not a file.',
      '',
      'To physically point at a spot on their screen, put a directive on its own line:',
      '[[point x=500 y=500 | short label]]',
      GRID_RULE,
      'Give the centre of the thing you mean. Never write a pixel count, a',
      'percentage, or a fraction; the grid is the only unit. Use at most 3 per',
      'reply, and only when the location is the answer (a button to press, a',
      'field to fix, a line to read).'
    );
  } else {
    lines.push(
      'The view of their screen is unavailable for this message, so answer from',
      'the conversation alone and say when you would need to see the screen.'
    );
  }
  lines.push(
    'Lead with the direct answer or the next step. Keep it short: brief paragraphs,',
    'short lists. Markdown is rendered.'
  );
  // Joined on newlines, not spaces. These are instructions, not prose: a small
  // model handed one 1000-character paragraph loses the pointer rule in the
  // middle of it, and the rule itself asks for a directive "on its own line".
  const prompt = lines.join('\n');
  // The skill is a procedure, not a sentence, so it keeps its line breaks and
  // sits at the end where the last instruction carries most weight.
  if (skill && skill.body) {
    return `${prompt}\n\n--- ${skill.name || 'How to answer'} ---\n${skill.body}`;
  }
  return prompt;
}

// Failure a different model can answer. Free models are queued and rate
// limited by design, and a retired id 404s until someone edits the list, so
// these are the everyday states rather than the exceptional ones: showing the
// user an error and asking them to go and choose a different model in Settings
// is making them do the app's job.
const FAILOVER_STATUS = new Set([404, 408, 429, 500, 502, 503, 504]);

function shouldFailover(err) {
  if (!err || err.name === 'AbortError') return false;
  // A rejected key, no credit, or a malformed request fails the same way on
  // every model; retrying is just a second wait before the same message.
  return FAILOVER_STATUS.has(err.status);
}

/**
 * The next free model to try after one has failed.
 *
 * @param {'vision'|'text'} kind vision models answer from the screen; text
 *        models are only ever the brain
 * @param {string[]} tried ids already attempted this request
 * @returns {string|null}
 */
function nextFreeModel(kind, tried) {
  const seen = new Set(tried || []);
  const pool = kind === 'text' ? BRAINS : CURATED;
  const hit = pool.find((m) => m.tier === 'free' && !seen.has(m.id));
  return hit ? hit.id : null;
}

class ApiError extends Error {
  constructor(status, message) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

/**
 * Streams one conversation turn to OpenRouter.
 *
 * @param {object} opts
 * @param {string} opts.apiKey
 * @param {string} opts.model OpenRouter model id
 * @param {string} opts.question the newest user message
 * @param {string|null} opts.imageBase64 JPEG of the current screen, base64 without prefix
 * @param {number} [opts.imageWidth] pixel width of that JPEG
 * @param {number} [opts.imageHeight] pixel height of that JPEG
 * @param {Array<{role: string, text: string}>} [opts.history] earlier turns, text only
 * @param {{name: string, body: string}|null} [opts.skill] answering procedure appended to the system prompt
 * @param {AbortSignal} opts.signal
 * @param {(text: string) => void} opts.onDelta
 * @returns {Promise<void>} resolves when the stream completes
 */
async function streamQuery({
  apiKey,
  model,
  question,
  imageBase64,
  imageWidth,
  imageHeight,
  history,
  skill,
  screenReport,
  viaRegion,
  signal,
  onDelta,
}) {
  const content = [];
  if (screenReport) {
    // Brain mode: the reading replaces the image entirely, so this request
    // carries no picture and any text model can serve.
    content.push({
      type: 'text',
      text: `What is on the user's screen right now:

${screenReport}`,
    });
  } else if (imageBase64) {
    content.push({
      type: 'image_url',
      image_url: { url: `data:image/jpeg;base64,${imageBase64}` },
    });
  }
  content.push({ type: 'text', text: question });

  // Earlier turns travel as plain text; only the newest message carries the
  // current screen, which keeps request size flat as the conversation grows.
  const messages = [
    {
      role: 'system',
      content: makeSystemPrompt({
        imageWidth: imageBase64 || screenReport ? imageWidth : 0,
        imageHeight: imageBase64 || screenReport ? imageHeight : 0,
        skill,
        viaReader: !!screenReport,
        viaRegion: !!viaRegion,
      }),
    },
  ];
  for (const m of Array.isArray(history) ? history : []) {
    if ((m.role === 'user' || m.role === 'assistant') && m.text) {
      messages.push({ role: m.role, content: m.text });
    }
  }
  messages.push({ role: 'user', content });

  const res = await fetch(ENDPOINT, {
    method: 'POST',
    signal,
    headers: {
      Authorization: `Bearer ${apiKey}`,
      'Content-Type': 'application/json',
      'X-Title': 'AevalBuddy',
    },
    body: JSON.stringify({
      model,
      stream: true,
      // Low, not default. Half of this reply is a reading task: the pointer
      // directive carries coordinates, and digits sampled at the model's
      // default temperature are digits with noise in them. The reader stage
      // has always run at 0 for the same reason; prose survives 0.2 fine.
      temperature: 0.2,
      messages,
    }),
  });

  if (!res.ok) {
    let message = `HTTP ${res.status}`;
    try {
      const data = await res.json();
      if (data && data.error && data.error.message) message = data.error.message;
    } catch (parseErr) {
      // Non-JSON error body; the status code alone is enough to map it.
    }
    throw new ApiError(res.status, message);
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  let gotContent = false;
  const handleLine = (rawLine) => {
    const line = rawLine.trim();
    // Skips SSE comments such as ": OPENROUTER PROCESSING" keepalives.
    if (!line.startsWith('data:')) return false;
    const payload = line.slice(5).trim();
    if (payload === '[DONE]') return true;
    let obj;
    try {
      obj = JSON.parse(payload);
    } catch (parseErr) {
      return false;
    }
    if (obj.error) {
      throw new ApiError(obj.error.code || 500, obj.error.message || 'stream error');
    }
    const delta =
      obj.choices && obj.choices[0] && obj.choices[0].delta
        ? obj.choices[0].delta.content
        : null;
    if (delta) {
      gotContent = true;
      onDelta(delta);
    }
    return false;
  };
  let finished = false;
  while (!finished) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    let newline;
    while ((newline = buffer.indexOf('\n')) >= 0) {
      const rawLine = buffer.slice(0, newline);
      buffer = buffer.slice(newline + 1);
      if (handleLine(rawLine)) {
        finished = true;
        break;
      }
    }
  }
  // Some models stream only reasoning or tool traffic and never any answer
  // text; surface that as an error instead of an empty panel.
  if (!gotContent) {
    throw new ApiError(204, 'empty response');
  }
}

// A directive comes in two shapes. A model that can see the screen gives a
// coordinate pair: [[point x=.. y=.. | label]]. A brain working from a
// reading has no coordinates and gives a name instead: [[point Save button]],
// which the app resolves against the reader's ELEMENTS table. Letting a blind
// model emit numbers is what put the pointer in the wrong place: it was
// copying digits it had no way to check, or inventing them outright.
// Both shapes are read loosely. The strict pair-only pattern turned a near
// miss into raw "[[point 120, 340 | Save]]" printed in the middle of the
// answer, which is the worst of the three outcomes: no pointer and visible
// machinery.
const POINT_RE =
  /^\[\[\s*point\s*[:=]?\s*\(?\s*(?:x\s*[=:]\s*)?(\d{1,5}(?:\.\d+)?%?)\s*(?:,|\s)\s*(?:y\s*[=:]\s*)?(\d{1,5}(?:\.\d+)?%?)\s*\)?\s*(?:[|,;\u2013\u2014-]\s*([^\]]{0,140}?))?\s*\]\]$/i;
const POINT_NAMED_RE =
  /^\[\[\s*point\s*[:=]?\s*(?:at\s+)?(?!x\s*[=:])([^\]|]{1,120}?)\s*(?:\|\s*[^\]]{0,140}?)?\s*\]\]$/i;
// Anything that opens like a directive and closes. Used only to decide that a
// malformed one is swallowed rather than shown.
const POINT_ANY_RE = /^\[\[\s*point\b/i;

// The longest text a directive is allowed to occupy before the buffer gives
// up waiting for its closing brackets and flushes it as ordinary text.
const POINT_HOLD_MAX = 220;

/**
 * Wraps a delta stream and strips [[point ...]] directives out of it before
 * they reach the display, collecting them as coordinates. Directives can
 * arrive split across any number of deltas.
 *
 * @param {(text: string) => void} onText receives display-safe text
 * @returns {{ push(chunk: string): void, flush(): void, points: Array, text: string }}
 */
function createPointExtractor(onText) {
  let buf = '';
  // Set when a directive has just been removed, so the space it was sitting
  // in does not survive it: "Press [[point ...]] to save" left "Press  to
  // save" with a visible double space. Only spaces and tabs are eaten; a
  // directive that had its own line keeps the blank line around it, which is
  // the paragraph break the markdown wants anyway.
  let closeGap = false;
  const out = {
    points: [],
    text: '',
    push(chunk) {
      buf += String(chunk || '');
      drain(false);
    },
    flush() {
      drain(true);
      if (buf) emit(buf);
      buf = '';
    },
  };

  function emit(text) {
    let t = text;
    if (!t) return;
    if (closeGap) {
      if (/[^\S\r\n]$/.test(out.text)) t = t.replace(/^[^\S\r\n]+/, '');
      if (!t) return; // still nothing to show; keep waiting to close the gap
      closeGap = false;
    }
    out.text += t;
    onText(t);
  }

  function drain(final) {
    for (;;) {
      const open = buf.indexOf('[[');
      if (open === -1) {
        // Keep a trailing single '[' in the buffer; the second one may be in
        // the next delta.
        if (!final && buf.endsWith('[')) {
          emit(buf.slice(0, -1));
          buf = '[';
        } else {
          emit(buf);
          buf = '';
        }
        return;
      }
      emit(buf.slice(0, open));
      buf = buf.slice(open);
      const close = buf.indexOf(']]');
      // A second '[[' before any ']]' means the first one was ordinary text.
      const nextOpen = buf.indexOf('[[', 2);
      if (nextOpen !== -1 && (close === -1 || nextOpen < close)) {
        emit('[[');
        buf = buf.slice(2);
        continue;
      }
      if (close === -1) {
        if (!final && buf.length <= POINT_HOLD_MAX) return; // wait for more
        // Not a directive after all; release the brackets and move on.
        emit('[[');
        buf = buf.slice(2);
        continue;
      }
      const candidate = buf.slice(0, close + 2);
      const m = candidate.match(POINT_RE);
      const named = m ? null : candidate.match(POINT_NAMED_RE);
      if (m) {
        out.points.push({ x: parseFloat(m[1]), y: parseFloat(m[2]), label: (m[3] || '').trim() });
        closeGap = true;
      } else if (named) {
        out.points.push({ x: null, y: null, label: named[1].trim() });
        closeGap = true;
      } else if (POINT_ANY_RE.test(candidate)) {
        // Meant as a directive, malformed enough that neither shape reads it.
        // Dropping it loses a pointer; emitting it shows the user brackets and
        // loses the pointer anyway.
        closeGap = true;
      } else {
        emit(candidate);
      }
      buf = buf.slice(close + 2);
    }
  }

  return out;
}

/**
 * Maps errors to messages that name the failure and the next action.
 * Returns null for aborts, which need no message.
 */
// Every message here names the failure and one next action. None of them says
// "press Enter to retry" any more: the input clears on send and an empty box
// sends the default question, so that instruction re-asked "what's on my
// screen" instead of the thing that failed. The error row carries a Retry
// button that re-sends the actual question.
function friendlyError(err, model) {
  if (!err) return 'Request failed for an unknown reason. Retry, or ask again.';
  if (err.name === 'AbortError') return null;
  const status = err.status;
  const detail = String(err.message || '');
  // OpenRouter refuses :free models until the account opts in, and reports
  // it with a data-policy message on various status codes.
  if (/data policy|free model publication/i.test(detail)) {
    return 'Your OpenRouter account blocks free models. At openrouter.ai open Settings, then Privacy, enable "Free model publication", and retry.';
  }
  if (status === 401) {
    return 'OpenRouter rejected the API key. Open Settings, check the key, and save it again.';
  }
  if (status === 402) {
    return `${model} bills per token and this account has no credit. Add a dollar at openrouter.ai/credits (it lasts thousands of questions at this rate), or open Settings and pick one from the Free group.`;
  }
  if (status === 403) {
    return `OpenRouter refused the request: ${detail}`;
  }
  if (status === 404) {
    return `${model} is not available right now, and the free model tried after it was not either. OpenRouter says: ${detail}. Pick another model in Settings.`;
  }
  if (status === 429) {
    return /:free$/.test(String(model || ''))
      ? 'Rate limited, and every free model this app knows about was busy too. Free models allow a few requests a minute; wait a moment and retry, or add credit and pick a paid model in Settings.'
      : 'Rate limited by the provider. Wait a moment, then retry.';
  }
  if (status === 400) {
    if (/image|vision|multimodal|modality/i.test(detail)) {
      return `The model ${model} does not accept images. Pick one from the Model list in Settings, or turn on "Use a separate brain" so a reader handles the screen for it.`;
    }
    return `Request rejected: ${detail}`;
  }
  if (status === 204) {
    return `The model ${model} streamed no answer text. Pick a different model in Settings.`;
  }
  if (status >= 500) {
    return 'OpenRouter returned a server error, on this model and on the fallback. Retry in a moment.';
  }
  if (err instanceof TypeError) {
    return 'Network request failed. Check your connection, then retry.';
  }
  return `Request failed: ${detail || err}. Retry, or ask again.`;
}

module.exports = {
  streamQuery,
  readScreen,
  shouldFailover,
  nextFreeModel,
  parseElements,
  matchElement,
  extractTask,
  friendlyError,
  listModels,
  testKey,
  createPointExtractor,
  makeSystemPrompt,
  DEFAULT_MODEL,
  DEFAULT_BRAIN,
};
