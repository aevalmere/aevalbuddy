# ui-shots: failure

- Workflow: ui-shots
- Run: [42465476908](https://github.com/aevalmere/aevalbuddy/actions/runs/42465476908), attempt 1, on push
- Commit: b684715f24f31cac20872ae31b60398d575d4149
- Ref: rust/harness
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-08T09:05:39Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/harness/b684715f24f31cac20872ae31b60398d575d4149 (/ in the ref becomes -)

| Step | Outcome | Log |
|---|---|---|
| Checkout | success |  |
| Node | success |  |
| Bridge unit checks | success | [bridge.log](bridge.log) |
| Install Playwright | success | [playwright.log](playwright.log) |
| Screenshots and renderer checks | failure | [shots.log](shots.log) |
| Upload screenshots | success |  |
| Publish to the shots branch | success |  |

errors.txt: 2 error and warning lines from the logs.
