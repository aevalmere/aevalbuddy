# rust: success

- Workflow: rust
- Run: [42924029162](https://github.com/aevalmere/aevaleron/actions/runs/42924029162), attempt 1, on push
- Commit: 5bc7114ac7d56a8afc26e8b7b674b89ad9e8ad73
- Ref: rust-rewrite
- Conclusion: **success**
- Finished: 2026-11-10T23:45:55Z

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
| Smoke | success | [smoke.log](smoke.log) |
| Capture check | success | [capture.log](capture.log) |
| Desktop selfcheck | success | [desktop.log](desktop.log) |
| Upload installer | success |  |

errors.txt: 0 error and warning lines from the logs.
