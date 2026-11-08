# ui-shots: failure

- Workflow: ui-shots
- Run: [42492595550](https://github.com/aevalmere/aevalbuddy/actions/runs/42492595550), attempt 1, on push
- Commit: fe4f1281d557112689b490d9658388e70e19a8bd
- Ref: rust/sheet
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-08T15:04:25Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/sheet/fe4f1281d557112689b490d9658388e70e19a8bd (/ in the ref becomes -)

| Step | Outcome | Log |
|---|---|---|
| Checkout | success |  |
| Node | success |  |
| Bridge unit checks | success | [bridge.log](bridge.log) |
| Install Playwright | success | [playwright.log](playwright.log) |
| Screenshots and renderer checks | failure | [shots.log](shots.log) |
| Upload screenshots | success |  |
| Publish to the shots branch | success |  |

errors.txt: 3 error and warning lines from the logs.
