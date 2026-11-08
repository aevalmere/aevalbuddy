# rust: failure

- Workflow: rust
- Run: [42466259458](https://github.com/aevalmere/aevalbuddy/actions/runs/42466259458), attempt 1, on push
- Commit: 511ed8116a7c8b1c667bc698a347ce9b2829ae85
- Ref: rust/capture
- Conclusion: **failure**
- Failed: Format, Clippy
- Finished: 2026-11-08T09:20:04Z

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
| Unit tests | success | [test.log](test.log) |
| Build installer | skipped |  |
| Smoke | skipped |  |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 23 error and warning lines from the logs.
