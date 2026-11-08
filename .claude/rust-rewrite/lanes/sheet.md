# Lane brief: sheet

Branch `rust/sheet`, cut from `rust-rewrite`. 23 items. Cross-lane tables (ownership, hotspots, contracts, decisions, merge order) are in `.claude/rust-rewrite/BACKLOG.md`; everything you need to do your items is in this file.

## Goal

An answer window reads like a typeset document and behaves like a native one: the glob reveals the answer inside the glass, markdown renders as real structure at a 640 px measure, streaming appends instead of re-rendering, code copies in one click, the usage line is quiet and "What was sent" is one click away, errors offer the fix, busy is a calm countdown, Start over can be undone, the Find view and agent trail work from the keyboard, and the confirm card shows the full text before Allow can be pressed.

## Files you own

`renderer/output.html`, `renderer/output.js`, `renderer/markdown.js`, `renderer/trail.js`, `renderer/glob.js`, the `renderer/overlay.css` sections "answer sheet", "answers", "find" and "agent trail", and (after harness C1) `src-tauri/src/selftest/probes/sheet.rs`, `tools/smoke-labels/sheet.txt`, `tools/ui-shots/lanes/sheet.mjs`.

## Contracts you provide

None. You can start immediately on items that need no contract (B-061, B-063, B-064, B-160, B-161).

## Contracts you consume

- **K-06** `output:meta`, **K-23** window channels (windows).
- **K-19** error, done and model payloads (llm; mock shapes in `tools/ui-shots/lanes/llm.mjs`).
- **K-20**, **K-21** confirm and consent payloads (agent).
- **K-40** tokens and components, **K-42** `AevalA11y.say`, **K-43** window menu (bar).
- **K-30**, **K-31** (harness).

## Smoke labels you port

`markdown`, `transcriptScroll`, `transcriptWheel`, `chatCeiling`, `scrollThumb`, `moreIndicator`, `noOutputInput`, `globMorphMs`.

## ui-shots scenes you own

design-visual §4 states 14-40, in `tools/ui-shots/lanes/sheet.mjs`.

## Shared-document entries you write

D-05, D-29; DESIGN entries for the answer sheet, markdown and trail specifics your items introduce.

## Standing item: port your smoke labels

Every PARITY label under "Smoke labels you port" and every new label below is printed by `src-tauri/src/selftest/probes/sheet.rs` through the K-30 API, with the Electron semantics from `.claude/rust-rewrite/audit/parity-ipc.md` §3 and `main.js` on `main`, and listed in `tools/smoke-labels/sheet.txt` in the same commit that makes it print `=ok`. Acceptance: the rust workflow's smoke log shows each label `=ok` and the labels gate passes.

New labels your items add, beyond the PARITY list above (you print these): `markdownStructure`, `readingMeasure`.

Labels another lane prints that your items rely on (do not print them yourself): `escKeepsAnswer` (windows).

## Items

| ID | Severity | Title |
|---|---|---|
| B-007 | P0 | The confirm card shows the full value and enables Allow only once it is visible |
| B-061 | S1 | The glob never hides its own words, and they arrive inside the glass |
| B-062 | S1 | The glob reads as one liquid body, and falls back cleanly when the sheet is not beside the bar |
| B-063 | S1 | Answers render as a typeset, accessible document |
| B-064 | S1 | A wide or maximized answer keeps a 640 px reading measure |
| B-065 | S1 | One title bar: the shared sheet bar, a glyph for the kind, tools that appear when wanted |
| B-066 | S1 | Nothing in an output says the same thing twice |
| B-067 | S1 | Esc in an output moves focus, never closes or stops anything |
| B-068 | S1 | Outputs announce through the relay, briefly |
| B-160 | S2 | Streaming appends instead of re-rendering the whole answer |
| B-161 | S2 | Code reads light and copies in one click; headings and rhythm follow the scale |
| B-162 | S2 | A quiet usage line and a "What was sent" panel |
| B-163 | S2 | Front and back outputs look different, and the target window wears a rim |
| B-164 | S2 | The sheet grows in the page and reports its rect |
| B-165 | S2 | The plan is the checklist |
| B-166 | S2 | Error rows offer the fix; busy is a calm note that counts down; a paid hop asks first |
| B-167 | S2 | Keyboard shortcuts in output windows |
| B-168 | S2 | "Start over" keeps the window and offers Undo |
| B-169 | S2 | Find's empty result offers next steps, and the results list works from the keyboard |
| B-170 | S2 | Task progress is heard |
| B-171 | S2 | The consent sheet says who receives the screen and that the mouse pauses |
| B-172 | S2 | Retry can pick another model |
| B-183 | S3 | Sheet refinements: Do it only for steps, a visible confirm settle, quieter chrome, one inset, one scale |

