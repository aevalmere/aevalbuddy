'use strict';

// AEVAL_FAKE_LLM=1: every OpenRouter chat request is answered from fixtures
// in tools/fixtures/llm/*.jsonl instead of the network, so Find and the agent
// behave the same way on every run of the smoke test. The catalogue fetch is
// not faked; the smoke run checks the real one.
//
// One JSON object per line. `purpose` says which caller it answers:
//
//   {"purpose":"find-expand","query":"payment","lines":[{"t":"invoice","k":"related","why":"..."}]}
//   {"purpose":"find-correlate","query":"how much do i owe","match":"total due","score":0.9,"why":"..."}
//       answers with the id of whichever numbered span in the request holds `match`
//   {"purpose":"agent","task":"smoke notepad","turns":[{"call":{"name":"type","args":{...}}},{"text":"Done."}]}
//       turn N answers the request holding N assistant messages; with `tools`
//       in the request a turn is a native tool call, without them it is the
//       fenced JSON the fallback path parses
//   {"purpose":"reader"|"answer","text":"..."}
//
// Any field may hold {{name}}, filled from setVar() at answer time (a window
// handle the test only learns once Notepad is open). A turn may carry
// `delayMs` to stand in for a slow free model; the delay honours abort.
//
// Every request body is kept in `requests` (newest last, capped), which is
// how the smoke run proves a password never reached a model.

const fs = require('fs');
const path = require('path');

const DIR = path.join(__dirname, '..', 'tools', 'fixtures', 'llm');
const requests = [];
const vars = {};
let fixtures = null;

function enabled() {
  return !!process.env.AEVAL_FAKE_LLM;
}

function load() {
  if (fixtures) return fixtures;
  fixtures = [];
  let files = [];
  try {
    files = fs.readdirSync(DIR).filter((f) => f.endsWith('.jsonl'));
  } catch (err) {
    files = [];
  }
  for (const f of files) {
    const text = fs.readFileSync(path.join(DIR, f), 'utf8');
    for (const line of text.split(/\r?\n/)) {
      if (!line.trim()) continue;
      try {
        fixtures.push(JSON.parse(line));
      } catch (err) {
        // A broken fixture line is skipped; the assert that needed it fails.
      }
    }
  }
  return fixtures;
}

function setVar(name, value) {
  vars[name] = value;
}

function fill(v) {
  if (typeof v === 'string') return v.replace(/\{\{(\w+)\}\}/g, (m, k) => (k in vars ? String(vars[k]) : m));
  if (Array.isArray(v)) return v.map(fill);
  if (v && typeof v === 'object') {
    const out = {};
    for (const [k, x] of Object.entries(v)) out[k] = fill(x);
    return out;
  }
  return v;
}

function lower(s) {
  return String(s || '').toLowerCase().trim();
}

function textOf(content) {
  if (typeof content === 'string') return content;
  if (Array.isArray(content)) return content.map((c) => (c && c.type === 'text' ? c.text : '')).join('\n');
  return '';
}

function sleep(ms, signal) {
  return new Promise((resolve, reject) => {
    if (!ms) return resolve();
    const t = setTimeout(resolve, ms);
    if (signal) {
      const abort = () => {
        clearTimeout(t);
        const e = new Error('aborted');
        e.name = 'AbortError';
        reject(e);
      };
      if (signal.aborted) abort();
      else signal.addEventListener('abort', abort, { once: true });
    }
  });
}

function sse(chunks) {
  const parts = chunks.map((c) => `data: ${JSON.stringify({ choices: [{ delta: { content: c } }] })}\n\n`);
  parts.push('data: [DONE]\n\n');
  return new Response(parts.join(''), { status: 200, headers: { 'Content-Type': 'text/event-stream' } });
}

function json(obj) {
  return new Response(JSON.stringify(obj), { status: 200, headers: { 'Content-Type': 'application/json' } });
}

function message(body, msg) {
  if (body.stream) return sse([msg.content || '']);
  return json({ choices: [{ message: msg, finish_reason: msg.tool_calls ? 'tool_calls' : 'stop' }] });
}

// The JSON fallback path's shape: tool name plus its arguments, flat.
function asJsonTurn(call) {
  return `\`\`\`json\n${JSON.stringify({ tool: call.name, ...(call.args || {}) })}\n\`\`\``;
}

let callSeq = 0;

async function respond(body, signal, meta) {
  const purpose = (meta && meta.purpose) || 'answer';
  requests.push({ purpose, model: body.model, body: JSON.stringify(body) });
  if (requests.length > 400) requests.shift();
  const all = load();
  const msgs = body.messages || [];
  const userText = msgs.filter((m) => m.role === 'user').map((m) => textOf(m.content)).join('\n');

  if (purpose === 'find-expand') {
    const q = lower(meta.query);
    const f = all.find((x) => x.purpose === 'find-expand' && lower(x.query) === q);
    if (f && f.delayMs) await sleep(f.delayMs, signal);
    const lines = f ? fill(f.lines || []) : [];
    return sse(lines.map((l) => `${JSON.stringify(l)}\n`));
  }

  if (purpose === 'find-correlate') {
    const q = lower(meta.query);
    const out = [];
    for (const f of all.filter((x) => x.purpose === 'find-correlate' && lower(x.query) === q)) {
      for (const line of userText.split('\n')) {
        const m = line.match(/^\[(\d+)\]\s?(.*)$/);
        if (m && lower(m[2]).includes(lower(f.match))) {
          out.push(`${JSON.stringify({ id: Number(m[1]), score: f.score || 0.8, why: fill(f.why || '') })}\n`);
        }
      }
    }
    return sse(out);
  }

  if (purpose === 'agent') {
    const task = lower(meta.task);
    const f = all.find((x) => x.purpose === 'agent' && task.includes(lower(x.task)));
    const turn = msgs.filter((m) => m.role === 'assistant').length;
    const step = f ? fill((f.turns || [])[turn] || f.after || { text: 'Done.' }) : { text: 'No fixture for this task.' };
    if (step.delayMs) await sleep(step.delayMs, signal);
    if (step.status) {
      return new Response(JSON.stringify({ error: { message: step.message || 'fake failure' } }), { status: step.status });
    }
    if (step.call) {
      if (Array.isArray(body.tools) && body.tools.length) {
        callSeq += 1;
        return message(body, {
          role: 'assistant',
          content: step.say || '',
          tool_calls: [
            {
              id: `call_${callSeq}`,
              type: 'function',
              function: { name: step.call.name, arguments: JSON.stringify(step.call.args || {}) },
            },
          ],
        });
      }
      return message(body, { role: 'assistant', content: asJsonTurn(step.call) });
    }
    const text = step.text || 'Done.';
    if (Array.isArray(body.tools) && body.tools.length) return message(body, { role: 'assistant', content: text });
    return message(body, { role: 'assistant', content: `\`\`\`json\n${JSON.stringify({ done: text })}\n\`\`\`` });
  }

  const f = all.find((x) => x.purpose === purpose);
  const text = fill(
    f
      ? f.text
      : purpose === 'reader'
        ? 'SCREEN: Notepad\nSTATE: a text file is open\nFOCUS: NONE\nTASK: assist\nTEXT: NONE\nELEMENTS:\n'
        : 'This is the fake model answering.'
  );
  if (f && f.delayMs) await sleep(f.delayMs, signal);
  return message(body, { role: 'assistant', content: text });
}

module.exports = { enabled, respond, setVar, requests };
