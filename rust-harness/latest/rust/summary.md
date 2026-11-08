# rust: failure

- Workflow: rust
- Run: [42465476955](https://github.com/aevalmere/aevalbuddy/actions/runs/42465476955), attempt 1, on push
- Commit: b684715f24f31cac20872ae31b60398d575d4149
- Ref: rust/harness
- Conclusion: **failure**
- Failed: Format
- Finished: 2026-11-08T09:09:03Z

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
| Clippy | success | [clippy.log](clippy.log) |
| Unit tests | success | [test.log](test.log) |
| Tauri CLI version | skipped |  |
| Cache Tauri bundler tools | skipped |  |
| Build installer | skipped |  |
| Smoke | skipped |  |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 10 error and warning lines from the logs.
