# ui-shots: failure

- Workflow: ui-shots
- Run: [42465866431](https://github.com/aevalmere/aevalbuddy/actions/runs/42465866431), attempt 1, on push
- Commit: d7e5e5f9ad28dbe0f182e0b17bce8c0a6e2215e8
- Ref: rust/platform
- Conclusion: **failure**
- Failed: Bridge unit checks
- Finished: 2026-11-08T09:09:22Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/platform/d7e5e5f9ad28dbe0f182e0b17bce8c0a6e2215e8 (/ in the ref becomes -)

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
