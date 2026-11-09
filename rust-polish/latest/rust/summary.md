# rust: failure

- Workflow: rust
- Run: [42550242083](https://github.com/aevalmere/aevalbuddy/actions/runs/42550242083), attempt 1, on push
- Commit: b4f81ca72133d80f7a998d8e83ef23d48d13cb84
- Ref: rust/polish
- Conclusion: **failure**
- Failed: Clippy, Unit tests, Capture check, Desktop selfcheck
- Finished: 2026-11-09T02:26:33Z

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
