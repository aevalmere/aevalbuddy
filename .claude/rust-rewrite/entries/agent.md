# Entries: agent lane

## PARITY · AGENT-06 · B-006
status: DIFF; evidence: UNIT agent::run::tests::confirm_payload_carries_full_value, UNIT agent::describe::tests::the_card_value_is_the_whole_text
Note: `agent:confirm-request` adds `value` (everything a `type` or `form_input` step will enter, dotted in a password field) and `truncated` (true when the 40-character label shows less than all of it), contract K-20. Electron sent only the label. The step label and the value are dotted in a password field.

## PARITY · AGENT-15 · B-051
status: DIFF; evidence: UNIT host::agent::tests::consent_payload_names_provider
Note: `agent:consent-request` adds `provider`, `providerName`, `model`, `modelName`, `free` and `recipients: [{id, name, model, modelName, free}]` for the provider and model the run will use (K-21), so the consent card names the real recipient.

## ARCHITECTURE · Agent · B-144
Add:
Every act row in `agent:step` (`kind` `action` or `read`) carries `planIndex`: the index of the plan item it serves, from the model's call when given, else the number of finished actions clamped to the plan length; `null` before a plan is shown (K-20).

## PARITY · AGENT-05 · D-27
status: DIFF; evidence: UNIT agent::gate::tests::space_on_focused_send_button_asks, UNIT agent::gate::tests::typed_space_on_button_asks, UNIT agent::gate::tests::space_in_text_field_is_free, UNIT agent::gate::tests::unknown_named_button_asks, UNIT agent::gate::tests::explorer_double_click_asks, UNIT agent::gate::tests::web_dialog_text_is_read, UNIT agent::gate::tests::localized_send_asks, UNIT agent::run::tests::space_on_focused_send_asks_before_any_key; SMOKE gateBypasses, gatePassesPlainInput, gateFailsClosed, gatePlanFirst, gateChords, gateKeyActivation (when the agent probe lands)
Note: decision D-27, the gate is an allowlist (B-008, B-049). Changed verdicts against Electron: a left, double or triple click on a readable, named control whose role is not menubar, menu, menuitem, tab, tabitem, treeitem, scrollbar, thumb, edit, document, combobox, spinner, dataitem, headeritem or titlebar, and whose name is not one of back, bold, collapse, expand, find, forward, italic, maximize, menu, minimize, more actions, more options, options, restore, restore down, search, see more, settings, show more, underline, zoom in, zoom out, now asks (`it is not known what button "Continue" does`); a right click on a readable, named control still passes. Any double click in File Explorer or on the desktop asks. Space left the free keys: it is free only when the focused control is a text field (edit, document, combobox, spinner) or would pass as a click target, asks when a Tab, Escape, Enter or shortcut before it in the same call can move the focus, and asks when the focus cannot be told. Typed text with a space gets the same check on the focused control. Enter also names the focused control it presses. A dialog's text is read for any `dialog` node or window nested in the tree around the target (web and app dialogs), not only class #32770, when the step asks anyway or the tree is already read. New verbs: shut down, restart, trash, clear all, clear history, unsubscribe, cancel subscription, deactivate, close account, end task, plus send, delete, remove, buy, pay and post in German, French, Spanish, Italian, Portuguese, Japanese and Chinese; word boundaries are Unicode. The smoke-v21 tables keep every MUST_ASK row asking and every MUST_PASS row passing; the agent probe ports them with these verdicts.
Reason: a name chosen by the page could clear a click on anything Electron's English verb list did not know, and Space pressed a focused Send button with no card.

## PARITY · DESK-09 · B-008
Replace "elementAt, windowStates, resolve," with:
elementAt, focused (the element with keyboard focus when the asked window is in front, else node null; B-008), windowStates, resolve,

## PARITY · DESK-09 · B-049
Add:
The helper reports a Pane, Group or Custom element whose localized control type is "dialog" or "alert dialog" with role `dialog`, and keeps an unnamed one in the tree, so the gate can read a web or XAML dialog's own text.

## ARCHITECTURE · Agent · D-27
Add:
The safety gate (`agent/gate.rs`) is an allowlist (decision D-27): a step runs without a card only when no check finds a reason to ask, and a click passes only on a readable, named control whose role or name is on the harmless list. Space, Enter and typed spaces are judged by the control with keyboard focus, which `inspect` reads through the helper's `focused` op (falling back to the tree) and passes to the gate as `Touches.focused`, with the tree it read as `Touches.tree` so the gate reads it once.

## PARITY · DESK-03 · B-050
Add:
`key` waits 2 s plus 40 ms a press (repeat, capped at 100, times the chords), never under 4 s, so a long repeat is not abandoned while the helper is still pressing keys.

## PARITY · DESK-02 · B-050
Add:
host.cs reads stdin on its own thread. Every request carries a cancel flag: the helper sets it when the request times out, and a `cancel` line (answered at once, `{ok, cancelled}`) sets it on every request running or queued. Every input primitive (SendInput, SetCursorPos, SetFocus, Invoke, Toggle, Select, SetValue, scroll patterns, focusWindow, closeWindow) checks it before acting and rejects with CANCELLED; key and button releases still go out, so nothing stays held. `type` and `key` check it per chunk and per chord. A drag stopped half way lets go where the pointer is. `Desktop::cancel()` writes the line past the op queue.

