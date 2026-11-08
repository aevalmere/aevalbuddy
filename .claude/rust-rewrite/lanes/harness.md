# Lane brief: harness

Branch `rust/harness`, cut from `rust-rewrite`. 9 items. Cross-lane tables (ownership, hotspots, contracts, decisions, merge order) are in `.claude/rust-rewrite/BACKLOG.md`; everything you need to do your items is in this file.

## Goal

Make every claim provable on a runner. Today the smoke step only proves the exe starts; after this lane, each lane's behaviour is checked by its own probe inside the real exe, the exe photographs itself and passes the contrast matrix, the Tauri installer is installed, smoked, photographed and uninstalled on a runner, and ui-shots runs every lane's scenes at two scales. At the end you switch on the strict gates and run the five review loops.

## Files you own

- `src-tauri/src/selftest.rs`, which you turn into `src-tauri/src/selftest/` (`mod.rs`, `report.rs`, `eval.rs`, `input.rs`, `shots.rs`, `migrate.rs`, `probes/mod.rs`, `probes/harness.rs`). Keep the names `lib.rs` calls (`smoke_enabled`, `scaffold`, `PING`, `quiet_run` semantics) so platform's `lib.rs` needs no edit.
- `src-tauri/src/backdrop/contrast.rs`, `contrast-probe.js`, `contrast-hide.css`.
- `.github/workflows/**`, `tools/**` (except `tools/fixtures/llm/**`, other lanes' `tools/ui-shots/lanes/<lane>.mjs` and `tools/smoke-labels/<lane>.txt` once created), `lib/backdrop-fixture.html`.
- You create, and then hand over, `src-tauri/src/selftest/probes/<lane>.rs`, `tools/smoke-labels/<lane>.txt` and `tools/ui-shots/lanes/<lane>.mjs` for all nine lanes. After your C1 merges you edit only your own (`harness`).

## Contracts you provide (C1 first)

Your C1 is the most depended-on push in the project. Land it fast and keep it small.

- **K-30** (B-009 steps 1-5, 7, 8): the runner, `Report::{must, log}`, `Cx::{eval, input, wait_for}`, `probes::<lane>::run(&mut Cx)` with a stub file per lane (`pub fn run(_cx: &mut Cx) {}`), empty `tools/smoke-labels/<lane>.txt` for the other eight lanes and `harness.txt` with `version` and `done`, and the strict smoke step in rust.yml. Document the API in a module comment in `selftest/mod.rs` with one worked example probe; lane agents learn it from there.
- **K-31** (B-053 first half): `tools/ui-shots/lanes/<lane>.mjs` stubs for all nine lanes exporting `{ scenes: [], mocks: {}, checks: [], cssLintEnforce: [] }`, imported by `shots.mjs`, `mock-bridge.js` and `bridge.test.mjs`, with the shape documented in a header comment of `shots.mjs`; shots published per ref.
- In the same C1: make sure `tools/ci-publish.mjs` and the shots publish retry on a non-fast-forward push (nine lanes publish at once), and add `lib/backdrop-fixture.html` to the ui-shots path filter.

`Cx::eval` needs `webview2-com`, which platform C1 adds to Cargo.toml. If platform C1 has not landed, push C1 without `eval` (Report, probes, labels, input, workflow, ui-shots plumbing) and add `eval` as soon as you can merge it; state this in Progress.

## Contracts you consume

- Platform C1: `webview2-com`, `png` in Cargo.toml.
- Every lane's probes, labels and scenes (they fill the files you create).
- B-147 needs bar's light tone (B-159); B-148 and B-149 run after all other lanes have merged.

## Smoke labels you print

`version`, `done`, `failed`, `error`, `probe.<lane>` (one per lane), `v2basics`, `v21basics`, `glassGates`, `v2desktop` (and its `skipped (not on CI)` note), `v2NotepadOpen`, `v2NotepadClosed`, every `[SHOT]` line, and `migrate.<STORE-id>` (B-146).

## Shared-document entries you write

PARITY evidence for SMK-01..08, SHOT-01..06, PKG-03..07, UPD-15 (verify), MIGRATE; ARCHITECTURE "Verification" (the probe API and the per-lane files).

## Lane notes

- Your workflow edits run on your own branch first; never push a workflow change that turns other lanes red for a reason outside their code. If a gate you add catches a defect in another lane's files, record it under "Found defects" (file, symptom, log line, sha) instead of fixing it.
- B-148 and B-149 are the final pass: start them only after the coordinator says every other lane has merged (or you see all eight `rust/<lane>` merges on `rust-rewrite`).

## Standing item: port your smoke labels

Every PARITY label under "Smoke labels you port" and every new label below is printed by `src-tauri/src/selftest/probes/harness.rs` through the K-30 API, with the Electron semantics from `.claude/rust-rewrite/audit/parity-ipc.md` §3 and `main.js` on `main`, and listed in `tools/smoke-labels/harness.txt` in the same commit that makes it print `=ok`. Acceptance: the rust workflow's smoke log shows each label `=ok` and the labels gate passes.

Your items add no new smoke labels.

## Items

| ID | Severity | Title |
|---|---|---|
| B-009 | P0 | A real smoke runner with a probe API every lane can use |
| B-010 | P0 | `AEVAL_SHOT`: photograph the real exe, run the 100/125/150 matrix through the contrast gate |
| B-052 | P1 | Install, smoke and photograph the Tauri installer on a runner |
| B-053 | S1 | ui-shots: per-lane scene files, both scales, real backdrops, a virtual clock, the shipped default width |
| B-145 | S2 | A CSS gate for off-system values |
| B-146 | S2 | MIGRATE: an Electron profile round-trips through Rust and back |
| B-147 | S3 | The contrast matrix covers the glass extremes and the light tone |
| B-148 | S2 | Final strict gates: every PARITY label present, every CSS file linted, installer Checks all required |
| B-149 | S2 | The five review loops on the real exe |

Work in severity order (P0, then S1 and P1, then S2 and P2, then S3), after your C1 if you have one.

### B-009 · P0 · harness · A real smoke runner with a probe API every lane can use
- **Sources:** parity-ipc B1, SMK-01..08, §3; parity-core and parity-features "no runtime proof"; review-desktop P2-5 (DXGI must be gated, consumed by B-029).
- **Files:** `src-tauri/src/selftest.rs` → `src-tauri/src/selftest/{mod.rs, report.rs, eval.rs, input.rs, probes/mod.rs}`; creates `src-tauri/src/selftest/probes/<lane>.rs` (one per lane, then owned by that lane); `tools/smoke-labels/<lane>.txt`; `.github/workflows/rust.yml` (smoke step).
- **Fix:**
  1. Keep the names lib.rs calls (`smoke_enabled`, `scaffold`, `PING`, `quiet_run` semantics) so lib.rs needs no edit; `scaffold` becomes the full runner when `AEVAL_SMOKE` is set and the shots runner (B-010) when `AEVAL_SHOT` is set (`smoke_enabled()` returns true for either).
  2. `Report`: `must(label, ok, why)` prints `[SMOKE] <label>=ok` or `[SMOKE] <label>=FAIL:<why>`; `log(label, value)` prints `[SMOKE] <label>=<value>`. The last line is `[SMOKE] done` (exit 0) or `[SMOKE] failed: a, b` (exit 1); an uncaught panic prints `[SMOKE] error=<msg>` and exits 1. Flush stdout before `app.exit`.
  3. `Cx::eval(window_label, js) -> Result<Value>` through `with_webview` and `ICoreWebView2::ExecuteScript` with its completion handler (webview2-com, pre-enumerated in platform C1), JSON-decoding the result, 5 s timeout.
  4. `Cx::input`: `SendInput` click, drag, wheel, key chord and text at physical points, plus `Cx::wait_for(timeout, cond)`.
  5. Blocks: run each lane's `probes::<lane>::run(&mut cx)` inside its own `catch_unwind`; a panic prints `must("probe.<lane>", false, msg)`. Keep the Electron block labels `v2basics`, `v21basics`, `glassGates`, `v2desktop` (each FAIL only when its block throws) mapped onto groups of lane probes.
  6. Run the desktop selfcheck inside the smoke with `Report::new("SMOKE")` (desktop/checks.rs already takes a prefix), `ownPoint` = pill centre via `windows::screen::dip_to_px`, `ownHandles` = every webview HWND, `notepad = GITHUB_ACTIONS set || (AEVAL_DESKTOP_NOTEPAD set && != "0")`. When neither is set print `[SMOKE] v2desktop=skipped (not on CI)`. Open Notepad for the v2desktop block and close it after (labels `v2NotepadOpen`, `v2NotepadClosed`, harness-owned) using the desktop client helpers.
  7. rust.yml "Smoke" step: fail on any `=FAIL` line, on a missing `[SMOKE] done`, and on any label listed in `tools/smoke-labels/*.txt` that does not appear; timeout 360 s (installer-check's budget).
  8. Ship this in harness C1 (K-30): the runner, Report, eval, input, the empty probe files and empty label files, with `version` and `done` already in `tools/smoke-labels/harness.txt`.
- **Acceptance:** rust.yml smoke log shows `[SMOKE] version=`, `[SMOKE] done`, and `[SMOKE] probe.<lane>=ok` for all nine lanes; a deliberately failing must in a throwaway commit turns the step red (state the sha in the harness Progress section, then revert).

### B-010 · P0 · harness · `AEVAL_SHOT`: photograph the real exe, run the 100/125/150 matrix through the contrast gate
- **Sources:** parity-ipc B2, SHOT-01..05; parity-features #14 (GLASS-04, GLASS-05, GLASS-08); parity-core LIFE-03 (consumer).
- **Files:** `src-tauri/src/selftest/shots.rs`, `src-tauri/src/backdrop/contrast.rs`, `src-tauri/src/backdrop/contrast-probe.js`, `src-tauri/src/backdrop/contrast-hide.css`, `.github/workflows/rust.yml`.
- **Fix:** `selftest::shots(app, dir)` when `AEVAL_SHOT=<dir>`: drive each surface into the named state through the same hooks IPC uses; photograph each window with `ICoreWebView2::CapturePreview` (PNG) via `with_webview`; use a real screen grab for `12-live-overlay` with `backdrop::pause()` then content protection lifted for one frame then `backdrop::resume()`. Write `<dir>/<name>.png` and print `[SHOT] <name>=<w>x<h>`; frames 01-17 with the `[SHOT] fit.<name>=window<h>:surface<h>` probes (Electron lib/shots.js, main.js:3080) and 18-30 (lib/shots-v2.js). With `AEVAL_SHOT_MATRIX_ONLY=1` render the 13 matrix surfaces over the white, black and wallpaper fixtures at the scale in `AEVAL_SHOT_SCALE` (WebView2 is forced by B-015), name them `m-<surface>-<backdrop>-<scale>.png`, run every frame through `backdrop::contrast`, print `[SHOT] matrix scale=<s> contrastMin=<x> failedFrames=<n>`, exit 1 when `failedFrames>0`. Always exit within 600 s. Add a rust.yml step that runs the matrix at 1.0 on the built exe and fails on `failedFrames>0`.
- **Acceptance:** rust.yml shots step log shows `[SHOT] 01-...` through `30-...` and `[SHOT] matrix scale=1 ... failedFrames=0`; installer job (B-052) `shots.written`, `shots125.*`, `shots150.*` pass.

### B-052 · P1 · harness · Install, smoke and photograph the Tauri installer on a runner
- **Sources:** parity-ipc B5, SHOT-06, PKG-03..07, UPD-15 (verify), §3.1; memory "test in the cloud".
- **Files:** `.github/workflows/rust.yml` (new job `installer`, needs `build`), `tools/installer-check.ps1`.
- **Fix:** A job that downloads the `aevalbuddy-tauri-nsis` artifact, reads the version from package.json, and runs `tools/installer-check.ps1 -Installer <setup.exe> -Version <v> -Out out` (all Checks: install.*, smoke.*, shots.*, shots125.*, shots150.*, uninstall.*). Add an "Electron upgrade" phase for B-016: install Electron v1.1.1 from the GitHub release, then the Tauri installer `/S`, then print `[INSTALL] electronRemoved=ok|FAIL` and `[INSTALL] uninstallEntries=<n>` (`reg query` of HKCU Uninstall for `AevalBuddy*`). Print `[INSTALL] launchAtLogin=<Run value>` and `[INSTALL] startupApproved=<hex>` after toggling via the smoke (B-081). Publish the installed exe's shots to the `shots` branch under `rust-exe/<ref-with-dashes>/latest/` (newest 15) and the logs through ci-publish. The job's results land under `ci-logs:<ref>/<sha>/rust/` like the others; until every lane's probes exist the job fails only on install, uninstall and shots exit codes (it never ignores a Check it runs).
- **Acceptance:** rust ci-logs `installer.log` shows every installer-check Check `=ok`, `[INSTALL] electronRemoved=ok`, `[INSTALL] uninstallEntries=1`; PNGs present on `origin/shots:rust-exe/<ref>/latest/`.

### B-053 · S1 · harness · ui-shots: per-lane scene files, both scales, real backdrops, a virtual clock, the shipped default width
- **Sources:** design-visual §4 (general changes), F27 (harness half); design-ux U8 (200% set); parity-features #14.
- **Files:** `tools/ui-shots/shots.mjs`, `tools/ui-shots/mock-bridge.js`, `tools/ui-shots/bridge.test.mjs`, `lib/backdrop-fixture.html`, `.github/workflows/ui-shots.yml`; creates `tools/ui-shots/lanes/<lane>.mjs` (one per lane, then owned by that lane).
- **Fix:** In harness C1 (K-31): each lane file exports `{ scenes, mocks, checks, cssLintEnforce }` (document the exact shape in a header comment of `shots.mjs`); `shots.mjs`, `mock-bridge.js` and `bridge.test.mjs` import all nine. Run every scene at `deviceScaleFactor` 1 and 1.5 (suffix `-150`), plus a `--text-scale: 2` variant for scenes flagged `textScale: true`. Add backdrops `dark-editor` (VS Code-like syntax-coloured page) and `photo-busy` (high-frequency edges, text, grids) to `lib/backdrop-fixture.html`. A virtual clock: stub `performance.now` and `requestAnimationFrame` and step them, so glob frames show real timing. Shoot the shipped default bar width (600 after B-087/B-106) as well as 640. Publish to `shots:rust-d-ui/<ref-with-dashes>/latest/` so parallel lanes do not overwrite each other, retrying the push on a race.
- **Acceptance:** ui-shots ci-logs `shots.log` lists scenes from every lane file at both scales; `origin/shots:rust-d-ui/<ref>/latest/` holds `*-150.png` and `dark-editor`/`photo-busy` variants.

### B-145 · S2 · harness · A CSS gate for off-system values
- **Sources:** design-visual F28 (gate half), F36 (rule); parity-features NEW-09.
- **Files:** `tools/ui-shots/bridge.test.mjs`.
- **Fix:** Grep CSS for `border-radius: (4|5|8|10|12|14|18|20)px` not via tokens, paddings and gaps off the 4-grid (2 px allowed with a `/* hairline */` or `/* optical */` comment), raw `rgb()`/hex colours outside `:root` token blocks. Enforce per file for every file listed in a lane's `cssLintEnforce` from day one; the final pass (B-148) enforces all CSS.
- **Acceptance:** ui-shots `bridge.log` shows `css-lint: <n> files enforced, 0 violations`.

### B-146 · S2 · harness · MIGRATE: an Electron profile round-trips through Rust and back
- **Sources:** parity-core §9 ("Every STORE row also needs MIGRATE").
- **Files:** `src-tauri/src/selftest/migrate.rs`, `tools/fixtures/profile-v1.1.1/` (new: `aevalbuddy.json`, `history.json`, `Local State` from an Electron v1.1.1 run with a test key).
- **Fix:** Under `AEVAL_SMOKE`, copy the fixture into a scratch userData, boot, read every STORE row's value through the store API, write a change, and assert the file still parses for Electron's rules (unknown keys kept, v1 key fields mirrored, values in Electron's types). Print `[SMOKE] migrate.<STORE-id>=ok` per row.
- **Acceptance:** SMOKE `migrate.STORE-01` … `migrate.STORE-44` present and ok.

### B-147 · S3 · harness · The contrast matrix covers the glass extremes and the light tone
- **Sources:** design-ux U51 (matrix half); design-visual F10 (gate half).
- **Files:** `src-tauri/src/selftest/shots.rs`, `src-tauri/src/backdrop/contrast-probe.js`.
- **Fix:** Add k = 0 and k = 1 to the matrix (wallpaper and white, 100% and 150%) and light-tone runs (white and photo-busy) once B-159 lands; the probe measures with the 98th percentile when k > 0.5.
- **Acceptance:** `[SHOT] matrix ... failedFrames=0` including `m-*-k100-*` and `m-*-light-*` frames.

### B-148 · S2 · harness · Final strict gates: every PARITY label present, every CSS file linted, installer Checks all required
- **Sources:** parity-ipc §3 (206 labels), §3.1; design-visual F28.
- **Files:** `tools/smoke-labels/required.txt` (new), `.github/workflows/rust.yml`, `tools/ui-shots/bridge.test.mjs`.
- **Fix:** After all lanes have merged (merge step 10), add `required.txt` (the 206 PARITY labels from parity-ipc §3 minus the 3 superseded, plus every SMOKE+ label named in this backlog) and fail when any is missing from the union of lane label files or from the log; enforce CSS lint on all CSS; make every installer-check Check required in the installer job.
- **Acceptance:** rust and ui-shots green on rust-rewrite with the strict gates on.

### B-149 · S2 · harness · The five review loops on the real exe
- **Sources:** parity-features #19 (REQUESTS 7, 12); memory "multi-perspective reviews".
- **Files:** `.claude/rust-rewrite/reviews/{designer,ease-of-use,accessibility,power-user,trust}.md` (new).
- **Fix:** After B-148 is green, run each lens over the installed-exe shot set (`rust-exe/<ref>/latest/`) and write one file per lens: findings with severity and the shot that shows them. File every S1/P1 finding as a new item in `BACKLOG.md` under "Post-merge findings" (append-only section) for the coordinator to route.
- **Acceptance:** the five files exist with at least the shot list reviewed; any S1 finding is listed in BACKLOG "Post-merge findings".

## Rules

The coordinator's rules, verbatim: "LF endings; never run the app locally; never weaken or skip a test or gate to get green; commit messages in sentence case describing the effect like the existing log, each ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; push only its own branch".

What each one means here:

- **LF endings** in every file you write. Before each commit run `git diff --cached --check` and `git ls-files --eol <changed files>` (expect `i/lf w/lf`). The repo has no `.gitattributes`, so your editor's endings are what gets committed.
- **Never run the app locally.** No `cargo tauri dev`, no launching a built exe, no `npm start` of the Electron app. This machine has no Rust toolchain and nine lanes share it; CI is the build and the test bed. Local checks are limited to `node --check <file>` for JS and reading code.
- **Never weaken or skip a test or gate to get green.** No `#[ignore]`, no deleted or loosened assertions, no raised thresholds, no `continue-on-error`, no label removed from a label file, no scene removed from ui-shots. An expected value changes only where one of your items specifies the new value (for example B-049's gate tables or B-087's defaults test).
- **Commits** in sentence case describing the effect, like `git log --oneline -15` on `rust-rewrite` (for example "Keep the first-run tour out of the capture and desktop checks"). Item IDs may go in the body. Every message ends with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Push only your own branch**, `rust/harness`: `git push origin HEAD:rust/harness`. Never push `rust-rewrite`, `main`, `ci-logs`, `shots` or another lane's branch; never force-push.
- **Never poll the GitHub API (60 req/hr shared by the whole machine).** No `gh run`, `gh api`, `gh pr`, or `curl` to api.github.com. CI results come only from `tools/ci-wait.mjs`, which reads the `ci-logs` branch with git.
- Edit only the files listed under "Files you own". If you need a change elsewhere, write it under Blockers (file, the change, which item needs it) and carry on with other items. Never create the per-lane files that harness C1 creates (`selftest/probes/<lane>.rs`, `tools/smoke-labels/<lane>.txt`, `tools/ui-shots/lanes/<lane>.mjs`); merge `origin/rust-rewrite` once they exist.
- Do not edit `DESIGN.md`, `PARITY.md`, `ARCHITECTURE.md`, `REQUESTS.md`, `BACKLOG.md` (only B-149 appends to its "Post-merge findings" section), the audits, other lanes' briefs, the Electron reference (`main.js`, `lib/**` except files you own, `package*.json`, `electron-builder.yml`) or `README.md`.
- Never commit a real API key. Tests and probes use fake keys and the fake LLM.

## Verification loop

Every cycle:

1. Make a batch of changes: one or more items, each with its unit tests, probe labels, ui-shots scenes and checks, and its entries.
2. `git diff --cached --check`, `node --check` on changed `.js` and `.mjs` files, re-read the diff against the item specs.
3. Commit, then push once: `git push origin HEAD:rust/harness` (first push: `git push -u origin HEAD:rust/harness`).
4. Wait for CI with the script below. If the push touched `renderer/`, `settings/`, `history/`, `src-tauri/bridge/` or `tools/ui-shots/`, wait on `ui-shots` as well as `rust`.
5. Exit 0: record the items as green in Progress with the sha. Exit 1: read what it printed, fix, and go to step 1. Exit 2: follow the printed sha, or wait again.

The coordinator's CI recipe, verbatim:

- Usage: `node tools/ci-wait.mjs <sha> [workflow=rust|ui-shots] [--timeout-min 30]` (sha can be HEAD/short). It `git fetch`es the `ci-logs` branch every 45 s; no GitHub API.
- Exit 0 = green; 1 = failure (prints summary.md, errors.txt, last 80 lines of the failing gate's log, and the `git show origin/ci-logs:<dir>/fmt.patch | git apply` command when rustfmt fails); 2 = timeout or cancelled (prints the newer sha to wait on if a later push superseded it).
- Full logs: `origin/ci-logs:<ref with / as ->/<sha>/<workflow>/` holds summary.md, meta.json, errors.txt, and prebuild/fmt/clippy/test/build/smoke/capture/desktop .log (ui-shots: bridge/playwright/shots .log). `<ref>/latest/<workflow>/` = newest run.
- ui-shots only runs when renderer/settings/history/bridge/tools/ui-shots paths change, so `ci-wait <sha> ui-shots` exits 2 on other pushes; that is expected.
- Pushing a new commit to the same branch cancels the running CI for the older sha (concurrency cancel-in-progress), so batch changes and push once per cycle.

For this lane the log directory is `origin/ci-logs:rust-harness/<sha>/<workflow>/` and the newest run is `origin/ci-logs:rust-harness/latest/<workflow>/`. Read a file with `git show "origin/ci-logs:rust-harness/latest/rust/smoke.log"` (use PowerShell or set `MSYS_NO_PATHCONV=1` in Git Bash, which otherwise mangles the `ref:path` argument). The rust workflow takes up to an hour on a cold cache; use `--timeout-min 70` when the build is cold. When the same sha is the tip of two branches (right after merging `origin/rust-rewrite` with nothing new on top), add `--ref rust/harness`.

## Starting and staying current

1. Your worktree starts at `rust-rewrite` HEAD. Create the branch with `git switch -c rust/harness` if it does not exist.
2. Read this brief, the cross-lane tables in `.claude/rust-rewrite/BACKLOG.md` (lanes, ownership, hotspots, contracts), `DESIGN.md`, and for each item the audit sections and PARITY rows it cites. The code is the ground truth for current behaviour; `main.js` on `main` is the ground truth for Electron behaviour.
3. Push your C1 (if "Contracts you provide" lists any) before anything else, with a commit message starting `C1 contracts:`. When it is green, write `C1 green at <sha>` in Progress.
4. Each cycle, `git fetch origin`. When a contract you need has landed (`git grep -n "<symbol>" origin/rust-rewrite -- <file>`) or `rust-rewrite` has moved, `git merge origin/rust-rewrite` (a merge commit; never rebase or force-push). A conflict in a file you own: resolve it. A conflict in a file you do not own means you edited it by mistake: take `origin/rust-rewrite`'s side and note it in Progress.
5. If CI fails for a reason entirely outside your files (a broken merge on `rust-rewrite`, a runner outage), record it under Blockers with the log line and keep working on what you can verify.

## Shared documents

Never edit `DESIGN.md`, `PARITY.md` or `ARCHITECTURE.md`. In the same commit as the change, append to `.claude/rust-rewrite/entries/harness.md`:

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

- C1 green at 3bc261f (rust and ui-shots; smoke log: version, scaffold.bridge, selftest.eval, selftest.input, probe.<lane>=ok for all nine, the four blocks ok, done, `[GATE] verdict=ok`; ui-shots ran every scene at 100% and 150%, 32 shots). Contents: `selftest/` (mod.rs with the API doc and example, report.rs, eval.rs, input.rs, probes/mod.rs, probes/<lane>.rs for all nine), `tools/smoke-labels/<lane>.txt` (harness.txt filled), `tools/smoke-gate.mjs` (with `--self-test`), rust.yml Smoke step (strict, 360 s), `tools/ui-shots/{kit,lanes}.mjs` + `lanes/<lane>.mjs` (the old scenes moved into bar, sheet and settings), shots.mjs framework (scales 1 and 1.5, textScale, virtual clock via Playwright `page.clock`), mock-bridge reads answers lazily, bridge.test checks lane shapes, ui-shots publishes per ref with 12 jittered tries, ci-publish 20 attempts, `lib/backdrop-fixture.html` in the ui-shots path filter.
- Note: `Cx::eval` first shipped on a Tauri-event backend; with webview2-com on rust-rewrite it now runs through `ICoreWebView2::ExecuteScript` (selftest/eval.rs: a start script keeps the settled value in `window.__aevalSelftest[id]`, a poll script collects it every 40 ms, since ExecuteScript does not await a Promise). No capability or event involved; the API did not change.
- Not in C1 (next): B-009 step 6 (desktop selfcheck and Notepad inside the smoke), the throwaway failing-must proof, B-010 shots, B-052 installer job, B-053 rest (backdrops dark-editor/photo-busy, default width), B-145, B-146, B-147.
- Infra (coordinator request, after github.com's release-asset endpoint started answering 504 to the bundler's NSIS download on every lane): the build no longer downloads NSIS. `tools/seed-nsis.ps1` fills `%LOCALAPPDATA%\tauri\NSIS` with exactly what tauri-bundler checks (NSIS_REQUIRED_FILES, NSIS.exe and nsis_tauri_utils.dll SHA-1s, copied from tauri-cli 2.12.4's nsis/mod.rs): from the cache, else the preinstalled NSIS if it is 3.13, else NSIS 3.13 from SourceForge (sha1 DB14F8AF... checked, same zip as tauri's), and the DLL vendored as `tools/nsis/nsis_tauri_utils.dll` (v0.5.3, sha1 75197FEE... checked; release URL as fallback). rust.yml and release.yml pin `TAURI_CLI: '2.12.4'` (the script refuses another version); rust.yml restores and saves the folder with actions/cache/restore+save (saved right after seeding, whatever later steps do), keeps the retry on a bundler download failure as a last resort, and warns if the bundler still replaced the toolset. Tested locally into a scratch folder: fresh seed, cached, corrupted DLL, CLI mismatch. Green at d14b471 (rust: Restore/Seed/Save NSIS toolset, Build installer and Smoke all success; nsis.log `[NSIS] ready: NSIS 3.13 from https://downloads.sourceforge.net/...`, DLL from tools/nsis). The integrator merges it first.
- B-009 step 6: green at d14b471 (smoke: pillCentre, ownWindowBlocked=ok, typeReadText read back from the smoke's Notepad, probe.<lane>=ok for all nine, v2NotepadOpen=ok, v2NotepadClosed=true, v2desktop=ok, done, `[GATE] verdict=ok`). The harness probe runs `desktop::selfcheck::run` under SMOKE (ownPoint = pill centre via `windows::screen::dip_to_px`, ownHandles = every webview HWND, Notepad on CI), and its failed musts count toward the exit code (`Report::absorb`). `cx.notepad()` (selftest/notepad.rs) opens the smoke's own Notepad (`v2NotepadOpen`) before the v2desktop lanes or on first use, and closes it after every lane (`v2NotepadClosed`). Lane order now ends agent, llm, so the Notepad lanes run last. The capture lane lists the 33 desktop labels; capture.rs should not run the selfcheck again.
- B-053 rest, in progress: `dark-editor` and `photo-busy` kinds in `lib/backdrop-fixture.html` (kit.mjs BACKDROPS, checked by the shape check), harness scenes over both, the bar at the default 600 px width (`barDefaultWidth`), and the glob on the virtual clock (`globOnClock`, frames at 48/144/240/336 ms). At b684715 everything passed but `globOnClock` (the open finished before 336 ms: Playwright's `page.clock` keeps flowing in real time after `install`), so a clock scene now stubs `performance.now` and `requestAnimationFrame` in every frame and moves them only by `t.tick`/`t.settle` in 16 ms frames; Date and timers stay real. `globOnClock` also reads the page's virtual time after each shot (must be 48,144,240,336), and the K-31 doc in shots.mjs now says t.settle moves a clock scene's clock (never between timed frames) while t.shot never does. The glob's timing itself is sheet's; sheet adds its own enforced `globOpenOnClock`.
- B-145, in progress (bridge.log at b684715: `css-lint: 1 files enforced, 0 violations`, `cssLint=ok`; waits on a fully green ui-shots run): `tools/ui-shots/css-lint.mjs` (radius, grid, colour rules; `--self-test`), run by bridge.test.mjs over every cssLintEnforce file (`css-lint: <n> files enforced, <v> violations`), the rest reported per file. Harness enforces `contrast-hide.css`.

## Blockers

(None yet. Format: `- B-xxx: <what blocks it>; <why>; <what would unblock it>`.)

## Found defects

(Defects your gates find in other lanes' files: `- <file>: <symptom>; <log line>; <sha>`. Do not fix them yourself.)
