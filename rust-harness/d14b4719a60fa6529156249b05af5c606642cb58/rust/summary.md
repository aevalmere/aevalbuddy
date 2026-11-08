# rust: success

- Workflow: rust
- Run: [42466730725](https://github.com/aevalmere/aevalbuddy/actions/runs/42466730725), attempt 1, on push
- Commit: d14b4719a60fa6529156249b05af5c606642cb58
- Ref: rust/harness
- Conclusion: **success**
- Finished: 2026-11-08T09:31:48Z

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
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | success |  |

errors.txt: 0 error and warning lines from the logs.
