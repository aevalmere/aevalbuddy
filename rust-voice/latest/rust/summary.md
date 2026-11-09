# rust: failure

- Workflow: rust
- Run: [42539737644](https://github.com/aevalmere/aevalbuddy/actions/runs/42539737644), attempt 1, on push
- Commit: e855069f0d910330c0ddab7f083c60acf5df4d80
- Ref: rust/voice
- Conclusion: **failure**
- Failed: Clippy, Unit tests, Capture check, Desktop selfcheck
- Finished: 2026-11-09T00:28:52Z

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
| Unit tests | failure | [test.log](test.log) |
| Restore NSIS toolset | skipped |  |
| Seed NSIS toolset | skipped |  |
| Save NSIS toolset | skipped |  |
| Build installer | skipped |  |
| Smoke | skipped |  |
| Capture check | failure | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 30 error and warning lines from the logs.
