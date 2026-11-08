# rust: failure

- Workflow: rust
- Run: [42514360933](https://github.com/aevalmere/aevalbuddy/actions/runs/42514360933), attempt 1, on push
- Commit: 84e2bd9cd2ae99415bacddbf4a72a4afb1ac1c36
- Ref: rust-rewrite
- Conclusion: **failure**
- Failed: Clippy, Unit tests
- Finished: 2026-11-08T19:21:19Z

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
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 13 error and warning lines from the logs.
