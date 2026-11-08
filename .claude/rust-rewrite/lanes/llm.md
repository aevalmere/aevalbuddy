# Lane brief: llm

Branch `rust/llm`, cut from `rust-rewrite`. 23 items. Cross-lane tables (ownership, hotspots, contracts, decisions, merge order) are in `.claude/rust-rewrite/BACKLOG.md`; everything you need to do your items is in this file.

## Goal

Answers keep coming on free models, the default, where 429 and 404 are everyday: every provider's key can be added, tested, ordered and shown; OpenRouter connects in one click; failover crosses providers only before any text and never to paid by surprise; each window asks on its own; stalls time out; errors name the fix; busy models retry on their own; the person can see exactly what was sent and pin a model for one question.

## Files you own

`src-tauri/src/llm/**` (including new `llm/auth.rs`), `src-tauri/src/ask.rs`, `src-tauri/src/find.rs`, `src-tauri/src/skills.rs`, `src-tauri/src/host/{mod,ask,find,settings}.rs`, `skills/**`, `tools/fixtures/llm/**`, your block in the IPC allowlists (after windows C1), and (after harness C1) `src-tauri/src/selftest/probes/llm.rs`, `tools/smoke-labels/llm.txt`, `tools/ui-shots/lanes/llm.mjs` (mocks only: the K-19 payloads the renderer lanes consume).

## Contracts you provide (C1 first)

- **K-15** `host::settings::route` in the existing `host/settings.rs`, called first from `host::ask::route`; an "Routed by host::settings" list in `entries/llm.md`, kept current.
- **K-16** `llm::settings::candidates`, `model_for`; **K-17** `ApiError { retry_after_ms, attempts, code }` (defaults until B-042 and B-120 fill them).
- **K-18** `llm::recipients`, registered as `Hooks.overlay_extra` once windows C1 has it, and in `settings_extra`.
- **K-19** the renderer payload shapes as Rust types with doc comments, plus mocks in `tools/ui-shots/lanes/llm.mjs` once harness C1 creates it.
- **K-26** `Fake::push_fixture(Value)` and fixture fields `status`, `retryAfter`, `stallAfterHeadersMs`, `provider`, so windows, agent and platform probes can script the fake LLM without touching your files.

## Contracts you consume

- **K-03** queries and `abort_query_for`, **K-04** `outputs::fresh`, **K-07** `overlay_extra` (windows).
- **K-08** `Store::set_raw`, **K-09** `Store::key_storage` (platform).
- **K-12** `region::set_active_on` (capture).
- **K-30**, **K-31** (harness).

## Smoke labels you port

`model`, `hasKey`, `brainDefault`, `brainEnabledDefault`, `skillDefault`, `modelList*`, `modelDefaultOffered`, `brainListDefault`, `heal*`, `brainPrompt`, `skills`, `skillInPrompt`, `promptShape`, `gridShared`, `skillFallback`, `skillRoute*`, `regionPrompt`, `point*`, `reader*`, `failover*`, `queryWithoutKey`, `historyRetryGuard`, `askHasUiaText`, `helpMe`, `retryNoDup`, `textOnly`, `find*`. Globs expand against `.claude/rust-rewrite/audit/parity-ipc.md` §3.

## Shared-document entries you write

D-08, D-10, D-13, D-14, D-15, D-18, D-19, D-28, D-38, D-39; PARITY evidence for MODEL, FAIL, ERR, ASK, FIND, POINT, SKILL, STORE-41 rows your items close.

## Lane notes

- B-039: confirm OpenRouter's current PKCE endpoint and parameters from openrouter.ai documentation before building, and say what you checked in Progress.
- The copy for errors, notes and stage labels is in `.claude/rust-rewrite/audit/design-ux.md` §4 (copy deck) and U14; use it verbatim and regenerate `tools/fixtures/llm/errors.json` from it.

## Standing item: port your smoke labels

Every PARITY label under "Smoke labels you port" and every new label below is printed by `src-tauri/src/selftest/probes/llm.rs` through the K-30 API, with the Electron semantics from `.claude/rust-rewrite/audit/parity-ipc.md` §3 and `main.js` on `main`, and listed in `tools/smoke-labels/llm.txt` in the same commit that makes it print `=ok`. Acceptance: the rust workflow's smoke log shows each label `=ok` and the labels gate passes.

New labels your items add, beyond the PARITY list above (you print these): `busyAutoRetry`, `concurrentCap`, `crossProviderHop`, `failoverStandInLive`, `findFailover`, `modelPin`, `nativeModelIds`, `perWindowAsk`, `providerOrder`, `recipients`, `sentPanel`, `timeoutFailsOver`.

## Items

