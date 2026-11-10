# rust: failure

- Workflow: rust
- Run: [42753961676](https://github.com/aevalmere/aevalbuddy/actions/runs/42753961676), attempt 1, on push
- Commit: 45c315b1cf89f11b31cb1028871e25f49dfc5af8
- Ref: rust-rewrite
- Conclusion: **failure**
- Failed: Format, Clippy
- Finished: 2026-11-10T04:12:54Z

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
| Restore NSIS toolset | skipped |  |
| Seed NSIS toolset | skipped |  |
| Save NSIS toolset | skipped |  |
| Build installer | skipped |  |
| Smoke | skipped |  |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 326 error and warning lines from the logs.
