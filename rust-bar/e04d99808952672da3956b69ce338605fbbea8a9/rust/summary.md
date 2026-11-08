# rust: failure

- Workflow: rust
- Run: [42491700898](https://github.com/aevalmere/aevalbuddy/actions/runs/42491700898), attempt 1, on push
- Commit: e04d99808952672da3956b69ce338605fbbea8a9
- Ref: rust/bar
- Conclusion: **failure**
- Failed: Smoke
- Finished: 2026-11-08T14:58:44Z

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
| Build installer | success | [build.log](build.log) |
| Smoke | failure | [smoke.log](smoke.log) |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | skipped |  |

errors.txt: 4 error and warning lines from the logs.
