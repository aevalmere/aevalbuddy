# voice-bench: success

- Workflow: voice-bench
- Run: [43568168299](https://github.com/aevalmere/aevaleron/actions/runs/43568168299), attempt 1, on push
- Commit: 6434ffa6f2bfd499db210259aad594ccc828f96d
- Ref: bench/parakeet
- Conclusion: **success**
- Failed: Build both engines in one exe
- Finished: 2026-11-14T07:06:51Z

| Step | Outcome | Log |
|---|---|---|
| Checkout | success |  |
| Node | success |  |
| Rust toolchain | success | [runner.log](runner.log) |
| Models and clips | success | [fetch.log](fetch.log) |
| Build the current engine | success | [build-now.log](build-now.log) |
| Build the candidate engine | success | [build-next.log](build-next.log) |
| Runtime DLLs | success | [dlls.log](dlls.log) |
| Time the current engine | success | [now.log](now.log) |
| Time the candidates | success | [next.log](next.log) |
| Build both engines in one exe | failure | [build-both.log](build-both.log) |
| Run both engines in one exe | skipped |  |
| Report | success | [report.log](report.log) |

errors.txt: 31 error and warning lines from the logs.
