# rust: failure

- Workflow: rust
- Run: [42491295258](https://github.com/aevalmere/aevalbuddy/actions/runs/42491295258), attempt 1, on push
- Commit: 6d65f22c40ebc7cc249bcff4d15037835be4e600
- Ref: rust/harness
- Conclusion: **failure**
- Failed: Format
- Finished: 2026-11-08T14:48:17Z

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

errors.txt: 24 error and warning lines from the logs.
