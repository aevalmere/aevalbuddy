# Shared-document entries from the harness lane

## ARCHITECTURE · 7. Verification · B-009
Replace "- `tauri build` produces the NSIS installer as an artifact; a scaffold smoke runs the built exe with
  `AEVAL_SMOKE=1` and requires `[SMOKE] scaffold.bridge=ok` (every page loaded and called through
  `window.aeval`)." with:
- `tauri build` produces the NSIS installer as an artifact, and the Smoke step runs the built exe with
  `AEVAL_SMOKE=1` (`src-tauri/src/selftest/`). Each lane proves its behaviour inside the real exe in
  its own probe, `selftest/probes/<lane>.rs` (`pub fn run(cx: &mut Cx)`), through one API documented
  with a worked example at the top of `selftest/mod.rs`: `cx.must(label, ok, why)` prints
  `[SMOKE] label=ok` or `=FAIL:<why>`; `cx.log(label, value)` prints `label=<value>`;
  `cx.eval(window, js)` returns the JSON value of an expression in that window's page (a Promise is
  awaited, 5 s timeout); `cx.input(Input::..)` sends real `SendInput` mouse and keyboard input at
  physical pixels (move, click, double and right click, drag, wheel, key chord, text);
  `cx.wait_for(timeout, cond)`, `cx.block_on(fut)` and `cx.on_main(f)` cover waiting, async code and
  the main thread. Probes run in a fixed lane order, each in its own `catch_unwind`
  (`probe.<lane>=ok|FAIL`), and roll up into Electron's block labels `v2basics`, `v21basics`,
  `glassGates` and `v2desktop` (`selftest/probes/mod.rs` BLOCKS). The run ends `[SMOKE] done` (exit
  0) or `[SMOKE] failed: a, b` (exit 1); a panic outside the probes prints `[SMOKE] error=<msg>`.
  `tools/smoke-gate.mjs` fails the step on any `=FAIL`, a missing `done`, or any label listed in
  `tools/smoke-labels/<lane>.txt` (one file per lane, owned by that lane) that never printed.

## ARCHITECTURE · 7. Verification · B-053
Add:
- ui-shots (`tools/ui-shots/shots.mjs`): each lane owns `tools/ui-shots/lanes/<lane>.mjs`, exporting
  `{ scenes, mocks, checks, cssLintEnforce }` in the shape documented at the top of `shots.mjs`
  (shared fixtures in `tools/ui-shots/kit.mjs`, the lane list in `tools/ui-shots/lanes.mjs`). Every
  scene runs at device scale 1 and 1.5 (`-150` suffix), and once more at `--text-scale: 2` (`-ts200`)
  when flagged; a scene may run on Playwright's virtual clock. Lane `mocks` merge over the common
  answers; `bridge.test.mjs` checks every lane file's shape. Pictures are published per branch to
  `shots:rust-d-ui/<ref with / as ->/latest/` and `/<sha>/`.