Work in severity order (P0, then S1 and P1, then S2 and P2, then S3), after your C1 if you have one.

### B-007 · P0 · sheet · The confirm card shows the full value and enables Allow only once it is visible
- **Sources:** review-llm-agent P0-1 (renderer half); design-ux U45 (sweep shares the gate).
- **Files:** `renderer/trail.js`, `renderer/overlay.css` (agent trail region).
- **Fix:** Render `value` (K-20) in a scrollable `pre`-wrapped block under the card title, max height 8 lines, `--font-mono` 13/20, with `aria-label="Text that will be typed"`. If `truncated` and the block overflows, Allow stays `aria-disabled="true"` until the block has been scrolled to its end or expanded with a `Show all` text button; then the 700 ms settle (U45) applies. Older payloads without `value` render as today.
- **Acceptance:** ui-shots scene `sheet-confirm-long-text` and check `confirmAllowWaitsForFullText` in `tools/ui-shots/lanes/sheet.mjs`.

### B-061 · S1 · sheet · The glob never hides its own words, and they arrive inside the glass
- **Sources:** design-visual F1.
- **Files:** `renderer/glob.js`, `renderer/output.html`, `renderer/overlay.css` (answer sheet region).
- **Fix:** Implement F1 steps 1-5 exactly: `#glob { z-index: 0 }` with `#opanel.globbing > :not(.gl) { position: relative; z-index: 1 }`; clip the words to the shape while globbing (`sheet.style.clipPath = path(<d translated by -sheetRect.left,-top>)`, `pt()` gains an offset; cleared in `finish()`); reveal at T_REVEAL 0.70 with the F1.3 CSS; close adds `closing` 80 ms before the shape reverses (F1.4); reduced motion keeps the 150 ms fade.
- **Acceptance:** virtual-clock scenes `glob-open-{0,4,10,18,30,45,62,72,82,90,100}` (§4 state 36); check `globWordsNeverOutsideShape` (pixels outside the shape mask are wallpaper in every frame); SMOKE+ `globMorphMs`.