| ID | Severity | Title |
|---|---|---|
| B-036 | P1 | Find and Ask send native model ids to native providers; native picks are saved and never healed against OpenRouter |
| B-037 | S1 | Providers are tried in the person's order; a key is only sent to the provider it belongs to |
| B-038 | S1 | Settings channels for every provider: save, test, clear, order, failover, status, hints |
| B-039 | S1 | Connect OpenRouter in one click, and a key test that catches the free-model privacy block |
| B-040 | S1 | Each window asks on its own; at most two answers stream at once |
| B-041 | P1 | A stalled connection fails over instead of spinning forever |
| B-042 | P1 | Failover never hands the retry a retired or blind model, and its copy only claims what happened |
| B-043 | P1 | The daily free cap and empty quotas say what fixes them, and never burn more requests in the same account |
| B-044 | P1 | Find's related and concept stages fail over like Ask |
| B-045 | S1 | Every disclosure names who actually receives the screen |
| B-046 | S1 | Failover across providers, never to a paid account by surprise |
| B-119 | S2 | Errors carry a code, the provider, the model's name and the actions that fix them |
| B-120 | S2 | Busy free models retry on their own, honouring Retry-After; 529 counts as busy |
| B-121 | S2 | Every answer can say exactly what was sent |
| B-122 | S2 | One question can be pinned to a model, and a retry can pick another |
| B-123 | P2 | Native model lists come from the providers, with the curated table as fallback |
| B-124 | P2 | The first question after picking an area captures fresh |
| B-125 | P2 | Wire robustness: errors in 200 bodies, bounded SSE, valid turn order, Gemini sampling, skill size |
| B-126 | P2 | Corporate networks work: system certificates and the system proxy |
| B-127 | P2 | Pointing by name works for non-Latin labels |
| B-128 | P2 | Find stays fast on one-letter queries and keeps offsets right on any text |
| B-129 | P2 | Pin the Ask gating rules in a test, and keep the tour's try card correct |
| B-181 | S3 | A door to custom answer styles |

Work in severity order (P0, then S1 and P1, then S2 and P2, then S3), after your C1 if you have one.

### B-036 · P1 · llm · Find and Ask send native model ids to native providers; native picks are saved and never healed against OpenRouter
- **Sources:** review-llm-agent P1-1 (Find and picker half); parity-features #1, #2 (save-model half).
- **Files:** `src-tauri/src/host/find.rs`, `src-tauri/src/host/settings.rs`, `src-tauri/src/llm/settings.rs`, `src-tauri/src/host/ask.rs`.
- **Fix:** `FindHost::model` branches on the active candidate: native → `llm::settings::model_for(settings, provider, Kind::Find)`; OpenRouter → `resolve_live`. The four model-save channels (`settings:save-model`, `save-brain`, `save-find-model`, `save-agent-model`) are routed in `host::settings::route` (K-15) and take an optional `provider`; for a native provider they write `nativeModels.<provider>.<kind>` (via `Store::set_raw`, K-08) and never run `heal`. `heal` and `heal_agent_model` skip ids that are not OpenRouter ids. Ask uses the native pick when one is stored.
- **Acceptance:** UNIT `host::find::tests::native_only_find_body_uses_native_default`; UNIT `host::settings::tests::native_pick_is_saved_per_provider_and_not_healed`; SMOKE+ `nativeModelIds` (fake LLM records the request URL and model for a native-only profile).

### B-037 · S1 · llm · Providers are tried in the person's order; a key is only sent to the provider it belongs to
- **Sources:** design-ux U2, §3.3, §3.5; review-llm-agent P2-12; parity-features #2; parity-core STORE-41 (DESIGN paragraph).
- **Files:** `src-tauri/src/llm/settings.rs`, `src-tauri/src/llm/provider.rs`, `src-tauri/src/llm/mod.rs`.
- **Fix:** `active()` becomes `candidates(&settings) -> Vec<Candidate{provider, model_for(kind)}>` (K-16) in `providerOrder` order (stored via `Store::set_raw("providerOrder", [...])`), skipping providers with no key, a key whose `Provider::detect` names a different provider, and providers in a cooldown (B-046). Default order: OpenRouter, Anthropic, OpenAI, Gemini. Migration on read: when `providerOrder` is absent and the legacy `provider` override is set, that provider goes first. `active()` stays as `candidates().first()` for callers not yet migrated.
- **Reconciled:** providerOrder replaces the hidden `provider` override (design-ux §3.3). Decision D-13.
- **Acceptance:** UNIT `llm::settings::tests::order_and_skips`; UNIT `llm::settings::tests::legacy_provider_migrates_to_top`; UNIT `llm::settings::tests::mismatched_key_is_skipped`; SMOKE+ `providerOrder`.

