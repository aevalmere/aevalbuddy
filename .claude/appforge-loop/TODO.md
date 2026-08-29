# TODO

- [x] D1 failover consults the live catalogue
- [x] D2 windows open on the display being captured
- [x] D3 history stops serialising itself after every answer
- [x] D4 history window resize is debounced
- [x] D5 the query field takes more than one line
- [x] D6 error copy follows the attempt count
- [x] D7 dead preload listen channels removed, and the gate enforces the class
- [x] D8 motion.stop guards null
- [x] D9 lastRegisterResults carries every action
- [x] D10 tray fires one capture per click
- [x] D11 the SSE reader is released on every exit
- [x] tools/check.js, wired as `npm run check`
- [x] DESIGN.md v1.1 records every departure with its reason
- [x] README covers the new gate and the multi-line field
- [x] D12 area selection actually returns an area (BLOCKER)
- [x] D13 a finished pointer no longer destroys the next one
- [x] every file in the repository read
- [x] build gate

## Worth doing next, not done here

- [ ] **Stream reasoning tokens.** Both defaults are reasoning models and
      OpenRouter can stream `delta.reasoning`. Today that time is a thinking dot;
      it could be something to read. A feature, not a defect, so it stayed out of
      a defect-driven run.
- [ ] **A second machine for the display work.** `displays=1` here, so D2's
      two-monitor path is reasoned and gate-guarded rather than exercised. The
      one thing in this run that a person with two monitors could falsify in
      thirty seconds.
- [ ] BLOCKED: nothing.
