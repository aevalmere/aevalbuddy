# ui-shots: failure

- Workflow: ui-shots
- Run: [42549174172](https://github.com/aevalmere/aevalbuddy/actions/runs/42549174172), attempt 1, on push
- Commit: 1a8114df2d6e5a44dd49764efa8cbf8be105f1de
- Ref: rust/polish
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-09T02:17:43Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/polish/1a8114df2d6e5a44dd49764efa8cbf8be105f1de (/ in the ref becomes -)

| Step | Outcome | Log |
|---|---|---|
| Checkout | success |  |
| Node | success |  |
| Bridge unit checks | success | [bridge.log](bridge.log) |
| Install Playwright | success | [playwright.log](playwright.log) |
| Screenshots and renderer checks | failure | [shots.log](shots.log) |
| Upload screenshots | success |  |
| Publish to the shots branch | success |  |

errors.txt: 6 error and warning lines from the logs.
