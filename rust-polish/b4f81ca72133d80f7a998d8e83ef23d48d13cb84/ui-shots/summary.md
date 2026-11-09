# ui-shots: failure

- Workflow: ui-shots
- Run: [42550242093](https://github.com/aevalmere/aevalbuddy/actions/runs/42550242093), attempt 1, on push
- Commit: b4f81ca72133d80f7a998d8e83ef23d48d13cb84
- Ref: rust/polish
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-09T02:28:41Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/polish/b4f81ca72133d80f7a998d8e83ef23d48d13cb84 (/ in the ref becomes -)

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
