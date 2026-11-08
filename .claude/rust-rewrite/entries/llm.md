# Entries from the llm lane

## Routed by host::settings

`host::settings::route` (K-15) runs first from `host::ask::route`, so each channel below is answered there and the matching core row in `ipc.rs` is never reached. windows deletes those rows under B-107.

- `settings:save-key`
- `settings:clear-key`
- `settings:save-model`
- `settings:save-brain`
- `settings:save-find-model`
- `settings:save-agent-model`
- `settings:clip-key`
- `settings:use-clip-key`
- `settings:test-key` (its arm moved here from `host::ask::route` once windows C1 added `host/settings.rs` to the `every_invoke_channel_has_a_route` scan)

## ARCHITECTURE · 12 (LLM) · C1 contracts

Add:
The llm lane's C1 contracts, for the lanes that consume them:

- K-15 `host::settings::route(app, label, channel, args) -> Option<Result<Value, String>>`: the Settings channels about providers, keys and models (list above).
- K-16 `llm::settings::candidates(&dyn LlmSettings) -> Vec<Candidate>`: the providers a request may go to, in the person's order (`providerOrder`, with the legacy `provider` override migrated to the top), skipping providers without a key, keys whose prefix names another provider, and providers in a cooldown (`llm::status`). `Candidate { provider, key }` with `model(Kind)`; `llm::settings::model_for(s, provider, kind)` is the model id a provider receives.
- K-17 `ApiError { retry_after_ms: Option<u64>, attempts: u32, code: Option<&'static str> }`, with the codes in `llm::errors::codes`. Defaults (`None`, 1, `None`) until B-042 and B-120 fill them.
- K-18 `llm::recipients(&dyn LlmSettings) -> Vec<Recipient>`: who receives the screen, first first, as `{id, name, model, modelName, free}`. `settings:get` carries it as `recipients`; the overlay gets it through `Hooks.overlay_extra` once windows C1 has that hook.
- K-19 the renderer payloads as serde types in `llm::payloads`: `QueryError` (`query:error`), `Sent` (`output:sent`), `Usage` (`output:usage`), `ProviderRow` (`settings_extra.providers`), `FailoverPrefs`, `ProviderStatusEvent` (`provider:status`), `ProviderModels`, `SaveKeyReply`, `TestKeyReply`, `ClipKeyReply`, `Action` and `Pin`.
- K-26 `Fake::push_fixture(Value)` puts a fixture ahead of all others; `Fake::clear_pushed()` goes back to the embedded set. Besides the lib/fake-llm.js fields a fixture may carry `provider` (matches only that provider id), `model` (only that model id), `status` (answers with that HTTP status and `{"error":{"message":…}}`), `retryAfter` (the `Retry-After` header, seconds), `stallAfterHeadersMs` (headers, then silence for that long before the body) and `times` (matches that many requests, then drops out). Each logged `FakeRequest` records `provider` and the `url` the real request would have gone to.

## ARCHITECTURE · 10.2 Extended payloads · B-119
Add:
- `query:error {gen, message, question, code, provider, model, modelName, tone, actions, detail}` (`llm::payloads::QueryError`). `code` is one of `llm::errors::codes`; `tone` is `error` or `note`; `actions` are the buttons in order, `{id, label, provider?, model?, url?, section?}`, with each id's meaning on `payloads::Action`; `detail` is the Copy details text (provider, model id, status, models tried, the provider's message). `retryInMs` joins with B-120.
- `query:done.usage` gains `modelId`, `modelName`, `provider` and `providerName`; `model` stays the short id for older pages.
- `query:stage` names a stand-in by its display name: `That model is busy. Trying <model name>…`, `… Reading with <model name>…`, `… Thinking with <model name>…` (C-21..C-23).

## ARCHITECTURE · 12 (LLM) · B-041
Add:
Timeouts: the HTTP client connects within 10 s and sends TCP keep-alives every 30 s. A stream that sends no bytes for 45 s (keep-alive comments count) ends with `ApiError::http(408)`, which `with_failover` treats like a busy model; a non-streamed call (the reader, agent turns) is capped at 90 s from request to last byte. `AEVAL_LLM_IDLE_MS` shortens the stall timeout for the self-test.

## ARCHITECTURE · 12 (LLM) · B-042
Add:
`Llm` remembers the model ids that answered 404 this session (`failover::BadModels`): `first_model` skips a retired free id as the first attempt and `next_free_model` skips it as a stand-in. Once the catalogue is cached, a stand-in must be listed and able to do the job (images for Vision, tool calls for Agent); a cold cache means curated order. An `ApiError` carries `attempts`, the number of models tried, and its text mentions a backup only when that is above 1.

