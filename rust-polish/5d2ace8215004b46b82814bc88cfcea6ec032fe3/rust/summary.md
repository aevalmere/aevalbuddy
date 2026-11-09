# rust: failure

- Workflow: rust
- Run: [42546358691](https://github.com/aevalmere/aevalbuddy/actions/runs/42546358691), attempt 1, on push
- Commit: 5d2ace8215004b46b82814bc88cfcea6ec032fe3
- Ref: rust/polish
- Conclusion: **failure**
- Failed: Unit tests, Capture check, Desktop selfcheck
- Finished: 2026-11-09T01:45:10Z

| Step | Outcome | Log |
|---|---|---|
| Checkout | success |  |
| Node | success |  |
| Rust toolchain | success |  |
| Cache cargo | success |  |
| Gate helper | success |  |
| Prebuild desktop helper | success | [prebuild.log](prebuild.log) |
| Format | success |  |
| Format patch | skipped |  |
| Clippy | success | [clippy.log](clippy.log) |
| Unit tests | failure | [test.log](test.log) |
| Restore NSIS toolset | skipped |  |
| Seed NSIS toolset | skipped |  |
| Save NSIS toolset | skipped |  |
| Build installer | skipped |  |
| Smoke | skipped |  |
| Capture check | failure | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 7 error and warning lines from the logs.
