# rust: failure

- Workflow: rust
- Run: [42551308267](https://github.com/aevalmere/aevalbuddy/actions/runs/42551308267), attempt 1, on push
- Commit: c965f44732a8850adc566d806b8849b200253d9c
- Ref: rust/polish
- Conclusion: **failure**
- Failed: Clippy, Unit tests, Capture check, Desktop selfcheck
- Finished: 2026-11-09T02:38:41Z

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

errors.txt: 70 error and warning lines from the logs.
