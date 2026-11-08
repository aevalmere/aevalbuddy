# Lane brief: settings

Branch `rust/settings`, cut from `rust-rewrite`. 12 items. Cross-lane tables (ownership, hotspots, contracts, decisions, merge order) are in `.claude/rust-rewrite/BACKLOG.md`; everything you need to do your items is in this file.

## Goal

Settings and History are as calm and clear as the rest: a Providers page that holds, tests, orders and explains every key with OpenRouter one click away, rows that are real disclosure buttons, a slider whose labels sit on its stops, models per provider with an automatic default, a Text size row, privacy copy that names who receives the screen, and a History list that reads calmly, works from the keyboard and can undo a delete.

## Files you own

`settings/**`, `history/**`, and (after harness C1) `src-tauri/src/selftest/probes/settings.rs`, `tools/smoke-labels/settings.txt`, `tools/ui-shots/lanes/settings.mjs`.

## Contracts you provide

None. You can start immediately on B-070, B-071, B-072 and B-175.

## Contracts you consume

- **K-18** recipients, **K-19** provider channels and payloads (llm; mock shapes in `tools/ui-shots/lanes/llm.mjs`).
- **K-22** text scale, **K-24** `history:list.enabled` and `overlayPos` (windows).
- **K-40** tokens and components, **K-41** `html[data-maximized]`, **K-42** `AevalA11y.say`, **K-43** window menu (bar).
- **K-31** (harness).

## Smoke labels you port

