// Turns the [BENCH] lines of tools/voice-bench/now and next into one table.
//
//   node tools/voice-bench/report.mjs <out.md> <log> [<log> ...]
//
// For each clip: every engine and model, its file size, the time of the first
// pass (model warm from loading, nothing cached), the median of the passes
// after it, how many times faster than the speech that is, and what it heard.
// Where the words are known (REFS), the word error rate is beside it. Two
// clips are far too few to rank accuracy; the published LibriSpeech figures
// are in the engines' own model cards.
import fs from 'node:fs';

const [out, ...logs] = process.argv.slice(2);
if (!out || !logs.length) {
  console.error('usage: node tools/voice-bench/report.mjs <out.md> <log> [<log> ...]');
  process.exit(2);
}

// What is said in the clips whose words are known.
const REFS = {
  dictation: 'um so i think we should meet at three no four pm tomorrow to go over the budget',
  jfk: 'and so my fellow americans ask not what your country can do for you ask what you can do for your country',
};
// Clips with no speech: anything heard is made up.
const SILENT = new Set(['noise', 'jobs-silence']);

const NUMBERS = { 3: 'three', 4: 'four' };
function words(text) {
  return text
    .toLowerCase()
    .replace(/\bp\.\s?m\.?/g, 'pm')
    .replace(/(\d)\s*pm\b/g, '$1 pm')
    .replace(/[^a-z0-9' ]+/g, ' ')
    .split(/\s+/)
    .filter(Boolean)
    .map((w) => NUMBERS[w] || w);
}

function distance(a, b) {
  let prev = b.map((_, j) => j + 1);
  prev.unshift(0);
  for (let i = 1; i <= a.length; i++) {
    const row = [i];
    for (let j = 1; j <= b.length; j++) {
      row[j] = Math.min(prev[j] + 1, row[j - 1] + 1, prev[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
    }
    prev = row;
  }
  return prev[b.length];
}

function unquote(s) {
  try {
    return JSON.parse(s.replace(/\\u\{([0-9a-f]+)\}/gi, (_, h) => String.fromCodePoint(parseInt(h, 16))).replace(/\\'/g, "'"));
  } catch {
    return s.replace(/^"|"$/g, '');
  }
}

const runs = [];
const notes = [];
for (const log of logs) {
  if (!fs.existsSync(log)) {
    notes.push(`missing: ${log}`);
    continue;
  }
  for (const line of fs.readFileSync(log, 'utf8').split(/\r?\n/)) {
    const m = line.match(/^\[BENCH\] (\w+) (.*)$/);
    if (!m) continue;
    if (m[1] !== 'run') {
      notes.push(`${m[1]}: ${m[2]}`);
      continue;
    }
    const text = m[2].match(/ text=(".*)$/);
    const head = text ? m[2].slice(0, text.index) : m[2];
    const f = Object.fromEntries(head.split(' ').map((kv) => kv.split(/=(.*)/s).slice(0, 2)));
    const times = (f.runs_ms || '').split(',').map(Number).filter((n) => !Number.isNaN(n)).sort((a, b) => a - b);
    runs.push({
      engine: f.engine,
      model: f.model,
      mb: Number(f.mb),
      clip: f.clip,
      audio: Number(f.audio_s),
      load: Number(f.load_ms),
      first: Number(f.first_ms),
      median: times.length ? times[Math.floor(times.length / 2)] : NaN,
      stages: f.stages || '',
      text: text ? unquote(text[1]) : '',
    });
  }
}

const cell = (s) => String(s).replace(/\|/g, '\\|').replace(/\r?\n/g, ' ');
const lines = ['# Speech engines, side by side', ''];
lines.push('`now` is the app today (whisper.cpp through whisper-rs, with its settings). `next` is transcribe.cpp on the CPU.');
lines.push('Times are milliseconds on this runner; "x" is seconds of speech per second of work.', '');

const clips = [...new Set(runs.map((r) => r.clip))];
for (const clip of clips) {
  const rows = runs.filter((r) => r.clip === clip).sort((a, b) => a.median - b.median);
  const ref = REFS[clip] && words(REFS[clip]);
  lines.push(`## ${clip} (${rows[0].audio.toFixed(1)} s)`, '');
  lines.push(`| Engine | Model | MB | First | Median | x | ${ref ? 'Word errors' : SILENT.has(clip) ? 'Made-up words' : ''} | Heard |`);
  lines.push('|---|---|---:|---:|---:|---:|---:|---|');
  for (const r of rows) {
    let score = '';
    if (ref) {
      const d = distance(ref, words(r.text));
      score = `${d} of ${ref.length} (${((100 * d) / ref.length).toFixed(0)}%)`;
    } else if (SILENT.has(clip)) {
      score = String(words(r.text).length);
    }
    const x = r.median > 0 ? ((r.audio * 1000) / r.median).toFixed(0) : '';
    lines.push(`| ${r.engine} | ${cell(r.model)} | ${r.mb} | ${r.first} | ${r.median} | ${x} | ${score} | ${cell(r.text)} |`);
  }
  lines.push('');
}

lines.push('## Loading a model', '', '| Engine | Model | MB | Load ms |', '|---|---|---:|---:|');
const seen = new Set();
for (const r of runs) {
  const key = `${r.engine} ${r.model}`;
  if (seen.has(key)) continue;
  seen.add(key);
  lines.push(`| ${r.engine} | ${cell(r.model)} | ${r.mb} | ${r.load} |`);
}
lines.push('', '## Stages (next only, last pass)', '', '| Model | Clip | mel, encode, decode ms |', '|---|---|---|');
for (const r of runs.filter((r) => r.stages)) lines.push(`| ${cell(r.model)} | ${r.clip} | ${r.stages.replace(/,/g, ', ')} |`);
lines.push('', '## Notes', '', ...notes.map((n) => `- ${cell(n)}`), '');

fs.writeFileSync(out, lines.join('\n'));
console.log(lines.join('\n'));
console.log(`[BENCH] report runs=${runs.length} clips=${clips.length}`);
if (!runs.length) process.exit(1);
