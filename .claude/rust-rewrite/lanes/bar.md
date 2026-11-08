# Lane brief: bar

Branch `rust/bar`, cut from `rust-rewrite`. 18 items. Cross-lane tables (ownership, hotspots, contracts, decisions, merge order) are in `.claude/rust-rewrite/BACKLOG.md`; everything you need to do your items is in this file.

## Goal

The bar and the design system feel finished: one token system and shared components every page uses, glass that reads liquid at "very very" while text stays legible, light content handled, a bar that takes several lines, shows which window it continues, is fully usable from the keyboard, lets a question be pinned to a model, and says what is needed without naming one vendor. Every page applies the text scale and speaks through one announcement path.

## Files you own

`renderer/index.html`, `renderer/bar.js`, `renderer/overlay.js`, `renderer/intent.js`, `renderer/glass.css`, `renderer/glass.js`, `renderer/frame.js`, new `renderer/icons.svg`, `renderer/overlay.css` from the top through the "rows under the bar" section plus the "layout editor" section and the "reduced motion, contrast" section to the end, and (after harness C1) `src-tauri/src/selftest/probes/bar.rs`, `tools/smoke-labels/bar.txt`, `tools/ui-shots/lanes/bar.mjs`.

## Contracts you provide (C1 first)

- **K-40** design-visual §3 tokens in `glass.css`, `.sheet-bar`, `.g-menu`, `renderer/icons.svg`, `AevalGlass.scrimFor(rect, level)`, `--text-scale`, `--accent-text`.
- **K-41** `html[data-maximized]` from `frame.js`.
- **K-42** `AevalA11y.say(text, priority)` (falls back to the local live region until windows' `a11y:announce` lands).
- Later, not C1: **K-43** the `frame.js` window menu (B-158).

## Contracts you consume

- **K-05** target and count, **K-22** text scale, **K-23** window channels (windows).
- **K-18** `overlay:state.recipients`, **K-19** `models:list.providers` and `query:submit {provider, model}` (llm).
- **K-30**, **K-31** (harness).

## Smoke labels you port

`barKeys`, `keySwallow`, `intentTable`, `intentPrefixes`, `intentWorstMs`, `intentFast`, `intentDoNeedsMargin`, `suggestionsOnce`, `styleSamples`, `styleStable`.

## ui-shots scenes you own

design-visual §4 states 1-13 and 54-57, in `tools/ui-shots/lanes/bar.mjs`. List `glass.css` and `overlay.css` in `cssLintEnforce` once your regions are clean (lint runs per file, so coordinate with sheet in Progress before enforcing `overlay.css`).

## Shared-document entries you write

D-20, D-21, D-22, D-23, D-34, D-35 (and the page half of D-24); DESIGN Material, Shape, Palette, Type, Spacing and Motion entries from design-visual §3.

## Lane notes

- Sheet, settings and capture build on K-40 the day it lands. After C1, never rename a token or class; add new ones instead.
- REQUESTS #6: the intent token's tooltip and menu text stay as they are.

## Standing item: port your smoke labels

Every PARITY label under "Smoke labels you port" and every new label below is printed by `src-tauri/src/selftest/probes/bar.rs` through the K-30 API, with the Electron semantics from `.claude/rust-rewrite/audit/parity-ipc.md` §3 and `main.js` on `main`, and listed in `tools/smoke-labels/bar.txt` in the same commit that makes it print `=ok`. Acceptance: the rust workflow's smoke log shows each label `=ok` and the labels gate passes.

New labels your items add, beyond the PARITY list above (you print these): `barMultiline`, `earlyKeys`, `tokenReply`.

Labels another lane prints that your items rely on (do not print them yourself): `a11yRelay` (windows), `barOpenMs` (windows), `escClearsBeforeStop` (windows), `escKeepsAnswer` (windows), `windowNudge` (windows).

## Items

| ID | Severity | Title |
|---|---|---|
| B-054 | S1 | One token system and the shared components every page uses |
| B-055 | S1 | "Very very" reads liquid while text stays legible |
| B-056 | S1 | A question can span several lines |
| B-057 | S1 | The bar shows where the next question goes |
| B-058 | S1 | Announcements are heard wherever focus is |
| B-059 | S1 | Every page applies the text scale and handles Ctrl+=, Ctrl+- and Ctrl+0 |
| B-060 | S1 | Esc in the bar clears text, then closes the bar, and leaves outputs alone |
| B-150 | S2 | The intent token is a glyph at rest and tells the truth while a task waits |
| B-151 | S2 | A keyboard layer for the bar, with hints that keyboard users can see |
| B-152 | S2 | `@` picks a model for one question |
| B-153 | S2 | Esc clears a half-typed task answer before it stops the task |
| B-154 | S2 | The first keystrokes after the hotkey are never lost |
| B-155 | S2 | Enter without a provider visibly points at the fix |
| B-156 | S2 | Bar copy names what is needed, not one vendor, and shows short notes |
| B-157 | S2 | In a contrast theme the working indicator stays visible |
| B-158 | S2 | A window menu moves, sizes, maximizes and closes from the keyboard |
| B-159 | S2 | Glass turns light over light content |
| B-182 | S3 | Motion and light details in the bar |

Work in severity order (P0, then S1 and P1, then S2 and P2, then S3), after your C1 if you have one.

### B-054 · S1 · bar · One token system and the shared components every page uses
- **Sources:** design-visual §3 (type, spacing, radius, shadows, motion), F7 (component), F20, F21 (component), F30 (tokens), F34, F35, F37 (tokens), F40, F5 (frame.js); design-ux U9, U26, U8 (CSS half); parity-features #19 (token pass).
- **Files:** `renderer/glass.css`, `renderer/glass.js`, `renderer/frame.js`, `renderer/icons.svg` (new), `renderer/index.html`, `renderer/overlay.css` (bar regions).
- **Fix:** In bar C1 (K-40, K-41): add design-visual §3's tokens to `glass.css :root` verbatim (type `--t-*` with `--font-small`, `--w-body: 450`, `--w-strong: 600`; spacing `--sp-*`, `--stage`, `--inset-*`; radius `--r-sheet 28 / --r-card 16 / --r-plate 12 / --r-row 8 / --r-key 6 / --r-cap 999`; `--shadow-control`, `--shadow-sheet`, `--shadow-menu`; motion `--ease-*`, `--d-*`), every type size as `calc(Npx * var(--text-scale))` with `--text-scale: 1`, and `--ctl`, `--ctl-s`, `--bar-h`, `--sheet-bar-h` scaled too. `--accent-text: #EBC0E9` (dark tone; `#823A80` in the light tone, B-159) for every accent-coloured text and icon on glass; `--accent-bright` stays for fills, rims and the lens ring. Components: `.sheet-bar` (44 px, padding 0 8 0 16, title `600 13px/18px`, tools `.g-icon.small` 28 px gap 4, last button 8 px from the glass), `.g-menu` (`glass thin`, r16, padding 4, `--shadow-menu`, rows min-height 36 padding 0 12 r12, hover `--fill-1`, selected `--fill-2` plus the shared check, entrance `drop-in 220ms var(--ease-morph)`, exit 120 ms fade), `renderer/icons.svg` sprite (one X, chevron, check, copy, gear, history, plus, stop, clock, bubble, viewfinder, pointer; 20 px grid; `stroke-width: 1.5` with `vector-effect: non-scaling-stroke`, or Fluent System Icons Regular filled paths; sizes 18 in 32 px buttons, 16 in 28, 14 inline). Focus ring: `outline: 2px solid rgb(255 255 255 / .92); outline-offset: 2px` plus `box-shadow: var(--own-shadow, none), 0 0 0 5px rgb(0 0 0 / .38)`; the bar's `#bar:focus-within > .gl` gets the U26 double ring. `frame.js` sets `html[data-maximized]` from `getCurrentWindow().onResized` + `isMaximized()` and hides its handles when maximized. The intent menu adopts `.g-menu`. Record the radius rule "28/16/12/8/6/capsule, inner = outer − inset" and the palette entry for `--accent-text` in `entries/bar.md`.
- **Acceptance:** check `tokensPresent` (every §3 token resolves in a page) and `accentTextContrast` (≥ 4.5:1 over the matrix worst case) in `tools/ui-shots/lanes/bar.mjs`; scenes `bar-focus-ring`, `bar-menu`; SMOKE `styleSamples`.

### B-055 · S1 · bar · "Very very" reads liquid while text stays legible
- **Sources:** design-visual F6, F11; design-ux U51 (measure half).
- **Files:** `renderer/glass.css`, `renderer/glass.js`.
- **Fix:** Implement design-visual F6 steps 1-6 and F11 exactly: edge-clear scrim (`--scrim-edge: calc(var(--scrim) - 0.26 * var(--spec))`, interior difference drawn in `.gl::before` inset by half the band); blur held for reading (thin `18px - 8px*k`, thick `32px - 8px*k`, saturation per F6.2 with F11's thin `1.1 + 0.9k`); lensing `band = round(4 + 20k + 8*spec)`, `scale = round(base*2k + 12*spec)`, `spec = max(0, 2k-1)`; thickness shade and rim per F6.4; `morph(200)` on window move; scrim slope doubled over dark backdrops; thin scrim `0.58 + 0.24y`. `measure()` uses the 98th percentile luma when k > 0.5. Record the level table (design-visual §3 "Glass levels") in `entries/bar.md` as the DESIGN Material update. `refract` on Settings/History `#app` is the settings lane's markup (B-073).
- **Acceptance:** scenes `glass-level-{0,25,50,75,100}` on wallpaper, white, dark-editor and photo-busy (design-visual §4 state 45, bar half); check `glassStopsDiffer` (pixel difference between 0.5 and 1.0 frames over photo-busy above a threshold); SMOKE `styleStable`; harness matrix stays `failedFrames=0`.

### B-056 · S1 · bar · A question can span several lines
- **Sources:** parity-features #10 (polish e51ca29).
- **Files:** `renderer/index.html`, `renderer/bar.js`, `renderer/overlay.js`, `renderer/overlay.css` (bar regions).
- **Fix:** `#query-input` becomes a `<textarea rows="1">`; Enter sends, Shift+Enter inserts a newline, Alt+Enter and Ctrl+Enter keep their meanings; auto-grow to 7 rows (INPUT_MAX_HEIGHT 280 already in layout.rs) and report height through the existing overlay resize path; Up/Down recall only when the caret is on the first/last line.
- **Acceptance:** SMOKE+ `barMultiline` (bar probe pastes three lines via `Cx::input`, `query:submit` text contains two `\n`); scene `bar-multiline`; SMOKE `barKeys`.

### B-057 · S1 · bar · The bar shows where the next question goes
- **Sources:** design-ux U5 (bar half), C-14.
- **Files:** `renderer/index.html`, `renderer/bar.js`, `renderer/overlay.css` (bar regions).
- **Fix:** When `overlay:state.visibleOutputs >= 2` (K-05), show `#target-chip` between the field and the area button: `Continues: <title>` (28 characters, ellipsis, full title in `title`) and a 24 px x button `aria-label="Start a new conversation instead"` that sends `chat:new`. One window: no chip. The input's `aria-describedby` gains `Continues the conversation "<title>".` Ctrl+Tab / Ctrl+Shift+Tab invoke `output:cycle-target`.
- **Acceptance:** scene `bar-target-chip`; check `targetChipOnlyWithTwoWindows`.

### B-058 · S1 · bar · Announcements are heard wherever focus is
- **Sources:** design-ux U7 (renderer half).
- **Files:** `renderer/glass.js`, `renderer/index.html`, `renderer/overlay.js`.
- **Fix:** `glass.js` exports `AevalA11y.say(text, priority)` (K-42): speaks into the page's own live region when `document.hasFocus()`, else invokes `a11y:announce`; every page listens to `a11y:say` and speaks it locally. The bar's `#announce` gets a second, assertive region (`role="alert"`, `aria-live="assertive"`) for errors and task questions.
- **Acceptance:** check `sayRoutesByFocus` (mock: unfocused page invokes `a11y:announce`); SMOKE+ `a11yRelay` (shared with windows).

### B-059 · S1 · bar · Every page applies the text scale and handles Ctrl+=, Ctrl+- and Ctrl+0
- **Sources:** design-ux U8 (renderer half).
- **Files:** `renderer/glass.js`.
- **Fix:** On `ui:text-scale {scale}` set `--text-scale` on `:root`. Handle Ctrl+= / Ctrl+- / Ctrl+0 in a `keydown` listener (pages see keys before the bridge blocks browser zoom): step the stored factor by 0.1 within 1.0..2.0 and invoke `settings:set-text-scale`. glass.js is loaded by every page, so outputs, Settings, History, lens and buddy follow.
- **Acceptance:** scenes flagged `textScale: true` render at 200% (harness variant); check `zoomKeysInvokeTextScale`.

### B-060 · S1 · bar · Esc in the bar clears text, then closes the bar, and leaves outputs alone
- **Sources:** design-ux U3 (bar half), U19 (Esc row).
- **Files:** `renderer/bar.js`, `renderer/overlay.js`.
- **Fix:** Esc with text clears it; Esc on an empty bar sends `overlay:collapse` (which no longer aborts or hides outputs, B-018). Tooltip and name of the close button: `Close the bar` / `Close (Esc)` (C-13).
- **Acceptance:** check `escClearsThenCloses`; SMOKE+ `escKeepsAnswer` (shared with windows).

### B-150 · S2 · bar · The intent token is a glyph at rest and tells the truth while a task waits
- **Sources:** design-visual F12; design-ux U18, C-12.
- **Files:** `renderer/index.html`, `renderer/bar.js`, `renderer/intent.js`, `renderer/overlay.css` (bar regions).
- **Fix:** Empty and unlocked: a 32 x 32 glyph button (`padding: 0; width: var(--ctl); justify-content: center`, no fill, no rim, `--label-2`, hover `--fill-1`), `.intent-name` visually hidden. Typed text inferred Ask: glyph only. Inferred Find or Do, or any locked intent: the word appears with the existing FLIP width morph (240 ms `--ease-morph`). Locked: accent glass. While a task waits, `renderToken` uses `agentWaiting()`: `Reply` (asking), `Start` (plan), `Continue` (paused), Do glyph, locked style, menu disabled, `aria-label="<name>. Enter sends this to the running task."`. An explicit lock (prefix, Ctrl+1/2, menu) is honoured: `onEnter` routes to `agent:continue` only when `!bar.lock`. Tooltip and menu text unchanged (REQUESTS #6).
- **Acceptance:** scenes `bar-token-rest`, `bar-token-find`, `bar-token-reply`, `bar-token-start`; check `tokenSaysReplyWhileAsking`; SMOKE `intentTable`, SMOKE+ `tokenReply`.

### B-151 · S2 · bar · A keyboard layer for the bar, with hints that keyboard users can see
- **Sources:** design-ux U19 (bar column), U43, U44, C-13.
- **Files:** `renderer/bar.js`, `renderer/overlay.js`, `renderer/index.html`, `renderer/overlay.css` (bar regions).
- **Fix:** Bar keys per U19's table: Ctrl+N new conversation (`chat:new`), Ctrl+Tab / Ctrl+Shift+Tab cycle target, Ctrl+W close the target (`output:close`), Ctrl+Shift+W close all, Ctrl+Shift+C copy the target's last answer (`output:last-answer`), Ctrl+. stop (`query:cancel` / `agent:stop`), Ctrl+Shift+R retry the target (`query:retry {label}`), Ctrl+R ask about an area (`region:pick`), Ctrl+, Settings, Ctrl+H History, F6 to the target answer. Tooltips gain the key and buttons `aria-keyshortcuts`. U43 semantics: `#examples` `role="group" aria-label="Suggestions"`; guess card `role="group" aria-labelledby="guess-text"`, buttons with `aria-keyshortcuts="Enter"`/`"Alt+Enter"` and their `<kbd>` hidden from the name; `#region-clear` `aria-label="Stop using the area and ask about the whole screen"`; `#close-btn` `Close the bar`. U44: the token's tooltip text also shows as a small glass tooltip under the token on `:focus-visible` after 600 ms and on hover after 400 ms (only on the token).
- **Acceptance:** check `barShortcutsInvoke` (each chord invokes the right channel on the mock); SMOKE `barKeys`, `keySwallow`.

### B-152 · S2 · bar · `@` picks a model for one question
- **Sources:** design-ux U20 (bar half).
- **Files:** `renderer/bar.js`, `renderer/index.html`, `renderer/overlay.css` (bar regions).
- **Fix:** `@` as the first character opens `#model-menu` (`.g-menu`, `role="listbox"`) listing every ready provider's models grouped (`role="group" aria-label="<Provider>"`) from `models:list.providers` (K-19), filtered by the text after `@`; Enter or Tab picks; Esc closes and leaves the text; no match: `No model called "<text>". Esc to keep typing.` The pick becomes `#model-chip` after the token (`Sonnet 5.5` plus x, `aria-label="Model for this question: <name> via <Provider>. Press to remove."`); Backspace at position 0 removes the chip before the lock. Ask and Do send `{provider, model}`; with Find the chip shows `title="Find uses the Find model"` and is ignored.
- **Acceptance:** scenes `bar-model-menu`, `bar-model-chip`; check `modelPinSentWithSubmit`.

### B-153 · S2 · bar · Esc clears a half-typed task answer before it stops the task
- **Sources:** design-ux U17 (renderer half).
- **Files:** `renderer/bar.js`.
- **Fix:** Send `bar:dirty {dirty}` on input, debounced 100 ms; on `bar:clear` clear the field and keep focus.
- **Acceptance:** check `dirtyFlagSentAndClearHonoured`; SMOKE+ `escClearsBeforeStop` (shared with windows).

### B-154 · S2 · bar · The first keystrokes after the hotkey are never lost
- **Sources:** design-ux U25 (renderer half).
- **Files:** `renderer/overlay.js`, `renderer/bar.js`.
- **Fix:** A capture-phase `keydown` on `document`: if `state.mode === 'panel'`, focus is not in the field and the key is printable without Ctrl/Alt, focus the field and let the key through. Invoke `bar:ready` once the field is focused after an open.
- **Acceptance:** SMOKE+ `earlyKeys` (bar probe sends keys 5 ms after the hotkey; the field holds them); SMOKE+ `barOpenMs` (shared with windows).

### B-155 · S2 · bar · Enter without a provider visibly points at the fix
- **Sources:** design-ux U23, C-15.
- **Files:** `renderer/bar.js`, `renderer/overlay.css` (bar regions).
- **Fix:** When `query:submit` returns `needs-key`, replay a 240 ms 2 px horizontal `nudge` on `#notice` (fade under reduced motion), announce `Answers need an AI provider. Press Tab to connect one.`, and make the notice button the next tab stop after the field.
- **Acceptance:** check `needsKeyNudgesAndAnnounces`; SMOKE `queryWithoutKey` (shared with llm).

### B-156 · S2 · bar · Bar copy names what is needed, not one vendor, and shows short notes
- **Sources:** design-ux C-1..C-11, U6 (bar half), U38 (status note); design-visual F26 (bar copy).
- **Files:** `renderer/overlay.js`, `renderer/bar.js`, `renderer/index.html`.
- **Fix:** Apply copy deck rows C-1 to C-11 verbatim (from `.claude/rust-rewrite/audit/design-ux.md` §4 "Bar"). Notices that mention a provider read from `overlay:state.recipients` (K-18). Listen `bar:note {text}` (K-23) and show it in the status row for 4 s (used by the window cap, B-178).
- **Acceptance:** check `noOpenRouterInBarCopy` (with an Anthropic-only mock no bar string names OpenRouter); scene `bar-notice-nokey`, `bar-notice-hotkey-taken`.

### B-157 · S2 · bar · In a contrast theme the working indicator stays visible
- **Sources:** design-ux U29.
- **Files:** `renderer/glass.css`.
- **Fix:** `@media (forced-colors: active) { .glass.orbit { outline: 2px solid Highlight; outline-offset: -2px } #pill.orbit { outline-style: dashed } .pill-dot { forced-color-adjust: none; background: Mark } }`.
- **Acceptance:** scenes `forced-colors-bar-working`, `forced-colors-answer-streaming` (§4 state 55).

### B-158 · S2 · bar · A window menu moves, sizes, maximizes and closes from the keyboard
- **Sources:** design-ux U30 (renderer half).
- **Files:** `renderer/frame.js`, `renderer/glass.css`.
- **Fix:** Alt+Space or a right-click on a title bar opens a `.g-menu` `role="menu"`: `Move`, `Size`, `Maximize`/`Restore`, `Close`. In Move or Size, arrows move or resize 16 px (Shift 64) through `window:nudge` (K-23); Enter keeps, Esc reverts. frame.js is shared, so outputs, Settings and History all get it (K-43).
- **Acceptance:** scene `window-menu-open`; check `windowMenuNudges`; SMOKE+ `windowNudge` (shared with windows).

### B-159 · S2 · bar · Glass turns light over light content
- **Sources:** design-visual F10.
- **Files:** `renderer/glass.js`, `renderer/glass.css`.
- **Fix:** `glass.js` computes the median luma under each window's largest glass layer (y50) beside y90; `html[data-tone="light"]` when y50 > 0.72 for 2 consecutive frames, back to dark when y50 < 0.55; 300 ms crossfade including label colours. Light tokens, rim, scrim (thin `0.42 + 0.30(1-y)`, thick `0.62 + 0.20(1-y)`), accent (`--accent-text: #823A80`) and text-shadow exactly as F10 lists. Record the DESIGN Material departure ("dark tint always" → adaptive tone) in `entries/bar.md`.
- **Reconciled:** Decision D-20. The contrast gate gains light-tone runs (B-147).
- **Acceptance:** scenes `light-tone-{bar,answer,settings,history}` over white and photo-busy at 1 and 1.5 (§4 state 56); check `toneFlipsWithHysteresis`.

### B-182 · S3 · bar · Motion and light details in the bar
- **Sources:** design-visual F32, F42, F36 (bar rows).
- **Files:** `renderer/glass.css`, `renderer/glass.js`, `renderer/overlay.css` (bar regions and the reduced-motion tail), `renderer/index.html`.
- **Fix:** `--d-reduced: 150ms` used by every reduced-motion fade (overlay.css tail; glob.js reads the token, which sheet consumes). Drop the bar's word blur on fold under reduced motion (`filter: none`). `interactive` on `#bar` (and support for `.interactive` on sheet bars at 0.06); optional: buttons brighten to `--fill-3` on `:active` instead of shrinking. Snap `.intent-token` padding and gap to the 4-grid.
- **Acceptance:** scenes `reduced-motion-bar-morph`, `reduced-motion-token-lock` (§4 state 54); css-lint clean for `glass.css` and `overlay.css` bar regions.

## Rules

The coordinator's rules, verbatim: "LF endings; never run the app locally; never weaken or skip a test or gate to get green; commit messages in sentence case describing the effect like the existing log, each ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; push only its own branch".

What each one means here:

- **LF endings** in every file you write. Before each commit run `git diff --cached --check` and `git ls-files --eol <changed files>` (expect `i/lf w/lf`). The repo has no `.gitattributes`, so your editor's endings are what gets committed.
- **Never run the app locally.** No `cargo tauri dev`, no launching a built exe, no `npm start` of the Electron app. This machine has no Rust toolchain and nine lanes share it; CI is the build and the test bed. Local checks are limited to `node --check <file>` for JS and reading code.
- **Never weaken or skip a test or gate to get green.** No `#[ignore]`, no deleted or loosened assertions, no raised thresholds, no `continue-on-error`, no label removed from a label file, no scene removed from ui-shots. An expected value changes only where one of your items specifies the new value (for example B-049's gate tables or B-087's defaults test).
- **Commits** in sentence case describing the effect, like `git log --oneline -15` on `rust-rewrite` (for example "Keep the first-run tour out of the capture and desktop checks"). Item IDs may go in the body. Every message ends with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Push only your own branch**, `rust/bar`: `git push origin HEAD:rust/bar`. Never push `rust-rewrite`, `main`, `ci-logs`, `shots` or another lane's branch; never force-push.
- **Never poll the GitHub API (60 req/hr shared by the whole machine).** No `gh run`, `gh api`, `gh pr`, or `curl` to api.github.com. CI results come only from `tools/ci-wait.mjs`, which reads the `ci-logs` branch with git.
- Edit only the files listed under "Files you own". If you need a change elsewhere, write it under Blockers (file, the change, which item needs it) and carry on with other items. Never create the per-lane files that harness C1 creates (`selftest/probes/<lane>.rs`, `tools/smoke-labels/<lane>.txt`, `tools/ui-shots/lanes/<lane>.mjs`); merge `origin/rust-rewrite` once they exist.
- Do not edit `DESIGN.md`, `PARITY.md`, `ARCHITECTURE.md`, `REQUESTS.md`, `BACKLOG.md` (only B-149 appends to its "Post-merge findings" section), the audits, other lanes' briefs, the Electron reference (`main.js`, `lib/**` except files you own, `package*.json`, `electron-builder.yml`) or `README.md`.
- Never commit a real API key. Tests and probes use fake keys and the fake LLM.

## Verification loop

Every cycle:

1. Make a batch of changes: one or more items, each with its unit tests, probe labels, ui-shots scenes and checks, and its entries.
2. `git diff --cached --check`, `node --check` on changed `.js` and `.mjs` files, re-read the diff against the item specs.
3. Commit, then push once: `git push origin HEAD:rust/bar` (first push: `git push -u origin HEAD:rust/bar`).
4. Wait for CI with the script below. If the push touched `renderer/`, `settings/`, `history/`, `src-tauri/bridge/` or `tools/ui-shots/`, wait on `ui-shots` as well as `rust`.
5. Exit 0: record the items as green in Progress with the sha. Exit 1: read what it printed, fix, and go to step 1. Exit 2: follow the printed sha, or wait again.

The coordinator's CI recipe, verbatim:

- Usage: `node tools/ci-wait.mjs <sha> [workflow=rust|ui-shots] [--timeout-min 30]` (sha can be HEAD/short). It `git fetch`es the `ci-logs` branch every 45 s; no GitHub API.
- Exit 0 = green; 1 = failure (prints summary.md, errors.txt, last 80 lines of the failing gate's log, and the `git show origin/ci-logs:<dir>/fmt.patch | git apply` command when rustfmt fails); 2 = timeout or cancelled (prints the newer sha to wait on if a later push superseded it).
- Full logs: `origin/ci-logs:<ref with / as ->/<sha>/<workflow>/` holds summary.md, meta.json, errors.txt, and prebuild/fmt/clippy/test/build/smoke/capture/desktop .log (ui-shots: bridge/playwright/shots .log). `<ref>/latest/<workflow>/` = newest run.
- ui-shots only runs when renderer/settings/history/bridge/tools/ui-shots paths change, so `ci-wait <sha> ui-shots` exits 2 on other pushes; that is expected.
- Pushing a new commit to the same branch cancels the running CI for the older sha (concurrency cancel-in-progress), so batch changes and push once per cycle.

For this lane the log directory is `origin/ci-logs:rust-bar/<sha>/<workflow>/` and the newest run is `origin/ci-logs:rust-bar/latest/<workflow>/`. Read a file with `git show "origin/ci-logs:rust-bar/latest/rust/smoke.log"` (use PowerShell or set `MSYS_NO_PATHCONV=1` in Git Bash, which otherwise mangles the `ref:path` argument). The rust workflow takes up to an hour on a cold cache; use `--timeout-min 70` when the build is cold. When the same sha is the tip of two branches (right after merging `origin/rust-rewrite` with nothing new on top), add `--ref rust/bar`.

## Starting and staying current

1. Your worktree starts at `rust-rewrite` HEAD. Create the branch with `git switch -c rust/bar` if it does not exist.
2. Read this brief, the cross-lane tables in `.claude/rust-rewrite/BACKLOG.md` (lanes, ownership, hotspots, contracts), `DESIGN.md`, and for each item the audit sections and PARITY rows it cites. The code is the ground truth for current behaviour; `main.js` on `main` is the ground truth for Electron behaviour.
3. Push your C1 (if "Contracts you provide" lists any) before anything else, with a commit message starting `C1 contracts:`. When it is green, write `C1 green at <sha>` in Progress.
4. Each cycle, `git fetch origin`. When a contract you need has landed (`git grep -n "<symbol>" origin/rust-rewrite -- <file>`) or `rust-rewrite` has moved, `git merge origin/rust-rewrite` (a merge commit; never rebase or force-push). A conflict in a file you own: resolve it. A conflict in a file you do not own means you edited it by mistake: take `origin/rust-rewrite`'s side and note it in Progress.
5. If CI fails for a reason entirely outside your files (a broken merge on `rust-rewrite`, a runner outage), record it under Blockers with the log line and keep working on what you can verify.

## Shared documents

Never edit `DESIGN.md`, `PARITY.md` or `ARCHITECTURE.md`. In the same commit as the change, append to `.claude/rust-rewrite/entries/bar.md`:

```
## <DESIGN|PARITY|ARCHITECTURE> · <section or row id> · <D-xx or B-xxx>
Add | Replace "<exact current text>" with:
<exact new text>
```

For a PARITY row whose status or evidence changes: `## PARITY · <row id> · B-xxx` and the line `status: <value>; evidence: <UNIT test or SMOKE label>`. Every decision your lane owns (see "Shared-document entries you write") gets an entry with its reason. The merge agent folds the entries in when your branch merges.

## Stop condition

Stop when every item below and the standing labels task is green on CI at your branch head (rust, and ui-shots when your changes touch its paths), each recorded in Progress with its sha and acceptance; or when every item that is not green has a named entry under Blockers saying what blocks it, why, and what would unblock it. Do not stop with an item that is neither green nor blocked. When you stop, make sure the final state is pushed and the last `ci-wait` result is in Progress.

## Progress

(One line per item as it turns green: `- B-xxx: green at <sha>; <acceptance names>`. Also `C1 green at <sha>` and any notes the next agent needs.)

- C1 green at c114f4a (rust and ui-shots). K-40 tokens, `.sheet-bar`, `.g-menu`, `AevalGlass.scrimFor`, `--text-scale`, `--accent-text`; K-41 `html[data-maximized]`; K-42 `AevalA11y.say`.
- C1 follow-up (icons): at c114f4a, 0319420 and 8c73071 every icon was blank (ui-shots 01 and 10 show empty buttons). The cause was that `renderer/icons.svg` was not well-formed XML: its header comment contained a double hyphen (`--ico`), so the whole image failed to load. 0319420 and 8c73071 changed how glyphs are drawn without fixing that. The fix commit removes the double hyphen and was checked with an XML parser. Icons are now a CSS mask of the whole sprite strip, slid to glyph `--i` and filled with currentColor (an image, allowed by `img-src 'self'`). Markup is `<span class="ico i-<name>" aria-hidden="true"></span>` or `AevalGlass.icon(name)`. Consumers should take the fix sha (recorded below once green), not c114f4a.
- C1 icons fixed at d84a9d3: ui-shots green, with icons visible in 01-bar-first-run and 10-white-bar-output. rust failed only at Build installer, because the NSIS download returned HTTP 504 (runner network). The Rust sources are the same as 8c73071, whose rust run was green.
- B-059: code landed in C1 (c114f4a): `ui:text-scale`, Ctrl+= / Ctrl+- / Ctrl+0 stepping 1.0..2.0 and invoking `settings:set-text-scale`, with a localStorage fallback. Acceptance (`zoomKeysInvokeTextScale`, the 200% scene variant) waits for harness C1 (`tools/ui-shots/lanes/bar.mjs`).
- In progress (S1 batch): B-055 (level table in glass.css/glass.js, D-21 entries), B-056 (textarea, seven rows, Shift+Enter, recall only from the first or last row), B-057 (target chip, Ctrl+Tab; shows once K-05 lands), B-058 (the bar speaks through `AevalA11y.say`; capture failure and a taken shortcut are assertive), B-060 (Esc clears, then closes; `Close the bar` / `Close (Esc)`), and B-157 (forced-colors working outline).

## Blockers

(None yet. Format: `- B-xxx: <what blocks it>; <why>; <what would unblock it>`.)
