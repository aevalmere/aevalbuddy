# rust: cancelled

- Workflow: rust
- Run: [42560055069](https://github.com/aevalmere/aevalbuddy/actions/runs/42560055069), attempt 1, on push
- Commit: 813106a5c5564ab5d667ec99d2251190c8fcbb35
- Ref: rust/polish
- Conclusion: **cancelled**
- Finished: 2026-11-09T04:19:29Z

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
| Smoke | cancelled | [smoke.log](smoke.log) |
| Capture check | skipped |  |
| Desktop selfcheck | skipped |  |
| Upload installer | skipped |  |

errors.txt: 0 error and warning lines from the logs.
