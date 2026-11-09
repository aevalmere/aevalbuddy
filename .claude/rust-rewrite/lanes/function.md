# Lane brief: function

Branch `rust/function`, cut from `rust-rewrite` after Andy installed and tested the port. His note: "make things like function intentionally and properly." Goal: every control does what it says, completely, with sensible states, and nothing claims a success it did not have.

## Files touched

Rust under `src-tauri/src/` (host/ask.rs, llm/client.rs, agent/run.rs, find.rs, llm/fake.rs, hotkeys.rs, voice/{mod,settings}.rs, windows/{mod,outputs}.rs, ipc.rs, host/agent.rs, the selftest probes that call `collapse_panel`), `preload.js` (the allowlists), and JS behaviour in `renderer/{output,bar,trail,voice}.js`, `settings/settings.js`, `history/history.js`. No CSS, menu placement or glass code (the polish lane owns those). Two ui-shots mock files changed with the behaviour they stand in for: `tools/ui-shots/mock-bridge.js` (the frame grows on `output:glass`, since `output:resize` is gone) and `tools/ui-shots/kit.mjs` (`clipboard:write` answers `{ok: true}`, as Rust now does).

## Findings

Severity order. "Fixed" names the commit; "deferred" says why.

| # | Finding | State |
|---|---|---|
| 1 | B-132: the llm client capped every non-streamed call at 90 s, so a native agent turn (allowed 180 s) was cut short with "no data for 90 s". | Fixed in ca3547d: `Meta.overall`, set by the agent to its own turn limit; UNIT in `idle_stream_times_out_as_408`. |
| 2 | B-122: the bar's `@` model menu never opened (no `models:list.providers`) and a pinned model was ignored by `query:submit`; "Retry with" a model on a native provider retried with the default model. | Fixed in ca3547d: `models:list` (no kind, no provider) lists every keyed provider; `query:submit`/`query:retry` take `{provider, model}`; a pinned model answers alone, with no reading step and no failover. |
| 3 | B-121: "What was sent" never showed (no `query:done.sent`) and its View button called a channel outside both allowlists. | Fixed in ca3547d: `sent` payload, last three pictures per window in memory only, `output:sent-image`; UNIT `sent_images_keep_last_three`. |
| 4 | Every Copy (answer, code, error details, conversation, task result, History's question) said "Copied" even when the clipboard write failed; Rust ignored the error. | Fixed in ad1d094: `clipboard:write` returns `{ok}`; pages say "Copied" only then and say the failure aloud. |
| 5 | Settings switches, radios, answer style, shortcuts and reset kept their new look when the save was refused or timed out; "Use thinking model" pretended success; Revoke claimed success after a failed save. | Fixed in ad1d094: the control goes back to the stored value and the page says "Couldn't save that change." |
| 6 | Check for updates stuck at "checking" with the button disabled if the call failed. | Fixed in ad1d094. |
| 7 | A question or task that could not be sent from the bar was cleared and lost. | Fixed in ad1d094: the words come back with "Couldn't send that." |
| 8 | Voice: an app shortcut could be set to the dictation chord (voice refused only the reverse), and turning dictation on did not check for a clash made since. | Fixed in ca3547d (`voice::dictation_uses`, `merge`). |
| 9 | History: opening a deleted conversation did nothing; typing a search under an open "Delete all N" kept the stale count. | Fixed in ad1d094. |
| 10 | The mic went silently back to idle when `voice:start` failed. | Fixed in ad1d094. |
| 11 | B-164: the sheet still sent `output:resize` beside `output:glass`. | Fixed in ca3547d: removed from both allowlists, the route and the page; `output:glass` keeps the placement height. |
| 12 | B-018: `collapse_panel`'s no-op argument. | Fixed in ca3547d. |
| 13 | B-046 cross-provider hop and B-120 busy auto-retry: a busy free model ends in an error with Retry rather than hopping or retrying by itself. | Deferred: new behaviour from design-ux, not Electron parity, with a paid-account guard and copy to agree with Andy first. Today's Retry and "Retry with" (now working, #2) cover it by hand. |
| 14 | B-044: Find's related and concept stages do not fail over to another free model. | Deferred: the stage reports its failure honestly; failover needs the streamed-terms borrow reworked and a fake-LLM probe to stay fixed. |
| 15 | B-040: no cap of two streaming answers. | Deferred: a resource limit, not a broken control; each window's Ask is already independent. |
| 16 | Trail's Continue and the task Retry can be pressed twice. | Deferred: Rust ignores the second (`agent:continue` is idempotent, `agent:start` answers `busy`). |

Checked and found working as a careful product would: Stop beside the streaming answer and Ctrl+.; Esc moves focus and never closes or stops (D-01); closing an output aborts only its own Ask (`abort_query_for`); a second question in the same window replaces the first atomically; confirm cards answer once and wait for long text; consent focuses the sheet, not Allow; tray items each route; voice errors, empty recordings and model downloads each say so; History's delete has Undo and survives the window closing.

## CSS the polish lane may want

None needed by these fixes. The new failure lines reuse existing status nodes and live regions.

## Progress

| Item | State |
|---|---|
| Lane gaps 1-5 from the brief (B-132, sent-image/B-121, B-164, voice chord, B-018) and B-122 | ca3547d (unit tests green there; fmt and clippy fixed in ad1d094) |
| Audit fixes 4-10 | ad1d094 |
| Green sha | a487563: rust fully green (Build installer and Smoke included, run 42546738757) and ui-shots green (run 42546738738). Commits after it change only this file. |
