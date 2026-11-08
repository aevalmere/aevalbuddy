# rust: failure

- Workflow: rust
- Run: [42514927195](https://github.com/aevalmere/aevalbuddy/actions/runs/42514927195), attempt 1, on push
- Commit: 4e95f64d97b7b9fdb80b2015530b9138e6da4081
- Ref: rust/llm
- Conclusion: **failure**
- Failed: Smoke
- Finished: 2026-11-08T19:42:12Z

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

errors.txt: 4 error and warning lines from the logs.
