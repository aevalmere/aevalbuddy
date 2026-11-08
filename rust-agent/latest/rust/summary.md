# rust: failure

- Workflow: rust
- Run: [42466517382](https://github.com/aevalmere/aevalbuddy/actions/runs/42466517382), attempt 1, on push
- Commit: cc1b2ff46d550e07552696963b685c1334b22a81
- Ref: rust/agent
- Conclusion: **failure**
- Failed: Unit tests
- Finished: 2026-11-08T09:22:54Z

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
| Unit tests | failure | [test.log](test.log) |
| Build installer | skipped |  |
| Smoke | skipped |  |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 12 error and warning lines from the logs.
