# rust: failure

- Workflow: rust
- Run: [42749101749](https://github.com/aevalmere/aevalbuddy/actions/runs/42749101749), attempt 1, on push
- Commit: 7c399857e6bae3f37f8f19f5dc37a7171592f897
- Ref: rust-rewrite
- Conclusion: **failure**
- Failed: Format, Unit tests
- Finished: 2026-11-10T03:26:58Z

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
| Restore NSIS toolset | skipped |  |
| Seed NSIS toolset | skipped |  |
| Save NSIS toolset | skipped |  |
| Build installer | skipped |  |
| Smoke | skipped |  |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 35 error and warning lines from the logs.
