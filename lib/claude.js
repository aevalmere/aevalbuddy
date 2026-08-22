'use strict';

const Anthropic = require('@anthropic-ai/sdk');

const MODEL = 'claude-sonnet-4-6';
const MAX_TOKENS = 2048;

const SYSTEM_PROMPT = [
  'You are AevalBuddy, a small on-screen assistant. The user sends a screenshot',
  'of their screen plus a question. Answer briefly and concretely. Lead with the',
  'direct answer or the next step to take. Prefer short paragraphs and short',
  'lists. If the screenshot lacks the information needed, say what is missing.',
].join(' ');

/**
 * Streams a vision query to Claude. Runs in the main process only, so the
 * API key never reaches a renderer.
 *
 * @param {object} opts
 * @param {string} opts.apiKey
 * @param {string} opts.question
 * @param {string|null} opts.imageBase64 JPEG screenshot, base64 without prefix
 * @param {AbortSignal} opts.signal
 * @param {(text: string) => void} opts.onDelta
 * @returns {Promise<void>} resolves when the stream completes
 */
async function streamQuery({ apiKey, question, imageBase64, signal, onDelta }) {
  const client = new Anthropic({ apiKey, maxRetries: 1 });

  const content = [];
  if (imageBase64) {
    content.push({
      type: 'image',
      source: { type: 'base64', media_type: 'image/jpeg', data: imageBase64 },
    });
  }
  content.push({ type: 'text', text: question });

  const stream = client.messages.stream(
    {
      model: MODEL,
      max_tokens: MAX_TOKENS,
      system: SYSTEM_PROMPT,
      messages: [{ role: 'user', content }],
    },
    { signal }
  );

  stream.on('text', (delta) => onDelta(delta));
  await stream.finalMessage();
}

/**
 * Maps SDK errors to messages that name the failure and the next action.
 * Returns null for aborts, which need no message.
 */
function friendlyError(err) {
  if (!err) return 'Request failed for an unknown reason. Press Enter to retry.';
  if (err.name === 'AbortError' || err instanceof Anthropic.APIUserAbortError) {
    return null;
  }
  if (err instanceof Anthropic.AuthenticationError) {
    return 'API key rejected. Open Settings, check the key, and save it again.';
  }
  if (err instanceof Anthropic.PermissionDeniedError) {
    return 'API key lacks permission for this model. Check your Anthropic plan.';
  }
  if (err instanceof Anthropic.RateLimitError) {
    return 'Rate limited by the API. Wait a moment, then press Enter to retry.';
  }
  if (err instanceof Anthropic.BadRequestError) {
    return `Request rejected: ${err.message}`;
  }
  if (err instanceof Anthropic.APIConnectionError) {
    return 'Network request failed. Check your connection, then press Enter to retry.';
  }
  if (err instanceof Anthropic.APIError) {
    if (err.status >= 500) {
      return 'The API returned a server error. Press Enter to retry.';
    }
    return `API error ${err.status}: ${err.message}`;
  }
  return `Request failed: ${err.message || err}. Press Enter to retry.`;
}

module.exports = { streamQuery, friendlyError, MODEL };
