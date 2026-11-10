# ui-shots: failure

- Workflow: ui-shots
- Run: [42778342279](https://github.com/aevalmere/aevalbuddy/actions/runs/42778342279), attempt 1, on push
- Commit: 6aec89acddb76ba34db5bd3654ebee10aad5dc34
- Ref: rust/polish
- Conclusion: **failure**
- Failed: Screenshots and renderer checks
- Finished: 2026-11-10T08:06:21Z
- Screenshots: https://github.com/aevalmere/aevalbuddy/tree/shots/rust-d-ui/rust/polish/6aec89acddb76ba34db5bd3654ebee10aad5dc34 (/ in the ref becomes -)

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
