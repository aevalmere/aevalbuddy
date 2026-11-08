# rust: failure

- Workflow: rust
- Run: [42516379428](https://github.com/aevalmere/aevalbuddy/actions/runs/42516379428), attempt 1, on push
- Commit: 691774d3b0a4b0e92eb9f03f142f4283ad83f462
- Ref: rust/platform
- Conclusion: **failure**
- Failed: Clippy
- Finished: 2026-11-08T19:46:25Z

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
| Unit tests | success | [test.log](test.log) |
| Restore NSIS toolset | skipped |  |
| Seed NSIS toolset | skipped |  |
| Save NSIS toolset | skipped |  |
| Build installer | skipped |  |
| Smoke | skipped |  |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 22 error and warning lines from the logs.
