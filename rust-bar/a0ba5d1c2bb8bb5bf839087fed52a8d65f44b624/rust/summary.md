# rust: cancelled

- Workflow: rust
- Run: [42508519570](https://github.com/aevalmere/aevalbuddy/actions/runs/42508519570), attempt 1, on push
- Commit: a0ba5d1c2bb8bb5bf839087fed52a8d65f44b624
- Ref: rust/bar
- Conclusion: **cancelled**
- Finished: 2026-11-08T18:09:48Z

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
| Clippy | cancelled | [clippy.log](clippy.log) |
| Unit tests | skipped |  |
| Restore NSIS toolset | skipped |  |
| Seed NSIS toolset | skipped |  |
| Save NSIS toolset | skipped |  |
| Build installer | skipped |  |
| Smoke | skipped |  |
| Capture check | skipped |  |
| Desktop selfcheck | skipped |  |
| Upload installer | skipped |  |

errors.txt: 0 error and warning lines from the logs.
