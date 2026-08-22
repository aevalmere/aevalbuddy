'use strict';

// OpenRouter chat-completions client. Plain fetch plus SSE parsing; the
// OpenAI-compatible wire format needs no SDK. Runs in the main process only,
// so the API key never reaches a renderer.

const ENDPOINT = 'https://openrouter.ai/api/v1/chat/completions';
const MODELS_ENDPOINT = 'https://openrouter.ai/api/v1/models';
const KEY_ENDPOINT = 'https://openrouter.ai/api/v1/key';

// A screen assistant is judged on how fast the first words appear, so the
// default is the quickest capable vision model rather than the cheapest.
// It is not a ":free" id: the free tier is queued and rate limited, which is
// what made answers crawl. This one bills per token at a rate where a
// question costs a small fraction of a cent, and the Free group below stays
// available for accounts with no credit at all.
const DEFAULT_MODEL = 'qwen/qwen3.7-flash';

// Hand-picked, in the order they appear in settings. Every id is checked
// against the live list before it is offered, so a retired model disappears
// instead of failing at query time. Notes describe the model; the price and
// context line under each one is built from live data.
const CURATED = [
  {
    id: 'qwen/qwen3.7-flash',
    tier: 'recommended',
    note: 'Quick to start talking and reliable at reading a screen. The default.',
  },
  {
    id: 'deepseek/deepseek-v4-flash-vision-exp',
    tier: 'fast',
    note: 'DeepSeek, built for vision. Good on dense text and code.',
  },
  { id: 'z-ai/glm-4.6v', tier: 'fast', note: 'GLM. Strong at reading interfaces and dialogs.' },
  {
    id: 'bytedance-seed/seed-1.6-flash',
    tier: 'fast',
    note: 'Cheapest of the quick ones. Short answers.',
  },
  {
    id: 'qwen/qwen3-vl-8b-instruct',
    tier: 'fast',
    note: 'Small. Best for one-line questions about one thing.',
  },
  {
    id: 'z-ai/glm-5v-turbo',
    tier: 'smart',
    note: 'The most capable here. Slower to start, better at multi-step problems.',
  },
  {
    id: 'qwen/qwen3.8-max',
    tier: 'smart',
    note: 'Qwen Max. Long reasoning, highest cost of this list.',
  },
];

const TIER_LABEL = {
  recommended: 'Recommended',
  fast: 'Fast',
  smart: 'Smarter, slower',
  free: 'Free (slower, rate limited)',
};

// Shown when the live list cannot be fetched (offline, endpoint change), so
// the picker is never empty.
const FALLBACK_MODELS = CURATED.map((c) => ({
  id: c.id,
  name: c.id,
  note: c.note,
  tier: c.tier,
  detail: '',
}));

const MAX_FREE_LISTED = 8;

let modelsCache = { at: 0, groups: null };

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
 * The model catalogue for the settings picker, as ordered groups.
 * Curated ids are verified against the live list; the free group is whatever
 * OpenRouter currently gives away with image input.
 *
 * @returns {Promise<{groups: Array<{tier: string, label: string, models: Array}>, live: boolean}>}
 */
async function listModels() {
  if (modelsCache.groups && Date.now() - modelsCache.at < 5 * 60 * 1000) {
    return { groups: modelsCache.groups, live: true };
  }
  const offline = () => ({
    groups: groupBy(modelsCache.groups ? flatten(modelsCache.groups) : FALLBACK_MODELS),
    live: false,
  });
  try {
    const res = await fetch(MODELS_ENDPOINT, { signal: AbortSignal.timeout(8000) });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();
    const all = Array.isArray(data.data) ? data.data : [];
    const byId = new Map(all.map((m) => [m.id, m]));

    const picked = [];
    for (const c of CURATED) {
      const live = byId.get(c.id);
      if (!live || !acceptsImages(live)) continue;
      picked.push({
        id: c.id,
        name: live.name || c.id,
        note: c.note,
        tier: c.tier,
        detail: describe(live),
      });
    }
    const free = all
      .filter((m) => typeof m.id === 'string' && m.id.endsWith(':free') && acceptsImages(m))
      .slice(0, MAX_FREE_LISTED)
      .map((m) => ({
        id: m.id,
        name: m.name || m.id,
        note: '',
        tier: 'free',
        detail: formatContext(m.context_length),
      }));
    const models = picked.concat(free);
    if (!models.length) return offline();
    const groups = groupBy(models);
    modelsCache = { at: Date.now(), groups };
    return { groups, live: true };
  } catch (err) {
    return offline();
  }
}

function flatten(groups) {
  return groups.reduce((acc, g) => acc.concat(g.models), []);
}

function groupBy(models) {
  const order = ['recommended', 'fast', 'smart', 'free'];
  return order
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

// The assistant speaks as a helper built into the desktop, never as a bot
// examining an attachment: "on your screen", not "in the screenshot". The
// pointer directive lets it summon the on-screen pointer at a location.
function makeSystemPrompt({ imageWidth, imageHeight, skill }) {
  const lines = [
    'You are AevalBuddy, an assistant built into the user\'s Windows desktop.',
    'This is an ongoing conversation; the user may follow up on earlier answers.',
  ];
  if (imageWidth && imageHeight) {
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
  signal,
  onDelta,
}) {
  const content = [];
  if (imageBase64) {
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
        imageWidth: imageBase64 ? imageWidth : 0,
        imageHeight: imageBase64 ? imageHeight : 0,
        skill,
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
      return `The model ${model} does not accept images. Pick a vision-capable model in Settings.`;
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
  friendlyError,
  listModels,
  testKey,
  createPointExtractor,
  makeSystemPrompt,
  DEFAULT_MODEL,
};
