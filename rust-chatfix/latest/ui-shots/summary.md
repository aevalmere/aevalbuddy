# ui-shots: failure

- Workflow: ui-shots
- Run: [42568106741](https://github.com/aevalmere/aevalbuddy/actions/runs/42568106741), attempt 1, on push
- Commit: a79e3dbfde4017d300f688df81302772053dd2df
- Ref: rust/chatfix
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-09T05:47:00Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/chatfix/a79e3dbfde4017d300f688df81302772053dd2df (/ in the ref becomes -)

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
