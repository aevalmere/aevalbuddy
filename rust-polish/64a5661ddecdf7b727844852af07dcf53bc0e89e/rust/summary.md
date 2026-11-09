# rust: cancelled

- Workflow: rust
- Run: [42547836950](https://github.com/aevalmere/aevalbuddy/actions/runs/42547836950), attempt 1, on push
- Commit: 64a5661ddecdf7b727844852af07dcf53bc0e89e
- Ref: rust/polish
- Conclusion: **cancelled**
- Finished: 2026-11-09T02:07:37Z

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
| Save NSIS toolset | success |  |
| Build installer | cancelled | [build.log](build.log) |
| Smoke | skipped |  |
| Capture check | skipped |  |
| Desktop selfcheck | skipped |  |
| Upload installer | skipped |  |

errors.txt: 0 error and warning lines from the logs.
