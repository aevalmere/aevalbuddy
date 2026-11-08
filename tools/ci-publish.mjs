// Publishes a workflow run's results to the orphan branch `ci-logs`, so an
// agent learns whether its push passed, and why not, with `git fetch` alone
// and no GitHub API call (tools/ci-wait.mjs waits for them). It is the last
// step of rust.yml and ui-shots.yml, run with `if: always()`:
//
//   <ref>/<sha>/<workflow>/summary.md   run link, conclusion, every step's outcome
//   <ref>/<sha>/<workflow>/meta.json    the same, for tools
//   <ref>/<sha>/<workflow>/errors.txt   error and warning lines from every log
//   <ref>/<sha>/<workflow>/*.log        one per gate (ANSI and CR stripped,
//                                       huge ones cut in the middle)
//   <ref>/latest/<workflow>/            the newest run's copy
//   <ref>/shas.txt                      the sha folders kept, newest first
//
// <ref> is the branch with / turned into -. Each attempt lays the files on
// the branch's current tip, so concurrent publishers never conflict: a
// rejected push fetches the new tip and tries again (8 attempts, jittered).
// Never forced. The newest 30 sha folders per ref are kept, and the folder of
// a ref nobody published to for 45 days is dropped.
//
// Environment: CI_WORKFLOW (folder name), CI_STATUS (job.status), CI_STEPS
// (one "Step name|outcome|log file" line per step, in order), CI_TOKEN (a
// token with contents: write), optional CI_LOGS (default $RUNNER_TEMP/ci-logs),
// CI_NOTE (one more line for the summary) and CI_REMOTE (a different remote,
// for trying this out), plus the runner's GITHUB_* variables.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const BRANCH = 'ci-logs';
const KEEP = 30;
const STALE_DAYS = 45;
const ATTEMPTS = 8;
const MAX_LINES = 6000; // per log: the first 200 and the last 5800 are kept
const ERR_LINES = 300; // per log, in errors.txt

const env = process.env;
function need(name) {
  if (!env[name]) {
    console.log(`::error title=ci-logs::${name} is not set`);
    process.exit(1);
  }
  return env[name];
}

const workflow = need('CI_WORKFLOW');
const sha = need('GITHUB_SHA');
const ref = need('GITHUB_REF_NAME');
const repo = need('GITHUB_REPOSITORY');
const runId = need('GITHUB_RUN_ID');
const attempt = env.GITHUB_RUN_ATTEMPT || '1';
const runUrl = `${env.GITHUB_SERVER_URL || 'https://github.com'}/${repo}/actions/runs/${runId}`;
const temp = env.RUNNER_TEMP || os.tmpdir();
const logsDir = env.CI_LOGS || path.join(temp, 'ci-logs');
const conclusion = env.CI_STATUS || 'unknown';
const remote = env.CI_REMOTE || `https://x-access-token:${need('CI_TOKEN')}@github.com/${repo}.git`;
const refDir = ref.replace(/\//g, '-');
const isSha = (s) => /^[0-9a-f]{40}$/.test(s);
const hide = (s) => String(s).replace(/x-access-token:[^@\s]+@/g, 'x-access-token:***@');

// --- The run's folder, in memory ---------------------------------------

const ANSI = /\x1b\[[0-?]*[ -/]*[@-~]/g;
// A rustc/clippy diagnostic, a rustfmt diff or a panic runs until the next
// empty line (30 lines at most).
const BLOCK = /^(error|warning)(\[[\w-]+\])?: |^Diff in |panicked at /;
const LINE =
  /^test .+ \.\.\. FAILED$|^test result: FAILED|=FAIL|\] pageerror|\] failed: |^\s*\w*Error\b[: ]|^npm (ERR!|error)|timed out/;

function readLog(file) {
  const buf = fs.readFileSync(file);
  const text = buf[0] === 0xff && buf[1] === 0xfe ? buf.toString('utf16le') : buf.toString('utf8');
  let lines = text.replace(/^﻿/, '').replace(ANSI, '').replace(/\r\n?/g, '\n').split('\n');
  if (lines.at(-1) === '') lines.pop();
  if (lines.length > MAX_LINES) {
    const cut = lines.length - MAX_LINES;
    lines = [...lines.slice(0, 200), `[ci-logs: ${cut} lines cut]`, ...lines.slice(-(MAX_LINES - 200))];
  }
  return lines;
}

