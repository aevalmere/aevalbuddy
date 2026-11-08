# rust: cancelled

- Workflow: rust
- Run: [42509160898](https://github.com/aevalmere/aevalbuddy/actions/runs/42509160898), attempt 1, on push
- Commit: 8add4494733087fe03349626f4a2e9877f15d3d9
- Ref: rust/windows
- Conclusion: **cancelled**
- Finished: 2026-11-08T18:21:41Z

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
