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
| 14 | windows | checkpoint | `c8c2a37` | `e825929` | pending (run for `075577b` superseded by #18's push) | green at `075577b` | Rust green at the lane sha. Contains `3c071b0`, the WS_EX_NOACTIVATE-after-collapse fix bar's `styleStable` waits on |
| 15 | llm | checkpoint | `8aacd1c` | `d1a91ec` | pending | green at `075577b` | Rust green at the lane sha |
| 16 | agent | checkpoint | `771d149` | `caf41ad` | pending | green at `075577b` | Green at the lane sha. Newer `0db3912` failed 2 unit tests, so it was not taken |
| 17 | settings | checkpoint | `493483b` | `075577b` | pending | green at `075577b` | Lane finished. Final tip; code last changed at `e415b9c` (ui-shots green there) |
| 18 | harness | checkpoint | `76c8c95` | see `git log` (merge before this log commit) | pending | pending | Lane finished; rust (Smoke included) and ui-shots green at the lane sha. Brings `tools/ui-shots/css-lint.mjs`, which unblocks settings' B-175. `b01e049` on top is Progress only |

## Reverts

None.

## Seam fixes

None.

## Open integration issues

- NSIS 504 outage (from about 08:40 UTC on 2026-11-08): `ff9f52c` (#9) seeds NSIS, and rust-rewrite was fully green at `6d34ad7`. A lane sha from before that seed has no Smoke result, so the rust-rewrite run after its merge is its first Smoke.
- harness tip (`d14b471` and before, since `9b90864`): ui-shots fails `globOnClock` and `globOnClock-150` ("done at 336 ms=true, at 536 ms=true (open takes 440 ms)"), harness's own check of the sheet's glob timing. It was a bug in the check (each `t.shot` advances the virtual clock 120 ms) and harness has the fix. Merge the rest of rust/harness only at a tip with green ui-shots. Never merge a harness sha that prints `deliberateFailure=FAIL`: `fc019b1` is B-009's throwaway proof that the gate fails.
- Coordinator rule (standing): a C1 or checkpoint commit whose only red step is a bundler download 504 in Build installer counts as green. If rust-rewrite's own Build installer flakes the same way, push an empty commit to re-run; do not revert.

## Queue and pending CI (as of the #14-#18 push, 2026-11-08)

- Pending: rust for the head that carries #14-#18 (`node tools/ci-wait.mjs origin/rust-rewrite`), and ui-shots for the same head (harness changed `tools/ui-shots/`). When rust is green with Build installer and Smoke passing, tell main the sha; the installer is the `aevalbuddy-tauri-nsis` artifact (`src-tauri/target/release/bundle/nsis/*.exe`) on that sha's rust workflow run (run URL in `origin/ci-logs:rust-rewrite/<sha>/rust/` meta).
- Coordinator plan now: a testable build for Andy. Merge each lane's green tip as it lands; a lane whose tip is not green stays at its last checkpoint; skip re-runs unless a real failure needs one.
- Queued: platform `941a42d` (its edit of harness-owned `tools/ui-shots/bridge.test.mjs` for `editMenuInFields` is approved by harness; on conflict keep harness's side unless harness adopted it; also B-085 one-click installer.nsi); windows `29ab55a`; agent `fe650d1`; bar (tip `688cb40` at last fetch; take it once its rust run is green with `styleStable`, now that #14 has the windows fix). Each needs `ci-wait <sha> --timeout-min 0` exit 0 first; all were pending at last check.
- Not green at last check: capture `9b0fee8` (smoke `lensFirstDraw`; newer tip `921d0c4` unchecked), sheet `1bedb11` (smoke `markdownStructure`, ui-shots `textInsetIs16InEveryKind`), platform `fef3044` (ui-shots `editMenuInFields`, fixed in `941a42d`).

## Base

- `243cda1` Record the audit, the backlog and one brief per fix lane (the base every lane branched from).
