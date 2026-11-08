# Integration log

The merge agent (integrator) is the only writer of `rust-rewrite` while the nine lanes in `BACKLOG.md` run. This file says what is merged at which sha, what was reverted and why, and what is open, so a successor can pick up without rereading the history.

## How the integrator works

- Local branch `integrate` tracks `origin/rust-rewrite`; pushes are `git push origin integrate:rust-rewrite`, fast-forward only, never forced.
- A lane is merged at an exact sha that `node tools/ci-wait.mjs <sha> --timeout-min 0` reports green, with `git merge --no-ff`. Messages: `Merge rust/<lane> C1 into rust-rewrite`, `Merge rust/<lane> checkpoint into rust-rewrite`.
- Order when several are ready: C1 platform, windows, harness, capture, llm, agent, bar; checkpoints and finals per BACKLOG "Merge order and expected conflicts".
- After each push: `node tools/ci-wait.mjs HEAD`, plus `HEAD ui-shots` when `renderer/`, `settings/`, `history/`, `src-tauri/bridge/` or `tools/ui-shots/` changed.
- `entries/<lane>.md` files stay unfolded through C1 and checkpoint merges; they are folded into DESIGN, PARITY and ARCHITECTURE only at each lane's final merge (phase 2), then deleted.
- A red rust-rewrite caused by two lanes interacting gets a seam fix commit from the integrator (named below). A red caused by one lane's own change gets `git revert -m 1` of that merge, and the lane is told.

## Merges

