# rust: failure

- Workflow: rust
- Run: [42466687549](https://github.com/aevalmere/aevalbuddy/actions/runs/42466687549), attempt 1, on push
- Commit: 8cdeb18f0b9d17089580e87c0cfa83d0bd388347
- Ref: rust/platform
- Conclusion: **failure**
- Failed: Clippy, Capture check, Desktop selfcheck
- Finished: 2026-11-08T09:26:42Z

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
| Unit tests | success | [test.log](test.log) |
| Build installer | skipped |  |
| Smoke | skipped |  |
| Capture check | failure | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 12 error and warning lines from the logs.
