# rust: cancelled

- Workflow: rust
- Run: [42748329240](https://github.com/aevalmere/aevalbuddy/actions/runs/42748329240), attempt 1, on push
- Commit: 4db0447f6b08ebf3c0ae18d4b0bdc51bd1c2060a
- Ref: rust-rewrite
- Conclusion: **cancelled**
- Finished: 2026-11-10T03:23:18Z

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
