# Defects

Found by reading main.js, lib/, renderer/ and settings/ against DESIGN.md, and
by the first run of `tools/check.js`. Severity is about what the user loses, not
about how much code moves.

| # | LOCATION | DEFECT | SEVERITY | CLASS | STATUS |
| --- | --- | --- | --- | --- | --- |
| D1 | `lib/openrouter.js` `nextFreeModel()` | Failover picks the next id straight out of the hardcoded `CURATED`/`BRAINS` arrays and never consults the live catalogue, although `listModels()` does exactly that a few lines above. A retired id is handed to the next attempt and burns one of only three, so the app can exhaust its whole failover budget on models that no longer exist. This is the v1 defect ("the default model had been retired out from under the app") one layer down, in the code written to survive it. | MAJOR | correctness | FIXED (iter 2) |
| D2 | `main.js` `workArea()` | Every window positions off `screen.getPrimaryDisplay()`, while `activeDisplay()` follows the cursor and both capture and the pointer use it. On a two-monitor desk the overlay opens on one screen and answers about the other. DESIGN.md v0.6 records this exact class as fixed for capture and the pointer; the windows were left behind. | MAJOR | correctness | FIXED (iter 3) |
| D3 | `lib/history.js` `trim()` | The comment says "only measure when the count alone could plausibly exceed the budget". There is no such guard: `JSON.stringify(cache)` runs over the entire history on the main process after every single answer, and again on every pop. Measured at 428ms per 60 saves against 1ms after the fix. A comment describing a guard nobody wrote is the arrow-tip defect again. | MAJOR | efficiency | FIXED (iter 4) |
| D4 | `main.js` `openHistory()` | The history window's `resize` handler writes the store on every event with no debounce, while `openSettings()` right above it debounces at `DROP_SETTLE_MS`. One drag is hundreds of synchronous whole-file writes. | MAJOR | efficiency | FIXED (iter 4) |
| D5 | `renderer/index.html` `#query-input` | The query field is `<input type="text">`: one line, no way to paste a stack trace or write a two-line question, on an app whose whole purpose is asking about errors and code. `INPUT_MIN_HEIGHT` 62, `INPUT_MAX_HEIGHT` 280 and the content-pinned window height all exist for a field that grows, and nothing grew. | MAJOR | missing feature | FIXED (iter 5) |
| D6 | `lib/openrouter.js` `friendlyError()` | The 404 copy asserts "the free model tried after it was not either", the 429 copy asserts "every free model this app knows about was busy too", and the 5xx copy asserts "on this model and on the fallback". None is checked. On a paid model no stand-in is ever tried, and the cap is three attempts, never "every". | MINOR | honesty | FIXED (iter 2) |
| D7 | `preload.js` | `buddy:move` and `settings:data` are on the listen allowlist and appear nowhere else in the repository: nothing sends them, nothing listens for them. Dead widening of the one surface that exists to be narrow. Same class as the `query:cancel` lie in v0.7. Found by `tools/check.js`. | MINOR | tidiness | FIXED (iter 6) |
| D8 | `lib/motion.js` `stop()` | `stop()` reads `win.id` with no null guard, and `onToggleHotkey()` calls `motion.stop(outputWin)` without checking it. Probed rather than assumed: reading `.id` on a *destroyed* window is safe (Electron keeps it), so this is narrower than it first looked, but a null window still throws, inside a 60Hz interval. | MINOR | correctness | FIXED (iter 6) |
| D9 | `main.js:158` | The `lastRegisterResults` initialiser omits `captureRegion`, so `settings:get` can report an `active` map missing a key the panel draws a row for. | MINOR | tidiness | FIXED (iter 6) |
| D10 | `lib/tray.js` `init()` | `click` and `double-click` both call `capture()`. Windows emits both clicks plus the double-click event, so one double click fires three screen captures and three panel expansions. | MINOR | correctness | FIXED (iter 6) |
| D11 | `lib/openrouter.js` `streamQuery()` | Every exit from the SSE loop except a clean `[DONE]` abandons the reader without releasing it, holding the socket until GC. On a free tier that rate limits per connection that is not a detail. | MINOR | resource | FIXED (iter 6) |
| D12 | `lib/region.js` `finish()` | **Area selection never worked.** `finish()` calls `close()` before `settle()`. `close()` calls `destroy()`, and `destroy()` emits `closed` *synchronously* (probed), and the `closed` handler settles the promise with `null` as its safety net. So every completed drag resolved as a cancel and the rectangle was discarded by a no-op `settle`. `Ctrl+Shift+A`, drag a box, release: nothing happens. This is the feature `lib/region.js` calls "the largest single lever on answer quality in the app". Survived because all three region assertions in the smoke run test *cancelling* -- the one path the feature exists for was the one nothing asked about. | BLOCKER | correctness | FIXED (iter 8) |

| D13 | `lib/cursor-buddy.js` `complete()` | The pointer window outlives `complete()` by 700ms so it can play its exit, and that `setTimeout(destroyWindow, 700)` was held by nobody. A sequence starting inside the gap reuses the same window and then has it destroyed underneath by the previous sequence's timer: the pointer appears and vanishes. Reachable when an answer carrying a point directive lands just as the user finishes the last one. | MINOR | correctness | FIXED (iter 9) |

## Closed with no change

`settings/settings.js` (675 lines) and `history/history.js` (206 lines) were
read in full in iteration 8 and produced nothing I could defend as a defect.
Recorded so the next person knows they were looked at rather than skipped.

## Corrected during the run

D8 was first written up as a BLOCKER, on the belief that reading `.id` off a
destroyed `BrowserWindow` throws -- which would have made the interval's own
destroyed-branch an unkillable 60Hz error loop. Probed it instead of shipping
the claim: Electron caches the id and it does not throw. The real defect is the
null case alone, and the severity came down to MINOR. Recorded because the
dramatic version was the more publishable one.

## Open at handoff

None. Every row above is FIXED, and each carries either a `must()` in the smoke
run or a check in `tools/check.js` that would catch the regression -- because a
fix with nothing watching it is exactly how the v1 defect survived a whole
release.
