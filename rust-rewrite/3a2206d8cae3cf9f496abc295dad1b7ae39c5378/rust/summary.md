# rust: failure

- Workflow: rust
- Run: [42867603329](https://github.com/aevalmere/aevaleron/actions/runs/42867603329), attempt 1, on push
- Commit: 3a2206d8cae3cf9f496abc295dad1b7ae39c5378
- Ref: rust-rewrite
- Conclusion: **failure**
- Failed: Clippy
- Finished: 2026-11-10T17:47:02Z

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

errors.txt: 12 error and warning lines from the logs.
