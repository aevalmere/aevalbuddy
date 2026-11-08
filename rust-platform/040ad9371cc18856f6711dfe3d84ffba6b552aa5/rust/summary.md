# rust: failure

- Workflow: rust
- Run: [42453899170](https://github.com/aevalmere/aevalbuddy/actions/runs/42453899170), attempt 1, on push
- Commit: 040ad9371cc18856f6711dfe3d84ffba6b552aa5
- Ref: rust/platform
- Conclusion: **failure**
- Failed: Format, Clippy, Unit tests, Capture check, Desktop selfcheck
- Finished: 2026-11-08T06:20:18Z

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
| Build installer | skipped |  |
| Scaffold smoke | skipped |  |
| Capture check | failure | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 164 error and warning lines from the logs.
