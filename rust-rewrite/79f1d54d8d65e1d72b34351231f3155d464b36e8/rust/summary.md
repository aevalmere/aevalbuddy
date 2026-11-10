# rust: cancelled

- Workflow: rust
- Run: [42749914506](https://github.com/aevalmere/aevalbuddy/actions/runs/42749914506), attempt 1, on push
- Commit: 79f1d54d8d65e1d72b34351231f3155d464b36e8
- Ref: rust-rewrite
- Conclusion: **cancelled**
- Finished: 2026-11-10T03:40:57Z

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
