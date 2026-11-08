# rust: failure

- Workflow: rust
- Run: [42525770478](https://github.com/aevalmere/aevalbuddy/actions/runs/42525770478), attempt 1, on push
- Commit: b66d255bbbe54719e203eeee81df3d32a6faa5a1
- Ref: rust/voice
- Conclusion: **failure**
- Failed: Format, Clippy, Unit tests, Capture check, Desktop selfcheck
- Finished: 2026-11-08T21:39:45Z

| Step | Outcome | Log |
|---|---|---|
| Checkout | success |  |
| Node | success |  |
| Rust toolchain | success |  |
| Cache cargo | success |  |
| Gate helper | success |  |
| Prebuild desktop helper | success | [prebuild.log](prebuild.log) |
| Format | failure | [fmt.log](fmt.log) |
| Format patch | success | [fmt.patch](fmt.patch) |
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

errors.txt: 416 error and warning lines from the logs.
