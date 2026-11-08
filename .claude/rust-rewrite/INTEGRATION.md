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
| 4 | bar | C1 follow-up | `d84a9d3` | `f355865` | cancelled by #5's push | pending | Icons as CSS masks of a well-formed `icons.svg`. Renderer only. Merged on the coordinator's call with ui-shots green and its rust run red only on an NSIS download 504 in Build installer |
| 5 | platform | C1 | `caadb51` | `17e55bb` | pending | n/a | `040ad93` + `3e74cba` (windows 0.62 core for webview2-com) + a Progress-only commit. At `3e74cba` every gate passed but Build installer (NSIS download 504); merged on the coordinator's rule |

## Reverts

None.

## Seam fixes

None.

## Open integration issues

- Coordinator rule (standing): a C1 or checkpoint commit whose only red step is a bundler download 504 in Build installer counts as green. If rust-rewrite's own Build installer flakes the same way, push an empty commit to re-run; do not revert.

## Base

- `243cda1` Record the audit, the backlog and one brief per fix lane (the base every lane branched from).