### B-038 · S1 · llm · Settings channels for every provider: save, test, clear, order, failover, status, hints
- **Sources:** design-ux §3.2, §3.5, U21 (Rust half), U2; parity-features #2; parity-ipc B14 (IPC-78).
- **Files:** `src-tauri/src/host/settings.rs`, `src-tauri/src/host/mod.rs` (`settings_extra`, `key_saved`), `src-tauri/src/host/ask.rs` (`settings:test-key`), `src-tauri/src/llm/client.rs` (`test_key`), `src-tauri/src/ipc.rs` and `preload.js` (llm block only).
- **Fix:** In `host::settings::route` (K-15): `settings:save-key {provider, key}` tests before storing and returns `{ok, status, message, keyHint}` or `{ok:false, error, detected}` (`detected` = provider the prefix names); `settings:clear-key {provider}`; `settings:test-key {provider}` returns `{ok, status, message, credit}`; `settings:set-provider-order {order}`; `settings:set-failover {acrossProviders, askBeforePaid}` (default `{true, true}`); `settings:clip-key` returns `{found, masked, provider}` for all four prefixes; `settings:use-clip-key` keeps testing before it advances the tour (Decision D-10) and its error reads `The clipboard no longer holds a key.` (C-33). Status values: `not-added`, `checking`, `ready`, `rejected`, `no-credit`, `busy`, `blocked` (free models off), `unchecked`. Listen `provider:status {id, status, statusText, busyUntil}` to Settings and the overlay. `settings_extra` adds `providers: [{id, name, order, hasKey, keyHint, status, statusText, checkedAt, busyUntil, credit}]`, `failover`, `keyHints`, `keyStorage` (from `Store::key_storage()`, K-09). List every channel this lane now routes in `entries/llm.md` so windows can delete the superseded core rows (B-107).
- **Acceptance:** UNIT `host::settings::tests::save_key_tests_before_storing`; UNIT `host::settings::tests::wrong_prefix_reports_detected`; UNIT `host::settings::tests::settings_extra_shape`; ui-shots mock in `tools/ui-shots/lanes/llm.mjs` exporting the payload shapes for settings (consumed by settings scenes).

### B-039 · S1 · llm · Connect OpenRouter in one click, and a key test that catches the free-model privacy block
- **Sources:** design-ux U1; copy deck C-57, C-65.
- **Files:** `src-tauri/src/llm/auth.rs` (new, declared in `llm/mod.rs`, so no crate-root edit), `src-tauri/src/llm/client.rs`, `src-tauri/src/host/settings.rs`, `src-tauri/src/ipc.rs` and `preload.js` (llm block only).
- **Fix:** `auth:openrouter-connect`: bind `127.0.0.1:<free port>`, build a PKCE S256 verifier and challenge, open `https://openrouter.ai/auth?callback_url=http://localhost:<port>/callback&code_challenge=<S256>&code_challenge_method=S256` in the default browser, read `code` from the callback, POST `https://openrouter.ai/api/v1/auth/keys` `{code, code_verifier, code_challenge_method:"S256"}`, store `key` with `set_provider_key("openrouter", ..)`, answer the tab with a plain page `AevalBuddy is connected. You can close this tab.`, emit `key-saved`. Timeout 5 min → `Didn't hear back from OpenRouter. Try again, or paste a key instead.` Verify the current OpenRouter PKCE parameters against openrouter.ai docs before building and note the check in Progress. `test_key(OpenRouter)`: after the credit check send one `max_tokens: 1` request to the default `:free` vision model; a data-policy refusal returns status `blocked` with C-65 text and action `Open OpenRouter privacy settings`; success text `Works. Checked just now.` (C-57).
- **Acceptance:** UNIT `llm::auth::tests::pkce_challenge_is_s256_of_verifier`; UNIT `llm::auth::tests::callback_parses_code_and_rejects_bad_requests` (local loopback round trip with a fake token endpoint); UNIT `llm::client::tests::data_policy_probe_reports_blocked` (fixture in `tools/fixtures/llm/errors.json`).

### B-040 · S1 · llm · Each window asks on its own; at most two answers stream at once
- **Sources:** design-ux U4; parity-features #4 (host half); review-core P2-7; parity-features "Low" (submit_to early return leaves busy stuck).
- **Files:** `src-tauri/src/host/ask.rs`, `src-tauri/src/host/mod.rs` (hook registration).
- **Fix:** `current()` becomes `HashMap<String /*label*/, Current>`; `abort(app, label)` cancels only that window's token, installed atomically (`map.insert(label, new)` returns the old one, which is cancelled). Register `Hooks.abort_query_for` (K-03) and use `AppState::query_started/ended` so `busy` means "any running". `query:cancel {label?}` cancels the caller's own window (or the focused output when no label). A global cap of 2 concurrent answers: a third waits with `query:stage` `Waiting for your other answer to finish` and starts when a slot frees. `submit_to` returning early (no output window) clears its own busy entry. The stamped `gen` stays per window.
- **Reconciled:** U4 allows two concurrent and parity-features asks for per-label state; review-core P2-7's atomic replace is the per-label insert. Decision D-15.
- **Acceptance:** UNIT `host::ask::tests::second_window_does_not_abort_first`; UNIT `host::ask::tests::third_query_waits_for_a_slot`; SMOKE+ `perWindowAsk`, SMOKE+ `concurrentCap`.

### B-041 · P1 · llm · A stalled connection fails over instead of spinning forever
- **Sources:** review-llm-agent P1-3.
- **Files:** `src-tauri/src/llm/client.rs`, `src-tauri/src/llm/sse.rs`, `src-tauri/src/llm/wire.rs`, `src-tauri/src/find.rs`.
- **Fix:** Client `connect_timeout(10 s)` and `tcp_keepalive(30 s)`. An idle timeout inside `next_chunk`: 45 s with no bytes at all (keep-alive comments count) returns `ApiError::http(408, ..)` so `with_failover` moves on. Non-streamed calls get an overall cap of 90 s.
- **Acceptance:** UNIT `llm::client::tests::idle_stream_times_out_as_408` (local server that sends headers then stalls; tokio time paused); SMOKE+ `timeoutFailsOver` (fake LLM with a stalled first model).

