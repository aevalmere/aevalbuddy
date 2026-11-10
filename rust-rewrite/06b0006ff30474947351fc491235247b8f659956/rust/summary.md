# rust: success

- Workflow: rust
- Run: [42750990205](https://github.com/aevalmere/aevalbuddy/actions/runs/42750990205), attempt 1, on push
- Commit: 06b0006ff30474947351fc491235247b8f659956
- Ref: rust-rewrite
- Conclusion: **success**
- Finished: 2026-11-10T03:54:56Z

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
| Save NSIS toolset | skipped |  |
| Build installer | success | [build.log](build.log) |
| Smoke | success | [smoke.log](smoke.log) |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | success |  |

errors.txt: 0 error and warning lines from the logs.
