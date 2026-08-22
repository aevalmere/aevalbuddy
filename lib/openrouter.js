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
async function listFreeVisionModels() {
  if (modelsCache.list && Date.now() - modelsCache.at < 5 * 60 * 1000) {
    return { models: modelsCache.list, live: true };
  }
  try {
    const res = await fetch(MODELS_ENDPOINT, { signal: AbortSignal.timeout(8000) });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();
    const models = (Array.isArray(data.data) ? data.data : [])
      .filter(
        (m) =>
          typeof m.id === 'string' &&
          m.id.endsWith(':free') &&
          m.architecture &&
          typeof m.architecture.modality === 'string' &&
          m.architecture.modality.includes('image')
      )
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

const SYSTEM_PROMPT = [
  'You are AevalBuddy, a small on-screen assistant. The user sends a screenshot',
  'of their screen plus a question. Answer briefly and concretely. Lead with the',
  'direct answer or the next step to take. Prefer short paragraphs and short',
  'lists. If the screenshot lacks the information needed, say what is missing.',
].join(' ');

class ApiError extends Error {
  constructor(status, message) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

/**
 * Streams a query to OpenRouter.
 *
 * @param {object} opts
 * @param {string} opts.apiKey
 * @param {string} opts.model OpenRouter model id
 * @param {string} opts.question
 * @param {string|null} opts.imageBase64 JPEG screenshot, base64 without prefix
 * @param {AbortSignal} opts.signal
 * @param {(text: string) => void} opts.onDelta
 * @returns {Promise<void>} resolves when the stream completes
 */
async function streamQuery({ apiKey, model, question, imageBase64, signal, onDelta }) {
  const content = [];
  if (imageBase64) {
    content.push({
      type: 'image_url',
      image_url: { url: `data:image/jpeg;base64,${imageBase64}` },
    });
  }
  content.push({ type: 'text', text: question });

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
      messages: [
        { role: 'system', content: SYSTEM_PROMPT },
        { role: 'user', content },
      ],
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

module.exports = { streamQuery, friendlyError, listFreeVisionModels, DEFAULT_MODEL };
