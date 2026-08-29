# Defects

Found by reading main.js, lib/, renderer/ and settings/ against DESIGN.md, and
by the first run of `tools/check.js`. Severity is about what the user loses, not
about how much code moves.

| # | LOCATION | DEFECT | SEVERITY | CLASS | STATUS |
| --- | --- | --- | --- | --- | --- |
| D1 | `lib/openrouter.js` `nextFreeModel()` | Failover picks the next id straight out of the hardcoded `CURATED`/`BRAINS` arrays and never consults the live catalogue, although `listModels()` does exactly that a few lines above. A retired id is handed to the next attempt and burns one of only three, so the app can exhaust its whole failover budget on models that no longer exist. This is the v1 defect ("the default model had been retired out from under the app") one layer down, in the code written to survive it. | MAJOR | correctness | OPEN |
| D2 | `main.js` `workArea()` | Every window positions off `screen.getPrimaryDisplay()`, while `activeDisplay()` follows the cursor and both capture and the pointer use it. On a two-monitor desk the overlay opens on one screen and answers about the other. DESIGN.md v0.6 records this exact class as fixed for capture and the pointer; the windows were left behind. | MAJOR | correctness | OPEN |
| D3 | `lib/history.js` `trim()` | The comment says "only measure when the count alone could plausibly exceed the budget". There is no such guard: `JSON.stringify(cache)` runs over the entire history on the main process after every single answer, and again on every pop. At the 4MB cap that is a multi-megabyte serialise per answer, on the thread that draws the UI. A comment describing a guard nobody wrote is the arrow-tip defect again. | MAJOR | efficiency | OPEN |
| D4 | `main.js` `openHistory()` | The history window's `resize` handler writes the store on every event with no debounce, while `openSettings()` right above it debounces at `DROP_SETTLE_MS`. One drag is hundreds of synchronous disk writes. | MAJOR | efficiency | OPEN |
| D5 | `renderer/index.html` `#query-input` | The query field is `<input type="text">`: one line, no way to paste a stack trace or write a two-line question, on an app whose whole purpose is asking about errors and code. `INPUT_MIN_HEIGHT` 62, `INPUT_MAX_HEIGHT` 280 and the content-pinned window height all exist for a field that grows, and nothing grows. | MAJOR | missing feature | OPEN |
| D6 | `lib/openrouter.js` `friendlyError()` | The 404 copy asserts "the free model tried after it was not either" and the 429 copy asserts "every free model this app knows about was busy too". Neither is checked. On a paid model no stand-in is ever tried, and the cap is three attempts, never "every". The app is telling the user something it did not verify, which is the failure DESIGN.md spends a section on. | MINOR | honesty | OPEN |
| D7 | `preload.js` | `buddy:move` and `settings:data` are on the listen allowlist and appear nowhere else in the repository: nothing sends them, nothing listens for them. Dead entries widening the one surface that exists to be narrow. Same class as the `query:cancel` lie in v0.7. Found by `tools/check.js`. | MINOR | tidiness | OPEN |
| D8 | `lib/motion.js` `stop()` / `isAnimating()` | `stop()` reads `win.id` with no null guard, and `onToggleHotkey()` calls `motion.stop(outputWin)` without checking it. Probed: reading `.id` on a *destroyed* window is safe (Electron caches it), so this is narrower than it first looked, but a null window still throws `Cannot read properties of null`. | MINOR | correctness | OPEN |
| D9 | `main.js:158` | The `lastRegisterResults` initialiser omits `captureRegion`, which `applyRegistrations()` then sets. Harmless today, and it means `settings:get` can report an `active` map missing a key it documents. | MINOR | tidiness | OPEN |
| D10 | `lib/tray.js` `init()` | `click` and `double-click` both call `capture()`, so double-clicking the tray icon fires three screen captures and three panel expansions. | MINOR | correctness | OPEN |
| D11 | `lib/openrouter.js` `streamQuery()` | When the stream throws mid-way (an `error` frame, or an `ApiError` from a later line) the `ReadableStreamDefaultReader` is never released and the body is never cancelled, so the socket is held until GC. | MINOR | resource | OPEN |

## Closed with no change

None yet.

## Open at handoff

All rows are OPEN. Statuses move to FIXED only in the iteration whose gate run
proves it, and each fix must land a check that would catch the regression.