### B-042 · P1 · llm · Failover never hands the retry a retired or blind model, and its copy only claims what happened
- **Sources:** review-llm-agent P1-4, P2-11; parity-features #3 (FAIL-05, AGENT-09, polish 2559dc7).
- **Files:** `src-tauri/src/llm/catalogue.rs`, `src-tauri/src/llm/failover.rs`, `src-tauri/src/llm/errors.rs`, `src-tauri/src/host/ask.rs`, `tools/fixtures/llm/errors.json`, `tools/fixtures/llm/catalogue.json`.
- **Fix:** `next_free_model`: among untried free curated ids prefer one present in the live catalogue and `usable_for(kind)` (Vision: accepts images; Agent: supports tools); a cold cache means curated order. Keep a per-session bad-model set (ids that returned 404) and skip them as first attempt and as stand-ins. Stamp `attempts` on the returned `ApiError` (K-17). Copy claims stand-ins only when `attempts > 1` (the polish-loop `{alsoTried}` rule), with the final wording from B-119. "Answering directly" after a reader stand-in succeeded reuses the model that last answered.
- **Reconciled:** review-llm (filter by `usable_for`) and parity-features (prefer live, else first free) are combined; the bad-model set is review-llm P2-11.
- **Acceptance:** UNIT `catalogue::tests::stand_in_is_live_and_sees_images`; UNIT `failover::tests::bad_model_set_is_skipped`; UNIT `errors::tests::single_attempt_copy_claims_no_stand_in`; SMOKE `failoverRateLimit`, `failoverCapped`, SMOKE+ `failoverStandInLive`.

### B-043 · P1 · llm · The daily free cap and empty quotas say what fixes them, and never burn more requests in the same account
- **Sources:** review-llm-agent P1-5.
- **Files:** `src-tauri/src/llm/errors.rs`, `src-tauri/src/llm/failover.rs`.
- **Fix:** In `message_for`, match the provider detail for `per-day`, `free-models-per-day`, `insufficient_quota`, `exceeded your current quota`, `RESOURCE_EXHAUSTED` and `quota`, and return code `daily-cap` / `no-credit` with text saying what fixes it (add credit, or wait until tomorrow). `should_failover` returns false for the account-wide daily cap inside the same provider. Cross-provider hops (B-046) still apply.
- **Reconciled:** P1-5 says "do not fail over" on the daily cap; design-ux §3.4 hops providers on 429. Both hold: no hop between free models of the same account, a hop to another provider is allowed under the paid-switch rule.
- **Acceptance:** UNIT `errors::tests::daily_cap_is_not_transient`; UNIT `failover::tests::daily_cap_stays_in_account`.

### B-044 · P1 · llm · Find's related and concept stages fail over like Ask
- **Sources:** review-llm-agent P1-7.
- **Files:** `src-tauri/src/find.rs`, `src-tauri/src/host/find.rs`.
- **Fix:** Run each stage through `with_failover(model, |t| llm.next_free_model(Kind::Find, t), || nothing_arrived, ..)` for `:free` ids, and `resolve_live` the Find model when Find opens. Native providers use their own model (B-036) with no failover.
- **Acceptance:** UNIT `find::tests::related_stage_fails_over_on_429`; SMOKE+ `findFailover`; SMOKE `findRelated`, `findConcept`.

### B-045 · S1 · llm · Every disclosure names who actually receives the screen
- **Sources:** design-ux U6 (Rust half); copy deck C-30, C-32, C-33.
- **Files:** `src-tauri/src/llm/mod.rs` (or `llm/settings.rs`), `src-tauri/src/host/mod.rs`.
- **Fix:** `llm::recipients(&settings) -> Vec<Recipient{id, name, model, modelName, free}>` (K-18): the ready providers in try order with the model each would use for Vision. Register `Hooks.overlay_extra` (K-07) returning `{recipients}` and add `recipients` to `settings_extra`. agent includes it in `agent:consent-request` (B-051).
- **Acceptance:** UNIT `llm::tests::recipients_follow_order_and_skip_unready`; SMOKE+ `recipients` (Anthropic-only profile: `overlay:state.recipients[0].id == "anthropic"`).