None from PARITY (Settings and History rows are proven by ui-shots checks and by other lanes' probes). Add probe labels only where an item names one.

## ui-shots scenes you own

design-visual §4 states 41-50, in `tools/ui-shots/lanes/settings.mjs`.

## Shared-document entries you write

DESIGN entries for the Providers page (design-ux §3), Settings rows and History specifics your items introduce; PARITY evidence for the Settings and History rows they close.

## Standing item: port your smoke labels

Every PARITY label under "Smoke labels you port" and every new label below is printed by `src-tauri/src/selftest/probes/settings.rs` through the K-30 API, with the Electron semantics from `.claude/rust-rewrite/audit/parity-ipc.md` §3 and `main.js` on `main`, and listed in `tools/smoke-labels/settings.txt` in the same commit that makes it print `=ok`. Acceptance: the rust workflow's smoke log shows each label `=ok` and the labels gate passes.

Your items add no new smoke labels.

## Items

| ID | Severity | Title |
|---|---|---|
| B-069 | S1 | A Providers page that holds, shows and orders every key |
| B-070 | S1 | Settings rows are real buttons with state, and actions look like actions |
| B-071 | S1 | History rows are two separate buttons, the delete target is 28 px, and deleting can be undone |
| B-072 | S1 | The glass slider's labels sit on their stops, with ticks and a magnetic snap |
| B-073 | S1 | Settings and History share the sheet bar, sit on the stage margin, refract, and go full bleed when maximized |
| B-074 | S1 | A Text size row |
| B-075 | S1 | The privacy and task copy says who receives the screen and what the mouse does |
| B-173 | S2 | Models per provider, with an automatic default |
| B-174 | S2 | History reads calmly, tells the truth when off, and works from the keyboard |
| B-175 | S2 | One layer of CSS in Settings and History, on the token scale |
| B-176 | S2 | Pickers and choices follow the shared menu and ARIA patterns; Delete all confirms inline |
| B-184 | S3 | Small Settings doors and notes |

Work in severity order (P0, then S1 and P1, then S2 and P2, then S3), after your C1 if you have one.

### B-069 · S1 · settings · A Providers page that holds, shows and orders every key
- **Sources:** design-ux U2, §3.1-3.6, U1 (renderer half), U21 (renderer half); copy deck C-50, C-51, C-57, C-58, C-65; parity-features #2.
- **Files:** `settings/settings.html`, `settings/settings.js`, `settings/settings.css`.
- **Fix:** Rail `Account` → `Providers`, page `#pg-providers`, `h1 Providers`. Build design-ux §3.1's layout and §3.2's state table verbatim (every state's status word, body text and actions; copy from `.claude/rust-rewrite/audit/design-ux.md` §3.2), §3.3's ordering (drag, `Move up`/`Move down`/`Use first`, Alt+Up/Down; announce `<Provider> is now first. It answers by default.`), the two failover switches (`When one is busy or fails, try the next`; `Ask me before switching from free models to a paid provider`) and the storage line (C-58, from `keyStorage`). OpenRouter's Not-added state leads with `Connect OpenRouter` (`auth:openrouter-connect`) and a `Paste a key instead` link. Channels and payloads: K-15/K-19 (`settings:save-key {provider,key}`, `clear-key {provider}`, `test-key {provider}`, `set-provider-order`, `set-failover`, `clip-key`, `provider:status`, `settings:get.providers/failover/keyHints/keyStorage`). §3.6 accessibility: `<ol id="providers" aria-label="Providers, in the order AevalBuddy tries them">`, row button names `OpenRouter, 1 of 4, Ready, default`, `aria-expanded`/`aria-controls`, visible field labels, focus returns to the row after Remove, Replace or a move; one `role="status"` line speaks status changes.
- **Acceptance:** scenes `settings-providers-{none,ready-two,rejected,no-credit,busy,blocked,testing,removing,wrong-provider}` (§4 state 42 expanded); checks `providerRowsHaveExpandedState`, `removeSendsProvider`, `reorderInvokesOrder`.

### B-070 · S1 · settings · Settings rows are real buttons with state, and actions look like actions
- **Sources:** design-ux U13; design-visual F23.
- **Files:** `settings/settings.html`, `settings/settings.js`, `settings/settings.css`.
- **Fix:** Each `.body` gets an id; the whole `.row-head` is the disclosure button (role and handler on the row, `.edit::before { content:""; position:absolute; inset:0 }` on a `position:relative` row), `aria-expanded`, `aria-controls`, `aria-labelledby="<h2 id> <summary id>"` (reads "API key, Connected, collapsed"); hover `--fill-1` with radii by position. Action rows get a trailing text button instead of a chevron (`Move…` for bar position, `Replay` for the tour) with value summaries (`Top right`, `4 steps`). Disclosure animates with `grid-template-rows: 0fr → 1fr` over `--d-med var(--ease-morph)`.
- **Acceptance:** checks `noButtonNamedEdit`, `everyDisclosureHasExpanded`; scene `settings-row-hover-disclosing` (§4 state 48).

### B-071 · S1 · settings · History rows are two separate buttons, the delete target is 28 px, and deleting can be undone
- **Sources:** design-ux U12, C-81.
- **Files:** `history/history.html`, `history/history.js`, `history/history.css`.
- **Fix:** Row markup `<div class="item" role="listitem"><button class="item-open">…</button><button class="del" aria-label='Delete "<title>"'>…</button></div>` inside `role="list"`; `.del` 28 x 28, visible at rest in `--label-3`, `--label` on hover and focus. Delete hides the row and shows a footer note `Deleted "<title>". Undo` for 6 s; `history:remove` is sent when the note expires or on `pagehide`. The Delete key on a row deletes with the same undo.
- **Acceptance:** checks `noNestedButtons`, `deleteIsUndoable`; scene `history-row-hover`, `history-deleted-undo`.

### B-072 · S1 · settings · The glass slider's labels sit on their stops, with ticks and a magnetic snap
- **Sources:** design-visual F9.
- **Files:** `settings/settings.css`, `settings/settings.js`.
- **Fix:** F9 exactly: absolute-positioned stop labels at `left: calc(11px + (100% - 22px) * n/4)` with `translateX(-50%)` (ends flush), track ticks via the radial-gradient background, snap to the nearest 0.25 within 0.04 on `change`, current stop label `--label`, others `--label-2`.
- **Acceptance:** check `sliderLabelsCentredOnStops` (each label's centre within 1 px of its stop); scenes `settings-glass-{0,25,50,75,100}`.

### B-073 · S1 · settings · Settings and History share the sheet bar, sit on the stage margin, refract, and go full bleed when maximized
- **Sources:** design-visual F7 (Settings/History), F24 (CSS half), F5 (renderer half), F6.5 (`refract`), F20 (adoption).
- **Files:** `settings/settings.html`, `settings/settings.css`, `history/history.html`, `history/history.css`.
- **Fix:** Use `.sheet-bar` (left-aligned title, 28 px close from the sprite; drop `justify-content: center` and the absolutely positioned close). `body { padding: var(--stage) }` on both pages (the native band moves to the glass with B-024). `class="glass thick refract"` on `#app`. `html[data-maximized] body { padding: 0 } html[data-maximized] #app { --r: 0px; border-radius: 0; }` (K-41).
- **Acceptance:** scenes `settings-maximized`, `history-maximized` (§4 states 47, 50); check `oneTitleBarSystem` (shared with sheet).

### B-074 · S1 · settings · A Text size row
- **Sources:** design-ux U8 (Settings half), C-59.
- **Files:** `settings/settings.html`, `settings/settings.js`.
- **Fix:** Settings, General: `Text size` slider 100% to 200% step 10 with the note `Starts from your Windows text size.`; reads `textScale` and `osTextScale` from `settings:get`, writes `settings:set-text-scale` (K-22).
- **Acceptance:** scene `settings-text-size`; check `textSizeRowInvokes`.

### B-075 · S1 · settings · The privacy and task copy says who receives the screen and what the mouse does
- **Sources:** design-ux U6 (Settings half), U31 (Settings half); copy deck C-32, C-52.
- **Files:** `settings/settings.html`, `settings/settings.js`.
- **Fix:** The Privacy "Who sees your screen" paragraph is generated from `settings:get.recipients` (K-18) with C-32's text. The Tasks note becomes C-52 (`Esc, the Stop button or the tray stops it; moving the mouse pauses it. Screenshots are never saved.`).
- **Acceptance:** check `privacyNamesRecipients` (mock Anthropic-first: paragraph names Anthropic before OpenRouter).

### B-173 · S2 · settings · Models per provider, with an automatic default
- **Sources:** design-ux U35, §3.3 (models half).
- **Files:** `settings/settings.html`, `settings/settings.js`, `settings/settings.css`.
- **Fix:** Top row `Choose models: Automatic (recommended)` / `Choose for each job`; the four job pickers show only in the second. A segmented `Models for:` with one segment per provider that has a key (default: the Default provider); each job row edits that provider's pick (`settings:save-model* {provider}`, K-19); `Show paid models` only on the OpenRouter segment, others show `Every model here is billed per question.`
- **Acceptance:** scenes `settings-models-automatic`, `settings-models-anthropic`, `settings-models-combo-open` (§4 state 43); check `modelSaveSendsProvider`.

### B-174 · S2 · settings · History reads calmly, tells the truth when off, and works from the keyboard
- **Sources:** design-visual F39; design-ux U36, U32 (renderer half), U37, C-80, C-82.
- **Files:** `history/history.html`, `history/history.js`, `history/history.css`.
- **Fix:** F39: focus the search on open but no halo until the first key or Tab; `Delete all` a quiet `--label-2` text button with danger on hover; sentence case time labels; group heads aligned with row text (`padding-left: 16px`). U36: rows show `20 min ago` / `3 h ago` today, else the time under its day group, and `12 Oct` under Earlier. U32: when `history:list.enabled` is false (K-24) the empty text is C-80 with a `Turn on` link (`settings:set-save-history`). U37: Up/Down move between rows, Enter opens, Delete deletes (with B-071's undo), a printable key focuses search with that character, Esc clears then closes; row context menu `Ask again` (`bar:prefill-open {text}`, K-23) and `Copy question`. Delete-all confirm text C-82.
- **Acceptance:** scenes `history-list`, `history-search-results`, `history-empty`, `history-off`, `history-delete-all` (§4 states 49, 50); checks `historyArrowsMove`, `historyOffSaysOff`.

### B-175 · S2 · settings · One layer of CSS in Settings and History, on the token scale
- **Sources:** design-visual F28 (settings half), F34, F35, F37 (adoption); parity-features NEW-09; design-ux U9 (Settings accent text).
- **Files:** `settings/settings.css`, `history/history.css`.
- **Fix:** Delete the superseded v0.x rules (settings.css:1-906 where overridden) and keep only the override values mapped to tokens (radii per K-40's table: groups r12, rail rows r8, combo options r8; keycaps from glass.css; spacing on the 4-grid; weights 450/600; `--font-small` for 12 px). Any accent text uses `--accent-text`. List both files in `cssLintEnforce` in `tools/ui-shots/lanes/settings.mjs`.
- **Acceptance:** ui-shots `css-lint` 0 violations for both files; scenes for every Settings page at default size (§4 state 41).

### B-176 · S2 · settings · Pickers and choices follow the shared menu and ARIA patterns; Delete all confirms inline
- **Sources:** design-visual F21 (Settings combo); design-ux U40, U41, U52, C-54.
- **Files:** `settings/settings.html`, `settings/settings.js`, `settings/settings.css`.
- **Fix:** The model combo popover becomes `.g-menu` (solid tier inside the page: `--solid` body, same rim and radius). Choice groups: roving tabindex, arrows move and select. Model lists: `role="group" aria-label="Free"` wrappers instead of `aria-hidden` headers; the combo button gets `aria-controls`. `showPage` uses `behavior: 'auto'` under reduced motion. Delete all: inline `Delete all <n> conversations?` with `Delete` / `Keep`, focus on Keep, result in the `role="status"` line.
- **Acceptance:** checks `radiosRoveWithArrows`, `deleteAllConfirmsInline`; scene `settings-combo-open`.

### B-184 · S3 · settings · Small Settings doors and notes
- **Sources:** design-ux U33, U47 (renderer half), U48 (renderer half), U49 (renderer half), U50, C-53, C-55, C-64; review-llm-agent P2-19 (skill note).
- **Files:** `settings/settings.html`, `settings/settings.js`, `settings/settings.css`.
- **Fix:** Shortcuts page: a read-only group `In AevalBuddy windows` listing U19's keys plus F6, Up/Down recall and Backspace (not the Ask/Find/Do keys); F1 in any Settings page opens it. `Bar position` row with the summary from `overlayPos` (`Top right`) and a `Move…` button (C-53). A taken shortcut shows the free alternatives as one-click buttons (`shortcuts:set` `free`). Answers page: `Open styles folder` (`skills:open-folder`) and `Add a .md file to make your own style.`; a truncated skill shows `Only the first 8 KB is used.` When `glass:tier` reasons include `transparency-off`, the Liquid glass section adds C-64. Recorder hint C-55.
- **Acceptance:** scenes `settings-shortcuts-capturing`, `settings-shortcuts-conflict` (§4 state 44), `settings-general`; check `shortcutConflictOffersFree`.

## Rules

The coordinator's rules, verbatim: "LF endings; never run the app locally; never weaken or skip a test or gate to get green; commit messages in sentence case describing the effect like the existing log, each ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; push only its own branch".

What each one means here:

- **LF endings** in every file you write. Before each commit run `git diff --cached --check` and `git ls-files --eol <changed files>` (expect `i/lf w/lf`). The repo has no `.gitattributes`, so your editor's endings are what gets committed.
- **Never run the app locally.** No `cargo tauri dev`, no launching a built exe, no `npm start` of the Electron app. This machine has no Rust toolchain and nine lanes share it; CI is the build and the test bed. Local checks are limited to `node --check <file>` for JS and reading code.
- **Never weaken or skip a test or gate to get green.** No `#[ignore]`, no deleted or loosened assertions, no raised thresholds, no `continue-on-error`, no label removed from a label file, no scene removed from ui-shots. An expected value changes only where one of your items specifies the new value (for example B-049's gate tables or B-087's defaults test).
- **Commits** in sentence case describing the effect, like `git log --oneline -15` on `rust-rewrite` (for example "Keep the first-run tour out of the capture and desktop checks"). Item IDs may go in the body. Every message ends with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Push only your own branch**, `rust/settings`: `git push origin HEAD:rust/settings`. Never push `rust-rewrite`, `main`, `ci-logs`, `shots` or another lane's branch; never force-push.
- **Never poll the GitHub API (60 req/hr shared by the whole machine).** No `gh run`, `gh api`, `gh pr`, or `curl` to api.github.com. CI results come only from `tools/ci-wait.mjs`, which reads the `ci-logs` branch with git.
- Edit only the files listed under "Files you own". If you need a change elsewhere, write it under Blockers (file, the change, which item needs it) and carry on with other items. Never create the per-lane files that harness C1 creates (`selftest/probes/<lane>.rs`, `tools/smoke-labels/<lane>.txt`, `tools/ui-shots/lanes/<lane>.mjs`); merge `origin/rust-rewrite` once they exist.
- Do not edit `DESIGN.md`, `PARITY.md`, `ARCHITECTURE.md`, `REQUESTS.md`, `BACKLOG.md` (only B-149 appends to its "Post-merge findings" section), the audits, other lanes' briefs, the Electron reference (`main.js`, `lib/**` except files you own, `package*.json`, `electron-builder.yml`) or `README.md`.
- Never commit a real API key. Tests and probes use fake keys and the fake LLM.

## Verification loop

Every cycle:

1. Make a batch of changes: one or more items, each with its unit tests, probe labels, ui-shots scenes and checks, and its entries.
2. `git diff --cached --check`, `node --check` on changed `.js` and `.mjs` files, re-read the diff against the item specs.
3. Commit, then push once: `git push origin HEAD:rust/settings` (first push: `git push -u origin HEAD:rust/settings`).
4. Wait for CI with the script below. If the push touched `renderer/`, `settings/`, `history/`, `src-tauri/bridge/` or `tools/ui-shots/`, wait on `ui-shots` as well as `rust`.
5. Exit 0: record the items as green in Progress with the sha. Exit 1: read what it printed, fix, and go to step 1. Exit 2: follow the printed sha, or wait again.

The coordinator's CI recipe, verbatim:

- Usage: `node tools/ci-wait.mjs <sha> [workflow=rust|ui-shots] [--timeout-min 30]` (sha can be HEAD/short). It `git fetch`es the `ci-logs` branch every 45 s; no GitHub API.
- Exit 0 = green; 1 = failure (prints summary.md, errors.txt, last 80 lines of the failing gate's log, and the `git show origin/ci-logs:<dir>/fmt.patch | git apply` command when rustfmt fails); 2 = timeout or cancelled (prints the newer sha to wait on if a later push superseded it).
- Full logs: `origin/ci-logs:<ref with / as ->/<sha>/<workflow>/` holds summary.md, meta.json, errors.txt, and prebuild/fmt/clippy/test/build/smoke/capture/desktop .log (ui-shots: bridge/playwright/shots .log). `<ref>/latest/<workflow>/` = newest run.
- ui-shots only runs when renderer/settings/history/bridge/tools/ui-shots paths change, so `ci-wait <sha> ui-shots` exits 2 on other pushes; that is expected.
- Pushing a new commit to the same branch cancels the running CI for the older sha (concurrency cancel-in-progress), so batch changes and push once per cycle.

For this lane the log directory is `origin/ci-logs:rust-settings/<sha>/<workflow>/` and the newest run is `origin/ci-logs:rust-settings/latest/<workflow>/`. Read a file with `git show "origin/ci-logs:rust-settings/latest/rust/smoke.log"` (use PowerShell or set `MSYS_NO_PATHCONV=1` in Git Bash, which otherwise mangles the `ref:path` argument). The rust workflow takes up to an hour on a cold cache; use `--timeout-min 70` when the build is cold. When the same sha is the tip of two branches (right after merging `origin/rust-rewrite` with nothing new on top), add `--ref rust/settings`.

## Starting and staying current

1. Your worktree starts at `rust-rewrite` HEAD. Create the branch with `git switch -c rust/settings` if it does not exist.
2. Read this brief, the cross-lane tables in `.claude/rust-rewrite/BACKLOG.md` (lanes, ownership, hotspots, contracts), `DESIGN.md`, and for each item the audit sections and PARITY rows it cites. The code is the ground truth for current behaviour; `main.js` on `main` is the ground truth for Electron behaviour.
3. Push your C1 (if "Contracts you provide" lists any) before anything else, with a commit message starting `C1 contracts:`. When it is green, write `C1 green at <sha>` in Progress.
4. Each cycle, `git fetch origin`. When a contract you need has landed (`git grep -n "<symbol>" origin/rust-rewrite -- <file>`) or `rust-rewrite` has moved, `git merge origin/rust-rewrite` (a merge commit; never rebase or force-push). A conflict in a file you own: resolve it. A conflict in a file you do not own means you edited it by mistake: take `origin/rust-rewrite`'s side and note it in Progress.
5. If CI fails for a reason entirely outside your files (a broken merge on `rust-rewrite`, a runner outage), record it under Blockers with the log line and keep working on what you can verify.

## Shared documents

Never edit `DESIGN.md`, `PARITY.md` or `ARCHITECTURE.md`. In the same commit as the change, append to `.claude/rust-rewrite/entries/settings.md`:

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

- Batch 1 (all twelve items, renderer side): Settings and History rewritten (`settings/*`, `history/*`), one CSS layer each. ui-shots green at bba8a89 (existing scenes 06-09 and their checks). Batch 2 merges bar C1 (K-40..K-43) and moves both pages onto it: shared tokens, `.sheet-bar`, `.g-menu` rows and `.g-check`, sprite icons (`ico i-<name>`); the status glyphs the sprite lacks (ring, spinner, circled check, cross, alert, clock, dash, grip) are drawn inline on its grid. Not yet green as items: acceptance scenes and checks live in `tools/ui-shots/lanes/settings.mjs`, which waits for harness C1 (K-31).
- Built against contracts not yet on `rust-rewrite`; each degrades to one missing feature, never a broken page (`call` and `listen` in both scripts): K-19 (`settings:get.providers/failover/keyHints/keyStorage`, `set-provider-order`, `set-failover`, `provider:status`, save-key and test-key `status`), K-18 `recipients`, K-22 (`textScale`, `osTextScale`, `set-text-scale`, `ui:text-scale`), K-23 `bar:prefill-open`, K-24 (`history:list.enabled`, `overlayPos`), llm `skills:open-folder` and skill `truncated`, `auth:openrouter-connect`, K-40/K-41 (`.sheet-bar`, `.g-menu`, `html[data-maximized]`).
- Kept for other lanes until they move: `#sec-key` wraps the provider list and the OpenRouter field keeps id `api-key-input` (buddy.rs tour step 1, host/mod.rs); `settings:section` accepts `sec-key`, `pg-account`, `providers`, `prov-<id>`, any page or row id.
- Integrator note: `history:list` rows would ideally carry `question` (the full first user message) for Ask again and Copy question; until then the title (first line, 90 chars) is used.

## Blockers

(None yet. Format: `- B-xxx: <what blocks it>; <why>; <what would unblock it>`.)
