# ui-shots: failure

- Workflow: ui-shots
- Run: [42508910504](https://github.com/aevalmere/aevalbuddy/actions/runs/42508910504), attempt 1, on push
- Commit: 941a42df36dd9418123d556ace9075ef0ebe28fb
- Ref: rust/platform
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-08T18:17:35Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/platform/941a42df36dd9418123d556ace9075ef0ebe28fb (/ in the ref becomes -)

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
