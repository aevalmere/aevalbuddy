# ui-shots: failure

- Workflow: ui-shots
- Run: [42547836963](https://github.com/aevalmere/aevalbuddy/actions/runs/42547836963), attempt 1, on push
- Commit: 64a5661ddecdf7b727844852af07dcf53bc0e89e
- Ref: rust/polish
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-09T02:03:04Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/polish/64a5661ddecdf7b727844852af07dcf53bc0e89e (/ in the ref becomes -)

| Step | Outcome | Log |
|---|---|---|
| Checkout | success |  |
| Node | success |  |
| Bridge unit checks | success | [bridge.log](bridge.log) |
| Install Playwright | success | [playwright.log](playwright.log) |
| Screenshots and renderer checks | failure | [shots.log](shots.log) |
| Upload screenshots | success |  |
| Publish to the shots branch | success |  |

errors.txt: 4 error and warning lines from the logs.
