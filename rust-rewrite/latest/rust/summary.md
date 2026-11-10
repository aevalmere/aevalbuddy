# rust: failure

- Workflow: rust
- Run: [42757052527](https://github.com/aevalmere/aevalbuddy/actions/runs/42757052527), attempt 1, on push
- Commit: 54ab63e9081213ee380ceefd485112e65bfa7d5d
- Ref: rust-rewrite
- Conclusion: **failure**
- Failed: Format
- Finished: 2026-11-10T04:41:47Z

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
| Restore NSIS toolset | skipped |  |
| Seed NSIS toolset | skipped |  |
| Save NSIS toolset | skipped |  |
| Build installer | skipped |  |
| Smoke | skipped |  |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 13 error and warning lines from the logs.
