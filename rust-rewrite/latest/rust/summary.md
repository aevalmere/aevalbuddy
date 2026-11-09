# rust: success

- Workflow: rust
- Run: [42544898600](https://github.com/aevalmere/aevalbuddy/actions/runs/42544898600), attempt 1, on push
- Commit: 5c4c7ba8446ce7e2dfe349da577a5dc1fd501703
- Ref: rust-rewrite
- Conclusion: **success**
- Finished: 2026-11-09T01:37:20Z

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
| Smoke | success | [smoke.log](smoke.log) |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | success |  |

errors.txt: 0 error and warning lines from the logs.
