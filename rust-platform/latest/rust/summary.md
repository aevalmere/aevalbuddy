# rust: failure

- Workflow: rust
- Run: [42463487474](https://github.com/aevalmere/aevalbuddy/actions/runs/42463487474), attempt 1, on push
- Commit: 3e74cba7abcdc2265260895b142e77e6540d52ac
- Ref: rust/platform
- Conclusion: **failure**
- Failed: Build installer
- Finished: 2026-11-08T08:51:14Z

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
