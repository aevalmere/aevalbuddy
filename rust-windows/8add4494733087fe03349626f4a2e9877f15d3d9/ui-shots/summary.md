# ui-shots: failure

- Workflow: ui-shots
- Run: [42509160656](https://github.com/aevalmere/aevalbuddy/actions/runs/42509160656), attempt 1, on push
- Commit: 8add4494733087fe03349626f4a2e9877f15d3d9
- Ref: rust/windows
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-08T18:20:50Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/windows/8add4494733087fe03349626f4a2e9877f15d3d9 (/ in the ref becomes -)

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
