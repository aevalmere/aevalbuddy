# rust: failure

- Workflow: rust
- Run: [42465748998](https://github.com/aevalmere/aevalbuddy/actions/runs/42465748998), attempt 1, on push
- Commit: 3fcfbd935bba4a959fbad1c056c6aaed8444687b
- Ref: rust/agent
- Conclusion: **failure**
- Failed: Build installer, Desktop selfcheck
- Finished: 2026-11-08T09:15:08Z

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
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 3 error and warning lines from the logs.
