# rust: failure

- Workflow: rust
- Run: [42492499739](https://github.com/aevalmere/aevalbuddy/actions/runs/42492499739), attempt 1, on push
- Commit: ca684b549e29f5bbd1b2e5fb8a7c60ca48a9b940
- Ref: rust/platform
- Conclusion: **failure**
- Failed: Desktop selfcheck
- Finished: 2026-11-08T15:09:10Z

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
| Restore NSIS toolset | success |  |
| Seed NSIS toolset | success | [nsis.log](nsis.log) |
| Save NSIS toolset | success |  |
| Build installer | success | [build.log](build.log) |
| Smoke | success | [smoke.log](smoke.log) |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 2 error and warning lines from the logs.
