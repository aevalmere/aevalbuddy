# ui-shots: failure

- Workflow: ui-shots
- Run: [42494854061](https://github.com/aevalmere/aevalbuddy/actions/runs/42494854061), attempt 1, on push
- Commit: fef30447e5904a69e22867b49eef07448d73a614
- Ref: rust/platform
- Conclusion: **failure**
- Failed: Bridge unit checks
- Finished: 2026-11-08T15:27:13Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/platform/fef30447e5904a69e22867b49eef07448d73a614 (/ in the ref becomes -)

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
