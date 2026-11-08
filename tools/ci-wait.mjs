// Waits for a CI run's results on the ci-logs branch (written by
// tools/ci-publish.mjs) with `git fetch` alone. Nothing here calls the GitHub
// API: unauthenticated, it allows 60 requests an hour for this whole machine,
// shared by every agent on it.
//
//   node tools/ci-wait.mjs <sha> [workflow=rust] [--timeout-min 30] [--ref <branch>]
//
// <sha> is a full or short sha, or anything `git rev-parse` takes (HEAD).
// The ref is found by scanning ci-logs for the sha; --ref only steers the
// cancellation hint. Fetches origin ci-logs every 45 s until
// <ref>/<sha>/<workflow>/summary.md exists, then prints it and, on a failure,
// errors.txt and the first failing gate's log tail.
//
// Exit 0: success. 1: failure. 2: no verdict, either timed out or the run was
// cancelled. A newer push to the same ref cancels the older run, which may
// then never publish; the hint names the newer sha to wait on instead.
import { spawnSync } from 'node:child_process';

const POLL_S = 45;
const TAIL = 80;
const ERR_MAX = 300;

function usage(code) {
  const text = 'usage: node tools/ci-wait.mjs <sha> [workflow=rust] [--timeout-min 30] [--ref <branch>]';
  if (code) console.error(text);
  else console.log(text);
  process.exit(code);
}

let shaArg = '';
let workflow = 'rust';
let timeoutMin = 30;
let refArg = '';
const argv = process.argv.slice(2);
for (let i = 0; i < argv.length; i++) {
  const a = argv[i];
  const eq = a.startsWith('--') ? a.indexOf('=') : -1;
  const flag = eq > 0 ? a.slice(0, eq) : a;
  const inline = eq > 0 ? a.slice(eq + 1) : undefined;
  if (flag === '--timeout-min') timeoutMin = Number(inline ?? argv[++i]);
  else if (flag === '--ref') refArg = inline ?? argv[++i] ?? '';
  else if (flag === '-h' || flag === '--help') usage(0);
  else if (a.startsWith('-')) usage(2);
  else if (!shaArg) shaArg = a;
  else workflow = a.replace(/^workflow=/, '');
}
if (!shaArg || !(timeoutMin >= 0)) usage(2);

