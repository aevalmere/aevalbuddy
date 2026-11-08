# rust: failure

- Workflow: rust
- Run: [42466187678](https://github.com/aevalmere/aevalbuddy/actions/runs/42466187678), attempt 1, on push
- Commit: 9e79ab976ae24c462f5f9571db23d696177701b5
- Ref: rust/windows
- Conclusion: **failure**
- Failed: Build installer
- Finished: 2026-11-08T09:23:51Z

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
| Build installer | failure | [build.log](build.log) |
| Smoke | skipped |  |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 1 error and warning lines from the logs.
