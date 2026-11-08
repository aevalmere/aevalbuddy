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
| 1 | capture | C1 | `f343f64` | `6fcad0f` | pending | n/a | Pushed together with #2 |
| 2 | bar | C1 | `c114f4a` | `8e20ad6` | pending | pending | Pushed together with #1 |

## Reverts

None.

## Seam fixes

None.

## Open integration issues

- platform C1 `040ad93`: red (fmt diff in `crash.rs`, compile errors from two `windows_result` versions). Not merged; waiting for a green platform C1.
- agent C1 `3ef7629`: red (fmt, capture check, desktop selfcheck). Not merged; waiting for a green agent C1.

## Base

- `243cda1` Record the audit, the backlog and one brief per fix lane (the base every lane branched from).
