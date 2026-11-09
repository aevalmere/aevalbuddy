# ui-shots: failure

- Workflow: ui-shots
- Run: [42552196207](https://github.com/aevalmere/aevalbuddy/actions/runs/42552196207), attempt 1, on push
- Commit: 54dab43d476d7f719b99622cbef0d4d4a0a0d89a
- Ref: rust/polish
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-09T02:51:54Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/polish/54dab43d476d7f719b99622cbef0d4d4a0a0d89a (/ in the ref becomes -)

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
