# rust: failure

- Workflow: rust
- Run: [42517587926](https://github.com/aevalmere/aevalbuddy/actions/runs/42517587926), attempt 1, on push
- Commit: 88bf4907b28490a655ae2d2670203dd970e7d9b1
- Ref: rust-rewrite
- Conclusion: **failure**
- Failed: Smoke
- Finished: 2026-11-08T20:07:47Z

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
| Build installer | success | [build.log](build.log) |
| Smoke | failure | [smoke.log](smoke.log) |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 6 error and warning lines from the logs.
