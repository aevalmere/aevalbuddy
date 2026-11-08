# Parity audit: features (PARITY sections 11-19, 26; REQUESTS 1-12; polish-loop)

Branch `rust-rewrite` at 544f982. Electron reference is `main` (= 069a4b8, the PARITY source tree; the working-tree
`main.js`/`lib/` are identical to it). Read-only audit: nothing was built or run. "DONE" means the code ports the
Electron behaviour. It does not mean the behaviour was seen working: CI runs fmt/clippy/unit/build/scaffold/capture
check/desktop selfcheck, plus `ui-shots` against a mock bridge, and none of the SMOKE+ labels in these sections exists
in the Rust self-test (`selftest.rs` is the phase-1 scaffold).

## 1. Summary

147 rows in scope (ASK 20, MODEL 8, FAIL 6, PROMPT 5, POINT 6, ERR 17, FIND 9, AGENT 22, DESK 12, CAP 13, GLASS 9,
LENS 2, TOUR 5, NEW 13).

| Status | Count | Rows |
|---|---|---|
| DONE | 123 | everything not listed below |
| PARTIAL | 14 | ASK-18, ASK-19, AGENT-09, AGENT-15, CAP-09, CAP-10, CAP-13, GLASS-05, LENS-01, TOUR-04, NEW-01, NEW-05, NEW-08, NEW-09 |
| BUG | 5 | FAIL-05, CAP-11, NEW-02, NEW-11, NEW-12 |
| MISSING | 3 | GLASS-04, GLASS-07, GLASS-08 |
| DEPARTED | 1 | GLASS-02 (transparency-off reason dropped, REQUESTS 5 / DESIGN.md "Liquid glass you set") |
| STUBBED | 0 | |
| UNSURE | 1 | ERR-17 (updater copy; belongs to the UPD audit) |

REQUESTS.md:

| # | Status | One line |
|---|---|---|
| 1 | partial | 123/147 rows here port at code level; almost none has runtime proof (no SMOKE+ for ASK/NEW, no shots harness, no contrast gate). |
| 2 | partial | All four wire formats exist and have unit tests. Everything around them is broken or missing: one OpenRouter-labelled key field with the provider guessed from the prefix, Remove and Test act on the wrong provider, nothing lets you pick a provider, native model picks are never saved, Find and Agent send OpenRouter ids to native APIs, and native keys are ignored whenever an OpenRouter key exists. |
| 3 | shipped | `hasSentFirstRequest` in the store, overlay:state and localStorage (ipc.rs:175-182,215-218; bar.js:316-345). An empty Enter does not count. |
| 4 | partial | Settings/History cannot actually fill the screen: `maximizable(false)` plus a 900 DIP max track width; output windows are capped at 900 and cannot maximise. |
| 5 | partial | The slider works live, except for a profile migrated from Electron with `surface:"solid"`: it stays solid at every slider value. |
| 6 | shipped | The hint lives only in the token's tooltip and menu (index.html:35,88-90). bar.js:102 keeps an sr-only copy for screen readers. ui-shots checks noLegend and hintOnLeftButton. |
| 7 | partial | Title bars and tokens are done in the renderer. Raw px/rgb values remain (settings.css 120 raw px, 56 rgb()). No SHOT matrix runs on the exe and no review has run. |
| 8 | shipped (gap) | glob.js plus `output:bar` work for the first window. Cascaded windows sit 36 px from the bar, past the 20 px margin, so the neck cannot reach. Not verified at runtime. |
| 9 | shipped | No input in output.html. ask_user and plan approval go through the bar (`agent:continue`). |
| 10 | shipped | frame.js zones plus the flags.rs WM_NCHITTEST band and forwarding poll. SMOKE+:resizeEdges does not exist. |
| 11 | partial | Windows float, drag and multiply. But there is one global query, close/new in the focused window aborts another window's stream, events to a spare that has not loaded are lost, and Find results and task trails take over the focused chat window. |
| 12 | missing | No polish-loop or review artefacts on the branch. |

## 2. Blocking (most severe first)

