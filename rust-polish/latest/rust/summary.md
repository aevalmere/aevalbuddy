# rust: failure

- Workflow: rust
- Run: [42778342609](https://github.com/aevalmere/aevalbuddy/actions/runs/42778342609), attempt 1, on push
- Commit: 6aec89acddb76ba34db5bd3654ebee10aad5dc34
- Ref: rust/polish
- Conclusion: **failure**
- Failed: Smoke
- Finished: 2026-11-10T08:15:46Z

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

errors.txt: 4 error and warning lines from the logs.
