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
const DEFAULT_MODEL = 'google/gemma-4-26b-a4b-it:free';

// The optional second stage. A free vision model reads the screen, then a
// much stronger free text model does the thinking over that reading. See
// readScreen below for what that trades away.
const DEFAULT_BRAIN = 'z-ai/glm-5.2:free';

// Hand-picked, in the order they appear in settings. Every id is checked
// against the live list before it is offered, so a retired model disappears
// instead of failing at query time. Notes describe the model; the price and
// context line under each one is built from live data.
const CURATED = [
  {
    id: 'google/gemma-4-26b-a4b-it:free',
    tier: 'free',
    note: 'Only 4B parameters active per token, so it is the quickest of the free ones. The default.',
  },
  {
    id: 'nvidia/nemotron-nano-12b-v2-vl:free',
    tier: 'free',
    note: 'Built specifically for reading images. Good at dense interfaces and small text.',
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
    id: 'nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free',
    tier: 'free',
    note: 'Reasons before answering. Slowest here, best on multi-step problems.',
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
    id: 'z-ai/glm-5.2:free',
    tier: 'free',
    note: 'GLM 5.2. The strongest free brain on OpenRouter. The default.',
  },
  {
    id: 'nvidia/nemotron-3-ultra-550b-a55b:free',
    tier: 'free',
    note: '550B parameters, a million tokens of context. Heavy, and slow to start.',
  },
  {
    id: 'nvidia/nemotron-3-super-120b-a12b:free',
    tier: 'free',
    note: 'Most of Ultra at a fraction of the wait.',
  },
  {
    id: 'nvidia/nemotron-3.5-lightning:free',
    tier: 'free',
    note: 'Built for speed. Use it when the reading is already doing the hard part.',
  },
  { id: 'z-ai/glm-5.3', tier: 'paid', note: 'The paid GLM. Noticeably sharper than 5.2.' },
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
// only: GLM 5.2 cannot see an image, so the only way to put it behind this
// app is to hand it a reading. What it costs is honest to state. Two calls
// run in sequence, so the wait is the reader's full reply plus the brain's
// first token, and the brain only knows what the reader wrote down, so a
// detail the reader skipped is a detail the brain cannot use. The ELEMENTS
// block exists for exactly that reason: without coordinates in the reading,
// the brain could never point at anything.
const READER_PROMPT = [
  'You are the eyes of a desktop assistant. You cannot answer the question;',
  'another model does that from your notes. Report only what is on screen,',
  'in this exact shape and nothing else:',
  '',
  'SCREEN: the application and window, one line',
  'STATE: what is happening right now, one line',
  'TASK: exactly one of debug, explain, rewrite, assist. debug if something has',
  '  failed or is showing an error. explain if the user is looking at something',
  '  unfamiliar. rewrite if the screen is text they are composing. assist otherwise.',
  'TEXT: the text that matters, copied exactly, including any error message',
  'ELEMENTS:',
  '- label (x,y) for each control worth pointing at, at most 8, where x and y',
  '  are the centre of it in the pixel coordinates given below',
  '',
  'Copy error text and identifiers character for character. Write nothing',
  'that is not visible. No advice, no preamble, no markdown.',
].join('\n');

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

async function readScreen({
  apiKey,
  model,
  imageBase64,
  imageWidth,
  imageHeight,
  question,
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
      // Enough for a full reading, short enough that the brain is not kept
      // waiting on a rambling one.
      max_tokens: 900,
      temperature: 0,
      messages: [
        {
          role: 'system',
          content:
            `${READER_PROMPT}\n\nThe view is ${imageWidth}x${imageHeight} pixels; ` +
            'x=0 y=0 is its top-left corner.',
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
function makeSystemPrompt({ imageWidth, imageHeight, skill, viaReader }) {
  const lines = [
    'You are AevalBuddy, an assistant built into the user\'s Windows desktop.',
    'This is an ongoing conversation; the user may follow up on earlier answers.',
  ];
  if (viaReader) {
    // The brain is text only. It gets a reading instead of the screen, and
    // must not pretend to more certainty than the reading supports.
    lines.push(
      'You cannot see the screen yourself. A reader model looked at it and wrote',
      'the report below. Treat the report as what is on screen and speak to the',
      'user directly about their screen, never mentioning the report, a reader,',
      'a screenshot, or an image. If the report does not cover something you need,',
      'say what you would need to see rather than inventing it.',
      `The screen is ${imageWidth}x${imageHeight} pixels; x=0 y=0 is its top-left corner.`,
      'To point at a spot, add a directive on its own line:',
      '[[point x=123 y=456 | short label]], taking the coordinates from the',
      "report's ELEMENTS list. Use at most 3 per reply, and only when the location",
      'is the answer.'
    );
  } else if (imageWidth && imageHeight) {
    lines.push(
      `With this message you can see the user's screen as a ${imageWidth}x${imageHeight} pixel view,`,
      'where x=0 y=0 is its top-left corner and x grows right, y grows down.',
      'Talk about it as their screen ("on your screen", "the dialog in the corner").',
      'Never say screenshot, image, picture, or "provided"; you are looking at their screen, not a file.',
      'To physically point at a spot on their screen, add a directive on its own line:',
      '[[point x=123 y=456 | short label]], where x and y are the centre of the thing',
      'you mean, in the pixel coordinates of that view, and the label is a few words.',
      'Use at most 3 per reply, and only when the location is the answer',
      '(a button to press, a field to fix, a line to read).'
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
  const prompt = lines.join(' ');
  // The skill is a procedure, not a sentence, so it keeps its line breaks and
  // sits at the end where the last instruction carries most weight.
  if (skill && skill.body) {
    return `${prompt}\n\n--- ${skill.name || 'How to answer'} ---\n${skill.body}`;
  }
  return prompt;
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

// One [[point x=.. y=.. | label]] directive, tolerant about spacing and an
// optional comma between the coordinates.
const POINT_RE = /^\[\[\s*point\s+x\s*=\s*(\d+)\s*[, ]\s*y\s*=\s*(\d+)\s*(?:\|\s*([^\]]{0,140}?))?\s*\]\]$/i;

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
    if (!text) return;
    out.text += text;
    onText(text);
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
      if (m) {
        out.points.push({ x: Number(m[1]), y: Number(m[2]), label: (m[3] || '').trim() });
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
function friendlyError(err, model) {
  if (!err) return 'Request failed for an unknown reason. Press Enter to retry.';
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
    return `Model unavailable: ${model}. OpenRouter says: ${detail}. Pick another model in Settings.`;
  }
  if (status === 429) {
    return /:free$/.test(String(model || ''))
      ? 'Rate limited. Free models allow only a few requests per minute. Wait and retry, or pick a Fast model in Settings.'
      : 'Rate limited by the provider. Wait a moment, then press Enter to retry.';
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
    return 'OpenRouter returned a server error. Press Enter to retry.';
  }
  if (err instanceof TypeError) {
    return 'Network request failed. Check your connection, then press Enter to retry.';
  }
  return `Request failed: ${detail || err}. Press Enter to retry.`;
}

module.exports = {
  streamQuery,
  readScreen,
  extractTask,
  friendlyError,
  listModels,
  testKey,
  createPointExtractor,
  makeSystemPrompt,
  DEFAULT_MODEL,
  DEFAULT_BRAIN,
};
