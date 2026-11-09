# rust: failure

- Workflow: rust
- Run: [42545598638](https://github.com/aevalmere/aevalbuddy/actions/runs/42545598638), attempt 1, on push
- Commit: ca3547dac49a93fdffafe276011bd5d9cec2da53
- Ref: rust/function
- Conclusion: **failure**
- Failed: Format, Clippy, Capture check, Desktop selfcheck
- Finished: 2026-11-09T01:36:13Z

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
| Capture check | failure | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 81 error and warning lines from the logs.
