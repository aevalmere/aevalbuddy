# Progress

Append-only. One block per iteration, with the real command output for every
gate that ran in that iteration. A remembered pass is not a pass.

## Iteration 0 - baseline pin
Goal: establish that the tree is green before changing anything, so a later
failure is answerable.

GATE: syntax | CMD: node --check over every .js | EXIT: 0 | checked 21 files, failures 0
GATE: smoke  | CMD: AEVAL_SMOKE=1 npx electron . | EXIT: 0 | ~90 observations, 6 must() invariants, all ok

Changed: nothing.
Result: baseline green. `AEVAL_SMOKE=1` is the closest thing this repo has to a
test suite and it is a good one: it boots the real app and asserts on windows,
tray, both live model defaults, and a deliberate unhandled rejection reaching
errors.log. That is the pin.
Next: read the source against DESIGN.md and log defects before touching code.

## Iteration 1 - a gate that can fail
Goal: the repo has no typecheck, no lint and no unit tests, so "syntax" was an
ad-hoc one-liner. Give it a real static gate first, because everything after
this is verified by it.

Wrote `tools/check.js`, wired as `npm run check`. Three checks, each one a class
of defect this repository has actually shipped rather than a style opinion:

1. **Syntax** - every .js parses. No build step, no typechecker, so a stray
   bracket in a rare branch reaches a user first.
2. **Palette** - DESIGN.md permits six colour values plus named exceptions, and
   says an unnamed inline hex "is still a defect, because it is a decision
   nobody can count". `renderer/region.css` once carried seven under a header
   comment claiming it used the palette. A comment cannot enforce that.
3. **IPC coherence** - the preload allowlist drifts both ways. A renderer
   invoking an unlisted channel fails only at runtime in a frameless window
   with no devtools. A listed channel nothing invokes is the v0.7 `query:cancel`
   defect exactly: every layer present except the one that calls it.

GATE: check  | CMD: node tools/check.js | EXIT: 0 | syntax=22 files, palette=47 hex values, ipc=47 invoked / 17 listened
GATE: syntax | folded into check
GATE: smoke  | not re-run this iteration; no app code changed

Its first run earned its keep: it reported `buddy:move` and `settings:data` as
listenable-but-never-sent. Grepping the whole repo, those two strings appear
**only** in the preload allowlist - nothing sends them, nothing listens for
them. Dead widening of the one surface that exists to be narrow. Logged as D7.

Changed: tools/check.js (new), package.json (check script), ledger.
Result: clean pass, and one new defect found by the gate itself.
Next: D1, the failover that can hand a retired model id to its own retry.