## ARCHITECTURE · 12 (LLM) · B-125
Add:
Wire limits: a 200 reply whose body is an `error` object is that error; an SSE line over 1 MB ends the stream with a 502; the history sent with a question starts at a user turn (`ask::sendable_history`); a skill body is cut at 8 KB and `skills.list()` marks it `truncated`. A model that declines (Anthropic `stop_reason: refusal`, a Gemini safety block) is an error with code `refused`, which never fails over. Provider messages pass through `errors::redact_keys`, so a key a provider quotes back never reaches a page or a log.

## ARCHITECTURE · 12 (LLM) · D-18
Add:
Gemini requests carry no `temperature`, like OpenAI and Anthropic requests.
Reason: Google's Gemini 3 guide recommends keeping temperature at its default because lower values can cause looping and worse reasoning, and the 0.2 the answer stream sends was chosen for OpenRouter's free models.

## PARITY · PROMPT-04 · D-18
Replace "Answer stream temperature 0.2; Find text calls temperature 0, max_tokens 800 default; agent turns temperature 0, tool_choice auto, parallel_tool_calls false." with:
Answer stream temperature 0.2; Find text calls temperature 0, max_tokens 800 default; agent turns temperature 0, tool_choice auto, parallel_tool_calls false. Gemini requests omit temperature (D-18).
status: changed; evidence: UNIT wire::tests::gemini_body_has_no_temperature

## PARITY · PROMPT-05 · B-125
Replace "SSE parsing: `data:` lines only (keepalive comments skipped), `[DONE]`, error objects -> ApiError(code), usage accumulated, TTFT measured on first content; empty stream = 204 `empty response` unless allowed." with:
SSE parsing: `data:` lines only (keepalive comments skipped), `[DONE]`, error objects -> ApiError(code), usage accumulated, TTFT measured on first content; empty stream = 204 `empty response` unless allowed. A line over 1 MB is a 502; 45 s without a byte is a 408 (B-041); a 200 body holding an `error` object is that error.
status: changed; evidence: UNIT sse::tests::buffer_is_capped, client::tests::idle_stream_times_out_as_408, client::tests::error_in_200_body_is_an_error

## PARITY · FAIL-01 · B-043
Replace "Failover statuses: 404, 408, 429, 500, 502, 503, 504; never on abort." with:
Failover statuses: 404, 408, 429, 500, 502, 503, 504, 529 (Anthropic's overloaded); never on abort, a refusal, or an account-wide limit (the daily free cap, an empty balance), which every model of the account shares.
status: changed; evidence: UNIT errors::tests::failover_statuses_are_the_everyday_ones, failover::tests::daily_cap_stays_in_account, errors::tests::daily_cap_is_not_transient

## PARITY · FAIL-05 · B-042
Replace "nextFreeModel: first untried free in the kind's pool (find uses brains); agent only tools-capable stand-ins when the catalogue is known." with:
nextFreeModel: first untried free in the kind's pool (find uses brains), skipping ids that answered 404 this session; when the catalogue is known, only listed stand-ins, and for Vision only ones that take images, for Agent only ones with tool calls.
status: changed; evidence: UNIT catalogue::tests::stand_in_is_live_and_sees_images, failover::tests::bad_model_set_is_skipped, client::tests::retry_after_and_retired_models_are_read

## PARITY · FAIL-06 · B-119
Replace "Short model name in stage labels: last path part, `:free` stripped, 28 chars max with ellipsis." with:
Stage labels name the stand-in by its catalogue display name (C-21..C-23). The short name (last path part, `:free` stripped, 28 chars max with ellipsis) stays in `query:done.usage.model` for older pages.
status: changed; evidence: UNIT failover::tests::short_names

## PARITY · ERR-01..ERR-12 · B-119
status: replaced by the copy deck (design-ux U14, C-61, C-70..C-74; Decision D-28): each error carries a code, the provider, the model's display name and the actions that fix it, in `tools/fixtures/llm/errors.json`. evidence: UNIT errors::tests::error_table_is_verbatim, errors::tests::codes_and_actions, errors::tests::gemini_invalid_key_is_bad_key, errors::tests::single_attempt_copy_claims_no_stand_in, host::ask::tests::error_rows_name_the_model_and_the_fix

## PARITY · POINT-03 · B-127
Replace "matchElement: exact name wins; containment and word-overlap scoring; min 0.5; margin 0.08 against runner-up." with:
matchElement: exact name wins; containment and word-overlap scoring; min 0.5; margin 0.08 against runner-up. Names compare by Unicode letters and digits, so Cyrillic and CJK labels match.
status: changed; evidence: UNIT elements::tests::cyrillic_and_cjk_names_match

## PARITY · CAP-09 · D-08
Replace "Every message captures fresh (follow-ups see the screen now). Region sticks to the conversation until cleared or New chat." with:
Every message captures fresh (follow-ups see the screen now); the shot taken when an area was picked stands in only while it is under 1 s old. Region sticks to the conversation until cleared or New chat.
Reason: in Electron the pick-time shot answered the first question however long the person took to type it, so a screen that changed meanwhile was described as it had been.
status: changed; evidence: UNIT host::ask::tests::stale_pick_shot_is_not_reused