| # | ID | Problem | Rust | Electron | Fix spec |
|---|---|---|---|---|---|
| 1 | REQ-2 / AGENT-15 / FIND-02 | Find related/concept and the Agent always use the OpenRouter model id, even when `active()` is a native provider. A user with only an Anthropic/OpenAI/Gemini key gets a 404 on every related Find ("unavailable right now") and on every task. | find.rs:1530-1536 (`host.model()`), host/find.rs:76-79, host/agent.rs:178-181, 321-327 | n.a. (new) | `Host::model()` in host/find.rs and host/agent.rs: if `active().provider.is_native()` use `llm::settings::model_for(&StoreSettings, p, Kind::Find/Agent)`, and skip `resolve_live`/`heal_agent_model` for native. Unit test: native-only MemorySettings, the Find body model is the native default. |
| 2 | REQ-2 | Settings has one key field: `aria-label="OpenRouter API key"`, OpenRouter-only steps, the provider guessed from the key's prefix. Remove sends no provider, so it always clears OpenRouter (an Anthropic-only user presses Remove, sees "No key yet", and the key stays). Test checks the *active* provider. Nothing writes `provider`, so `active()` always picks OpenRouter when that key exists and native keys are dead weight. Model pickers list native models but `settings:save-model` writes the OpenRouter `model`/`brain`/... keys, so a native pick is ignored (and later healed off the OpenRouter id). | settings/settings.html:47-81; settings/settings.js:131-147; ipc.rs:262-268, 511-557; host/settings.rs:17-22, 48-53; llm/settings.rs:47-57 | n.a. | **Renderer:** four key rows (OpenRouter, Anthropic, OpenAI, Gemini), each with save/test/remove that pass `{provider}`. An "Answer with" picker. The model pickers pass `{provider}` to `models:list` and to save. **ipc.rs:** `settings:save-model*` take `provider`; for a native provider write `nativeModels.<provider>.<kind>`. New `settings:set-provider` writes `provider` (add to both allowlists and preload.js). `settings:get` already returns `keys`. |
| 3 | FAIL-05 / AGENT-09 / polish 2559dc7 | `next_free_model` takes the first untried free curated id without checking the live catalogue, so failover can spend one of its 3 tries on another retired id (a guaranteed 404) or on a vision model that lost image input (a 400, which does not fail over). The 404/429/5xx copy also claims stand-ins were tried when none were. | llm/catalogue.rs:401-408; llm/failover.rs:39-68; llm/errors.rs:176-181, 196-199, 207-210 | openrouter.js:857-864 (same blind spot); fixed on polish-loop 2559dc7 | catalogue.rs: among untried free ids, prefer one present in `all` (Vision: `accepts_images`), else the first free one. A cold cache means no opinion. failover.rs: stamp the attempt count on the returned `ApiError` (new field `attempts`). errors.rs: the polish-loop wording (`Rate limited.{alsoTried} ...`, `... not available right now.{alsoTried} OpenRouter says ...`, `server error.{alsoTried}`). Update tools/fixtures/llm/errors.json and add a test that the stand-in is in catalogue.json. |
| 4 | NEW-02 | Closing or resetting an output aborts whatever query is running whenever *that window is focused*: `busy_here = s.busy && focused_output == label`. To reproduce: ask in output-1, click output-2, close output-2, and output-1's answer is cut. Separately, `current()` is a single global, so a new question in a fresh window aborts the stream in another window. | windows/outputs.rs:327-345 (close), 357-366 (reset); host/ask.rs:47-77, 373 | main.js:1500-1503 (one chat, so abort was right) | Record the query's label: `AppState.query_label: Option<String>`, set in `submit_to` and cleared with `busy`. close/reset abort only when `query_label == label`. Change `current()` to a `HashMap<label, Current>` and make `abort(app, label)` per label. Keep `busy` as "any running" for the pill. |
| 5 | NEW-02 | Events to an output that has not loaded are lost. `spare()` returns a window even when `ready == false` (two quick questions after New chat, or right after a close), and `windows::emit` does not buffer. `on_loaded` replays only `chat.messages`, so the streamed deltas before the load are gone and the answer starts mid-sentence. | windows/outputs.rs:85-95 (spare), 461-470 (on_loaded); windows/mod.rs:54-61 | n.a. (one long-lived window) | `spare()` prefers `ready` spares. For an output that is not ready, `emit` queues `(channel, payload)` per label and `on_loaded`/`on_hello` flushes the queue after `chat:sync`. Or keep the streaming text on `Output` and send it with `chat:sync`. |
| 6 | CAP-11 / polish e796cd2 | `region::finish` destroys the window before settling. The window's `Destroyed`/`Focused(false)` handlers call `settle(None)` on the main thread while `finish` runs on a tokio thread, so a completed drag can resolve as a cancel. This is the exact Electron BLOCKER, here as a race instead of a certainty. | region.rs:83-87, 131-137 | lib/region.js finish (fixed on polish-loop) | `finish`: `settle(finish_rect(origin, r)); close(app);`. Port polish-loop's `regionDragReturnsTheBox` check into the capture check. |
| 7 | NEW-12 / REQ-5 | A profile from Electron v1.1.1 with `surface:"solid"` migrates to `glassIntensity 0`, but `surface` stays `solid` (kept for rollback). backdrop keeps reporting reason `setting`, and glass.js treats any reason not in IGNORED as solid. Result: moving the slider to "Very very" leaves every surface solid while Settings reads "Very very". | backdrop.rs:152-156, 169-174; ipc.rs:598-609; renderer/glass.js:129-137 | n.a. (new) | backdrop.rs `reasons_for`: drop `surface_solid` (None on the slider is the solid path, DESIGN.md), or have `settings:set-glass` call `s.set_surface("auto")` when `v > 0`. Unit test: a solid profile, set 0.7, tier is live. |
| 8 | NEW-11 / REQ-4 | Settings and History cannot go full screen. Every page is built `.maximizable(false)`, and `frame_meta` sets max width 900 DIPs, which flags.rs writes to `ptMaxTrackSize` (Windows caps the maximised size there too). Double-click maximise yields at most 900 wide, and DESIGN's "past 900 px the rail widens" can never trigger. Outputs are also capped at `MAX_W 900`. | windows/mod.rs:141, 895-901; windows/flags.rs:407-412; store/mod.rs:42 | n.a. (new) | Settings/History `Meta.max = (0,0)`, and allow maximize for those labels. Outputs: lift `MAX_W` for user resize (keep 900 for the default). Runtime check: toggleMaximize, then the window's width equals the work area width. |
| 9 | polish 7682d15 | Every window places itself on the **primary** display's work area, while capture and the pointer use the cursor's display. On two monitors the answer is about one screen and opens on the other. Nothing handles a display being removed or changed. | windows/screen.rs:139-142 (used by outputs.rs:131,147,171; windows/mod.rs:253) | main.js workArea() = primary (fixed on polish-loop) | Latch `screen::active()` in `open_bar`/`trigger_capture`/area hotkey/unhide. `work_area()` returns the latched display's work area, falling back to live when the id is gone. Re-place on WM_DISPLAYCHANGE / ScaleFactorChanged. |
| 10 | polish e51ca29 | The bar is still `<input type="text">`, so a pasted stack trace loses its newlines. | renderer/index.html:51; renderer/bar.js:439-540 | same on main (fixed on polish-loop) | textarea, Enter sends, Shift+Enter inserts a newline, autoGrow to 7 rows (INPUT_MAX_HEIGHT 280 already exists in layout.rs:12). |
| 11 | NEW-01 | Find results and task trails go to `outputs::target`, which is the focused chat window: Find switches its view to `find` and a task flips its kind to Trail and appends to that conversation. DESIGN says each answer, Find result and task trail gets its own window. | host/find.rs:181-195; host/agent.rs:150-164 | n.a. | `outputs::fresh(app)` (always a spare). Use it in agent `start_run` and in Find's `ensure_label`; follow-ups typed into a Find/Trail window still go to `target`. |
| 12 | polish 7f520b8 | The tray opens on `Click(Up)` and on `DoubleClick`. A double click on Windows produces both, so one double click calls `trigger_or_trail` three times (open/close/open). | tray.rs:259-268 | tray.js (fixed on polish-loop) | Drop the `DoubleClick` arm. |
| 13 | ASK-19 / ASK-18 / CAP-09 | Reopening from history does not clear the global area (`region::ACTIVE`), so the next question still crops to the old box. `chat:new` from an output window does not clear it either. The first question after a pick uses the shot taken at pick time instead of capturing fresh. The area is global, not per conversation. | ipc.rs:415-427; windows/outputs.rs:357-380, 383-460; host/ask.rs:463-473 | main.js:1921-1942, 1960-1978, 1544-1551 | Call `region::clear_active()` in `history:open` and `outputs::reset`. Capture fresh every time (drop the `pending_shot` reuse, or use it only when under ~1 s old). Optionally move the area onto `Output.chat.region` and read it per label. |
| 14 | GLASS-08 / GLASS-07 / GLASS-04 / GLASS-05 | The contrast gate (`backdrop/contrast.rs`) has no caller. There is no `styleStable`, and no `AEVAL_SHOT` harness, so `backdrop::pause/resume` (the live photograph) are never called. | backdrop/contrast.rs:10; backdrop.rs:615-630; selftest.rs | smoke-glass.js, shots-matrix.js, contrast.js | Agent (e): port the shots harness (pause, then capture, then resume), run contrast-probe.js per frame at 100/125/150 % with failedFrames=0, and add a styleStable sampling smoke. |
| 15 | LENS-01 / TOUR-04 / ASK-11 | The lens and the pointer send their first `lens:draw`/`buddy:show` on `PageLoadEvent::Finished`, which ARCHITECTURE 10.3 forbids (event before listener; the bridge only gates invokes). If the listener is not registered yet, the first hit frame or the first model pointer is lost. | lens.rs:111-120; buddy.rs:296-306 | n.a. | lens.js/buddy.js invoke a `*:ready` (or `glass:hello`) after `on()`. Rust marks ready and flushes on that invoke, not on Finished. |
| 16 | CAP-10 | `region::pick` calls `changed()` (whose hook runs `apply_registrations` in a spawned task) *before* `select()` builds the window, so the Escape claim can be computed while `region::is_active` is still false. | region.rs:217-233; host/mod.rs:420-442 | main.js:1232-1238 | Call `changed()` from inside `select()` after `open_window` succeeds. |
| 17 | NEW-05 | Cascaded windows are offset 24 px in y, so their glass sits 36 px from the bar, beyond the 20 px margins: the glob neck cannot cross. | windows/layout.rs:137-154 | n.a. | Cascade along the bar edge (x only) so every new window keeps the 12 px gap, or have glob.js fall back to its "no bar" grow when the gap exceeds the margin. |
| 18 | polish 3de320f | History `trim` serialises the whole list after every answer and again on every pop (O(n^2) at the 4 MB cap). It runs on a tokio worker, not the UI thread, so it costs CPU but causes no jank. | history.rs:74-77, 283-289 | lib/history.js (fixed on polish-loop) | Sum string lengths with a deliberately high estimate and pop until it fits (polish-loop algorithm). Test: the real serialised length is at most 4 MB. |
| 19 | NEW-08 / NEW-09 / REQ-7, REQ-12 | Consistency and formatting were never reviewed. Raw values remain in the CSS. | settings.css, history.css, overlay.css | n.a. | Token pass, then the five review loops (designer, ease-of-use, accessibility, power-user, trust) on exe screenshots. |
| 20 | polish 7f520b8 | `buddy:move` and `settings:data` are still on the listen allowlist with no sender. | ipc.rs:133-134; preload.js:122-123 | fixed on polish-loop | Remove both from both lists in one commit. |
| 21 | CAP-13 | The fidelity check is a proxy (Lanczos vs box on stripes and edge energy), not SSIM/edge energy vs the Electron capture of the 12 px fixture. | desktop/checks.rs:425-453 | new | Store the Electron fixture capture on `shots` and compare against it. |