| # | Lane | Kind | Lane sha | Merge commit | rust | ui-shots | Notes |
|---|---|---|---|---|---|---|---|
| 1 | capture | C1 | `f343f64` | `6fcad0f` | green at `3bc8979` | green at `3bc8979` | Pushed together with #2 |
| 2 | bar | C1 | `c114f4a` | `8e20ad6` | green at `3bc8979` | green at `3bc8979` | Pushed together with #1. Its icons draw nothing (malformed `icons.svg`); the bar fix follow-up is merged next |
| 3 | agent | C1 | `89fd79a` | `3849c7a` | cancelled by #4's push | n/a | `3ef7629` (red: fmt) plus its rustfmt fix |
| 4 | bar | C1 follow-up | `d84a9d3` | `f355865` | cancelled by #5's push | green at `db41550` | Icons as CSS masks of a well-formed `icons.svg`. Renderer only. Merged on the coordinator's call with ui-shots green and its rust run red only on an NSIS download 504 in Build installer |
| 5 | platform | C1 | `caadb51` | `17e55bb` | cancelled by #6's push; covered by `7c88959` | n/a | `040ad93` + `3e74cba` (windows 0.62 core for webview2-com) + a Progress-only commit. At `3e74cba` every gate passed but Build installer (NSIS download 504); merged on the coordinator's rule |
| 6 | harness | C1 | `3bc261f` | `bd5e3f7` | `7c88959`: Format, Clippy, Unit tests, Capture, Desktop green; Build installer NSIS 504, Smoke skipped | green at `7c88959` | K-30 selftest API and probes per lane, K-31 ui-shots lane files (old scenes moved into bar, sheet, settings), strict smoke step |
| 7 | windows | C1 | `3127827` | `9c61540` | `75d531f`: Format, Clippy, Unit tests, Capture, Desktop green; Build installer NSIS 504, Smoke skipped | n/a | `1397118` + rustfmt fix. Lane blocks in both allowlists, K-03..K-07, K-22..K-24; drops `hud:docked`, `buddy:move`, `settings:data` (B-095). At `3127827` only Build installer (NSIS 504) was red |
| 8 | llm | C1 | `3eb0c05` | `d5a640f` | green at `6d34ad7` (Build installer and Smoke included) | n/a | `e3bba78` + rustfmt/stall-test fixes + its merge of windows and harness C1 + `51b3d4e` (registers `llm::recipients` as `Hooks.overlay_extra`, K-07) + a Progress-only commit. K-15 `host::settings::route`, K-16, K-17, K-18, K-19 payload types, K-26 fake scripts. At `51b3d4e` only Build installer (NSIS 504) was red. All seven C1s are now in |
| 9 | harness | cherry-pick | `d14b471` | `ff9f52c` | green at `6d34ad7` (Build installer and Smoke included) | n/a | NSIS seeding only (rust.yml, release.yml, `tools/seed-nsis.ps1`, `tools/nsis/nsis_tauri_utils.dll`), on the coordinator's call, because the harness tip's ui-shots fails its own `globOnClock` check. Conflicts: rust.yml took the harness side (all of its delta from rust-rewrite is NSIS and build retry); `lanes/harness.md` kept rust-rewrite's side. The rest of rust/harness merges once its tip has green ui-shots |
| 10 | agent | checkpoint | `3918098` | `1de8048` | green at `8c50561` | green at `8c50561` | Fully green at the lane sha, Smoke included |
| 11 | bar | checkpoint | `462b98f` | `ae0d323` | green at `8c50561` | green at `8c50561` | Lane sha red only on the NSIS 504 (Smoke skipped); ui-shots green. Newer tip `e04d998` fails its own new `styleStable` smoke label, so it was not taken |
| 12 | sheet | checkpoint | `a2ab4d8` | `31e213c` | green at `8c50561` | green at `8c50561` | Lane sha red only on the NSIS 504 (Smoke skipped); ui-shots green |
| 13 | settings | checkpoint | `bf5e553` | `24a1c0e` | green at `8c50561` | green at `8c50561` | Lane sha red only on the NSIS 504 (Smoke skipped); ui-shots green |
| 14 | windows | checkpoint | `c8c2a37` | `e825929` | covered by `5541e36` (red only on Smoke `buddyReload`) | green at `075577b` | Rust green at the lane sha. Contains `3c071b0`, the WS_EX_NOACTIVATE-after-collapse fix bar's `styleStable` waits on |
| 15 | llm | checkpoint | `8aacd1c` | `d1a91ec` | covered by `5541e36` (red only on Smoke `buddyReload`) | green at `075577b` | Rust green at the lane sha |
| 16 | agent | checkpoint | `771d149` | `caf41ad` | covered by `5541e36` (red only on Smoke `buddyReload`) | green at `075577b` | Green at the lane sha. Newer `0db3912` failed 2 unit tests, so it was not taken |
| 17 | settings | checkpoint | `493483b` | `075577b` | covered by `5541e36` (red only on Smoke `buddyReload`) | green at `075577b` | Lane finished. Final tip; code last changed at `e415b9c` (ui-shots green there) |
| 18 | harness | checkpoint | `76c8c95` | `85a2f88` | covered by `5541e36` (red only on Smoke `buddyReload`) | green at `5541e36` | Lane finished; rust (Smoke included) and ui-shots green at the lane sha. Brings `tools/ui-shots/css-lint.mjs`, which unblocks settings' B-175. `b01e049` on top is Progress only |
| 19 | windows | checkpoint | `8886c02` | `6e93f2e` | covered by `5541e36` (red only on Smoke `buddyReload`) | n/a | Rust green at the lane sha |
| 20 | bar | checkpoint | `688cb40` | `ecbbfc3` | covered by `5541e36` (red only on Smoke `buddyReload`) | green at `5541e36` | Lane finished. Rust (`styleStable=ok`) and ui-shots green at the lane sha. The log commit corrects `lanes/bar.md` Progress and Blockers lines that still said `styleStable` waited on windows |
| 21 | sheet | checkpoint | `f660cd0` | `f6dc648` | covered by `5541e36` (red only on Smoke `buddyReload`) | green at `5541e36` | Lane finished; rust and ui-shots green at the lane sha. Bar's B-156 placeholder `Answer the task’s question` still matches sheet's `/question/i` (`barAnswersTask`), so no seam fix was needed |
| 22 | capture | checkpoint | `96b569f` | `7273f74` | covered by `5541e36` (red only on Smoke `buddyReload`) | green at `5541e36` | Rust green at the lane sha (run 42509614984, `lensFirstDraw=ok`); ui-shots green at `27f4063`. `bf07bec` on top is Progress only |
| 23 | platform | checkpoint | `f045fc2` | `f32b46d` | `5541e36`: all green but Smoke (`buddyReload`) | green at `5541e36` | Lane finished; rust (Build installer and Smoke included) and ui-shots green at the lane sha. `2143d68` on top is Progress only. Brings the harness-approved `tools/ui-shots/bridge.test.mjs` `editMenuInFields` change and B-085's one-click `src-tauri/installer.nsi` (an interactive install launches the app at the end) |
| 24 | llm | checkpoint | `4e95f64` | `89a4f60` | pending | n/a | The real Settings backend (keys, status, Connect OpenRouter, failover, model lists). Merged on the coordinator's call: all green but Smoke `buddyReload`, the same known failure as `5541e36`, not llm's. `535f6bb` on top is Progress only |
| 25 | sheet | checkpoint | `49f1eeb` | `a0ed3c0` | pending | pending | Rust and ui-shots green at the lane sha; `barAnswersTask` matches bar's exact placeholder |
| 26 | platform | checkpoint | `b816e2d` | see `git log` | `88bf490` (run 42517587926): all green but Smoke `buddyReload` and `uiaTree` | n/a | Fully green at the lane sha (`suggestionsOnce=ok`). Fixes a dropped settings save: MoveFileExW refuses to replace a file another handle has open (antivirus, search indexer), so `durable::replace` falls back to `fs::rename` with a longer backoff. Likely the cause of `suggestionsOnce`'s `disk=false` on windows `0069af0`. Platform's B-112 TODO is already covered by the `ea1a0d0` seam fix. Pushed before capture's fix, superseding `cb1db3c`'s run |
| 27 | capture | checkpoint (rust/capture-fix) | `2734165` | see `git log` | pending | n/a | Fixes `buddyReload`: the smoke run's real cursor rested on the reload point's bubble, so the check failed depending on cursor position. Green at the lane sha (`buddyReload` ok in 456 ms); touches only `buddy.rs` and capture's probe |

