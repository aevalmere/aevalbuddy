# rust: failure

- Workflow: rust
- Run: [42514189577](https://github.com/aevalmere/aevalbuddy/actions/runs/42514189577), attempt 1, on push
- Commit: 2619bbc07c418bf141387aa3199733dbdce01dca
- Ref: rust/llm
- Conclusion: **failure**
- Failed: Format, Unit tests, Capture check, Desktop selfcheck
- Finished: 2026-11-08T19:23:09Z

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

errors.txt: 107 error and warning lines from the logs.
