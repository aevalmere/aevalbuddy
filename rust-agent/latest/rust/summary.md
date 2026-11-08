# rust: failure

- Workflow: rust
- Run: [42494149341](https://github.com/aevalmere/aevalbuddy/actions/runs/42494149341), attempt 1, on push
- Commit: 0db39124cabec1ef79e51677e6ee318cfd87eb9c
- Ref: rust/agent
- Conclusion: **failure**
- Failed: Unit tests
- Finished: 2026-11-08T15:22:03Z

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
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 13 error and warning lines from the logs.
