# ui-shots: failure

- Workflow: ui-shots
- Run: [42516379427](https://github.com/aevalmere/aevalbuddy/actions/runs/42516379427), attempt 1, on push
- Commit: 691774d3b0a4b0e92eb9f03f142f4283ad83f462
- Ref: rust/platform
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-08T19:52:11Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/platform/691774d3b0a4b0e92eb9f03f142f4283ad83f462 (/ in the ref becomes -)

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
