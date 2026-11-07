# Progress

## Iteration 0 - 2026-11-05 (SHA 7feeddb)
Goal: publish shots to the `shots` branch, fetch, review all 36.
GATE: installer    | run 42052205865 | EXIT: 1 | smoke updateFeed=FAIL:retrying; update.cause "HttpError: 529" from GitHub (external). Shots step ran, 36 png published.
GATE: update-check | run 42052205790 | EXIT: 0 |
GATE: visual-review | 36/36 read | 13 defects (2 MAJOR, 11 MINOR), see DEFECTS.md
Changed: .github/workflows/installer.yml
Result: shots readable at raw.githubusercontent.com/aevalmere/aevalbuddy/shots/latest/<name>.png
Next: D1 harness (Find/lens frames empty), D2 naming, D3 selection colour, D12.

## Iteration 1 - 2026-11-05 (SHA 0ee7c5c)
Goal: D1, D2, D3, D12.
Changed: main.js (stopFind hook, ACTION_LABELS), lib/shots-v2.js, settings/settings.html+js, history/history.html, overlay/settings/history css (::selection), DESIGN.md (one name per action)
Gates: pending (see iteration 2 block for this SHA's results)
GATE: installer    | run 42054232333 | EXIT: 0 (SHA 0ee7c5c) |
GATE: update-check | run 42054232307 | EXIT: 0 | warning hopA.installOnQuit (pre-existing notice level)
GATE: visual-review | 6 frames changed: 18 now shows "2 exact · 2 related"; 12 selection is grape; 09/16 names fixed; 19 blank (new D15); 20 still blank (D16)

## Iteration 2 - 2026-11-05 (SHA ada58b9)
Goal: D4, D6, D10, D14 (contrast floor), D15 (Find view stale on switch), D16 probe.
Changed: renderer/overlay.css, renderer/region.css, renderer/cursor-buddy.css, renderer/trail.js, settings/settings.css+html, history/history.css, main.js (lensProbe), lib/shots-v2.js, DESIGN.md
D5 closed as not a defect by pixel measurement.
GATE: installer    | run 42055917723 | EXIT: 1 | smoke updateFeed=FAIL:retrying, update.cause "HttpError: 529" (GitHub, external; same as iter 0). All other smoke asserts ok; 36 shots published.
GATE: update-check | run 42055917519 | EXIT: 1 | update-check.ps1 line 154: Invoke-WebRequest latest/download/latest.yml -> 504 Gateway Time-out (GitHub, external). Link answers 302 when re-checked.
GATE: visual-review | 22 frames changed; D15 verified (19 shows 4 hits, current tinted); D6/D10/D14 verified. lens.probe: "windows=1 visible hits=- hitEls=0 frameOn=false vis=visible": the lens page processed no lens:draw at all.
Result: no gate regression from this loop's diff (both failures are GitHub 5xx on endpoints the diff does not touch). Asserts unchanged.

## Iteration 3 - 2026-11-05 (SHA 0eb14ac)
Goal: copy pass D7, D8, D9, D11, D17.
Changed: renderer copy (bar placeholder, buddy bubble, tour step 4, rate-limit error, region size label)
GATE: installer    | run 42100460036 | EXIT: 0 (success) | smoke updateFeed=ok, lensHits=ok; 36 shots published to shots/latest (commit "Screenshots for 0eb14ac")
GATE: update-check | run 42100459818 | EXIT: 0 (success) | warning "after a polite close, registered=1.1.1" (pre-existing)
GATE: visual-review | 03 "Ask | anything about your screen"; 11 bubble has no "click to close"; 14 rate-limit copy is 3 lines; 28-4 carries Ctrl+Shift+U and no widow. 20-lens still a bare backdrop; lens.probe unchanged: "windows=1 loading=false crashed=false hits=- hitEls=0 frameOn=false bridge=object script=function"
Result: D7 D8 D9 D11 D17 verified. D16 still open (lensHits=ok in smoke asserts the hit list main computes, not what the lens page renders: the assert cannot see D16).

## Iteration 4 - 2026-11-06 (SHA 47d723e)
Goal: D16, root cause and fix.
Root cause: lib/lens.js draw() sent lens:draw only when !webContents.isLoading(), and redrew from did-finish-load. isLoading() is still true inside did-finish-load (Chromium's DidStopLoading comes after it), so that redraw was skipped too. A lens window created by setHits/setFrame whose updates all arrived while it loaded never got a single lens:draw. Smoke's lensHits/findStep passed only because live Find keeps sending after load.
Changed: lib/lens.js (aevalReady flag: false on did-start-loading, true on did-finish-load before the redraw), lib/smoke-v2-desktop.js (new assert lensFirstDraw: fresh lens, one setHits + one setFrame, nothing after, must show 3 hit boxes + ring + frame within 5s)
GATE: installer    | run 42178713758 | EXIT: 0 (success) | lensFirstDraw=hits:ring:frame=3:true:true ok; lensHits=ok; updateFeed=ok (current)
GATE: update-check | run 42178713874 | EXIT: 0 (success)
GATE: visual-review | 20-lens now shows the frame edge, the agent chip, 2 exact boxes, the related ring and its why chip. lens.probe: "hits=4 hitEls=4 hitOpacity=1 frameOn=true frameOpacity=1". New D21: the why chip covers the 4th hit.
Result: D16 FIXED. Assert added, none loosened.

## Iteration 5 - 2026-11-06 (SHA a042e2b)
Goal: D13, bounded retry on GitHub 5xx and timeouts in both gates without loosening an assert.
Changed: main.js (smoke updateFeed: up to 3 checks, pauses 10s/30s, only when the logged cause is HttpError 5xx / 5xx gateway text / timeout or the check does not settle in 45s; logs update.attempts; same updateFeed + updateQuiet asserts on the final answer), tools/update-check.ps1 (Get-Release: 3 release downloads retried on 5xx or timeout after 10/30/60s, rethrows the last error; any other status throws at once)
GATE: installer    | run 42179520580 | EXIT: 0 (success) | update.attempts=current, updateFeed=ok, lensFirstDraw ok
GATE: update-check | run 42179520607 | EXIT: 0 (success) | release.latestYmlPath ok; warning "after a polite close" (pre-existing)
Result: D13 FIXED as a gate-robustness change. Not verified under a real 5xx (none happened this run); the retry branch is reachable only through GitHub misbehaving. Assert lines unchanged (diffed).

## Iteration 6 - 2026-11-06 (SHA 986a5cb)
Goal: D18, D19, D20 naming and copy.
Changed: settings/settings.html (row "Doing tasks" -> "Permissions"), renderer/output.html + output.js ("Docked to input"/"Dock to input" -> "Docked"/"Dock"), lib/shots.js (sample list: no dangling colon, both commands in the code block), DESIGN.md (records both names), README.md (button name)
GATE: installer    | run 42180230233 | EXIT: 0 (success)
GATE: update-check | run 42180230129 | EXIT: 0 (success)
GATE: visual-review | 04 list reads "1. Install the package / 2. Restart the dev server" then npm i + npm run dev; 07 badge "Docked"; 29 row "Permissions". All three verified.
Result: D18 D19 D20 FIXED.

## Iteration 7 - 2026-11-06 (SHA ea01008)
Goal: D21, why chip covering the next hit.
Changed: renderer/lens.js (whySpot: below, above, level right, level left; first spot on-display that covers no other hit, else fewest covered), lib/smoke-v2-desktop.js (new assert lensWhyClear, using the shot's geometry)
GATE: installer    | run 42180969117 | EXIT: 0 (success) | lensWhyClear=covers=0 at=243,201 ok; lensFirstDraw ok; update.attempts=current
GATE: update-check | run 42180969295 | EXIT: 0 (success)
GATE: visual-review | 20-lens: all 4 hits visible; why chip sits level, right of the ring; frame + agent chip intact. Full pass over shots7 (36 frames): one remaining widow, "saved." alone on the Agent page note (29).
Result: D21 FIXED. D22 opened (widow).

## Iteration 8 - 2026-11-06 (SHA 7a66931)
Goal: D22, the widow on the settings Agent note.
Changed: settings/settings.css (.note gets text-wrap: pretty, the rule cursor-buddy.css already uses)
GATE: installer    | run 42181710230 | EXIT: 0 (success)
GATE: update-check | run 42181710223 | EXIT: 0 (success)
GATE: visual-review | 29 note now breaks "...Screenshots are / never saved."; 08/09 unchanged. Full pass over the ea01008/7a66931 shots: no MAJOR defect left; two observations kept below, not fixed.
Result: D22 FIXED.
STOP: (g) iteration cap for this session (5 iterations, 4-8), with both workflows green on the final SHA and every logged defect closed. Kept as observations, not defects:
  - 25-agent-error: the shot's fixture footer repeats the failed step's reason. The real footer is model text, so this is fixture-only.
  - lib/agent.js:894: a loop crash with an unmapped error code reads "The agent stopped: CODE: message". Rare path, not shot, not verified.
  - The 5xx retry branch (iter 5) has not run against a real GitHub 5xx yet.
  - Actions annotate a Node 20 deprecation warning for checkout/setup-node/upload-artifact v4.

## v2.1 Phase A - 2026-11-07
Goal: implement PLAN-v2.1.md Phase A (single input, trust-safety CRITICAL/HIGH, power-user P0/HIGH, a11y blockers, ease-of-use P0s).
GATE: syntax       | CMD: node --check on every touched file | EXIT: 0
GATE: installer    | run 42316786470 on 0a7fba7 | failure | findStep (hit identity collided within a line), pointFromUia, agentRun, agentJsonMode (knock-on from findStep's scrolling)
GATE: installer    | run 42317435007 on 1e5cdda | success | intentTable 42/42, gateBypasses 37/37
GATE: update-check | run 42317435014 on 1e5cdda | success
Visual review: 02, 03, 08, 18, 21, 23 looked at; Agent model row renamed Task model after it.
Result: Phase A implemented; see DECISIONS.md v2.1 for replaced asserts.

## v2.1 Phase B - finishing loop (2026-11-07)
Baseline: 9d36686, installer run 42322582929 failure, update-check run 42322582799 failure (hopC.smokeExit, the same smoke run).
Root causes:
- smoke styleStable: WS_EX_NOACTIVATE differs pill vs bar by design (setFocusable). Gate compared to the first sample (D-B10).
- smoke collapseClickable: renderer/overlay.js cached ignoreState=false from the bar; main re-enabled click-through on collapse, so the first move over the orb hit the early return and never asked for hit-testing.
- collapse morph never visible: main sent the pill state only after shrinking the window.
- contrast: sr-only live regions measured (1:1); ::selection text and link underline survived the hide pass; History and Settings overrode glass.css --danger with the pre-glass #E4818C (Delete all 4.09 on white).
- shots at 125/150: fixture covered 2/3 of the screen (D-B12); bar scene inherited selected text from shots.js, which also made the 100% Find scene search for it.
Cost before: snapCost wallP95=54.4ms blockP95=15.4ms; glassCost live 12.9% (renderer 10.6, gpu 1, browser 1.2) vs solid 0.2% of one core.

## Iteration 1 - 2026-11-07 (SHA 00c4c80)
Goal: green smoke and contrast on the glass.
Changed: renderer/overlay.js, main.js, lib/smoke-glass.js, lib/contrast.js, lib/backdrop.js, lib/shots-matrix.js, history/history.css, settings/settings.css
GATE: syntax       | node --check on touched JS | EXIT: 0
GATE: installer    | run 42325086519 | failure | smoke: collapseClickable only. styleStable=ok. Shots 100/125/150 contrast failedFrames=0 (100 min 5.43, 125 min 5.59)
GATE: update-check | run 42325086471 | failure | hopC.smokeExit (same smoke)
Cost: snapCost wallP95=48.2ms blockP95=14.9ms; glassCost live 11.9% (renderer 9.7, gpu 0.9, browser 1.3) | solid 0.3%
Gate-definition files changed this iteration: lib/smoke-glass.js (stricter NOACTIVATE check, D-B10), lib/contrast.js (visible-only, D-B11). Both recorded; neither loosens a measurement of visible text.
Result: collapseClickable now finds the overlay under the pill centre (at=overlay), but reads exStyle before moving the pointer (host.cs samples styles first), so click-through=true is the orb at rest. Next: read the style after the move.

## Iteration 2 - 2026-11-07 (SHA c192646)
Goal: collapseClickable reads the orb after the pointer arrives; glass the leftover surfaces.
Changed: lib/smoke-glass.js (re-read exStyle after move), renderer/region.css+html (glass chip/hint/keycaps, tokens, no mascot), renderer/cursor-buddy.css (dead .face/.eye/.who/.bubble-tail and blink removed), history/history.css (no inner type=search box, accent focus halo), renderer/overlay.css (chat tools visible at rest, Copy/Do it capsules, Find current row without stripe, rows on grid), renderer/glass.css (.edge-fade scroll edge), settings/history/output html (edge-fade class), DESIGN.md
Found while reviewing iteration 1's annotations: most matrix frames logged runs=0. The D-B11 clip walked the zero-height overflow-hidden body that holds the fixed panels, so the gate measured nothing on bar/answer/find/trail and passed vacuously. Logged as BLOCKER P9, fixed in iteration 3 (pushed before iteration 2's runs finished). Iteration 2's contrast numbers are not evidence.
c192646 runs: installer 42325987356 success, update-check 42325987328 success. collapseClickable=ok (at=overlay, ex=0x8200088: hit-testing on, NOACTIVATE set), styleStable=ok. glassCost live 9.4% (renderer 7.7) | solid 0.2%; snapCost wallP95=61.3ms blockP95=15.1ms. Contrast on this SHA is vacuous (P9).

## Iteration 3 - 2026-11-07 (SHA 1fdaaaf)
Goal: P9, the contrast gate must measure the overlay and answer pages.
Changed: lib/contrast.js (overflow clips only along the containing-block chain), lib/shots-matrix.js (a frame with no measured text fails)
GATE: installer    | run 42326263372 | failure | only the new guard: m-pill-* runs=0 (the orb shows no text). Every text surface measured and passed.
GATE: update-check | run 42326263461 | success
Min contrast per surface (all scales, all backdrops): bar 4.62 (white-150), history 5.43 (white-100), agent-confirm 5.54 (white-150), tour 6.32 (white-100), settings 6.36 (white-150), error 8.54, answer/thinking/agent-running/agent-done 8.57 (white-100), find 8.70, lens 17.55. By backdrop: white 4.62, wallpaper 5.26, black 6.98.
Cost: glassCost live 10% (renderer 8.1, gpu 0.7, browser 1.1) | solid 0.2%; snapCost wallP95=47.9ms blockP95=15.1ms
Result: P9 FIXED. Pill exempted by name (text:false), recorded in DECISIONS D-B13.

## Iteration 4 - 2026-11-07 (SHA f9b3776)
Goal: orb measured as text-free; History search inner ring; answer action spacing; concentric Find rows.
Changed: lib/shots-matrix.js (pill text:false, D-B13), history/history.css, renderer/overlay.css
GATE: installer    | run 42327184070 | success | smoke all ok: styleStable ok, collapseClickable ok (ex=0x8200088), topmostHolds ok; shots 100/125/150 failedFrames=0
GATE: update-check | run 42327184032 | success
Min contrast per surface: bar 4.62 (white-150), history 5.43, agent-confirm 5.54, tour 6.32, settings 6.36, error 8.54, answer/thinking/agent 8.57, find 8.70, lens 17.55. By backdrop: white 4.62, wallpaper 5.26, black 6.98.
Cost: glassCost live 14% (renderer 11.7, gpu 0.9, browser 1.4) | solid 0.3%; snapCost wallP95=59ms blockP95=15.3ms (gate is blockP95 < 25 ms)
Visual review (s4): History search is one capsule with an accent halo; Find rows concentric; answer actions spaced. Found: tour Skip still a bare word.
Result: both workflows green. P1-P9 FIXED.

## Iteration 5 - 2026-11-07 (SHA b29f35d)
Goal: P10, the tour's Skip as a capsule.
Changed: renderer/cursor-buddy.css, renderer/buddy.js (comment)
GATE: installer    | run 42328225909 | success | styleStable ok, collapseClickable ok (ex=0x8200088), topmostHolds ok, shots failedFrames=0
GATE: update-check | run 42328225872 | success
Min contrast: bar 4.62 (white-150), history 5.43, agent-confirm 5.54, settings 6.36, tour 7.32 (was 6.32), error 8.54, answer/thinking/agent 8.57, find 8.70, lens 17.55. By backdrop: white 4.62, wallpaper 5.26, black 6.98.
Cost: glassCost live 11.9% (renderer 9.8, gpu 0.9, browser 1.2) | solid 0.2%; snapCost wallP95=53.8ms blockP95=15.3ms
Visual review (s4 + m-tour-wallpaper-125 of b29f35d): tour Skip is a capsule. Found: at 100% the chat toolbar's -10 px bottom margin lets a scrolled user bubble slide under its icons (m-agent-confirm-*-100).
Result: P10 FIXED. P11 opened.

## Iteration 6 - 2026-11-07 (SHA 1326fd6)
Goal: P11, the chat toolbar as its own row.
Changed: renderer/overlay.css
GATE: installer    | run 42329154359 | success | styleStable ok, collapseClickable ok (at=overlay, ex=0x8200088), topmostHolds ok, shots 100/125/150 failedFrames=0
GATE: update-check | run 42329154358 | success
Min contrast per surface: bar 4.62 (white-150), history 5.43 (white-100), agent-confirm 5.54 (white-150), settings 6.36 (white-150), tour 7.32, error 8.54, answer/thinking/agent-running/agent-done 8.57, find 8.70, lens 17.55. By backdrop: white 4.62, wallpaper 5.26, black 6.98.
Cost: glassCost live 11.9% of one core (renderer 9.7, gpu 0.9, browser 1.3) | solid 0.2%; snapCost wallP95=52.9ms blockP95=15.3ms (gate < 25 ms)
Visual review: m-agent-confirm-white-100 toolbar clear of the transcript, scrolled bubble fades under it. Observation kept, not fixed: in 04-answer an earlier answer's hidden Copy/Do it row still reserves 40 px, so a follow-up question sits far below it.
Result: P11 FIXED. Both workflows green.
STOP: (g) iteration cap (6) reached with both workflows green on the final SHA and every logged defect closed.