## Reverts

None.

## Seam fixes

- `f90f883` (after #22): `84e2bd9` was red on Clippy and Unit tests (E0063, missing `key` in `windows::screen::Display` at `src/windows/motion.rs:227`) and on ui-shots `cssLint` (4 raw `rgb()` colours in `renderer/lens.css`). Both came in with the capture merge: capture added `Display::key` without seeing windows' motion.rs test helper, and capture's lens.css had not met harness's new css-lint. Fix: `key: id as u32` in the helper (windows should know), and the lens colours moved into `:root` tokens with unchanged values (`--loupe-light`, `--loupe-shadow`, `--chip-fill`, `--chip-light`, `--plate-rim`; capture should know).
- After #23 (B-112): lib.rs's `aeval-frame` handler calls `backdrop::serve_request(&request)` instead of `backdrop::serve(request.uri().path())`, so a request from a foreign origin gets the 403; the `#[allow(dead_code)]` moved from `serve_request` to `serve`, which only the tests now call. Platform's lib.rs, capture's `serve_request`, done at the seam on the coordinator's call.

## Open integration issues

- NSIS 504 outage (from about 08:40 UTC on 2026-11-08): `ff9f52c` (#9) seeds NSIS, and rust-rewrite was fully green at `6d34ad7`. A lane sha from before that seed has no Smoke result, so the rust-rewrite run after its merge is its first Smoke.
- harness tip (`d14b471` and before, since `9b90864`): ui-shots fails `globOnClock` and `globOnClock-150` ("done at 336 ms=true, at 536 ms=true (open takes 440 ms)"), harness's own check of the sheet's glob timing. It was a bug in the check (each `t.shot` advances the virtual clock 120 ms) and harness has the fix. Merge the rest of rust/harness only at a tip with green ui-shots. Never merge a harness sha that prints `deliberateFailure=FAIL`: `fc019b1` is B-009's throwaway proof that the gate fails.
- Coordinator rule (standing): a C1 or checkpoint commit whose only red step is a bundler download 504 in Build installer counts as green. If rust-rewrite's own Build installer flakes the same way, push an empty commit to re-run; do not revert.

## Queue and pending CI (as of `5541e36`, 2026-11-08)

- `5541e36` (#18-#23 plus both seam fixes), run 42515162283: Format, Clippy, Unit tests, Build installer, Capture check and Desktop selfcheck green; ui-shots green. Smoke red on `buddyReload` only ("shown=true crash=Ok(()) back=false in 5050ms"), so Upload installer was skipped and there is no `aevalbuddy-tauri-nsis` artifact yet. The `smoke: deliberate ... panic` lines in stderr are platform's guard probe, not failures.
- `buddyReload` (capture's probe, `selftest/probes/capture.rs` `reload`): after `Page.crash` the buddy page is reloaded (crash.rs logs it) but within 5 s one of `lets_mouse_through`, `is_ready` or "a fresh page" stays false. Both lanes passed it alone, so it is an interaction, most likely with windows' merges (#14, #19: IPC caller checks in `ad2180e`, extended-style handling in `3c071b0`). Sent to capture via the coordinator; not a seam fix.
- `suggestionsOnce` (bar's probe) failed once on windows `0069af0` (`disk=false`: the flag was in memory but not yet in `aevalbuddy.json`) and passed on `5541e36`. Probably timing against platform's new durable save; platform told, low priority.
- When rust is green with Build installer and Smoke passing, tell main the sha; the installer is the `aevalbuddy-tauri-nsis` artifact (`src-tauri/target/release/bundle/nsis/*.exe`) on that sha's rust workflow run (the "Run:" line `ci-wait` prints).
- Coordinator plan: a testable build for Andy. Merge each lane's green tip as it lands; a lane whose tip is not green stays at its last checkpoint; skip re-runs unless a real failure needs one.
- `88bf490` also failed `uiaTree` once ("readTree did not finish in 12000ms" on the Notepad in harness's desktop probe), which passed on every earlier head; treated as a runner hiccup unless it repeats.
- Queued: none beyond new green tips. llm `4e95f64` and sheet `49f1eeb` are in at #24 and #25; their head is expected red only on `buddyReload`.
- Red at last check: agent `fe650d1`, `8fa0e89`: unit tests; agent stays at `771d149`.
- Final at their merged sha: settings `493483b`, harness `76c8c95`, bar `688cb40`, sheet `f660cd0` (newer `49f1eeb` queued), platform `f045fc2`, windows `8886c02`.
- Follow-up, not a blocker: sheet B-164 sends `output:resize` until windows' B-027 lands.

## Base

- `243cda1` Record the audit, the backlog and one brief per fix lane (the base every lane branched from).
