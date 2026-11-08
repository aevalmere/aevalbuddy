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
