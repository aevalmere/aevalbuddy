# rust: failure

- Workflow: rust
- Run: [42465202390](https://github.com/aevalmere/aevalbuddy/actions/runs/42465202390), attempt 1, on push
- Commit: 9f3adb9c66873f353b8610887dcd0c9ae98b61fa
- Ref: rust/capture
- Conclusion: **failure**
- Failed: Clippy, Unit tests
- Finished: 2026-11-08T09:04:34Z

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
| Build installer | skipped |  |
| Scaffold smoke | skipped |  |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 37 error and warning lines from the logs.
