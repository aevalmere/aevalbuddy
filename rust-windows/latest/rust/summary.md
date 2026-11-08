# rust: failure

- Workflow: rust
- Run: [42491645192](https://github.com/aevalmere/aevalbuddy/actions/runs/42491645192), attempt 1, on push
- Commit: bbb2d19800ac23c88e0160b35ff08aeab462e983
- Ref: rust/windows
- Conclusion: **failure**
- Failed: Format, Unit tests, Capture check, Desktop selfcheck
- Finished: 2026-11-08T14:55:52Z

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
| Capture check | failure | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 17 error and warning lines from the logs.
