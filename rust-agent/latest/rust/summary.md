# rust: failure

- Workflow: rust
- Run: [42465345230](https://github.com/aevalmere/aevalbuddy/actions/runs/42465345230), attempt 1, on push
- Commit: 9f0b3f4aded12cb88da23fb698509ca5fab036e0
- Ref: rust/agent
- Conclusion: **failure**
- Failed: Format
- Finished: 2026-11-08T09:05:31Z

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
| Clippy | success | [clippy.log](clippy.log) |
| Unit tests | success | [test.log](test.log) |
| Build installer | skipped |  |
| Scaffold smoke | skipped |  |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 120 error and warning lines from the logs.
