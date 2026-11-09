# ui-shots: failure

- Workflow: ui-shots
- Run: [42546358615](https://github.com/aevalmere/aevalbuddy/actions/runs/42546358615), attempt 1, on push
- Commit: 5d2ace8215004b46b82814bc88cfcea6ec032fe3
- Ref: rust/polish
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-09T01:45:22Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/polish/5d2ace8215004b46b82814bc88cfcea6ec032fe3 (/ in the ref becomes -)

| Step | Outcome | Log |
|---|---|---|
| Checkout | success |  |
| Node | success |  |
| Bridge unit checks | success | [bridge.log](bridge.log) |
| Install Playwright | success | [playwright.log](playwright.log) |
| Screenshots and renderer checks | failure | [shots.log](shots.log) |
| Upload screenshots | success |  |
| Publish to the shots branch | success |  |

errors.txt: 5 error and warning lines from the logs.