const git = (args) => spawnSync('git', args, { encoding: 'utf8', maxBuffer: 1 << 28 });
const blob = (p) => {
  const r = git(['cat-file', 'blob', `origin/ci-logs:${p}`]);
  return r.status === 0 ? r.stdout : '';
};
const json = (p) => {
  try {
    return JSON.parse(blob(p));
  } catch {
    return null;
  }
};
const say = (s) => process.stderr.write(`[ci-wait] ${s}\n`);
const short = (s) => String(s).slice(0, 10);
const toDir = (r) => r.replace(/\//g, '-');
const full = (s) => /^[0-9a-f]{40}$/.test(s);

const sha = (() => {
  if (/^[0-9a-f]{40}$/i.test(shaArg)) return shaArg.toLowerCase();
  const r = git(['rev-parse', '--verify', '--quiet', `${shaArg}^{commit}`]);
  if (r.status === 0) return r.stdout.trim();
  if (/^[0-9a-f]{7,39}$/i.test(shaArg)) return shaArg.toLowerCase(); // a prefix of a folder name
  console.error(`ci-wait: ${shaArg} is neither a sha nor a commit git knows here`);
  process.exit(2);
})();

// The branches this sha was pushed to, from the remote-tracking refs.
const holders = (() => {
  if (refArg) return [refArg];
  if (!full(sha)) return [];
  const r = git(['for-each-ref', '--contains', sha, '--format=%(refname:lstrip=3)', 'refs/remotes/origin/']);
  if (r.status !== 0) return [];
  return r.stdout
    .split('\n')
    .map((s) => s.trim())
    .filter((s) => s && !['HEAD', 'ci-logs', 'shots'].includes(s));
})();

let lastFetchError = '';
function fetchLogs() {
  const r = git(['fetch', '--quiet', 'origin', '+refs/heads/ci-logs:refs/remotes/origin/ci-logs']);
  const err = r.status === 0 ? '' : r.stderr.trim();
  if (err && err !== lastFetchError) {
    say(/couldn't find remote ref/i.test(err) ? 'origin has no ci-logs branch yet' : `fetch failed: ${err}`);
  }
  lastFetchError = err;
}

function findRun() {
  const r = git(['ls-tree', '-r', '--name-only', 'origin/ci-logs']);
  if (r.status !== 0) return null;
  const wf = workflow.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const re = new RegExp(`^([^/]+)/(${sha}[0-9a-f]*)/${wf}/summary\\.md$`);
  const runs = [];
  for (const line of r.stdout.split('\n')) {
    const m = re.exec(line.trim());
    if (!m) continue;
    const dir = `${m[1]}/${m[2]}/${workflow}`;
    runs.push({ refDir: m[1], dir, meta: json(`${dir}/meta.json`) || {} });
  }
  // The same sha pushed to two refs: the ref it was pushed to here, then the
  // newest run.
  const want = new Set(holders.map(toDir));
  runs.sort(
    (a, b) =>
      want.has(b.refDir) - want.has(a.refDir) || String(b.meta.finished || '').localeCompare(String(a.meta.finished || '')),
  );
  return runs[0] || null;
}

// A different sha in a holder ref's latest/ that is not an ancestor of ours:
// a newer push, which cancelled this run.
const older = new Map();
function newerRun() {
  for (const ref of holders) {
    const meta = json(`${toDir(ref)}/latest/${workflow}/meta.json`);
    if (!meta || !full(String(meta.sha)) || meta.sha.startsWith(sha)) continue;
    if (!older.has(meta.sha)) {
      let isOlder = false;
      if (full(sha)) {
        if (git(['cat-file', '-e', `${meta.sha}^{commit}`]).status !== 0) {
          git(['fetch', '--quiet', 'origin', `+refs/heads/${ref}:refs/remotes/origin/${ref}`]);
        }
        isOlder = git(['merge-base', '--is-ancestor', meta.sha, sha]).status === 0;
      }
      older.set(meta.sha, isOlder);
    }
    if (!older.get(meta.sha)) return { ref, meta };
  }
  return null;
}

function hint(run) {
  if (!run) return;
  const { ref, meta } = run;
  say(`hint: ${ref} has moved on: its latest ${workflow} run is for ${short(meta.sha)} (${meta.conclusion}, ${meta.run_url}).`);
  say('      A newer push to a ref cancels the older run, and a cancelled run may never publish.');
  say(`      Wait on that one instead: node tools/ci-wait.mjs ${meta.sha}${workflow === 'rust' ? '' : ` ${workflow}`}`);
}

function report({ dir, meta }) {
  const summary = blob(`${dir}/summary.md`);
  const conclusion = meta.conclusion || (/Conclusion: \*\*(\w+)\*\*/.exec(summary) || [])[1] || 'unknown';
  console.log(summary.trimEnd());
  if (conclusion === 'success') return 0;

  const errors = blob(`${dir}/errors.txt`).trimEnd();
  if (errors) {
    const lines = errors.split('\n');
    console.log(`\n== errors.txt ==\n${lines.slice(0, ERR_MAX).join('\n')}`);
    if (lines.length > ERR_MAX) console.log(`[${lines.length - ERR_MAX} more lines in errors.txt]`);
  }
  const steps = meta.steps || [];
  const gate = steps.find((s) => s.outcome === 'failure' && s.log && s.log.endsWith('.log'));
  if (gate) {
    const lines = blob(`${dir}/${gate.log}`).trimEnd().split('\n');
    const n = Math.min(TAIL, lines.length);
    console.log(`\n== ${gate.name}: last ${n} lines of ${gate.log} ==\n${lines.slice(-TAIL).join('\n')}`);
  }
  const patch = steps.find((s) => s.log && s.log.endsWith('.patch'));
  if (patch) console.log(`\nrustfmt's fix: git show origin/ci-logs:${dir}/${patch.log} | git apply`);
  console.log(`\nAny file in full: git show origin/ci-logs:${dir}/<file>`);
  if (conclusion === 'cancelled') {
    say(`the ${workflow} run for ${short(sha)} was cancelled`);
    hint(newerRun());
    return 2;
  }
  return 1;
}

const start = Date.now();
const deadline = start + timeoutMin * 60000;
const pushed = holders.length ? ` on ${holders.join(', ')}` : '';
say(`waiting up to ${timeoutMin} min for ${workflow} results for ${short(sha)}${pushed}`);
let hinted = false;
for (;;) {
  fetchLogs();
  const run = findRun();
  if (run) process.exit(report(run));
  const left = deadline - Date.now();
  if (left <= 0) break;
  if (!hinted) {
    const newer = newerRun();
    if (newer) {
      hint(newer);
      hinted = true;
    }
  }
  say(`${Math.floor((Date.now() - start) / 60000)} min: not published yet`);
  await new Promise((resolve) => setTimeout(resolve, Math.min(POLL_S * 1000, left)));
}
say(
  `timed out after ${timeoutMin} min with no ${workflow} results for ${short(sha)}: the run is still queued or ` +
    'running, was cancelled before it could publish, or never started (ui-shots runs only when its paths change).',
);
hint(newerRun());
process.exit(2);
