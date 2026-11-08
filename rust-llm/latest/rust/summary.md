# rust: failure

- Workflow: rust
- Run: [42463910933](https://github.com/aevalmere/aevalbuddy/actions/runs/42463910933), attempt 1, on push
- Commit: 1b3898040a383093bcb9843b3d5a609237fcf0f8
- Ref: rust/llm
- Conclusion: **failure**
- Failed: Format, Unit tests, Capture check, Desktop selfcheck
- Finished: 2026-11-08T08:46:39Z

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
| Unit tests | failure | [test.log](test.log) |
| Build installer | skipped |  |
| Scaffold smoke | skipped |  |
| Capture check | failure | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 47 error and warning lines from the logs.
