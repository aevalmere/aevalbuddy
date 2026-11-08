# ui-shots: failure

- Workflow: ui-shots
- Run: [42514360898](https://github.com/aevalmere/aevalbuddy/actions/runs/42514360898), attempt 1, on push
- Commit: 84e2bd9cd2ae99415bacddbf4a72a4afb1ac1c36
- Ref: rust-rewrite
- Conclusion: **failure**
- Failed: Bridge unit checks
- Finished: 2026-11-08T19:18:44Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust-rewrite/84e2bd9cd2ae99415bacddbf4a72a4afb1ac1c36 (/ in the ref becomes -)

| Step | Outcome | Log |
|---|---|---|
| Checkout | success |  |
| Node | success |  |
| Bridge unit checks | failure | [bridge.log](bridge.log) |
| Install Playwright | skipped |  |
| Screenshots and renderer checks | skipped |  |
| Upload screenshots | success |  |
| Publish to the shots branch | success |  |

errors.txt: 1 error and warning lines from the logs.
