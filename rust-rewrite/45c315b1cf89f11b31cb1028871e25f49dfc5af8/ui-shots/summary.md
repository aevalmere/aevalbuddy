# ui-shots: failure

- Workflow: ui-shots
- Run: [42753961333](https://github.com/aevalmere/aevalbuddy/actions/runs/42753961333), attempt 1, on push
- Commit: 45c315b1cf89f11b31cb1028871e25f49dfc5af8
- Ref: rust-rewrite
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-10T04:20:48Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust-rewrite/45c315b1cf89f11b31cb1028871e25f49dfc5af8 (/ in the ref becomes -)

| Step | Outcome | Log |
|---|---|---|
| Checkout | success |  |
| Node | success |  |
| Bridge unit checks | success | [bridge.log](bridge.log) |
| Install Playwright | success | [playwright.log](playwright.log) |
| Screenshots and renderer checks | failure | [shots.log](shots.log) |
| Upload screenshots | success |  |
| Publish to the shots branch | success |  |

errors.txt: 1 error and warning lines from the logs.
