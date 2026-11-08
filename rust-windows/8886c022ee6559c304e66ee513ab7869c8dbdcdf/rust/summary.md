# rust: success

- Workflow: rust
- Run: [42509614141](https://github.com/aevalmere/aevalbuddy/actions/runs/42509614141), attempt 1, on push
- Commit: 8886c022ee6559c304e66ee513ab7869c8dbdcdf
- Ref: rust/windows
- Conclusion: **success**
- Finished: 2026-11-08T18:31:20Z

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
