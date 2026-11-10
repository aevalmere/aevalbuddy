# ui-shots: failure

- Workflow: ui-shots
- Run: [42786394311](https://github.com/aevalmere/aevalbuddy/actions/runs/42786394311), attempt 1, on push
- Commit: fbce49a73b7788aa0314a9bfcedb032c00fe1dfd
- Ref: rust-rewrite
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-10T09:09:24Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust-rewrite/fbce49a73b7788aa0314a9bfcedb032c00fe1dfd (/ in the ref becomes -)

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
