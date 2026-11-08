# rust: failure

- Workflow: rust
- Run: [42509076371](https://github.com/aevalmere/aevalbuddy/actions/runs/42509076371), attempt 1, on push
- Commit: fe650d139bafc54e44c7df6cab8f6b1da6237bf3
- Ref: rust/agent
- Conclusion: **failure**
- Failed: Format
- Finished: 2026-11-08T18:19:29Z

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

errors.txt: 12 error and warning lines from the logs.
