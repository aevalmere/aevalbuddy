# rust: failure

- Workflow: rust
- Run: [42465866439](https://github.com/aevalmere/aevalbuddy/actions/runs/42465866439), attempt 1, on push
- Commit: d7e5e5f9ad28dbe0f182e0b17bce8c0a6e2215e8
- Ref: rust/platform
- Conclusion: **failure**
- Failed: Format, Clippy, Capture check, Desktop selfcheck
- Finished: 2026-11-08T09:14:39Z

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
| Clippy | failure | [clippy.log](clippy.log) |
| Unit tests | success | [test.log](test.log) |
| Build installer | skipped |  |
| Smoke | skipped |  |
| Capture check | failure | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 51 error and warning lines from the logs.