### B-046 · S1 · llm · Failover across providers, never to a paid account by surprise
- **Sources:** design-ux §3.4, U15 (cross-provider half); review-llm-agent P1-5 (interaction).
- **Files:** `src-tauri/src/host/ask.rs`, `src-tauri/src/llm/failover.rs`, `src-tauri/src/llm/settings.rs`.
- **Fix:** Candidates in order (B-037), skipping Not added, Key rejected, No credit (native), Busy until cooldown, and Free models off (OpenRouter unless a paid OpenRouter model is picked). Inside OpenRouter's free models the existing up-to-three failover. Hop to the next provider only before any answer text was shown, on 429, 5xx, 408, 404, 401, 402 or a timeout while online; never on 400, a refusal, Stop, after text started, or with a pinned model (B-122). Free to paid with `askBeforePaid` on (default): emit `query:error` with `code: "paid-hop"`, tone `note`, text C-63 (`OpenRouter's free models are busy. Ask <Provider> (<model name>) instead? <Provider> bills your account for it.`) and actions `[{id:"hop", label:"Ask <Provider>", provider, model}, {id:"wait", label:"Keep waiting"}, {id:"never-ask", label:"Don't ask again"}]`. While switching, stage `OpenRouter is busy. Asking <Provider>…`; after, `query:done.usage.via` = `<Provider>` and `usage.hopReason`. Cooldowns: 429 marks Busy for Retry-After or 60 s; 401 marks Key rejected until the key changes or a test passes; 402 marks No credit and re-tests silently after 10 min; each change emits `provider:status`. A newly rejected key raises one bar notice `Anthropic rejected your key, so answers come from OpenRouter for now.` with action `Fix key`.
- **Reconciled:** review-llm-agent's "with_failover never crosses providers" was a correctness check of today's code; design-ux §3.4 makes crossing the product behaviour with the paid guard. Decision D-14.
- **Acceptance:** UNIT `host::ask::tests::hop_only_before_text`; UNIT `host::ask::tests::free_to_paid_asks_first`; UNIT `llm::settings::tests::cooldowns`; SMOKE+ `crossProviderHop` (fake LLM: OpenRouter 429 x3, Anthropic ready with `askBeforePaid=false` → answer via Anthropic).

### B-119 · S2 · llm · Errors carry a code, the provider, the model's name and the actions that fix them
- **Sources:** design-ux U14, U34, U46 (C-61); review-llm-agent P2-10; parity-features ERR-12; copy deck C-21, C-22, C-23, C-61, C-70..C-74.
- **Files:** `src-tauri/src/llm/errors.rs`, `src-tauri/src/host/ask.rs`, `src-tauri/src/llm/catalogue.rs`, `tools/fixtures/llm/errors.json`.
- **Fix:** `query:error` payload becomes `{gen, message, question, code, provider, model, modelName, retryInMs?, tone: 'error'|'note', actions: [{id, label, ...}]}` (K-19). Codes and actions per design-ux U14's table (`bad-key`: Fix key, Ask <next>; `no-credit`: Add credit, Use a free model; `free-busy`/`rate-limited`: B-120; `model-gone`/`no-vision`: Choose model; `data-policy`: Open OpenRouter privacy settings, Test again; `offline`: none, waits; `server`/`unknown`: Retry, Copy details; plus `daily-cap` from B-043 and `paid-hop` from B-046). Use catalogue display names everywhere people read (stage labels C-21..C-23, usage line, errors), ids only in `title` and Copy details. Fix the misleading texts: 404 on a paid model claims no stand-in; 5xx says "on this model" unless a fallback was tried; a mid-stream body error is not "No connection" when online; a Gemini 400 "API key not valid" uses the bad-key wording; a Gemini safety block or Anthropic `refusal` says the model declined; an empty detail prints the error kind, not "Error". New wording from the copy deck rows above.
- **Acceptance:** UNIT `errors::tests::error_table_is_verbatim` (errors.json regenerated to the new deck); UNIT `errors::tests::codes_and_actions`; UNIT `errors::tests::gemini_invalid_key_is_bad_key`.

### B-120 · S2 · llm · Busy free models retry on their own, honouring Retry-After; 529 counts as busy
- **Sources:** design-ux U15, C-71, C-72; review-llm-agent P2-4 (classification half).
- **Files:** `src-tauri/src/host/ask.rs`, `src-tauri/src/llm/errors.rs`, `src-tauri/src/llm/client.rs`.
- **Fix:** Parse `Retry-After` (seconds or HTTP date) into `ApiError.retry_after_ms` (K-17); add 529 (Anthropic `overloaded_error`) to the retry and failover classes. For `free-busy` and `rate-limited` (after free-model failover and any provider hop), emit `query:error` with `tone: 'note'`, `retryInMs`, text `Free models are busy right now. Trying again in <n> s.` and actions `Try now` (+ `Ask <Provider> instead` when another provider is ready); wait Retry-After (default 20 s, then 40 s) and retry automatically at most twice; the final text is `Free models are still busy. Try again in a minute, or add another provider for busy times.` with `Retry` and `Add a provider`.
- **Acceptance:** UNIT `errors::tests::retry_after_parsed`; UNIT `host::ask::tests::busy_retries_twice_then_stops`; SMOKE+ `busyAutoRetry`.

### B-121 · S2 · llm · Every answer can say exactly what was sent
- **Sources:** design-ux U16 (Rust half).
- **Files:** `src-tauri/src/host/ask.rs`, `src-tauri/src/ipc.rs` and `preload.js` (llm block only).
- **Fix:** `query:done` gains `sent: {provider, providerName, model, modelName, free, image: "screen"|"region"|null, imageSize: [w,h], windowText: {app, chars}|null, history: n, reader: {modelName}|null, msgId}` (K-19). Keep the JPEGs of the last three answers per window in memory only (never on disk); new invoke `output:sent-image {msgId}` returns a `data:image/jpeg;base64,…` URL (the page CSP already allows `img-src data:`) or `null` when evicted. No frame-protocol change, so no capture dependency.
- **Reconciled:** Decision D-38.
- **Acceptance:** UNIT `host::ask::tests::done_payload_has_sent`; UNIT `host::ask::tests::sent_images_keep_last_three`; SMOKE+ `sentPanel` (fake LLM answer: `sent.image == "screen"` and `output:sent-image` returns a data URL).

### B-122 · S2 · llm · One question can be pinned to a model, and a retry can pick another
- **Sources:** design-ux U20 (Rust half).
- **Files:** `src-tauri/src/host/ask.rs`, `src-tauri/src/llm/catalogue.rs`.
- **Fix:** `query:submit` and `query:retry` accept `{provider, model}`; a pinned model never fails over and never hops. `models:list` gains `{providers: [{id, name, groups}]}` with every ready provider's models (existing fields unchanged for current callers).
- **Acceptance:** UNIT `host::ask::tests::pinned_model_is_used_and_never_fails_over`; SMOKE+ `modelPin`.

### B-123 · P2 · llm · Native model lists come from the providers, with the curated table as fallback
- **Sources:** parity-features #2 (provider detail: hardcoded lists, `sees_images`/`supports_tools` true for every native id).
- **Files:** `src-tauri/src/llm/client.rs`, `src-tauri/src/llm/provider.rs`, `src-tauri/src/llm/catalogue.rs`.
- **Fix:** `list_models` for Anthropic (`GET /v1/models`), OpenAI (`GET /v1/models`) and Gemini (`GET /v1beta/models`), cached 5 min like OpenRouter's, filtered to chat models; image and tool support from metadata where given (Gemini `supportedGenerationMethods`, Anthropic/OpenAI by family table), curated table as fallback and for defaults. A stored native id missing from the live list heals to the provider default in memory (never on disk).
- **Acceptance:** UNIT `llm::client::tests::native_list_parsing` (fixtures per provider in `tools/fixtures/llm/`); UNIT `catalogue::tests::native_retired_id_heals_in_memory`.

### B-124 · P2 · llm · The first question after picking an area captures fresh
- **Sources:** parity-features #13 (CAP-09).
- **Files:** `src-tauri/src/host/ask.rs`.
- **Fix:** Use the pick-time shot only when it is under 1 s old; otherwise capture fresh.
- **Reconciled:** Decision D-08.
- **Acceptance:** UNIT `host::ask::tests::stale_pick_shot_is_not_reused`.

### B-125 · P2 · llm · Wire robustness: errors in 200 bodies, bounded SSE, valid turn order, Gemini sampling, skill size
- **Sources:** review-llm-agent P2-9, P2-13, P2-16, P2-17, P2-19.
- **Files:** `src-tauri/src/llm/client.rs`, `src-tauri/src/llm/sse.rs`, `src-tauri/src/llm/wire.rs`, `src-tauri/src/host/ask.rs`, `src-tauri/src/skills.rs`, `src-tauri/src/llm/prompt.rs`.
- **Fix:** A 200 body with an `error` object returns `stream_error(&data["error"])`. Cap the SSE line buffer at 1 MB and error out. Drop leading assistant turns in `query_messages`. Omit `temperature` for Gemini (as for OpenAI and Anthropic). Cap a skill body at 8 KB, mark it `truncated` in `skills.list()` so Settings can say so.
- **Reconciled:** Decision D-18 (Gemini temperature omitted).
- **Acceptance:** UNIT `client::tests::error_in_200_body_is_an_error`; UNIT `sse::tests::buffer_is_capped`; UNIT `ask::tests::history_never_starts_with_assistant`; UNIT `wire::tests::gemini_body_has_no_temperature`; UNIT `skills::tests::large_skill_is_truncated`.

### B-126 · P2 · llm · Corporate networks work: system certificates and the system proxy
- **Sources:** review-llm-agent P2-18.
- **Files:** `src-tauri/src/llm/client.rs` (reqwest features are pre-enumerated in platform C1).
- **Fix:** Use rustls with native roots (`rustls-tls-native-roots`) so a TLS-inspection proxy's root in the Windows store is trusted, and honour the Windows system proxy (reqwest's system-proxy support in the locked version; if it does not read the registry/WinHTTP settings, read them with `WinHttpGetIEProxyConfigForCurrentUser` and pass `Proxy::all`). Apply the same client config to the updater only through a shared builder if platform agrees (else leave updater as is and note it).
- **Reconciled:** Decision D-19.
- **Acceptance:** UNIT `client::tests::builder_uses_native_roots_and_proxy` (inspect config); Progress records a manual reasoning check since a proxy cannot be simulated on the runner.

### B-127 · P2 · llm · Pointing by name works for non-Latin labels
- **Sources:** review-llm-agent P2-15.
- **Files:** `src-tauri/src/llm/elements.rs`.
- **Fix:** `norm_label` keeps Unicode letters and digits (`char::is_alphanumeric`), lowercased, like the UIA path in host/ask.rs.
- **Acceptance:** UNIT `elements::tests::cyrillic_and_cjk_names_match`; SMOKE `pointByName`.

### B-128 · P2 · llm · Find stays fast on one-letter queries and keeps offsets right on any text
- **Sources:** review-llm-agent P2-14.
- **Files:** `src-tauri/src/find.rs`, `src-tauri/src/host/find.rs`.
- **Fix:** Cap hits at 500; build the collapsed text once per span; serialize hits once per change; fold per character with an offset map so offsets, `match` and rects never drift (`İ`, `ﬁ`, `™`).
- **Acceptance:** UNIT `find::tests::one_letter_query_is_capped`; UNIT `find::tests::folding_keeps_offsets`; SMOKE `findLiteralMs` (< 50).

### B-129 · P2 · llm · Pin the Ask gating rules in a test, and keep the tour's try card correct
- **Sources:** parity-core LIFE-14; parity-ipc B7 (IPC-69, EVT-31).
- **Files:** `src-tauri/src/host/ask.rs`, `src-tauri/src/host/mod.rs`.
- **Fix:** UNIT for the exact-text and bar-context gating (main.js:1810, 2027). `on_try` opens the bar with prefill `open Notepad and write a haiku` (kept for any card that still sends `try`, though B-033 retires that card).
- **Acceptance:** UNIT `host::ask::tests::exact_text_gating_matches_main_js`; UNIT `host::tests::try_card_prefills_haiku`.

### B-181 · S3 · llm · A door to custom answer styles
- **Sources:** design-ux U49 (Rust half).
- **Files:** `src-tauri/src/host/settings.rs`, `src-tauri/src/skills.rs`.
- **Fix:** Invoke `skills:open-folder` creates `<exe>\skills` if missing and opens it in Explorer (absolute `explorer.exe` path).
- **Acceptance:** UNIT `skills::tests::folder_path_is_exe_relative`.

## Rules

The coordinator's rules, verbatim: "LF endings; never run the app locally; never weaken or skip a test or gate to get green; commit messages in sentence case describing the effect like the existing log, each ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; push only its own branch".

What each one means here:

- **LF endings** in every file you write. Before each commit run `git diff --cached --check` and `git ls-files --eol <changed files>` (expect `i/lf w/lf`). The repo has no `.gitattributes`, so your editor's endings are what gets committed.
- **Never run the app locally.** No `cargo tauri dev`, no launching a built exe, no `npm start` of the Electron app. This machine has no Rust toolchain and nine lanes share it; CI is the build and the test bed. Local checks are limited to `node --check <file>` for JS and reading code.
- **Never weaken or skip a test or gate to get green.** No `#[ignore]`, no deleted or loosened assertions, no raised thresholds, no `continue-on-error`, no label removed from a label file, no scene removed from ui-shots. An expected value changes only where one of your items specifies the new value (for example B-049's gate tables or B-087's defaults test).
- **Commits** in sentence case describing the effect, like `git log --oneline -15` on `rust-rewrite` (for example "Keep the first-run tour out of the capture and desktop checks"). Item IDs may go in the body. Every message ends with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Push only your own branch**, `rust/llm`: `git push origin HEAD:rust/llm`. Never push `rust-rewrite`, `main`, `ci-logs`, `shots` or another lane's branch; never force-push.
- **Never poll the GitHub API (60 req/hr shared by the whole machine).** No `gh run`, `gh api`, `gh pr`, or `curl` to api.github.com. CI results come only from `tools/ci-wait.mjs`, which reads the `ci-logs` branch with git.
- Edit only the files listed under "Files you own". If you need a change elsewhere, write it under Blockers (file, the change, which item needs it) and carry on with other items. Never create the per-lane files that harness C1 creates (`selftest/probes/<lane>.rs`, `tools/smoke-labels/<lane>.txt`, `tools/ui-shots/lanes/<lane>.mjs`); merge `origin/rust-rewrite` once they exist.
- Do not edit `DESIGN.md`, `PARITY.md`, `ARCHITECTURE.md`, `REQUESTS.md`, `BACKLOG.md` (only B-149 appends to its "Post-merge findings" section), the audits, other lanes' briefs, the Electron reference (`main.js`, `lib/**` except files you own, `package*.json`, `electron-builder.yml`) or `README.md`.
- Never commit a real API key. Tests and probes use fake keys and the fake LLM.

