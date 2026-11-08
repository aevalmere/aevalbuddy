# rust: failure

- Workflow: rust
- Run: [42509612971](https://github.com/aevalmere/aevalbuddy/actions/runs/42509612971), attempt 1, on push
- Commit: 8fa0e89e52bf111cd6224f8d71b51af3afbd9501
- Ref: rust/agent
- Conclusion: **failure**
- Failed: Smoke
- Finished: 2026-11-08T18:31:54Z

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
| Smoke | failure | [smoke.log](smoke.log) |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 8 error and warning lines from the logs.