function errorLines(lines) {
  const out = [];
  for (let i = 0; i < lines.length && out.length < ERR_LINES; i++) {
    if (BLOCK.test(lines[i])) {
      let j = i;
      while (j < lines.length && lines[j] !== '' && j - i < 30) out.push(lines[j++]);
      out.push('');
      i = j - 1;
    } else if (LINE.test(lines[i])) {
      out.push(lines[i]);
    }
  }
  return out;
}

const steps = (env.CI_STEPS || '')
  .split(/\r?\n/)
  .map((l) => l.trim())
  .filter(Boolean)
  .map((l) => {
    const [name = '', outcome = '', log = ''] = l.split('|').map((s) => s.trim());
    // An empty file (fmt.patch when rustfmt had nothing to change) is no log.
    const file = path.join(logsDir, log);
    const has = log && fs.existsSync(file) && fs.statSync(file).size > 0;
    return { name, outcome: outcome || 'not run', log: has ? log : '' };
  });

const files = new Map();
const errors = [];
const present = fs.existsSync(logsDir)
  ? fs.readdirSync(logsDir).filter((f) => fs.statSync(path.join(logsDir, f)).isFile()).sort()
  : [];
for (const name of new Set([...steps.map((s) => s.log).filter(Boolean), ...present])) {
  const file = path.join(logsDir, name);
  if (!name.endsWith('.log')) {
    files.set(name, fs.readFileSync(file));
    continue;
  }
  const lines = readLog(file);
  files.set(name, lines.length ? `${lines.join('\n')}\n` : '');
  const errs = errorLines(lines);
  if (errs.length) errors.push(`== ${name} ==`, ...errs, '');
}
const errorCount = errors.filter((l) => l && !l.startsWith('== ')).length;
files.set('errors.txt', errors.join('\n'));

const failed = steps.filter((s) => s.outcome === 'failure').map((s) => s.name);
const finished = new Date().toISOString().replace(/\.\d+Z$/, 'Z');
const meta = {
  workflow,
  workflow_name: env.GITHUB_WORKFLOW || workflow,
  run_id: Number(runId),
  run_attempt: Number(attempt),
  run_url: runUrl,
  event: env.GITHUB_EVENT_NAME || '',
  sha,
  ref,
  ref_dir: refDir,
  conclusion,
  failed,
  finished,
  error_lines: errorCount,
  steps,
};
files.set('meta.json', `${JSON.stringify(meta, null, 2)}\n`);

const cell = (s) => String(s).replace(/\|/g, '\\|');
files.set(
  'summary.md',
  [
    `# ${workflow}: ${conclusion}`,
    '',
    `- Workflow: ${meta.workflow_name}`,
    `- Run: [${runId}](${runUrl}), attempt ${attempt}${meta.event ? `, on ${meta.event}` : ''}`,
    `- Commit: ${sha}`,
    `- Ref: ${ref}`,
    `- Conclusion: **${conclusion}**`,
    ...(failed.length ? [`- Failed: ${failed.join(', ')}`] : []),
    `- Finished: ${finished}`,
    ...(env.CI_NOTE ? [`- ${env.CI_NOTE}`] : []),
    '',
    '| Step | Outcome | Log |',
    '|---|---|---|',
    ...steps.map((s) => `| ${cell(s.name)} | ${s.outcome} | ${s.log ? `[${s.log}](${s.log})` : ''} |`),
    '',
    `errors.txt: ${errorCount} error and warning lines from the logs.`,
    '',
  ].join('\n'),
);

const README = `# ci-logs

Results of every CI run on rust-rewrite and rust/** (workflows rust and
ui-shots), written by tools/ci-publish.mjs at the end of each run, pass or
fail. Wait for a push's results with git alone:

    node tools/ci-wait.mjs <sha> [rust|ui-shots] [--timeout-min 30]

    <ref>/<sha>/<workflow>/summary.md   run link, conclusion, every step's outcome
    <ref>/<sha>/<workflow>/errors.txt   error and warning lines from the logs
    <ref>/<sha>/<workflow>/*.log        one per gate
    <ref>/latest/<workflow>/            the newest run
    <ref>/shas.txt                      the sha folders kept, newest first

<ref> is the branch name with / turned into -.
`;

// --- Laying it on the branch -------------------------------------------

function readJson(file) {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch {
    return null;
  }
}

