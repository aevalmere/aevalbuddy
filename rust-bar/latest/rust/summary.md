# rust: cancelled

- Workflow: rust
- Run: [42463859962](https://github.com/aevalmere/aevalbuddy/actions/runs/42463859962), attempt 1, on push
- Commit: 8c73071abb2363e90be63682ea00bc3e4f19bf28
- Ref: rust/bar
- Conclusion: **cancelled**
- Finished: 2026-11-08T08:43:12Z

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
| Build installer | cancelled | [build.log](build.log) |
| Scaffold smoke | skipped |  |
| Capture check | skipped |  |
| Desktop selfcheck | skipped |  |
| Upload installer | skipped |  |

errors.txt: 0 error and warning lines from the logs.
