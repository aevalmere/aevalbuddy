# rust: failure

- Workflow: rust
- Run: [42465942919](https://github.com/aevalmere/aevalbuddy/actions/runs/42465942919), attempt 1, on push
- Commit: a2ab4d8d8577cfd39fa272fe0a27c50c90a16ffd
- Ref: rust/sheet
- Conclusion: **failure**
- Failed: Build installer
- Finished: 2026-11-08T09:26:38Z

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
| Unit tests | success | [test.log](test.log) |
| Build installer | failure | [build.log](build.log) |
| Smoke | skipped |  |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 1 error and warning lines from the logs.
