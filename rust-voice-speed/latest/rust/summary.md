# rust: failure

- Workflow: rust
- Run: [42585027206](https://github.com/aevalmere/aevalbuddy/actions/runs/42585027206), attempt 1, on push
- Commit: de37306d16f02520be0c5445d2fca849bb908f6d
- Ref: rust/voice-speed
- Conclusion: **failure**
- Failed: Smoke
- Finished: 2026-11-09T08:44:17Z

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

errors.txt: 6 error and warning lines from the logs.
