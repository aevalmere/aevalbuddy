# ui-shots: failure

- Workflow: ui-shots
- Run: [42749102010](https://github.com/aevalmere/aevalbuddy/actions/runs/42749102010), attempt 1, on push
- Commit: 7c399857e6bae3f37f8f19f5dc37a7171592f897
- Ref: rust-rewrite
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-10T03:35:50Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust-rewrite/7c399857e6bae3f37f8f19f5dc37a7171592f897 (/ in the ref becomes -)

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
