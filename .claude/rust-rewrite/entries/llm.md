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
- `settings:test-key`: its arm stays in `host::ask::route` and calls `host::settings::test_key`, because the `every_invoke_channel_has_a_route` scan does not read `host/settings.rs` until windows C1 adds it. The arm moves into `host::settings::route` after that.

## ARCHITECTURE · 12 (LLM) · C1 contracts

Add:
The llm lane's C1 contracts, for the lanes that consume them:

- K-15 `host::settings::route(app, label, channel, args) -> Option<Result<Value, String>>`: the Settings channels about providers, keys and models (list above).
- K-16 `llm::settings::candidates(&dyn LlmSettings) -> Vec<Candidate>`: the providers a request may go to, in the person's order (`providerOrder`, with the legacy `provider` override migrated to the top), skipping providers without a key, keys whose prefix names another provider, and providers in a cooldown (`llm::status`). `Candidate { provider, key }` with `model(Kind)`; `llm::settings::model_for(s, provider, kind)` is the model id a provider receives.
- K-17 `ApiError { retry_after_ms: Option<u64>, attempts: u32, code: Option<&'static str> }`, with the codes in `llm::errors::codes`. Defaults (`None`, 1, `None`) until B-042 and B-120 fill them.
- K-18 `llm::recipients(&dyn LlmSettings) -> Vec<Recipient>`: who receives the screen, first first, as `{id, name, model, modelName, free}`. `settings:get` carries it as `recipients`; the overlay gets it through `Hooks.overlay_extra` once windows C1 has that hook.
- K-19 the renderer payloads as serde types in `llm::payloads`: `QueryError` (`query:error`), `Sent` (`output:sent`), `Usage` (`output:usage`), `ProviderRow` (`settings_extra.providers`), `FailoverPrefs`, `ProviderStatusEvent` (`provider:status`), `ProviderModels`, `SaveKeyReply`, `TestKeyReply`, `ClipKeyReply`, `Action` and `Pin`.
- K-26 `Fake::push_fixture(Value)` puts a fixture ahead of all others; `Fake::clear_pushed()` goes back to the embedded set. Besides the lib/fake-llm.js fields a fixture may carry `provider` (matches only that provider id), `model` (only that model id), `status` (answers with that HTTP status and `{"error":{"message":…}}`), `retryAfter` (the `Retry-After` header, seconds), `stallAfterHeadersMs` (headers, then silence for that long before the body) and `times` (matches that many requests, then drops out). Each logged `FakeRequest` records `provider` and the `url` the real request would have gone to.
