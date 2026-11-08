# rust: failure

- Workflow: rust
- Run: [42464460250](https://github.com/aevalmere/aevalbuddy/actions/runs/42464460250), attempt 1, on push
- Commit: 13971186c40f571a5e28c7901f82b803f359883f
- Ref: rust/windows
- Conclusion: **failure**
- Failed: Format, Capture check, Desktop selfcheck
- Finished: 2026-11-08T08:54:23Z

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
| Capture check | failure | [capture.log](capture.log) |
| Desktop selfcheck | failure | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 30 error and warning lines from the logs.
