# rust: cancelled

- Workflow: rust
- Run: [42514861592](https://github.com/aevalmere/aevalbuddy/actions/runs/42514861592), attempt 1, on push
- Commit: f90f883c79d02c58de46064bbe4145b7239eb95d
- Ref: rust-rewrite
- Conclusion: **cancelled**
- Finished: 2026-11-08T19:27:52Z

| Step | Outcome | Log |
|---|---|---|
| Checkout | success |  |
| Node | success |  |
| Rust toolchain | success |  |
| Cache cargo | success |  |
| Gate helper | success |  |
| Prebuild desktop helper | success | [prebuild.log](prebuild.log) |
| Format | success |  |
| Format patch | skipped |  |
| Clippy | success | [clippy.log](clippy.log) |
| Unit tests | success | [test.log](test.log) |
| Restore NSIS toolset | success |  |
| Seed NSIS toolset | success | [nsis.log](nsis.log) |
| Save NSIS toolset | skipped |  |
| Build installer | cancelled | [build.log](build.log) |
| Smoke | skipped |  |
| Capture check | skipped |  |
| Desktop selfcheck | skipped |  |
| Upload installer | skipped |  |

errors.txt: 0 error and warning lines from the logs.
