# rust: failure

- Workflow: rust
- Run: [42464380118](https://github.com/aevalmere/aevalbuddy/actions/runs/42464380118), attempt 1, on push
- Commit: 97d71a9b71967e7f0b440e69a078ed4829695442
- Ref: rust/agent
- Conclusion: **failure**
- Failed: Build installer
- Finished: 2026-11-08T08:57:26Z

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
| Scaffold smoke | skipped |  |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 1 error and warning lines from the logs.