Low, noted only: GLASS-02 inherits an Electron bug (backdrop.rs:464-476 = backdrop.js:141-149). After a capture-failed solid tier, the retry runs at fast pace, not every 30 s, and recovery never rebroadcasts `live`. `submit_to` returning early when no output window can be made leaves `busy` stuck (host/ask.rs:378-384, after `abort` and the gen bump). `windows::overlay_state` holds the state lock across store reads and `updater::online()` (windows/mod.rs:256-317).

### Fixes grouped by file ownership

- **llm/** (split b): #3 (catalogue.rs next_free_model, failover.rs attempts, errors.rs copy plus errors.json). Optional for #2: live native model lists in client.rs `list_models` (Anthropic `/v1/models`, OpenAI `/v1/models`, Gemini `models`), curated table as fallback. Today it is 3 hardcoded ids per provider (provider.rs:105-160) and `sees_images`/`supports_tools` are `true` for every native id.
- **host/** + **ipc.rs** (merge glue): #1 (host/find.rs, host/agent.rs model mapping), #2 (ipc.rs save-model* with provider, `settings:set-provider`), #4 (host/ask.rs per-label `current`, `query_label`), #7 (ipc.rs set-glass surface reset), #13 (ipc.rs history:open, host/ask.rs fresh capture), #20 (ipc.rs LISTEN_CHANNELS plus preload.js).
- **windows/** + **tray.rs** (split a): #4 (outputs.rs:327-366 close/reset use `query_label`), #5 (outputs.rs spare/ready, mod.rs emit queue), #8 (mod.rs create_page/frame_meta, store MAX_W), #9 (screen.rs latch), #11 (outputs.rs `fresh`), #12 (tray.rs), #17 (layout.rs cascade).
- **region.rs / lens.rs / buddy.rs / backdrop.rs / history.rs** (split c): #6, #7 (backdrop alternative), #15, #16, #18.
- **renderer pages** (split d): #2 (settings.html/js key rows, provider picker, provider-aware pickers), #10 (index.html/bar.js textarea), #15 (lens.js/buddy.js ready invoke), #19 (CSS tokens).
- **self-test / CI** (agent e): #14, #21, plus the missing SMOKE+ labels: helpMe, retryNoDup, textOnly, outputFloating, outputMulti, outputDrag, noOutputLayout, noOutputInput, resizeEdges, globMorphMs, suggestionsOnce, captureActiveDisplay, captureFailNull, dpiRoundTrip.

## 3. polish-loop port check (`git log main..polish-loop`)

| Commit | Same bug in Rust? | Where |
|---|---|---|
| 2559dc7 Stop failover handing the retry another retired model | **yes** (stand-in pick and copy). The boot warm-up is ported. | catalogue.rs:401-408; errors.rs:176-210; warm-up at lib.rs:166, host/mod.rs:380-384 |
| e796cd2 Make the area selector return the area | **yes, as a race** (close before settle; a destroy/blur handler can settle None first) | region.rs:83-87, 131-137 |
| 004d5c7 Stop a finished pointer taking the next one with it | no (the 700 ms destroy checks `is_active()` first) | buddy.rs:757-771 |
| 7682d15 Open the overlay on the screen it is answering about | **yes** (work area is the primary; no display-change handling) | windows/screen.rs:139-142 |
| e51ca29 Let the question take more than one line | **yes** (still `<input>`) | renderer/index.html:51 |
| 3de320f Stop serialising the whole history... | **yes** for trim (off the UI thread). No for resize writes (debounced 50 ms). | history.rs:74-77, 283-289; windows/mod.rs:1024-1032 |
| 7f520b8 Five small things | dead listen channels **yes** (ipc.rs:133-134). motion.stop null guard n.a. captureRegion missing from the active map: no (hotkeys.rs:345-358 iterates ACTIONS). Tray click plus double-click **yes** (tray.rs:259-268). Stream reader leak and 200-with-no-body: no (client.rs:282-322; drop releases the body, empty becomes 204). | as listed |
| b60833c Record the departures | n.a. (docs) | |
| 086cc7a Give the repo a gate that can fail | n.a. (node tooling). Rust has fmt/clippy and `ipc::tests::every_invoke_channel_has_a_route`; nothing catches dead *listen* channels. | ipc.rs tests |
| dd29408 Close the ledger | n.a. | |
| 63ee020 README overview | n.a. | |

## 4. Full table

Runtime column: which proof is still missing on the installed exe. "unit" means a covering `cargo test` exists.

| ID | Status | Rust location | Evidence / notes | Runtime proof needed |
|---|---|---|---|---|
| ASK-01 | DONE | host/ask.rs:358-368 | regex `^help me[.!]?$`, not on retry | SMOKE+:helpMe |
| ASK-02 | DONE | host/ask.rs:369-372 | true if any provider has a key | SMOKE:queryWithoutKey |
| ASK-03 | DONE | host/ask.rs:373-395 | abort, gen++, DEFAULT_PROMPT (ask.rs:12) | SMOKE+ |
| ASK-04 | DONE | host/ask.rs:397-410, 786-806 | resume per window via `last_question` | SMOKE+:retryNoDup |
| ASK-05 | DONE | host/ask.rs:405-413; client.rs:409-430 | unit stream_query_sends_the_screen_and_streams_text | DIFF |
| ASK-06 | DONE | host/ask.rs:440-446 | chat:user, then query:start before capture | SHOT:05 |
| ASK-07 | DONE | host/ask.rs:194-206, 455-474 | tokio::join of shot, models, exact text | SMOKE+ |
| ASK-08 | DONE | host/ask.rs:478-486; ask.rs:21-33 | UTF-16 length; unit visual_regex_matches_the_renderer | SMOKE+:textOnly |
| ASK-09 | DONE | host/ask.rs:504-640 | reader failover, then brain failover, then "Answering directly" | SMOKE+ (fake LLM) |
| ASK-10 | DONE | host/ask.rs:548-556; skills.rs | unit routing_matches_the_self_test | SMOKE:skillRoute |
| ASK-11 | DONE | host/ask.rs:262-332 | max 3, shot's display; first show races, see TOUR-04 | SMOKE:pointResolved |
| ASK-12 | DONE | host/ask.rs:650-679 | history saved, pill error cleared, first-answer | SMOKE+ |
| ASK-13 | DONE | host/ask.rs:681-702 | reader stage names the reader model | SHOT:14 |
| ASK-14 | DONE | host/ask.rs:704-716 | newest gen only, then updater::idle | UPDATE |
| ASK-15 | DONE | host/mod.rs:165-196 | heal on disk for a chosen id; skipped for native | SMOKE:healRetired |
| ASK-16 | DONE | host/ask.rs:93-130 | 16000 chars, no OCR, 1500/4000 ms | SMOKE:askHasUiaText |
| ASK-17 | DONE | host/ask.rs:132-191 | latest-only check by Arc::ptr_eq | SMOKE+ |
| ASK-18 | PARTIAL | windows/mod.rs:956-967; ipc.rs:401-410; outputs.rs:357-380 | bar: abort, detach, region clear (old window stays, NEW-02). Output chat:new does not clear the area | SMOKE+ |
| ASK-19 | PARTIAL | ipc.rs:415-427; outputs.rs:383-460 | the global area (`region::ACTIVE`) is not cleared | SMOKE+ |
| ASK-20 | DONE | state.rs:97-108 | unit chat_ids_look_like_electrons | - |
| MODEL-01 | DONE | wire.rs:28-30, 72-83, 175-182 | unit openrouter_body_is_the_electron_body_plus_usage | DIFF |
| MODEL-02 | DONE | catalogue.rs:81-84 | unit defaults_are_live_in_the_recorded_catalogue | SMOKE:modelDefaultOffered |
| MODEL-03 | DONE | catalogue.rs:86-192 | unit curated_table_order_and_tiers | - |
| MODEL-04 | DONE | catalogue.rs:164-170, 295-322 | offline list from curated | SMOKE:modelList |
| MODEL-05 | DONE | client.rs:556-596 | 5 min TTL, 8 s; unit catalogue_is_cached | - |
| MODEL-06 | DONE | catalogue.rs:327-372 | units detail_lines, list_hides_paid_and_tags_agents | SMOKE:modelList* |
| MODEL-07 | DONE | catalogue.rs:378-399 | unit retired_ids_heal_to_a_live_free_model | SMOKE:healRetired |
| MODEL-08 | DONE | client.rs:516-552, 640-682 | unit test_key_messages; native gets "Key works." | - |
| FAIL-01 | DONE | errors.rs:93-103 | unit failover_statuses_are_the_everyday_ones | SMOKE:failoverRateLimit |
| FAIL-02 | DONE | failover.rs:52-57 | unit auth_paid_and_partial_answers_never_switch | SMOKE:failoverKeepsAuthError |
| FAIL-03 | DONE | failover.rs:11, 58-62 | unit rate_limits_move_to_the_next_free_model_at_most_three_times | SMOKE:failoverCapped |
| FAIL-04 | DONE | failover.rs:52; host/ask.rs:595 | `nothing_shown` | DIFF |
| FAIL-05 | BUG | catalogue.rs:401-408 | matches main but violates the retired-model constraint; see Blocking #3 | unit (stand-in in catalogue) |
| FAIL-06 | DONE | failover.rs:15-24 | unit short_names | - |
| PROMPT-01 | DONE | prompt.rs:15-63; client.rs:338-389 | strings compared by hand to openrouter.js:423-457; unit reader_rejects_prose_and_tags_its_errors | DIFF byte-equal |
| PROMPT-02 | DONE | prompt.rs:150-226 | unit system_prompt_variants | SMOKE:promptShape |
| PROMPT-03 | DONE | prompt.rs:66-94 | unit exact_text_is_marked_and_capped | DIFF |
| PROMPT-04 | DONE | client.rs:442-447, 464-472, 496-507 | native raises max_tokens to 16000 (wire.rs:42) | DIFF |
| PROMPT-05 | DONE | client.rs:282-322; sse.rs; wire.rs:591-680 | units openrouter_stream, anthropic_stream, gemini_stream | - |
| POINT-01 | DONE | points.rs | 5 unit tests | SMOKE:pointExtract |
| POINT-02 | DONE | elements.rs:99-135 | units rows_in_every_shape, at_most_eight_rows | SMOKE:readerElements |
| POINT-03 | DONE | elements.rs:158-200 | unit names_resolve_only_when_clear | SMOKE:pointByName |
| POINT-04 | DONE | ask.rs:62-73 | unit fractions_take_grid_or_pixels | SMOKE:pointGridMapping |
| POINT-05 | DONE | host/ask.rs:276 | region or display | SMOKE:regionPointTarget |
| POINT-06 | DONE | host/ask.rs:227-258, 300-310 | score 70, 8-point lead, role words | SMOKE:pointFromUia |
| ERR-01..13 | DONE | errors.rs:105-217 | unit error_table_is_verbatim (errors.json). The 404/429/5xx copy is untrue (polish 2559dc7); ERR-12 with an empty detail prints "Error", JS printed the error name | - |
| ERR-14 | DONE | agent/describe.rs:230-240 | unit passwords_are_dotted_and_errors_named | - |
| ERR-15 | DONE | lib.rs:53-80 | presence toast | SMOKE |
| ERR-16 | DONE | crash.rs:65-66 | | SMOKE:crashLogged |
| ERR-17 | UNSURE | updater.rs | outside this scope (UPD) | - |
| FIND-01 | DONE | find.rs:73-236, 442-496 | 5 units | SMOKE:findLiteral |
| FIND-02 | DONE | find.rs:551-558, 1557-1650 | unit related_terms_arrive_after_the_pause; native provider broken, see Blocking #1 | SMOKE:findRelated |
| FIND-03 | DONE | find.rs:559-569, 1651-1760 | unit concepts_come_from_the_correlate_stage | SMOKE:findConcept |
| FIND-04 | DONE | find.rs:1160-1175, 840-850 | unit ask_mode_says_what_would_leave_the_computer | SHOT:19 |
| FIND-05 | DONE | find.rs:1293-1440, 1082-1101 | poll 2 s, nudge 250 ms, 6 windows | SMOKE+ |
| FIND-06 | DONE | find.rs:570-577, 807-863 | strings verbatim | SHOT:26,27 |
| FIND-07 | DONE | find.rs:1107-1158, 1247-1291; host/find.rs:233-268 | unit results_show_once_there_is_something_to_say | SMOKE:findStep |
| FIND-08 | DONE | host/find.rs:214-231; find.rs:1482 | px to DIP | SMOKE:lensHits |
| FIND-09 | DONE | find.rs | literal path is keyless and offline | SMOKE:findOffline |
| AGENT-01 | DONE | agent/run.rs:44-50 | constants equal agent.js:21-27 | DIFF |
| AGENT-02 | DONE | agent/tools.rs | unit the_toolset_is_the_browser_one | DIFF |
| AGENT-03 | DONE | agent/tools.rs:150-170 | | DIFF |
| AGENT-04 | DONE | agent/tools.rs; run.rs:60-63 | unit json_turns_are_found_in_prose | SMOKE:agentJsonMode |
| AGENT-05 | DONE | agent/gate.rs | 5 units; no DIFF harness | SMOKE:gate* |
| AGENT-06 | DONE | agent/run.rs:441-450, 1099 | random id | SMOKE:agentConfirmGate |
| AGENT-07 | DONE | agent/run.rs:1162, 1225; describe.rs | | SMOKE:passwordRedacted |
| AGENT-08 | DONE | agent/run.rs | unit only_the_newest_observation_and_image_go_in_full | - |
| AGENT-09 | PARTIAL | agent/run.rs:1922-1966 | faithful port; the stand-in may be retired (FAIL-05); native: OpenRouter id (Blocking #1) | DIFF |
| AGENT-10 | DONE | agent/run.rs:814-840 | 150 ms own-act window | SMOKE:agentTakeover |
| AGENT-11 | DONE | agent/run.rs (resume); host/agent.rs:245 | | SMOKE:agentTakeover |
| AGENT-12 | DONE | host/agent.rs:470-503 | trail plus final text into history | SMOKE+ |
| AGENT-13 | DONE | agent/run.rs:52-76, 2105, 1971 | | SMOKE:agentMaxSteps |
| AGENT-14 | DONE | host/agent.rs:432-452 | the OS foreground via listWindows | SMOKE:agentFocusKept |
| AGENT-15 | PARTIAL | host/agent.rs:117-205 | flow matches; the trail takes over the focused chat window (NEW-01); native model mapping missing | SMOKE+ |
| AGENT-16 | DONE | host/agent.rs:28, 340-361 | | SHOT:22 |
| AGENT-17 | DONE | host/agent.rs:333-338; buddy.rs | glide timing not inspected | SHOT:22 |
| AGENT-18 | DONE | host/agent.rs:390-427 | units app_names..., links... | SMOKE+ |
| AGENT-19 | DONE | host/agent.rs:455-463 | | SMOKE+ |
| AGENT-20 | DONE | agent/run.rs:1539-1600, 1862 | | DIFF |
| AGENT-21 | DONE | agent/run.rs | unit trail_text_numbers_the_steps | - |
| AGENT-22 | DONE | agent/run.rs:1831-1833 | | SMOKE+ |
| DESK-01 | DONE | desktop/client.rs:121-127, 363-376; protocol.rs:182 | host.cs/ps1 included verbatim | SMOKE:desktopReady |
| DESK-02 | DONE | desktop/protocol.rs | codec units | - |
| DESK-03 | DONE | desktop/protocol.rs:11-20 | unit timeouts | - |
| DESK-04 | DONE | desktop/client.rs:69-95 | unit deaths_disable_after_three_in_a_minute | SMOKE+ |
| DESK-05 | DONE | desktop/client.rs:198 | taskkill /T | SMOKE:helperNotAtBoot |
| DESK-06 | DONE | desktop/client.rs:523-549; win.rs:91-100 | WebView2 pids | SMOKE:desktopOwnExcluded |
| DESK-07 | DONE | desktop/client.rs:620-630 | | SMOKE:agentTakeover |
| DESK-08 | DONE | windows/screen.rs:194-206 | unit dip_round_trip_on_a_scaled_display | SMOKE+:dpiRoundTrip |
| DESK-09 | DONE | lib/desktop/host.cs (reused) | CI desktop selfcheck | selfcheck |
| DESK-10 | DONE | desktop/protocol.rs:25-40 | | - |
| DESK-11 | DONE | host.cs | | SMOKE:passwordRedacted |
| DESK-12 | DONE | desktop/client.rs:151-190 | SHA set in rust.yml:78, release.yml:99 | SMOKE:desktopReadyMs |
| CAP-01 | DONE | capture.rs:155-158; screen active_display | `contains`, not `nearest` (minor) | SMOKE+:captureActiveDisplay |
| CAP-02 | DONE | capture.rs:160-178; encode.rs:132-150 | unit whole_screen_matches_electron_thumbnail_sizes | SMOKE:capture |
| CAP-03 | DONE | encode.rs:164-200 | unit crop_scales_by_what_arrived | SMOKE:regionCapture |
| CAP-04 | DONE | encode.rs:66-90 | Lanczos3 | SMOKE:regionCapture |
| CAP-05 | DONE | encode.rs:112-127 | 3 units | SMOKE:capture |
| CAP-06 | DONE | capture.rs:181-205 | None on failure; capture thread uses catch_unwind | SMOKE+:captureFailNull |
| CAP-07 | DONE | capture/screen.rs | unit matching_picks_most_overlap_by_id_not_order | - |
| CAP-08 | DONE | capture.rs:252-257; checks.rs:318 | captureExcludesOwn in CI capture check | capture check |
| CAP-09 | PARTIAL | host/ask.rs:463-473 | the first question after a pick reuses the pick-time shot; the area is global | SMOKE+ |
| CAP-10 | PARTIAL | region.rs:217-233 | Escape claim races window creation | SMOKE:regionEscape |
| CAP-11 | BUG | region.rs:83-87, 131-137 | close before settle (polish e796cd2) | regionDragReturnsTheBox |
| CAP-12 | DONE | capture.rs; encode.rs | DIP mapping, unit-tested | SMOKE+ at 125/150 % |
| CAP-13 | PARTIAL | desktop/checks.rs:425-453 | proxy gate only | RUNNER:captureFidelity |
| GLASS-01 | DONE | backdrop.rs:38-42, 370-404, 441-541 | 1/6, q72, 500 ms/5 s | SMOKE:glassTier |
| GLASS-02 | DEPARTED | backdrop.rs:152-167 | transparency-off dropped (REQ 5). Rest kept; inherits the Electron recovery bug (see Low) | SMOKE:glassTier |
| GLASS-03 | DONE | backdrop.rs:542-600 | | SHOT:08 |
| GLASS-04 | MISSING | backdrop.rs:615-630 | pause/resume have no caller (no shots harness) | SHOT:12 |
| GLASS-05 | PARTIAL | backdrop.rs:633-760; checks.rs:487-501 | fixture renders; no AEVAL_SHOT matrix | SHOT matrix |
| GLASS-06 | DONE | capture.rs thread; checks.rs:477-480 | must blockP95 < 25 ms | SMOKE:snapCost |
| GLASS-07 | MISSING | - | no styleStable | SMOKE:styleStable |
| GLASS-08 | MISSING | backdrop/contrast.rs:10 | probe ported, no caller | SHOT failedFrames=0 |
| GLASS-09 | DONE | lens.rs:206-247; backdrop::crop | | SHOT:20 |
| LENS-01 | PARTIAL | lens.rs:111-120, 136-200 | first draw on PageLoadEvent::Finished (10.3 race) | SMOKE:lensFirstDraw |
| LENS-02 | DONE | host/find.rs:301; host/agent.rs:482 | | SMOKE+ |
| TOUR-01 | DONE | buddy.rs:60-110 | unit (buddy.rs:811-816) | SMOKE:onboardingSteps |
| TOUR-02 | DONE | lib.rs:154-165 | quiet_run guard | SMOKE+ |
| TOUR-03 | DONE | buddy.rs:27, 589, 752 | | SMOKE:walkthrough |
| TOUR-04 | PARTIAL | buddy.rs:296-306, 507-548 | queued show flushed on Finished (10.3 race) | SMOKE:buddyPoints |
| TOUR-05 | DONE | buddy.rs | page-drawn pointer | SMOKE+ |
| NEW-01 | PARTIAL | windows/outputs.rs:89-125; layout.rs:108-156 | chats float; Find/Trail reuse the focused chat window | SMOKE+:outputFloating |
| NEW-02 | BUG | windows/outputs.rs:327-366, 85-95, 461-470; host/ask.rs:47-77 | wrong-window abort; global query; events lost before load | SMOKE+:outputMulti |
| NEW-03 | DONE | renderer/output.html:36; frame.js:30-47 | ui-shots outputTitleBar/outputControls | SMOKE+:outputDrag |
| NEW-04 | DONE | ipc.rs:441; renderer | ui-shots noDockUi | SMOKE+:noOutputLayout |
| NEW-05 | PARTIAL | renderer/glob.js; ipc.rs:369-380; output.js:343-348, 547-560 | first window fine; cascaded gap 36 px | SMOKE+:globMorphMs, SMOKE:noSetOpacity |
| NEW-06 | DONE | output.html (no input); bar.js; windows/mod.rs:285-289 | ui-shots noOutputInput, askUserRoutedFromBar | SMOKE+:noOutputInput |
| NEW-07 | DONE | frame.js:51-150; windows/flags.rs:366-414, 444-490 | page zones plus NCHITTEST band plus forward poll | SMOKE+:resizeEdges |
| NEW-08 | PARTIAL | renderer CSS | styled, not reviewed | SHOT matrix, reviews |
| NEW-09 | PARTIAL | renderer CSS | raw px/rgb remain | SHOT, reviews |
| NEW-10 | DONE | ipc.rs:175-182, 215-218; store; bar.js:316-345 | unit first_request_needs_text | SMOKE+:suggestionsOnce |
| NEW-11 | BUG | windows/mod.rs:141, 895-901; flags.rs:407-412 | maximise capped at 900 DIP | SHOT:07 |
| NEW-12 | BUG | backdrop.rs:152-156, 169-174; ipc.rs:598-609 | a migrated solid profile stays solid | RUNNER glassLive |
| NEW-13 | DONE | renderer/index.html:35, 88-90; bar.js:102 (sr-only) | ui-shots noLegend, hintOnLeftButton | - |

### REQ-2 provider detail (for the owner of #1/#2)

- Request/stream/error formats: Anthropic Messages (system lifted, base64 image blocks, tool_use/tool_result, `disable_parallel_tool_use`, error `type` mapped to a status), OpenAI Chat Completions (`max_completion_tokens`, `stream_options.include_usage`, no temperature), Gemini generateContent/streamGenerateContent?alt=sse (`inlineData`, `functionCall`/`functionResponse`, `parametersJsonSchema`, thought parts filtered, `thoughtSignature` replayed via `_native`). All in wire.rs:72-125, 268-470, 591-680, 690-805, with units for each. HTTP errors use `error.message`, which all three supply (wire.rs:544-552). Nothing has been verified live.
- Vision input: yes, for all three (wire.rs:282-298, 371-383; OpenAI passes `image_url` through).
- Model lists: hardcoded, 3 per provider (provider.rs:105-160). Ids like `claude-opus-5-5`, `gpt-6.1-sol`, `gemini-4-argon` are asserted "checked 2026-11-07" but are not fetched; a retired native id has no failover and no heal. `supports_tools` is false for every OpenAI id (JSON fallback, client.rs:621-633).
- Selection: `active()` = override (nothing writes it), else OpenRouter, else the first native provider (llm/settings.rs:47-57).
