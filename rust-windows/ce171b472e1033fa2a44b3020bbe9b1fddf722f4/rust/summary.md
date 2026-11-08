# rust: failure

- Workflow: rust
- Run: [42493679802](https://github.com/aevalmere/aevalbuddy/actions/runs/42493679802), attempt 1, on push
- Commit: ce171b472e1033fa2a44b3020bbe9b1fddf722f4
- Ref: rust/windows
- Conclusion: **failure**
- Failed: Format, Capture check, Desktop selfcheck
- Finished: 2026-11-08T15:17:30Z

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
| Capture check | failure | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 11 error and warning lines from the logs.