### B-062 · S1 · sheet · The glob reads as one liquid body, and falls back cleanly when the sheet is not beside the bar
- **Sources:** design-visual F2, F25; parity-features #17 (NEW-05, renderer half).
- **Files:** `renderer/glob.js`, `renderer/output.html`, `renderer/overlay.css` (answer sheet region).
- **Fix:** Rewrite `shapeAt()` to F2's timeline table exactly (D_OPEN 480, D_CLOSE 260, `easeSpring = bezier(0.2, 0.9, 0.25, 1)`, phases A drop / B stretch-and-pinch with horizontal tangents and OVERLAP 6 / C recoil / Land / Words at 0.70). Add `#glob-thin` (`glass thin`, clipped by the same path, opacity 1→0 over t 0.10→0.55, `--scrim` from `AevalGlass.scrimFor(barRect, 'thin')`, K-40); the `#glob-mask` black rect over the bar plus 1 px; rim `stroke-width: calc(2 * var(--px))`; `#glob { filter: drop-shadow(0 4px 12px rgb(0 0 0 / .28)) }`. Fallback (F25): when the bar-to-sheet gap exceeds 14 px or they do not overlap on the cross axis, skip the shape and scale the sheet 0.94→1 toward the bar over 280 ms `--ease-morph` with the F1 reveal; close mirrors in 160 ms. Keep `globSmooth < 34 ms`.
- **Acceptance:** check `globBudArea` (at t = 0.04 the shape area is < 8% of the sheet's); scenes `glob-close-{0,25,50,75,100}`, `glob-above-bar`, `glob-side`, `glob-cascaded`, `glob-seam-150` (§4 states 37-40); check `globSmooth`.

### B-063 · S1 · sheet · Answers render as a typeset, accessible document
- **Sources:** design-visual F3; design-ux U11.
- **Files:** `renderer/markdown.js`, `renderer/output.js`, `renderer/overlay.css` (answers region).
- **Fix:** Lists stay open across blank lines when the next non-blank line is an item of the same type; `<ol start="N">`; nesting by `floor(leadingSpaces/2)` inside the current `<li>`; consecutive text lines join into one paragraph; tables (header `^\|.*\|$` plus separator `^\|?\s*:?-{3,}`) with alignment and `inline()` per cell inside `<div class="md-table"><table>`, with F3's table CSS; nested list CSS. Semantics: `#` → `<h2>`, `##` → `<h3>`, `###`+ → `<h4>` (classes `md-h md-hN` kept), paragraphs `<p class="md-p">`, fences `<pre><code>`, links `<a href="URL" data-href="URL" title="URL">label</a>` with ` <span class="md-host">(host)</span>` in `--label-2` 13 px when the label is not the host (click stays intercepted).
- **Acceptance:** scene `sheet-markdown-fixture` (§4 state 20: h1-h3, loose and tight ordered lists, nested lists, table, long code, inline code by punctuation, blockquote, hr, links) at 1 and 1.5; checks `looseListIsOneOl`, `tableRenders`, `headingsAreHeadings`, `linksShowHost`; SMOKE `markdown`, SMOKE+ `markdownStructure`.

### B-064 · S1 · sheet · A wide or maximized answer keeps a 640 px reading measure
- **Sources:** design-visual F4.
- **Files:** `renderer/overlay.css` (answer sheet and find regions).
- **Fix:** Delete the `padding: 2px 2px 2px 0` shorthand that cancels the measure; `--measure: 640px`; `#chat { padding-block: 2px; padding-inline-start: max(0px, calc((100% - var(--measure)) / 2)); padding-inline-end: max(8px, calc((100% - var(--measure)) / 2)); }`; `pre` and `.md-table` may use the full measure; same measure for `#find-view`.
- **Reconciled:** DESIGN says 760; 640 is about 68 characters at 15/24 Segoe UI Variable Text. Decision D-05.
- **Acceptance:** scene `sheet-maximized-1600` (§4 state 21); check `readingMeasure640` (chat text column width ≤ 640 at 1600 px); SMOKE+ `readingMeasure`.

### B-065 · S1 · sheet · One title bar: the shared sheet bar, a glyph for the kind, tools that appear when wanted
- **Sources:** design-visual F7 (outputs), F18, F20 (adoption), F42 (sheet bar light); design-ux U9 (`#copy-all[data-done]`).
- **Files:** `renderer/output.html`, `renderer/output.js`, `renderer/overlay.css` (answer sheet region).
- **Fix:** Use `.sheet-bar` (K-40) in output.html. Replace `.sheet-kind` text with the bar token's glyph (bubble, viewfinder, pointer from `icons.svg`) at 16 px in `--label-2`, keeping the word in `aria-label`. At rest show only Close; Copy, History and Start over fade in on `#sheet-bar:hover, :focus-within, body[data-scrolled="1"]` (opacity 0→1, 150 ms) keeping their slots. Hide the title-bar Copy while the conversation has one answer. All glyphs from the sprite. `class="interactive"` on `#sheet-bar` (0.06 strength). `#copy-all[data-done]` uses `--accent-text`.
- **Acceptance:** scenes `sheet-title-rest`, `sheet-title-hover` (§4 state 22 half); check `oneTitleBarSystem` (output, settings and history title bars share height and font).

### B-066 · S1 · sheet · Nothing in an output says the same thing twice
- **Sources:** design-visual F8 (renderer half); design-ux U9 (`.trow-note.waiting`).
- **Files:** `renderer/output.js`, `renderer/trail.js`, `renderer/overlay.css` (answer sheet and agent trail regions).
- **Fix:** First user message of a conversation gets `is-title` and is `.sr-only` (follow-ups show the chip; copy-all includes it). Drop the trail `Task` title span. Trail head status while asking: `Step N`. Row note shortened to `Answer in the bar`, colour `--accent-text`. When a failed row already shows the reason, the banner shows only the remedy (Rust side B-143).
- **Acceptance:** scenes `sheet-first-answer`, `sheet-task-asking`, `sheet-task-failed`; check `questionNotShownTwice`.

### B-067 · S1 · sheet · Esc in an output moves focus, never closes or stops anything
- **Sources:** design-ux U3 (output half).
- **Files:** `renderer/output.js`.
- **Fix:** Esc in an output: if the bar is open invoke `ui:focus-bar`, else `output:blur` (B-018). Remove the `overlay:collapse` call at output.js:574-576.
- **Acceptance:** check `outputEscNeverCollapses`; SMOKE+ `escKeepsAnswer`.

### B-068 · S1 · sheet · Outputs announce through the relay, briefly
- **Sources:** design-ux U7 (output half), C-37.
- **Files:** `renderer/output.js`, `renderer/trail.js`.
- **Fix:** All announcements go through `AevalA11y.say` (K-42). Answer done: `Answer ready. <first sentence, at most 160 characters>. Press F6 to read it.` (polite). Errors: the error text (assertive). Task asks: `The task has a question: <question>. Type your answer in the bar.` (assertive).
- **Acceptance:** check `answerAnnounceIsShort` (mock: `a11y:announce` text ≤ 220 characters, starts `Answer ready.`).

### B-160 · S2 · sheet · Streaming appends instead of re-rendering the whole answer
- **Sources:** design-visual F14.
- **Files:** `renderer/output.js`, `renderer/markdown.js`, `renderer/overlay.css` (answers region).
- **Fix:** Split the raw text at the last blank line outside a code fence; render earlier blocks once and append; re-render only the tail per frame. New blocks get `animation: md-in 160ms var(--ease-out)` (`from { opacity: 0; filter: blur(2px) }`). While busy the tail element gets a static 6 px trailing dot in `--label-2`, removed on done.
- **Acceptance:** scene `sheet-mid-stream` (§4 state 15); check `streamingKeepsEarlierNodes` (node identity of block 1 unchanged across 10 deltas); check `selectionSurvivesStream`.

### B-161 · S2 · sheet · Code reads light and copies in one click; headings and rhythm follow the scale
- **Sources:** design-visual F15, F16; design-ux U11 (Copy code button).
- **Files:** `renderer/markdown.js`, `renderer/output.js`, `renderer/overlay.css` (answers region).
- **Fix:** F15 CSS for inline code and `pre`; each fence wrapped in `<div class="md-code">` with the info string (`fs-meta`, `--label-2`) and a 28 px `.g-icon.small` Copy button (`aria-label="Copy code"`, top 6 right 6) shown on hover/focus-within and always on the last block; copies `codeBuf.join('\n')` and shows a check for 1.5 s; horizontal edge fade only when scrollable. F16: `.md-h1, .md-h2 { font: 600 17px/22px var(--font-text) }`, `.md-h3 15/22`, `.md-h { margin: 20px 0 6px }`, `.md-p { margin: 0 0 12px }`, `.md-gap { display: none }`, `.md-body { line-height: 24px }`, list spacing per F16.
- **Acceptance:** scene `sheet-markdown-fixture` (shared with B-063); check `codeCopyButtonCopies`.

### B-162 · S2 · sheet · A quiet usage line and a "What was sent" panel
- **Sources:** design-visual F17; design-ux U16 (renderer half), C-29; design-ux §3.4 (hop suffix).
- **Files:** `renderer/output.js`, `renderer/overlay.css` (answers region).
- **Fix:** Default usage line: `<model name> via <Provider> · <total s>` plus `· <Provider> was busy` after a hop; `free` only when paid models are on; full telemetry in `title` and the context menu (`Copy details`). `.msg-meta { flex-wrap: nowrap }`, `.usage` ellipsis in `--label-3`, hidden below 360 px (container query). A quiet `What was sent` disclosure (`aria-expanded`) opens U16's panel built from `query:done.sent` (K-19): `Sent to <Provider> (<model name>):` / `Your question` / `A screenshot of the whole screen, 1920 x 1080 [View]` (View loads `output:sent-image`) or `The text of the <app> window, 2,340 characters` / `<n> earlier messages from this conversation` / (free only) `Free models' providers may keep and train on what they receive.`
- **Reconciled:** F17 wants "Gemma 3 27B · 3.2 s"; U16/C-29 want "… via OpenRouter". Combined as above.
- **Acceptance:** scenes `sheet-done`, `sheet-sent-panel`; check `usageLineIsShort`.

### B-163 · S2 · sheet · Front and back outputs look different, and the target window wears a rim
- **Sources:** design-visual F19 (renderer half); design-ux U5 (target rim).
- **Files:** `renderer/output.js`, `renderer/overlay.css` (answer sheet region).
- **Fix:** From `output:meta` (K-06) set `body[data-front]` and `body[data-target]`. Back: title `--label-2`, the lighter shadow and `.gl::after` at .7 per F19, 150 ms transition. Target while the bar is open: `body[data-target="1"] #opanel > .gl::after` accent-text rim per U5.
- **Acceptance:** scenes `sheet-front-back`, `sheet-target-rim` (§4 state 22).

### B-164 · S2 · sheet · The sheet grows in the page and reports its rect
- **Sources:** design-visual F13 (renderer half).
- **Files:** `renderer/output.js`, `renderer/overlay.css` (answer sheet region).
- **Fix:** `:root { interpolate-size: allow-keywords; } #opanel { height: auto; transition: height var(--d-med) var(--ease-morph); overflow: clip; }`; `body[data-grow="up"] #opanel { top: auto; bottom: var(--stage); }` from `output:meta.grow`; `requestResize()` sends `output:glass {top, height}` once per frame instead of a window resize (K-23). Keep sending `output:resize` too until windows' B-027 lands on rust-rewrite, then stop.
- **Acceptance:** check `sheetReportsGlassRect`; SMOKE `answerFollowsTranscript` (shared with windows).

### B-165 · S2 · sheet · The plan is the checklist
- **Sources:** design-visual F22 (renderer half).
- **Files:** `renderer/trail.js`, `renderer/overlay.css` (agent trail region).
- **Fix:** Each act row nests under the plan item it serves (`agent:step.planIndex`, K-20) as a 13 px `--label-2` detail line, collapsed when done. Without `planIndex`, mark plan item i done when i acts completed and keep acts behind the "Steps" disclosure. Pending items `--label-2`, running `--label` with the arc, done `--label-2` with a check.
- **Acceptance:** scenes `sheet-task-running`, `sheet-task-done-expanded` (§4 states 29, 33); check `planChecksOff`.

### B-166 · S2 · sheet · Error rows offer the fix; busy is a calm note that counts down; a paid hop asks first
- **Sources:** design-ux U14, U15 (renderer half), §3.4 (paid-hop card); design-visual F31.
- **Files:** `renderer/output.js`, `renderer/overlay.css` (answers region).
- **Fix:** `addErrorRow(message, question, actions, tone)` renders `query:error.actions` (K-19) as `.retry-btn` capsules in order, each mapped to its effect (`Fix key` → `overlay:open-settings {section: 'prov-<id>'}`, `Choose model` → Settings Models, `Add credit` / privacy → `open-external`, `Retry`/`Try now` → `query:retry`, `hop` → `query:retry {label, provider, model}`, `never-ask` → `settings:set-failover {askBeforePaid:false}` then hop, `Copy details` → clipboard). `tone: 'note'` uses the neutral note row with a clock icon and a countdown from `retryInMs` (text updates each second, not announced; announced at start and at retry). F31: `.error-row { background: var(--fill-1); box-shadow: var(--chip-rim); border-radius: var(--r-plate) }`, danger glyph only in `--danger`, Retry wraps under long text.
- **Acceptance:** scenes `sheet-error-short`, `sheet-error-long`, `sheet-busy-countdown`, `sheet-paid-hop` (§4 state 18); check `errorActionsInvoke`.

### B-167 · S2 · sheet · Keyboard shortcuts in output windows
- **Sources:** design-ux U19 (output column), C-25, C-26, C-27.
- **Files:** `renderer/output.js`, `renderer/output.html`.
- **Fix:** Ctrl+N (new conversation, focus to the bar), Ctrl+Tab / Ctrl+Shift+Tab (focus next/previous output via `output:cycle-target`), Ctrl+W (`output:close {label}`), Ctrl+Shift+W (`output:close-all`), Ctrl+Shift+C (copy this conversation), Ctrl+. (stop), Ctrl+Shift+R (retry here), Ctrl+, (Settings), Ctrl+H (History), F6 to the bar. Tooltips `Close (Ctrl+W)`, `Copy conversation (Ctrl+Shift+C)`, `History (Ctrl+H)`, `Stop (Ctrl+.)`, with `aria-keyshortcuts`.
- **Acceptance:** check `outputShortcutsInvoke`.

### B-168 · S2 · sheet · "Start over" keeps the window and offers Undo
- **Sources:** design-ux U22 (renderer half), C-24, C-34, C-35.
- **Files:** `renderer/output.js`, `renderer/output.html`.
- **Fix:** Rename `+` to `Start over` (`aria-label="Start over in this window"`, tooltip `Start over`). On `chat:reset` keep the sheet visible with one neutral row `Started over. The last conversation is in History.` and `Undo` (`output:undo-reset`, K-23); the empty sheet reads `Your next question goes here.`
- **Acceptance:** scene `sheet-started-over`; check `startOverOffersUndo`.

### B-169 · S2 · sheet · Find's empty result offers next steps, and the results list works from the keyboard
- **Sources:** design-ux U24, U28.
- **Files:** `renderer/trail.js`, `renderer/output.html`, `renderer/overlay.css` (find region).
- **Fix:** Under the empty text, buttons `Ask about "<query>"` (`find:to-ask`) and, when scope is the window, `Look on the whole screen` (`find:scope`); no key hints. Results keep `role="listbox"` with roving focus: rows `tabindex="-1"` except the current (`0`), Up/Down/Home/End move focus and send `find:step {index}`, Enter goes to the hit.
- **Acceptance:** scenes `find-empty`, `find-results` (§4 states 24, 25); check `findListboxArrows`.

### B-170 · S2 · sheet · Task progress is heard
- **Sources:** design-ux U27, C-39.
- **Files:** `renderer/trail.js`.
- **Fix:** Each row gets an `.sr-only` prefix (`Planned:`, `Now:`, `Done:`, `Skipped:`, `Failed:`). Announce each new act when it starts, throttled to one per 2 s (`Step 5: Type "notes.txt" in Notepad`), failures immediately, via `AevalA11y.say`; confirm cards stay assertive with `aria-describedby` on `.confirm-reason`.
- **Acceptance:** check `trailRowsHaveSrPrefixes`, `stepAnnouncesThrottled`.

### B-171 · S2 · sheet · The consent sheet says who receives the screen and that the mouse pauses
- **Sources:** design-ux U31, U6 (consent half), C-30, C-31.
- **Files:** `renderer/trail.js`.
- **Fix:** Build C-30 from `agent:consent-request.{provider, modelName, recipients}` (K-21): `Each step sends a screenshot and the window's text to <Provider> (<model name>).` plus the free-model line only for free models. C-31: `Move the mouse to pause it. Press Esc to stop it.`
- **Acceptance:** scene `sheet-consent` (§4 state 27); check `consentNamesProvider`.

### B-172 · S2 · sheet · Retry can pick another model
- **Sources:** design-ux U20 (renderer half).
- **Files:** `renderer/output.js`, `renderer/overlay.css` (answers region).
- **Fix:** Retry on answers and error rows becomes a split button: `Retry` and a 28 px chevron `aria-label="Retry with another model"` opening the same grouped list (`models:list.providers`, `.g-menu`); sends `query:retry {label, provider, model}`.
- **Acceptance:** scene `sheet-retry-menu`; check `retryWithModelInvokes`.

### B-183 · S3 · sheet · Sheet refinements: Do it only for steps, a visible confirm settle, quieter chrome, one inset, one scale
- **Sources:** design-ux U39, U45, C-20, C-28; design-visual F26 (wait message, confirm hint), F30 (plates), F33, F34, F35, F36 (sheet rows), F37 (sheet plates), F41.
- **Files:** `renderer/output.js`, `renderer/trail.js`, `renderer/overlay.css` (answer sheet, answers, find and agent trail regions).
- **Fix:** Show "Do it" only when at least two list lines match `^\s*(\d+[.)]|[-*])\s+(click|double-click|right-click|press|open|type|select|choose|go to|drag|enter)\b`; its tooltip C-28. Confirm buttons `aria-disabled="true"` with a 700 ms fill sweep (static under reduced motion), then enabled (shares B-007's gate). Wait text C-20 (`Waiting for <model name> · N s. Free models queue when they're busy.`, explanation in `title`). Remove the confirm card's visible "Tab to a button, then Enter" hint (still announced). Plates (`.confirm-card`, `.msg.consent`) lose their drop shadow. Delete `#opanel::after`; scrollbar thumb `rgb(255 255 255 / .18)` at rest, `.40` while `body[data-scrolling]` (800 ms after a scroll). Display face only at 18 px and up; `strong` 600; off-grid values in F36's sheet rows snapped; plates r12 (`--r-plate`); text starts 16 px from the sheet edge in every kind with marks hung in the gutter (F41: `.find-row { padding: 8px 12px 8px 4px }`, `.find-view { margin: 0 -10px 0 -12px }`, trail matched).
- **Acceptance:** checks `doItOnlyForSteps`, `confirmSweepThenEnable`, `textInsetIs16InEveryKind`; css-lint clean for overlay.css sheet regions.

## Rules

The coordinator's rules, verbatim: "LF endings; never run the app locally; never weaken or skip a test or gate to get green; commit messages in sentence case describing the effect like the existing log, each ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; push only its own branch".

What each one means here:

- **LF endings** in every file you write. Before each commit run `git diff --cached --check` and `git ls-files --eol <changed files>` (expect `i/lf w/lf`). The repo has no `.gitattributes`, so your editor's endings are what gets committed.
- **Never run the app locally.** No `cargo tauri dev`, no launching a built exe, no `npm start` of the Electron app. This machine has no Rust toolchain and nine lanes share it; CI is the build and the test bed. Local checks are limited to `node --check <file>` for JS and reading code.
- **Never weaken or skip a test or gate to get green.** No `#[ignore]`, no deleted or loosened assertions, no raised thresholds, no `continue-on-error`, no label removed from a label file, no scene removed from ui-shots. An expected value changes only where one of your items specifies the new value (for example B-049's gate tables or B-087's defaults test).
- **Commits** in sentence case describing the effect, like `git log --oneline -15` on `rust-rewrite` (for example "Keep the first-run tour out of the capture and desktop checks"). Item IDs may go in the body. Every message ends with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Push only your own branch**, `rust/sheet`: `git push origin HEAD:rust/sheet`. Never push `rust-rewrite`, `main`, `ci-logs`, `shots` or another lane's branch; never force-push.
- **Never poll the GitHub API (60 req/hr shared by the whole machine).** No `gh run`, `gh api`, `gh pr`, or `curl` to api.github.com. CI results come only from `tools/ci-wait.mjs`, which reads the `ci-logs` branch with git.
- Edit only the files listed under "Files you own". If you need a change elsewhere, write it under Blockers (file, the change, which item needs it) and carry on with other items. Never create the per-lane files that harness C1 creates (`selftest/probes/<lane>.rs`, `tools/smoke-labels/<lane>.txt`, `tools/ui-shots/lanes/<lane>.mjs`); merge `origin/rust-rewrite` once they exist.
- Do not edit `DESIGN.md`, `PARITY.md`, `ARCHITECTURE.md`, `REQUESTS.md`, `BACKLOG.md` (only B-149 appends to its "Post-merge findings" section), the audits, other lanes' briefs, the Electron reference (`main.js`, `lib/**` except files you own, `package*.json`, `electron-builder.yml`) or `README.md`.
- Never commit a real API key. Tests and probes use fake keys and the fake LLM.

## Verification loop

Every cycle:

1. Make a batch of changes: one or more items, each with its unit tests, probe labels, ui-shots scenes and checks, and its entries.
2. `git diff --cached --check`, `node --check` on changed `.js` and `.mjs` files, re-read the diff against the item specs.
3. Commit, then push once: `git push origin HEAD:rust/sheet` (first push: `git push -u origin HEAD:rust/sheet`).
4. Wait for CI with the script below. If the push touched `renderer/`, `settings/`, `history/`, `src-tauri/bridge/` or `tools/ui-shots/`, wait on `ui-shots` as well as `rust`.
5. Exit 0: record the items as green in Progress with the sha. Exit 1: read what it printed, fix, and go to step 1. Exit 2: follow the printed sha, or wait again.

The coordinator's CI recipe, verbatim:

- Usage: `node tools/ci-wait.mjs <sha> [workflow=rust|ui-shots] [--timeout-min 30]` (sha can be HEAD/short). It `git fetch`es the `ci-logs` branch every 45 s; no GitHub API.
- Exit 0 = green; 1 = failure (prints summary.md, errors.txt, last 80 lines of the failing gate's log, and the `git show origin/ci-logs:<dir>/fmt.patch | git apply` command when rustfmt fails); 2 = timeout or cancelled (prints the newer sha to wait on if a later push superseded it).
- Full logs: `origin/ci-logs:<ref with / as ->/<sha>/<workflow>/` holds summary.md, meta.json, errors.txt, and prebuild/fmt/clippy/test/build/smoke/capture/desktop .log (ui-shots: bridge/playwright/shots .log). `<ref>/latest/<workflow>/` = newest run.
- ui-shots only runs when renderer/settings/history/bridge/tools/ui-shots paths change, so `ci-wait <sha> ui-shots` exits 2 on other pushes; that is expected.
- Pushing a new commit to the same branch cancels the running CI for the older sha (concurrency cancel-in-progress), so batch changes and push once per cycle.

For this lane the log directory is `origin/ci-logs:rust-sheet/<sha>/<workflow>/` and the newest run is `origin/ci-logs:rust-sheet/latest/<workflow>/`. Read a file with `git show "origin/ci-logs:rust-sheet/latest/rust/smoke.log"` (use PowerShell or set `MSYS_NO_PATHCONV=1` in Git Bash, which otherwise mangles the `ref:path` argument). The rust workflow takes up to an hour on a cold cache; use `--timeout-min 70` when the build is cold. When the same sha is the tip of two branches (right after merging `origin/rust-rewrite` with nothing new on top), add `--ref rust/sheet`.

## Starting and staying current

1. Your worktree starts at `rust-rewrite` HEAD. Create the branch with `git switch -c rust/sheet` if it does not exist.
2. Read this brief, the cross-lane tables in `.claude/rust-rewrite/BACKLOG.md` (lanes, ownership, hotspots, contracts), `DESIGN.md`, and for each item the audit sections and PARITY rows it cites. The code is the ground truth for current behaviour; `main.js` on `main` is the ground truth for Electron behaviour.
3. Push your C1 (if "Contracts you provide" lists any) before anything else, with a commit message starting `C1 contracts:`. When it is green, write `C1 green at <sha>` in Progress.
4. Each cycle, `git fetch origin`. When a contract you need has landed (`git grep -n "<symbol>" origin/rust-rewrite -- <file>`) or `rust-rewrite` has moved, `git merge origin/rust-rewrite` (a merge commit; never rebase or force-push). A conflict in a file you own: resolve it. A conflict in a file you do not own means you edited it by mistake: take `origin/rust-rewrite`'s side and note it in Progress.
5. If CI fails for a reason entirely outside your files (a broken merge on `rust-rewrite`, a runner outage), record it under Blockers with the log line and keep working on what you can verify.

## Shared documents

Never edit `DESIGN.md`, `PARITY.md` or `ARCHITECTURE.md`. In the same commit as the change, append to `.claude/rust-rewrite/entries/sheet.md`:

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

Built in 2add4c5 (answers), 91b2881 (glob), d537e9e (trail, confirm, Find), 57cf07d (scenes and checks), 1bedb11 (probe labels). Every item below is verified by the push that adds this section; its sha is the one reported to main as "sheet green".

- Standing labels: noOutputInput, markdown, markdownStructure, readingMeasure, transcriptScroll, transcriptWheel, chatCeiling, scrollThumb, moreIndicator, globMorphMs printed by probes/sheet.rs and listed in tools/smoke-labels/sheet.txt.
- B-007: confirmAllowWaitsForFullText; sheet-confirm-long-text.
- B-061: glob-open-*; globWordsNeverOutsideShape, globOpenOnClock; SMOKE globMorphMs.
- B-062: glob-close-*, glob-above-bar, glob-side, glob-cascaded; globBudArea, globSmooth, globFallsBackToScale.
- B-063: sheet-markdown-fixture; markdownNeverInjects, looseListIsOneOl, tableRenders, headingsAreHeadings, linksShowHost; SMOKE markdown, markdownStructure.
- B-064: sheet-maximized-1600; readingMeasure640; SMOKE readingMeasure.
- B-065: sheet-title-rest, sheet-title-hover; outputTitleBarIsSheetBar, oneTitleBarSystem.
- B-066: sheet-first-answer, sheet-task-asking, sheet-task-failed; questionNotShownTwice, failureShownOnce.
- B-067: outputEscNeverCollapses (escKeepsAnswer is the windows lane's label).
- B-068: answerAnnounceIsShort.
- B-160: sheet-mid-stream, sheet-mid-stream-fence; streamingKeepsEarlierNodes, selectionSurvivesStream.
- B-161: codeCopyButtonCopies.
- B-162: sheet-done, sheet-sent-panel; usageLineIsShort.
- B-163: sheet-front-back, sheet-target-rim.
- B-164: sheetReportsGlassRect (output:resize still sent until windows' B-027 lands).
- B-165: sheet-task-running, sheet-task-done-expanded; planChecksOff.
- B-166: sheet-error-short, sheet-error-long, sheet-busy-countdown, sheet-paid-hop; errorActionsInvoke, longErrorFixesWrap.
- B-167: outputShortcutsInvoke.
- B-168: sheet-started-over; startOverOffersUndo.
- B-169: find-empty, find-results; findListboxArrows.
- B-170: trailRowsHaveSrPrefixes, stepAnnouncesThrottled.
- B-171: sheet-consent; consentNamesProvider.
- B-172: sheet-retry-menu; retryWithModelInvokes.
- B-183: doItOnlyForSteps, confirmSweepThenEnable, textInsetIs16InEveryKind, sheetCssLint (B-145's rules run inline on the sheet's regions of overlay.css; the shared gate in bridge.test.mjs takes over at B-148).

Notes for the next agent and the integrator:
- 1bedb11 failed only because a paused ui-shots clock leaked into every later scene (the context owns page.clock). sheet.mjs now resumes the clock when each paused scene ends (onStoppedClock); harness 76c8c95 (on rust/harness, not yet on rust-rewrite) gives clock scenes their own context, and the two are compatible.
- The markdownStructure probe's table used a two-dash separator, which B-063's spec (`-{3,}`) does not accept; the probe now uses three.

## Blockers

(None yet. Format: `- B-xxx: <what blocks it>; <why>; <what would unblock it>`.)
