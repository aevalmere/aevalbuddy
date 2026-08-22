'use strict';

// OpenRouter chat-completions client. Plain fetch plus SSE parsing; the
// OpenAI-compatible wire format needs no SDK. Runs in the main process only,
// so the API key never reaches a renderer.

const ENDPOINT = 'https://openrouter.ai/api/v1/chat/completions';
const MODELS_ENDPOINT = 'https://openrouter.ai/api/v1/models';

// Picked from the live model list because it is free and its modality is
// exactly image plus text. Free listings rotate over time; settings offers a
// live picker and a free-text id, so a vanished default is a settings
// change, not a code change.
const DEFAULT_MODEL = 'dots-studio/dots-3-note-preview:free';

// Shown when the live list cannot be fetched (offline, endpoint change).
const FALLBACK_FREE_VISION_MODELS = [
  { id: 'dots-studio/dots-3-note-preview:free', name: 'Dots Studio: Dots3-Note Preview (free)' },
  { id: 'thinkingmachines/inkling-small:free', name: 'Thinking Machines: Inkling Small (free)' },
  { id: 'thinkingmachines/inkling:free', name: 'Thinking Machines: Inkling (free)' },
];

let modelsCache = { at: 0, list: null };

/**
 * Lists current OpenRouter models that are free and accept image input.
 * Falls back to a static list when the network or endpoint fails, so the
 * settings picker always renders something.
 */
// OpenRouter has shipped two shapes for model modality over time; accept
// either so a schema shift degrades to the fallback list instead of an
// empty picker.
function acceptsImages(m) {
  const a = m.architecture;
  if (!a) return false;
  if (typeof a.modality === 'string' && a.modality.includes('image')) return true;
  return Array.isArray(a.input_modalities) && a.input_modalities.includes('image');
}

async function listFreeVisionModels() {
  if (modelsCache.list && Date.now() - modelsCache.at < 5 * 60 * 1000) {
    return { models: modelsCache.list, live: true };
  }
  try {
    const res = await fetch(MODELS_ENDPOINT, { signal: AbortSignal.timeout(8000) });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();
    const models = (Array.isArray(data.data) ? data.data : [])
      .filter((m) => typeof m.id === 'string' && m.id.endsWith(':free') && acceptsImages(m))
      .map((m) => ({ id: m.id, name: m.name || m.id }));
    if (models.length) {
      modelsCache = { at: Date.now(), list: models };
      return { models, live: true };
    }
    return { models: FALLBACK_FREE_VISION_MODELS, live: false };
  } catch (err) {
    return { models: modelsCache.list || FALLBACK_FREE_VISION_MODELS, live: false };
  }
}

// The assistant speaks as a helper built into the desktop, never as a bot
// examining an attachment: "on your screen", not "in the screenshot". The
// pointer directive lets it summon the on-screen pointer at a location.
function makeSystemPrompt({ imageWidth, imageHeight }) {
  const lines = [
    'You are AevalBuddy, an assistant built into the user\'s Windows desktop.',
    'This is an ongoing conversation; the user may follow up on earlier answers.',
  ];
  if (imageWidth && imageHeight) {
    lines.push(
      `With this message you can see the user's screen as a ${imageWidth}x${imageHeight} pixel view.`,
      'Talk about it as their screen ("on your screen", "the dialog in the corner").',
      'Never say screenshot, image, picture, or "provided"; you are looking at their screen, not a file.',
      'To physically point at a spot on their screen, add a directive on its own:',
      '[[point x=123 y=456 | short label]] with pixel coordinates from the screen view',
      'and a label of a few words. Use at most 3 per reply, and only when showing',
      'the location genuinely helps (a button to press, a field to check).'
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
  return lines.join(' ');
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
    return 'OpenRouter reports payment required. Check your balance at openrouter.ai, or switch to a :free model in Settings.';
  }
  if (status === 403) {
    return `OpenRouter refused the request: ${detail}`;
  }
  if (status === 404) {
    return `Model unavailable: ${model}. OpenRouter says: ${detail}. Pick another model in Settings.`;
  }
  if (status === 429) {
    return 'Rate limited. Free models allow a few requests per minute; wait, then press Enter to retry.';
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
  listFreeVisionModels,
  createPointExtractor,
  DEFAULT_MODEL,
};
