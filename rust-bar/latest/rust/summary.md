# rust: failure

- Workflow: rust
- Run: [42464112632](https://github.com/aevalmere/aevalbuddy/actions/runs/42464112632), attempt 1, on push
- Commit: d84a9d3ee7bfbdfa13fafe8058c210fbfc07e0c3
- Ref: rust/bar
- Conclusion: **failure**
- Failed: Build installer
- Finished: 2026-11-08T08:51:06Z

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