## Verification loop

Every cycle:

1. Make a batch of changes: one or more items, each with its unit tests, probe labels, ui-shots scenes and checks, and its entries.
2. `git diff --cached --check`, `node --check` on changed `.js` and `.mjs` files, re-read the diff against the item specs.
3. Commit, then push once: `git push origin HEAD:rust/llm` (first push: `git push -u origin HEAD:rust/llm`).
4. Wait for CI with the script below. If the push touched `renderer/`, `settings/`, `history/`, `src-tauri/bridge/` or `tools/ui-shots/`, wait on `ui-shots` as well as `rust`.
5. Exit 0: record the items as green in Progress with the sha. Exit 1: read what it printed, fix, and go to step 1. Exit 2: follow the printed sha, or wait again.

The coordinator's CI recipe, verbatim:

- Usage: `node tools/ci-wait.mjs <sha> [workflow=rust|ui-shots] [--timeout-min 30]` (sha can be HEAD/short). It `git fetch`es the `ci-logs` branch every 45 s; no GitHub API.
- Exit 0 = green; 1 = failure (prints summary.md, errors.txt, last 80 lines of the failing gate's log, and the `git show origin/ci-logs:<dir>/fmt.patch | git apply` command when rustfmt fails); 2 = timeout or cancelled (prints the newer sha to wait on if a later push superseded it).
- Full logs: `origin/ci-logs:<ref with / as ->/<sha>/<workflow>/` holds summary.md, meta.json, errors.txt, and prebuild/fmt/clippy/test/build/smoke/capture/desktop .log (ui-shots: bridge/playwright/shots .log). `<ref>/latest/<workflow>/` = newest run.
- ui-shots only runs when renderer/settings/history/bridge/tools/ui-shots paths change, so `ci-wait <sha> ui-shots` exits 2 on other pushes; that is expected.
- Pushing a new commit to the same branch cancels the running CI for the older sha (concurrency cancel-in-progress), so batch changes and push once per cycle.

For this lane the log directory is `origin/ci-logs:rust-llm/<sha>/<workflow>/` and the newest run is `origin/ci-logs:rust-llm/latest/<workflow>/`. Read a file with `git show "origin/ci-logs:rust-llm/latest/rust/smoke.log"` (use PowerShell or set `MSYS_NO_PATHCONV=1` in Git Bash, which otherwise mangles the `ref:path` argument). The rust workflow takes up to an hour on a cold cache; use `--timeout-min 70` when the build is cold. When the same sha is the tip of two branches (right after merging `origin/rust-rewrite` with nothing new on top), add `--ref rust/llm`.

## Starting and staying current

1. Your worktree starts at `rust-rewrite` HEAD. Create the branch with `git switch -c rust/llm` if it does not exist.
2. Read this brief, the cross-lane tables in `.claude/rust-rewrite/BACKLOG.md` (lanes, ownership, hotspots, contracts), `DESIGN.md`, and for each item the audit sections and PARITY rows it cites. The code is the ground truth for current behaviour; `main.js` on `main` is the ground truth for Electron behaviour.
3. Push your C1 (if "Contracts you provide" lists any) before anything else, with a commit message starting `C1 contracts:`. When it is green, write `C1 green at <sha>` in Progress.
4. Each cycle, `git fetch origin`. When a contract you need has landed (`git grep -n "<symbol>" origin/rust-rewrite -- <file>`) or `rust-rewrite` has moved, `git merge origin/rust-rewrite` (a merge commit; never rebase or force-push). A conflict in a file you own: resolve it. A conflict in a file you do not own means you edited it by mistake: take `origin/rust-rewrite`'s side and note it in Progress.
5. If CI fails for a reason entirely outside your files (a broken merge on `rust-rewrite`, a runner outage), record it under Blockers with the log line and keep working on what you can verify.

## Shared documents

Never edit `DESIGN.md`, `PARITY.md` or `ARCHITECTURE.md`. In the same commit as the change, append to `.claude/rust-rewrite/entries/llm.md`:

```
## <DESIGN|PARITY|ARCHITECTURE> · <section or row id> · <D-xx or B-xxx>
Add | Replace "<exact current text>" with:
<exact new text>
```

For a PARITY row whose status or evidence changes: `## PARITY · <row id> · B-xxx` and the line `status: <value>; evidence: <UNIT test or SMOKE label>`. Every decision your lane owns (see "Shared-document entries you write") gets an entry with its reason. The merge agent folds the entries in when your branch merges.

## Stop condition

Stop when every item below and the standing labels task is green on CI at your branch head (rust, and ui-shots when your changes touch its paths), each recorded in Progress with its sha and acceptance; or when every item that is not green has a named entry under Blockers saying what blocks it, why, and what would unblock it. Do not stop with an item that is neither green nor blocked. When you stop, make sure the final state is pushed and the last `ci-wait` result is in Progress.

## Progress

(One line per item as it turns green: `- B-xxx: green at <sha>; <acceptance names>`. Also `C1 green at <sha>` and any notes the next agent needs.)

- C1 contracts K-15, K-16, K-17, K-18, K-19, K-26: pushed, waiting on CI. The contracts are summarised in `entries/llm.md`. Two parts wait on other lanes: registering `llm::recipients` as `Hooks.overlay_extra` needs windows C1 (K-07), and the K-19 mocks in `tools/ui-shots/lanes/llm.mjs` need harness C1 (K-31) to create that file.
- B-036 to B-181 and the smoke-label port: not started.

## Blockers

(None yet. Format: `- B-xxx: <what blocks it>; <why>; <what would unblock it>`.)
