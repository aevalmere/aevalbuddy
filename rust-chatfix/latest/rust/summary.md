# rust: failure

- Workflow: rust
- Run: [42569041773](https://github.com/aevalmere/aevalbuddy/actions/runs/42569041773), attempt 1, on push
- Commit: 74e88e834b0649c8fe38db0d25ed239d70bfaadb
- Ref: rust/chatfix
- Conclusion: **failure**
- Failed: Smoke
- Finished: 2026-11-09T06:09:40Z

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
| Build installer | success | [build.log](build.log) |
| Smoke | failure | [smoke.log](smoke.log) |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 8 error and warning lines from the logs.
