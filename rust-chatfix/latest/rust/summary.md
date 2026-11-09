# rust: failure

- Workflow: rust
- Run: [42591963739](https://github.com/aevalmere/aevalbuddy/actions/runs/42591963739), attempt 1, on push
- Commit: ef3e653315ff192ca4b2f6577bea7bf4a88e5586
- Ref: rust/chatfix
- Conclusion: **failure**
- Failed: Format, Capture check, Desktop selfcheck
- Finished: 2026-11-09T09:25:45Z

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

errors.txt: 12 error and warning lines from the logs.
