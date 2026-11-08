# rust: failure

- Workflow: rust
- Run: [42466195345](https://github.com/aevalmere/aevalbuddy/actions/runs/42466195345), attempt 1, on push
- Commit: 75d531f2e95c5927b11057b98a6b99d6c44bbafb
- Ref: rust-rewrite
- Conclusion: **failure**
- Failed: Build installer
- Finished: 2026-11-08T09:23:32Z

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
