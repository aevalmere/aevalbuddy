# rust: failure

- Workflow: rust
- Run: [42464490387](https://github.com/aevalmere/aevalbuddy/actions/runs/42464490387), attempt 1, on push
- Commit: bc63c8bd7ec9196428b7bb9bce75358ddf7b6443
- Ref: rust/llm
- Conclusion: **failure**
- Failed: Format, Capture check, Desktop selfcheck
- Finished: 2026-11-08T08:56:29Z

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
| Build installer | skipped |  |
| Scaffold smoke | skipped |  |
| Capture check | failure | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 20 error and warning lines from the logs.
