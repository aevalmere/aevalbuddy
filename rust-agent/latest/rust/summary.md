# rust: failure

- Workflow: rust
- Run: [42522753998](https://github.com/aevalmere/aevalbuddy/actions/runs/42522753998), attempt 1, on push
- Commit: 5af02f5842689bbd67420e0cecc2b9684f9c6070
- Ref: rust/agent
- Conclusion: **failure**
- Failed: Smoke
- Finished: 2026-11-08T21:14:10Z

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
| Smoke | failure | [smoke.log](smoke.log) |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 6 error and warning lines from the logs.
