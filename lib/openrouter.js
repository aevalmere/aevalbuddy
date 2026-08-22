'use strict';

// OpenRouter chat-completions client. Plain fetch plus SSE parsing; the
// OpenAI-compatible wire format needs no SDK. Runs in the main process only,
// so the API key never reaches a renderer.

const ENDPOINT = 'https://openrouter.ai/api/v1/chat/completions';

// Picked from the live model list because it is free and accepts image
// input. Free listings rotate over time; the model id is editable in
// settings, so a vanished default is a settings change, not a code change.
const DEFAULT_MODEL = 'thinkingmachines/inkling:free';

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
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    let newline;
    while ((newline = buffer.indexOf('\n')) >= 0) {
      const line = buffer.slice(0, newline).trim();
      buffer = buffer.slice(newline + 1);
      // Skips SSE comments such as ": OPENROUTER PROCESSING" keepalives.
      if (!line.startsWith('data:')) continue;
      const payload = line.slice(5).trim();
      if (payload === '[DONE]') return;
      let obj;
      try {
        obj = JSON.parse(payload);
      } catch (parseErr) {
        continue;
      }
      if (obj.error) {
        throw new ApiError(obj.error.code || 500, obj.error.message || 'stream error');
      }
      const delta =
        obj.choices && obj.choices[0] && obj.choices[0].delta
          ? obj.choices[0].delta.content
          : null;
      if (delta) onDelta(delta);
    }
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
  if (status === 401) {
    return 'OpenRouter rejected the API key. Open Settings, check the key, and save it again.';
  }
  if (status === 402) {
    return 'OpenRouter reports payment required. Check your balance at openrouter.ai, or switch to a :free model in Settings.';
  }
  if (status === 403) {
    return `OpenRouter refused the request: ${err.message}`;
  }
  if (status === 404) {
    return `Model not found: ${model}. Fix the model id in Settings; free ids change over time.`;
  }
  if (status === 429) {
    return 'Rate limited. Free models allow a few requests per minute; wait, then press Enter to retry.';
  }
  if (status === 400) {
    if (/image|vision|multimodal|modality/i.test(String(err.message))) {
      return `The model ${model} does not accept images. Pick a vision-capable model in Settings.`;
    }
    return `Request rejected: ${err.message}`;
  }
  if (status >= 500) {
    return 'OpenRouter returned a server error. Press Enter to retry.';
  }
  if (err instanceof TypeError) {
    return 'Network request failed. Check your connection, then press Enter to retry.';
  }
  return `Request failed: ${err.message || err}. Press Enter to retry.`;
}

module.exports = { streamQuery, friendlyError, DEFAULT_MODEL };
