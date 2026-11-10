# rust: failure

- Workflow: rust
- Run: [42885440546](https://github.com/aevalmere/aevaleron/actions/runs/42885440546), attempt 1, on push
- Commit: 4ef0eea0447909a6fa39a70ca7fa60839f21f907
- Ref: rust-rewrite
- Conclusion: **failure**
- Failed: Smoke
- Finished: 2026-11-10T19:42:33Z

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

errors.txt: 10 error and warning lines from the logs.