## PARITY · AGENT-10 · B-048
status: DIFF; evidence: UNIT agent::run::tests::pause_during_dodge_drops_the_action; SMOKE agentTakeover, SMOKE+ takeoverStopsStep (when the agent probe lands)
Note: a takeover that lands after the gate passed a step (while the pointer travels or the window list is read) drops that step (row `skipped`, why `you took over`) and the rest of the turn; each dropped call answers `Not executed: the user took over.` after the person hands back. A takeover between two calls of one turn drops the calls left. Electron acted on the screen the person had changed.

## PARITY · AGENT-12 · B-050
Add:
A step that ends in TIMEOUT is reported to the model as an unknown outcome, with the window read again, and the rest of the turn does not run. The run checks that it has not ended right before every op that acts (after the window list is read, after a stale ref is re-read, between finding a window by title and focusing it), and when a run ends while a step is in the helper (Stop, Esc, the tray, the wall clock) it sends `cancel`.

## PARITY · AGENT-15 · B-047
status: DIFF; evidence: UNIT host::agent::tests::native_only_run_uses_native_model, UNIT host::agent::tests::pinned_model_never_fails_over, SMOKE+ agentNativeModel
Note: a run picks its provider once, from the first candidate (K-16), and keeps it for its whole life: a native provider uses `model_for`, OpenRouter resolves against the live catalogue, and only an OpenRouter id is ever healed. `agent:start` and `agent:do-it` take optional `{provider, model}`; a provider that does not exist or has no usable key answers `needs-key` rather than running elsewhere. The trail opens in `outputs::fresh` (K-04), or in the window that just showed the first-run consent card.

## PARITY · AGENT-09 · B-047
Add:
A pinned model never fails over.

## PARITY · AGENT-09 · B-131
Add:
When a stand-in changes the tool dialect, `messages[0]` is rewritten for the new dialect, so a model with native tools is not told it has none.

## PARITY · AGENT-15 · B-051
status: DIFF; evidence: UNIT host::agent::tests::consent_payload_names_provider, UNIT host::agent::tests::consent_recipients_come_from_the_ready_list
Note: `recipients` is the ready list (K-18) with the run's own provider first, on the model the run will use; a pinned provider is listed alone.

## PARITY · AGENT-15 · B-130
Add:
A start holds a claim from the check until the run exists, taken under the run lock, so a second start during the catalogue fetch answers `busy`.

## PARITY · AGENT-01 · B-132
Replace "60 s model timeout (-> 408)" with:
60 s model timeout on OpenRouter and 180 s on a native provider (-> 408)
Replace "retry backoff 2 s/6 s after acting" with:
after acting, a retry waits what Retry-After asked, up to 20 s, else 2 s/6 s, and a 529 is retried too

## PARITY · AGENT-02 · B-133
Replace "26 tools with exact names, descriptions, JSON schemas" with:
26 tools with exact names, descriptions (zoom adds that coordinates stay in the screenshot's pixels), JSON schemas

## PARITY · AGENT-17 · B-133
Add:
A coordinate is checked before anything asks or moves: it needs numeric components, a finite positive screenshot scale, and a point inside the latest screenshot and the area the person chose. Otherwise the step fails with the reason and the model reads the window again.

## PARITY · AGENT-04 · B-135
Add:
The scan for the last object is bounded by the reply's length (bytes, plus 64 KB), so a reply of nothing but braces cannot stall the run.

## PARITY · AGENT-07 · B-136
Add:
`get_page_text` goes through the same `redact` as the tree.

## PARITY · DESK-04 · B-137
status: DIFF; evidence: UNIT desktop::client::tests::start_timeout_counts_as_death, UNIT desktop::client::tests::non_utf8_line_does_not_stop_reader
Note: a reply timeout marks the helper not ready and drops it at once, rejecting anything else in flight with HELPER_DOWN; a start timeout counts toward the three deaths; stdout is read by bytes with invalid UTF-8 replaced.

## PARITY · DESK-05 · B-138
Add:
The helper runs in a job with KILL_ON_JOB_CLOSE, so it dies with the app however the app ends. taskkill and cmd run from %SystemRoot%\System32 by absolute path.

## PARITY · DESK-01 · B-139
Add:
$AevalCache and $AevalPrebuilt cross stdin base64-encoded like the sources, so an install path with accents, CJK or smart quotes keeps the prebuilt helper.

## PARITY · DESK-06 · B-140
Add:
Registered windows give their handle from the `windows::flags` cache; only an unregistered window hops to the main thread for it.

## PARITY · DESK-09 · B-141
Add:
A click or drag sends its button events as plain MOUSEINPUT with only the button flag, after MoveTo has corrected the position, so absolute-coordinate rounding cannot move the press.

## ARCHITECTURE · Desktop helper · B-142
Add:
A window's first tree read registers its structure-changed handler outside `genLock`, with a placeholder generation published first, so one slow app cannot stall every other window's read.

## PARITY · AGENT-13 · D-30
Replace "`The agent stopped without saying whether the task was done. Check the app before relying on it.`" with:
`The task stopped without saying whether it was done. Check the app before relying on it.`
Add:
Also `No connection. The task stopped after step N; nothing else was done.`, the internal failure `The task stopped because of a problem inside AevalBuddy. Nothing else was done.` (C-60) and `A task is already running.` (C-62). After three failed steps whose last failure has a remedy, the final message is only the remedy (`Start Outlook normally, then Retry.`), because the failed row already says why.
Reason: the surfaces call it a task, and the banner repeated the reason the row had just given.

## DESIGN · Tauri rewrite: the requests that changed the surfaces · D-30
Add:
Task copy says "The task stopped", never "The agent stopped", and gives a remedy once: when a failed step's row already states the reason, the final message carries only what to do about it.
