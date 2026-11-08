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
