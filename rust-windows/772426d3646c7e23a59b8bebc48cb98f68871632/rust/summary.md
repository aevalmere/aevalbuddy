# rust: cancelled

- Workflow: rust
- Run: [42514259154](https://github.com/aevalmere/aevalbuddy/actions/runs/42514259154), attempt 1, on push
- Commit: 772426d3646c7e23a59b8bebc48cb98f68871632
- Ref: rust/windows
- Conclusion: **cancelled**
- Finished: 2026-11-08T19:24:57Z

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