function lay(root) {
  const refRoot = path.join(root, refDir);
  const put = (dir) => {
    fs.rmSync(dir, { recursive: true, force: true });
    fs.mkdirSync(dir, { recursive: true });
    for (const [name, body] of files) fs.writeFileSync(path.join(dir, name), body);
  };
  put(path.join(refRoot, sha, workflow));
  // latest/ follows the newest run: run ids grow, so an older run that
  // finishes late (a cancelled one) leaves a newer run's copy alone.
  const latest = path.join(refRoot, 'latest', workflow);
  const prev = readJson(path.join(latest, 'meta.json'));
  if (!prev || !(Number(prev.run_id) > Number(runId))) put(latest);

  const index = path.join(refRoot, 'shas.txt');
  const indexed = fs.existsSync(index) ? fs.readFileSync(index, 'utf8').split('\n') : [];
  const order = [...new Set([sha, ...indexed.filter(isSha), ...fs.readdirSync(refRoot).filter(isSha)])];
  for (const old of order.slice(KEEP)) fs.rmSync(path.join(refRoot, old), { recursive: true, force: true });
  const kept = order.slice(0, KEEP).filter((s) => fs.existsSync(path.join(refRoot, s)));
  fs.writeFileSync(index, `${kept.join('\n')}\n`);

  for (const d of fs.readdirSync(root)) {
    const latestRoot = path.join(root, d, 'latest');
    if (d.startsWith('.') || d === refDir || !fs.existsSync(latestRoot)) continue;
    const times = fs
      .readdirSync(latestRoot)
      .map((w) => Date.parse(readJson(path.join(latestRoot, w, 'meta.json'))?.finished || ''))
      .filter((t) => t > 0);
    if (times.length && Date.now() - Math.max(...times) > STALE_DAYS * 864e5) {
      fs.rmSync(path.join(root, d), { recursive: true, force: true });
    }
  }
  fs.writeFileSync(path.join(root, 'README.md'), README);
}

const work = fs.mkdtempSync(path.join(temp, 'ci-logs-git-'));
function git(args, check = true) {
  const r = spawnSync('git', args, { cwd: work, encoding: 'utf8', maxBuffer: 1 << 28 });
  if (check && r.status !== 0) throw new Error(`git ${args[0]}: ${hide(r.stderr || r.error || '').trim()}`);
  return r;
}
const sleep = (ms) => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);

git(['init', '--quiet']);
git(['config', 'user.name', 'github-actions[bot]']);
git(['config', 'user.email', '41898282+github-actions[bot]@users.noreply.github.com']);
git(['config', 'core.autocrlf', 'false']);
git(['config', 'core.longpaths', 'true']);

const where = `${refDir}/${sha}/${workflow}`;
for (let n = 1; n <= ATTEMPTS; n++) {
  try {
    const tip = git(['ls-remote', remote, `refs/heads/${BRANCH}`]).stdout.trim();
    if (tip) {
      git(['fetch', '--quiet', '--depth=1', remote, `refs/heads/${BRANCH}`]);
      git(['checkout', '--quiet', '--force', '-B', BRANCH, 'FETCH_HEAD']);
    } else {
      // The first publish ever: an orphan branch. A rival that creates it
      // first wins the push, and this one retries on top of it.
      git(['symbolic-ref', 'HEAD', `refs/heads/${BRANCH}`]);
      git(['update-ref', '-d', `refs/heads/${BRANCH}`], false);
      git(['read-tree', '--empty']);
    }
    git(['clean', '-fdxq']);
    lay(work);
    git(['add', '-A']);
    git(['commit', '--quiet', '--allow-empty', '-m', `${workflow} ${conclusion}: ${sha.slice(0, 7)} on ${ref}, run ${runId}`]);
    const push = git(['push', '--quiet', remote, `HEAD:refs/heads/${BRANCH}`], false);
    if (push.status === 0) {
      console.log(`published ${where} to ${BRANCH} (attempt ${n}): ${conclusion}`);
      process.exit(0);
    }
    const why = push.stderr.split('\n').find((l) => /rejected|fatal|error/.test(l)) || push.stderr;
    console.log(`attempt ${n}: push refused, retrying on the new tip: ${hide(why).trim()}`);
  } catch (e) {
    console.log(`attempt ${n}: ${e.message}`);
  }
  if (n < ATTEMPTS) sleep(1000 + Math.random() * 2500 * n);
}
console.log(`::error title=ci-logs::could not publish ${where} after ${ATTEMPTS} attempts`);
process.exit(1);
