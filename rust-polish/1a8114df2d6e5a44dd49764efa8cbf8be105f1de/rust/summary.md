# rust: failure

- Workflow: rust
- Run: [42549174168](https://github.com/aevalmere/aevalbuddy/actions/runs/42549174168), attempt 1, on push
- Commit: 1a8114df2d6e5a44dd49764efa8cbf8be105f1de
- Ref: rust/polish
- Conclusion: **failure**
- Failed: Clippy, Unit tests, Capture check, Desktop selfcheck
- Finished: 2026-11-09T02:16:16Z

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
| Clippy | failure | [clippy.log](clippy.log) |
| Unit tests | failure | [test.log](test.log) |
| Restore NSIS toolset | skipped |  |
| Seed NSIS toolset | skipped |  |
| Save NSIS toolset | skipped |  |
| Build installer | skipped |  |
| Smoke | skipped |  |
| Capture check | failure | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 70 error and warning lines from the logs.
