# rust: failure

- Workflow: rust
- Run: [42447541769](https://github.com/aevalmere/aevalbuddy/actions/runs/42447541769), attempt 1, on push
- Commit: be1a69b3e7fd0e4d83515f6698891c62d370bebe
- Ref: rust/ci-logs-probe
- Conclusion: **failure**
- Failed: Format, Clippy, Unit tests, Capture check, Desktop selfcheck
- Finished: 2026-11-08T04:42:38Z

| Step | Outcome | Log |
|---|---|---|
| Checkout | success |  |
| Node | success |  |
| Rust toolchain | success |  |
| Cache cargo | success |  |
| Gate helper | success |  |
| Prebuild desktop helper | success | [prebuild.log](prebuild.log) |
| Format | failure | [fmt.log](fmt.log) |
| Format patch | success | [fmt.patch](fmt.patch) |
| Clippy | failure | [clippy.log](clippy.log) |
| Unit tests | failure | [test.log](test.log) |
| Build installer | skipped |  |
| Scaffold smoke | skipped |  |
| Capture check | failure | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 36 error and warning lines from the logs.
