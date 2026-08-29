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

## Iteration 2 - D1, D6: failover that knows what exists
Goal: stop failover handing its own retry a retired model id, and stop the
error copy asserting stand-ins that were never tried.

GATE: check | CMD: node tools/check.js | EXIT: 0 | syntax=22, palette=47, ipc=47/17, lineEndings=42
GATE: smoke | CMD: AEVAL_SMOKE=1 npx electron . | EXIT: 0 |
  failoverPicksLiveModel=ok  failoverPicksLiveBrain=ok  errorCopyCountsAttempts=ok
  failoverRateLimit=2:switched  failoverKeepsAuthError=threw:401  failoverCapped=3<=3:true

Changed: lib/openrouter.js, main.js.
Result: D1 and D6 closed. Fixing D1 exposed why it was blind - the catalogue
cache was only ever warmed by opening Settings, so the ordinary path was the
one running without it. Warmed at boot now.
Next: D2, the windows left on the primary display.

## Iteration 3 - D2: the overlay opens where the answer is about
Goal: windows follow the same display capture and the pointer already follow.

GATE: check | EXIT: 0
GATE: smoke | EXIT: 0 | displays=1  windowsOnCaptureDisplay=ok  staleDisplayFallsBack=ok

Changed: main.js.
Result: D2 closed. Honest limit: this machine reports `displays=1`, so the
two-monitor path is reasoned and gate-guarded, not empirically exercised. The
invariant was written to be one that holds on either, since the two-monitor
desk is the case that cannot be asked of an automated run.
Next: D3 and D4, both history.

## Iteration 4 - D3, D4: history stops paying for its own cap
Goal: remove the whole-history serialise from the per-answer path, and debounce
the history window's resize writes.

GATE: check | EXIT: 0
GATE: smoke | EXIT: 0 | historyTrimMs=1  historyBytes=4185557<=4194304  historyRespectsByteCap=ok
BENCH: 60 saves of a 120KB conversation, same input, same 34 entries kept:
  old 428ms, new 1ms.

Changed: lib/history.js, main.js.
Result: D3 and D4 closed. The estimator deliberately reads high, and the
invariant measures the real serialised length rather than trusting it.
Next: D5, the field that cannot hold the question.

## Iteration 5 - D5: the question takes more than one line
Goal: a textarea, on the existing 32px control rhythm, Enter sends and
Shift+Enter is a newline.

GATE: check | EXIT: 0
GATE: smoke | EXIT: 0 | queryFieldRows=32px->74px  queryFieldTakesMultipleLines=ok  panelFollowsTheField=ok
GATE: shots | CMD: AEVAL_SHOT_OFFSCREEN=1 npm run shots | EXIT: 0 |
  03c-panel-multiline=420x121  fit.panelMultiline=window121:surface121

Changed: renderer/index.html, renderer/overlay.css, renderer/overlay.js,
lib/shots.js, main.js.
Result: D5 closed. Looked at the photograph rather than the numbers: chrome
holds the first line, the pasted trace keeps its indentation, window and
surface agree so there is no dead band.
Next: the remaining minor rows.

## Iteration 6 - D7 to D11: five small things that were true
Goal: close the rest, and make the gate enforce D7's class rather than mention
it.

GATE: check | EXIT: 0 | zero notes: all 17 listen channels are genuinely sent
GATE: smoke | EXIT: 0 | shortcut.captureRegion=true  tray=ok  queryWithoutKey=done:chat2

Teaching the gate to fail on dead listen channels first meant teaching it that
main.js routes the streaming channels through a local `send` helper; matching
only `webContents.send` had reported four live channels as dead. With that
fixed it failed on exactly `buddy:move` and `settings:data` and nothing else,
which is the fix being enforced rather than remembered.

Changed: preload.js, lib/motion.js, lib/tray.js, lib/openrouter.js, main.js,
tools/check.js.
Result: D7 to D11 closed.
Next: record the departures in DESIGN.md, then the build gate.

## Iteration 7 - documentation and the build gate
Goal: DESIGN.md is the authority and says anything outside it is a defect, so a
change that departs from it is unfinished until the departure is recorded there
with its reason. Then the expensive gate.

Changed: DESIGN.md (v1.1 section), README.md (npm run check, the multi-line
field, tools/check.js in the file list), ledger.

## Iteration 8 - D12: area selection never worked
Goal: finish the defect hunt properly. The first pass reviewed main.js, the
libs and two renderers, and skipped roughly 1400 lines: settings/settings.js,
history/history.js, lib/region.js, lib/cursor-buddy.js. Claiming the app had
been reviewed while a quarter of it had not been read is its own defect.

settings.js and history/history.js came back clean - nothing I could defend as
a defect. lib/region.js did not.

`finish()` calls `close()` before `settle()`. `close()` calls `destroy()`,
`destroy()` emits 'closed', and the 'closed' handler settles the promise with
null as a safety net for a window that goes away unexpectedly. Whether that
mattered turned entirely on whether 'closed' is synchronous.

PROBE: a 25-line Electron script, the exact shape region.js uses ->
  closed fired during destroy() = true
  promise resolved with null
So the safety net caught every *successful* selection. Draw a box, release,
and the app treated it as a cancel. That is the feature region.js's own header
calls "the largest single lever on answer quality in the app".

Why it survived: all three region assertions in the smoke run are about
cancelling - selector cancels, Escape cancels, the scrim is not stranded. The
one path the feature exists for was the one nothing ever asked about.

Fix: settle first, close second. The 'closed' handler stays as the safety net
it was meant to be.

GATE: check | EXIT: 0
GATE: smoke, defect deliberately reinstated | EXIT: 1 | failed: regionDragReturnsTheBox
GATE: smoke, fix restored | EXIT: 0 | regionDrag=320x200  regionDragReturnsTheBox=ok
  and all three cancel paths still pass

Recorded because I got it wrong twice on the way. First I reported the
reinstated-defect run as hanging; it was PowerShell buffering the process
output into a variable while it wrote to stderr, not the app. Streaming the
output showed a clean exit 1. The check is sound and does not stall.

Changed: lib/region.js, main.js.
Result: D12 closed, and it is the most valuable find of the run.

## Iteration 9 - D13: the pointer handover
Goal: finish the last unread file, lib/cursor-buddy.js.

`complete()` leaves the pointer window alive for 700ms so it can play its exit,
and nothing held that timer. A sequence starting inside the gap reuses the same
window and then has it destroyed underneath it by the previous sequence's
timer, so the pointer appears and vanishes. Reachable when an answer carrying a
point directive lands just as the user finishes the last one.

The timer is held now and cleared by anything that takes ownership of the
window: createWindow, destroyWindow, and complete itself.

GATE: check | EXIT: 0
GATE: smoke | EXIT: 0 | buddySurvivesTheHandover=ok

The new check shows a second sequence and then waits 900ms - past the old
deadline - because the window still being there afterwards is the entire claim.
Unlike D12 I did not reinstate this defect to watch the check fail: it holds by
construction, since without cancelPendingDestroy the 700ms timer fires inside
the 900ms wait and debugWindow() returns null. Stating that rather than
implying it was measured.

Note on the harness, twice bitten: `$out = & npx electron .` in PowerShell
buffers, and a run that writes to stderr looks like a hang. Streaming through
Tee-Object shows the truth. Both "hangs" in this session were that, not the app.

Changed: lib/cursor-buddy.js, main.js.
Result: D13 closed. Every file in the repository has now been read.
