# Lane brief: windows

Branch `rust/windows`, cut from `rust-rewrite`. 31 items. Cross-lane tables (ownership, hotspots, contracts, decisions, merge order) are in `.claude/rust-rewrite/BACKLOG.md`; everything you need to do your items is in this file.

## Goal

The window manager behaves the way people expect with several answers open: Esc never throws work away, each window owns its own question, the bar shows which window the next question continues, windows open on the display in use and maximize properly, text follows the Windows text size, announcements reach the focused window, content grows inside the page instead of resizing the OS window per line, IPC checks who is calling, and the keyboard layer has the channels it needs. You also own the IPC allowlists that every Rust lane adds to.

## Files you own

`src-tauri/src/windows/{mod,flags,layout,motion,outputs,stage}.rs` and new files under `windows/` (not `screen.rs`, which is capture's), `src-tauri/src/state.rs`, `src-tauri/src/ipc.rs`, `preload.js`, and (after harness C1) `src-tauri/src/selftest/probes/windows.rs`, `tools/smoke-labels/windows.txt`.

## Contracts you provide (C1 first)

- **Allowlist blocks:** the reserved per-lane blocks in INVOKE_CHANNELS, LISTEN_CHANNELS and the two `preload.js` Sets, exactly as BACKLOG.md "Hotspots" shows, identical in both files. Capture, llm and agent cannot add channels until this lands, so it is the first thing you push.
- **Route scan:** add `include_str!("host/settings.rs")` to `every_invoke_channel_has_a_route`.
- **K-03** queries and `Hooks.abort_query_for`; **K-07** `Hooks.overlay_extra`; **K-04** `outputs::fresh` and `outputs::close_all`; **K-05** `overlay:state.target` and `visibleOutputs`; **K-06** `output:meta`; **K-22** text-scale channels; **K-23** window channels; **K-24** `history:list.enabled`, `settings:get.overlayPos`.
- Behaviour may be stubbed in C1 (a route that returns `null`, an emitter that sends a fixed payload), but every allowlisted channel has a route (invoke) or an emitter (listen) in the same commit, so `every_invoke_channel_has_a_route` and B-095's sender test hold.

## Contracts you consume

- **K-01**, **K-02** crash guards (platform) for B-090; **K-08** `Store::set_raw` (platform) for B-022.
- **K-10**, **K-11** active display and forced scale (capture) for B-025.
- **K-15** the list of channels `host::settings::route` owns (llm, in `entries/llm.md`) for B-107.
- **K-26** fake LLM fixtures (llm) and **K-30** selftest API (harness) for probes.

## Smoke labels you port

`windows`, `widthDefaults`, `pillWidthHeld`, `pillPinned`, `panelWidthRange`, `panelHeightPinned`, `panelWidthHeld`, `widthAfterCycle`, `motionAnchor`, `mode`, `taskbarHidden`, `overlayOnTop`, `outputOnTop`, `settingsWindow`, `captureProtection`, `contentProtectedAll`, `windowsChecked`, `noSetOpacity`, `collapseClickable`, `topmostHolds`, `lowerForBrowser`, `hudMode`, `hudSnapFound`, `hudExitMode`, `storedPos`, `hudNudge`, `hudDragSettle`, `barOpensUnlocked`, `bootMs`, `toggleVisibility`, `agentHotkeyTrail`, `regionHotkey`, `collapseReturnsFocus`, `outputAutoHeight`, `outputFloating`, `outputMulti`, `outputDrag`, `noOutputLayout`, `resizeEdges`, `dodge`, `answerFollowsTranscript`, `answerHeightUnderLongChat`, `eventTargeting`. Exact names and Electron semantics: `.claude/rust-rewrite/audit/parity-ipc.md` §3.

## Shared-document entries you write

D-01, D-02, D-04, D-16 (layout half), D-17, D-24, D-31, D-32, D-36; PARITY evidence and notes for POS, WIN, IPC, EVT rows your items close (including the POS-09, IPC-02, POS-11, IPC-06 and answerFollowsTranscript redefinitions).

## Lane notes

- Your allowlist blocks are edited by three other lanes in parallel. Never reorder, rename or move the marker lines; never edit inside another lane's block.
- Bar, sheet and settings implement the page side of K-22, K-23 and K-24 from your payload shapes; write each shape in a doc comment next to its route so they can read it on `origin/rust-rewrite`.

## Standing item: port your smoke labels

Every PARITY label under "Smoke labels you port" and every new label below is printed by `src-tauri/src/selftest/probes/windows.rs` through the K-30 API, with the Electron semantics from `.claude/rust-rewrite/audit/parity-ipc.md` §3 and `main.js` on `main`, and listed in `tools/smoke-labels/windows.txt` in the same commit that makes it print `=ok`. Acceptance: the rust workflow's smoke log shows each label `=ok` and the labels gate passes.

New labels your items add, beyond the PARITY list above (you print these): `a11yRelay`, `barOpenMs`, `escClearsBeforeStop`, `escKeepsAnswer`, `historyOpenClearsArea`, `keyboardChannels`, `maximizeFull`, `outputGlass`, `perWindowAbort`, `reopenSettingsAtOnce`, `startOverUndo`, `targetCycle`, `textScale`, `windowCap`, `windowNudge`.

Labels another lane prints that your items rely on (do not print them yourself): `captureActiveDisplay` (capture).

## Items

| ID | Severity | Title |
|---|---|---|
| B-018 | S1 | Esc never stops an answer or hides output windows; collapsing gives the foreground back |
| B-019 | S1 | Each output window owns its query; close and reset touch only their own; nothing is lost before a page loads |
| B-020 | S1 | Find results and task trails open their own window |
| B-021 | S1 | The bar knows which window the next question continues |
| B-022 | S1 | Text follows the Windows text size and the person's own setting |
| B-023 | S1 | Announcements reach the window that has focus |
| B-024 | S1 | Settings, History and outputs maximize to the full work area, and the resize band sits on the glass |
| B-025 | P1 | Windows open on the display the person is using and stay on it |
| B-026 | P1 | Reopening a conversation or starting over clears the picked area |
| B-027 | S1 | Output content grows inside the page; the OS window stops resizing for every streamed line |
| B-089 | P2 | IPC checks which window is calling |
| B-090 | P2 | A panicking IPC call settles its promise and clears busy; window procs never unwind across Win32 |
| B-091 | P2 | Click-through cannot be re-applied after it was turned off, and the poll stops when idle |
| B-092 | P2 | A window closed and reopened at once keeps its frame and actually shows |
| B-093 | P2 | Debounce and animation maps do not grow forever |
| B-094 | P2 | `overlay_state` gathers slow values before taking the shared lock |
| B-095 | P2 | Remove the two listen channels nobody sends |
| B-096 | P2 | Pin the settings:get key set and the overlay:state shape in tests |
| B-097 | P2 | The 160 px output minimum is recorded and pinned |
| B-098 | P2 | The last dragged output height seeds the next new window |
| B-099 | P2 | Cascaded windows stay 12 px from the bar |
| B-100 | S2 | Channels for the keyboard layer |
| B-101 | S2 | Windows can be moved and resized from the keyboard |
| B-102 | S2 | Esc clears a half-typed answer to the task before it stops the task |
| B-103 | S2 | The front output is marked |
| B-104 | S2 | Hotkey-to-field latency is measured |
| B-105 | S2 | "Start over" keeps the old conversation for 10 s and can be undone |
| B-106 | S2 | `PANEL_WIDTH` 600 |
| B-107 | S2 | Delete core rows superseded by `host::settings::route` |
| B-178 | S3 | Windows do not pile up past six |
| B-179 | S3 | Settings knows the bar position; History knows whether saving is on |

Work in severity order (P0, then S1 and P1, then S2 and P2, then S3), after your C1 if you have one.

### B-018 · S1 · windows · Esc never stops an answer or hides output windows; collapsing gives the foreground back
- **Sources:** design-ux U3; parity-core POS-09; parity-ipc IPC-02.
- **Files:** `src-tauri/src/windows/mod.rs`, `src-tauri/src/windows/flags.rs`, `src-tauri/src/ipc.rs`.
- **Fix:** `collapse_panel` never calls `abort_query` and never calls `outputs::hide_all` (keep `hide_all` for the `toggleVisibility` chord only). The global Escape path (`on_escape`, windows/mod.rs:679) follows the same rule. Add `flags::blur(hwnd)`: if `GetForegroundWindow() == hwnd`, walk `GetWindow(hwnd, GW_HWNDNEXT)` to the first visible, non-tool window of another process and `SetForegroundWindow` it; call it in `collapse_panel` and `exit_hud` before `set_focusable(false)`. Also record the foreground HWND in `trigger_capture` before `show_focused` and prefer it when it is still a window. New invoke `output:blur` (windows block) applies the same blur to the calling output window.
- **Reconciled:** Electron collapse aborts and hides (POS-09, IPC-02); U3 wins because output windows float independently since REQUESTS #11 and free models queue for 20-30 s. The blur half of POS-09 is kept. Decision D-01.
- **Acceptance:** SMOKE+ `collapseReturnsFocus` (Notepad is foreground after `overlay:collapse`); SMOKE+ `escKeepsAnswer` (Esc during a fake-LLM stream: the stream completes and the output stays visible); SMOKE `pillWidthHeld`, `collapseClickable`.

### B-019 · S1 · windows · Each output window owns its query; close and reset touch only their own; nothing is lost before a page loads
- **Sources:** parity-features #4 and #5 (NEW-02); design-ux U4 (window half); review-core P2-7 (consumer).
- **Files:** `src-tauri/src/state.rs`, `src-tauri/src/windows/outputs.rs`, `src-tauri/src/windows/mod.rs`.
- **Fix:** In C1 (K-03): `AppState.queries: BTreeMap<String, u64>` (label → gen of the running Ask), helpers `query_started(label, gen)`, `query_ended(label, gen)`, `query_running(label) -> bool`, and `busy` recomputed as "any running" for the pill; `Hooks.abort_query_for: Option<StrHook>` (llm registers). `outputs::close` and `outputs::reset` abort only when `query_running(label)`, through `abort_query_for(label)`. `spare()` prefers spares with `ready == true`. `windows::emit` to an output whose `ready == false` queues `(channel, payload)` per label; `on_loaded` and `on_hello` flush the queue after `chat:sync`, in order.
- **Acceptance:** UNIT `outputs::tests::close_of_other_window_keeps_running_query`; UNIT `windows::tests::events_to_unready_output_are_replayed_in_order`; SMOKE+ `outputMulti` (3 open, all visible, distinct labels) and SMOKE+ `perWindowAbort` (ask in output-1, close output-2, output-1's stream completes).

### B-020 · S1 · windows · Find results and task trails open their own window
- **Sources:** parity-features #11 (NEW-01, AGENT-15).
- **Files:** `src-tauri/src/windows/outputs.rs`.
- **Fix:** Add `outputs::fresh(app) -> Option<String>` (C1, K-04): always a new or spare idle window, never the focused chat. llm uses it in host/find.rs `ensure_label`, agent in host/agent.rs `start_run`. Follow-ups typed while a Find or Trail window is the target still go to `target`.
- **Acceptance:** UNIT `outputs::tests::fresh_never_returns_the_focused_chat`; SMOKE+ `outputFloating`.

### B-021 · S1 · windows · The bar knows which window the next question continues
- **Sources:** design-ux U5 (Rust half), U19 (cycle).
- **Files:** `src-tauri/src/windows/mod.rs`, `src-tauri/src/windows/outputs.rs`, `src-tauri/src/ipc.rs`, `preload.js`.
- **Fix:** `overlay:state` gains `target: {label, title, kind} | null` and `visibleOutputs: n` (K-05). New listen `output:meta {grow: 'down'|'up', front: bool, target: bool}` sent to an output whenever any of the three changes (K-06; `front` is B-103, `grow` B-027). New invoke `output:cycle-target {dir: 1|-1}` moves the target through visible outputs by `touched` order and returns `{label}`. Clicking an output still retargets (`outputs::touch`). Merge `Hooks.overlay_extra` (K-07, registered by llm with `recipients`) into `overlay:state`.
- **Acceptance:** UNIT `windows::tests::overlay_state_has_target_and_count`; SMOKE+ `targetCycle` (two outputs, cycle, `overlay:state.target.label` changes).

### B-022 · S1 · windows · Text follows the Windows text size and the person's own setting
- **Sources:** design-ux U8 (Rust half).
- **Files:** `src-tauri/src/windows/mod.rs` (or a new `src-tauri/src/windows/text_scale.rs`), `src-tauri/src/ipc.rs`, `preload.js`.
- **Fix:** Read `Windows.UI.ViewManagement.UISettings.TextScaleFactor` at boot and on `TextScaleFactorChanged` (feature `UI_ViewManagement`, pre-enumerated in platform C1). Effective scale = OS factor x stored `textScale` (1.0..2.0, step 0.1, stored with `Store::set_raw("textScale", ..)`, K-08), clamped to 2.5. Emit `ui:text-scale {scale}` to every window on change and to a window in `on_hello`. New invoke `settings:set-text-scale {scale}` stores and broadcasts; `settings:get` gains `textScale` (the stored factor) and `osTextScale`.
- **Acceptance:** UNIT `windows::tests::text_scale_clamps_and_multiplies`; SMOKE+ `textScale` (set 1.5, every page's `--text-scale` reads 1.5 x OS factor via `Cx::eval`).

### B-023 · S1 · windows · Announcements reach the window that has focus
- **Sources:** design-ux U7 (Rust half).
- **Files:** `src-tauri/src/windows/mod.rs`, `src-tauri/src/ipc.rs`, `preload.js`.
- **Fix:** New invoke `a11y:announce {text, priority: 'polite'|'assertive'}`: Rust forwards it as listen `a11y:say {text, priority}` to the focused AevalBuddy window, else to the overlay if the bar is open, else (optional) `UiaRaiseNotificationEvent` on the overlay HWND (feature `Win32_UI_Accessibility`, pre-enumerated). Text is capped at 300 characters.
- **Acceptance:** UNIT `windows::tests::announce_goes_to_focused_window`; SMOKE+ `a11yRelay` (announce from an unfocused output; the overlay's assertive region receives it, read with `Cx::eval`).

### B-024 · S1 · windows · Settings, History and outputs maximize to the full work area, and the resize band sits on the glass
- **Sources:** parity-features #8 (NEW-11, REQ-4); design-visual F5 (Rust half), F24 (Rust half).
- **Files:** `src-tauri/src/windows/mod.rs` (`frame_meta`, `create_page`), `src-tauri/src/windows/outputs.rs`, `src-tauri/src/windows/flags.rs`.
- **Fix:** `frame_meta().max = (0.0, 0.0)` (unbounded) for Settings and History, `.maximizable(true)` for settings, history and `output-*`. Outputs: lift `MAX_W` for user resize and maximize (keep 900 only as the default width cap). Settings and History get `margin = stage::MARGIN` like outputs, and their window bounds add the margin, so the native hit-test band, click-through and the visible glass agree; when maximized the margin is 0. Keep the minimums. Consumes `html[data-maximized]` styling from bar/settings (K-41).
- **Acceptance:** UNIT `windows::tests::settings_history_have_no_max`; SMOKE+ `maximizeFull` (toggleMaximize on Settings, History and an output: window width equals the work-area width).

### B-025 · P1 · windows · Windows open on the display the person is using and stay on it
- **Sources:** parity-features #9 (polish 7682d15); review-core P1-1 (windows half); parity-core LIFE-03 (flags half).
- **Files:** `src-tauri/src/windows/mod.rs`, `src-tauri/src/windows/outputs.rs`, `src-tauri/src/windows/layout.rs`, `src-tauri/src/windows/motion.rs`, `src-tauri/src/windows/flags.rs`.
- **Fix:** Call `screen::latch_active()` (K-10, capture) in `open_bar`, `trigger_capture`, the area hotkey and unhide; place the overlay and new outputs in the latched display's work area (`screen::work_area_of`), falling back to live when the device is gone. `motion::set_glass` writes back through the same display it read (`matching_px` / HMONITOR), never re-deriving from DIP. Outputs clamp to the work area of the display they are on, and store their position relative to it. On WM_DISPLAYCHANGE / ScaleFactorChanged re-place the overlay and clamp outputs. `flags::scale_of` returns `screen::forced_scale()` when set.
- **Acceptance:** UNIT `layout::tests::output_clamps_to_its_own_display`; UNIT `motion::tests::set_glass_round_trips_in_overlap_band`; SMOKE+ `captureActiveDisplay` (shared with capture: overlay opens on the display of the foreground window).

### B-026 · P1 · windows · Reopening a conversation or starting over clears the picked area
- **Sources:** parity-ipc B6 (IPC-18); parity-features #13 (ASK-18, ASK-19).
- **Files:** `src-tauri/src/ipc.rs` (`history:open`), `src-tauri/src/windows/outputs.rs` (`reset`, `open_conversation`).
- **Fix:** Call `crate::region::clear_active()` in `history:open` before `outputs::open_conversation`, and in `outputs::reset` (output `chat:new`). The per-output `user_height = 0.0` already covers Electron's `setOutputHeight(0)`.
- **Acceptance:** SMOKE+ `historyOpenClearsArea` (pick an area, open History, reopen: `region::active_region()` is None); UNIT where testable.

### B-027 · S1 · windows · Output content grows inside the page; the OS window stops resizing for every streamed line
- **Sources:** design-visual F13 (Rust half); parity-core POS-11; parity-ipc B15 (IPC-06).
- **Files:** `src-tauri/src/windows/outputs.rs`, `src-tauri/src/windows/flags.rs`, `src-tauri/src/ipc.rs`, `preload.js`.
- **Fix:** When an output is shown set its outer height once to `max(user_height, output_max_height(wa)) + 2*MARGIN` and stop animating bounds for content growth (keep `T_OUT_GROW` for user moves and dodges). New invoke `output:glass {top, height}` (renderer debounces to one per frame) stores the visible sheet rect; `flags` hit-testing, the native resize band and click-through use it. `output:resize` keeps working as a compatibility alias that updates the stored height only. The area below the sheet is click-through (`glass:passthrough`). For dodges and user glides, `motion` starts from the stored in-flight target `o.rect`, not the live mid-glide rect (POS-11); a height report during a glide is stored and applied on landing (B15). Tell the renderer whether the sheet grows down or up (`output:meta.grow`, K-06).
- **Reconciled:** POS-11 and B15 assume OS-window growth; F13 replaces that for content, so they now apply to user moves and dodges only. `answerFollowsTranscript` is redefined as "the glass height equals the transcript height" and PARITY records it. Decision D-04.
- **Acceptance:** UNIT `outputs::tests::content_growth_never_sets_bounds`; SMOKE `answerFollowsTranscript` (redefined: `output:glass.height` equals the eval'd `#opanel` height within 1 px); SMOKE+ `outputGlass` (hit-test band follows the reported rect); SMOKE+ `dodge`.

### B-089 · P2 · windows · IPC checks which window is calling
- **Sources:** review-core P2-2 (authorization half).
- **Files:** `src-tauri/src/ipc.rs`.
- **Fix:** A channel → allowed-labels table consulted at the top of `aeval_invoke` (after the allowlist): agent, settings-write, shortcut, update and `app:quit` channels only from `main`/overlay and `settings` (and `history` for history channels); output windows get their own channels plus `query:*`, `chat:*`, `output:*`, `ui:*`, `a11y:announce`; lens and buddy get only their own channels and `glass:hello`. A refused call returns `Err("not allowed from <label>")`. Each lane's new channels get a row; document the table in `entries/windows.md` for ARCHITECTURE.
- **Acceptance:** UNIT `ipc::tests::output_cannot_start_agent`; UNIT `ipc::tests::every_invoke_channel_has_a_label_rule`.

### B-090 · P2 · windows · A panicking IPC call settles its promise and clears busy; window procs never unwind across Win32
- **Sources:** review-core P2-5 (IPC and subclass half); parity-core WIN-16 (create_page call site).
- **Files:** `src-tauri/src/ipc.rs`, `src-tauri/src/windows/flags.rs`, `src-tauri/src/windows/mod.rs`.
- **Fix:** Wrap the body of `aeval_invoke` in `crash::guard_async` (K-01) / `AssertUnwindSafe(..).catch_unwind()`, return `Err`, and clear the caller's busy entry. Wrap the subclass proc body in `catch_unwind` (return `DefSubclassProc` on panic). Call `crash::guard_webview` (K-02) from `create_page` with a recreate callback for browser-process exit.
- **Acceptance:** UNIT `ipc::tests::panicking_route_returns_err`; SMOKE `rendererReload` (printed by platform's probe; it exercises the `guard_webview` call this item adds in `create_page`).

### B-091 · P2 · windows · Click-through cannot be re-applied after it was turned off, and the poll stops when idle
- **Sources:** review-core P2-3.
- **Files:** `src-tauri/src/windows/flags.rs`.
- **Fix:** Re-check `requested` and `effective` under the lock right before `set_ignore_cursor_events`, or apply on main. Poll at 60 Hz only while the cursor is within 64 px of a click-through window's rect, else 4 Hz.
- **Acceptance:** UNIT `flags::tests::stale_snapshot_does_not_reapply_ignore`; SMOKE `collapseClickable`.

### B-092 · P2 · windows · A window closed and reopened at once keeps its frame and actually shows
- **Sources:** review-core P2-4.
- **Files:** `src-tauri/src/windows/flags.rs`, `src-tauri/src/windows/mod.rs`.
- **Fix:** `flags::forget(hwnd)` removes only entries for that hwnd. Track a closing set; `open_settings` / `open_history` create a fresh window when the existing one is closing.
- **Acceptance:** UNIT `flags::tests::forget_by_hwnd_keeps_new_window`; SMOKE+ `reopenSettingsAtOnce`.

### B-093 · P2 · windows · Debounce and animation maps do not grow forever
- **Sources:** review-core P2-10.
- **Files:** `src-tauri/src/windows/mod.rs`, `src-tauri/src/windows/motion.rs`, `src-tauri/src/windows/outputs.rs`.
- **Fix:** Remove `GEN` and `generations` entries in `outputs::on_destroyed` and `flags::forget`.
- **Acceptance:** UNIT `windows::tests::closed_output_leaves_no_gen_entries`.

### B-094 · P2 · windows · `overlay_state` gathers slow values before taking the shared lock
- **Sources:** review-core P2-11; parity-features "Low".
- **Files:** `src-tauri/src/windows/mod.rs`.
- **Fix:** Read `region::active_region()`, `updater::online()` and store values before locking state; build the payload from the copies.
- **Acceptance:** UNIT `windows::tests::overlay_state_lock_scope` (or a code-review note in Progress if not testable) and SMOKE `mode`.

### B-095 · P2 · windows · Remove the two listen channels nobody sends
- **Sources:** parity-features #20 (polish 7f520b8); parity-ipc EVT-18, EVT-19.
- **Files:** `src-tauri/src/ipc.rs`, `preload.js`.
- **Fix:** Remove `buddy:move` and `settings:data` from `LISTEN_CHANNELS` and the preload Set in one commit; add a test that every listen channel has an emitter (text scan of `emit`/`emit_to` call sites, like the invoke scan).
- **Acceptance:** UNIT `ipc::tests::every_listen_channel_has_a_sender`; UNIT `listen_allowlist_matches_preload`.

### B-096 · P2 · windows · Pin the settings:get key set and the overlay:state shape in tests
- **Sources:** parity-ipc B11 (IPC-22); parity-core POS-13.
- **Files:** `src-tauri/src/ipc.rs`, `src-tauri/src/windows/mod.rs`.
- **Fix:** Split the key list out of `settings_data` into a pure function and assert it equals the 28 Electron keys (main.js:2525-2562) plus the documented extras (`keys`, `glassIntensity`, `hasSentFirstRequest`, `textScale`, `osTextScale`, `overlayPos`); the hook-merged extras (`providers`, `failover`, `keyHints`, `keyStorage`, `recipients`) are listed in the test as hook-provided. Add a serde snapshot test of `overlay:state`'s field set.
- **Acceptance:** UNIT `ipc::tests::settings_get_key_set`; UNIT `windows::tests::overlay_state_field_set`.

### B-097 · P2 · windows · The 160 px output minimum is recorded and pinned
- **Sources:** parity-core POS-02, POS-05.
- **Files:** `src-tauri/src/windows/layout.rs`.
- **Fix:** Keep `OUTPUT_MIN_HEIGHT = 160`; add `#[test] fn constants_match_main_js()` asserting every layout constant against main.js (with 160, gap 12 and panel width 600 as the documented departures). DESIGN entry "Output windows: minimum 320 x 160, because the 44 px title bar plus one row and the stage margins do not fit in 90".
- **Reconciled:** Decision D-02.
- **Acceptance:** UNIT `layout::tests::constants_match_main_js`.

### B-098 · P2 · windows · The last dragged output height seeds the next new window
- **Sources:** parity-core POS-20, STORE-16.
- **Files:** `src-tauri/src/windows/outputs.rs`.
- **Fix:** In `create` and `show`, seed `user_height` from `store::get().get_output_height()` when it is above 0; `reset` still zeroes it.
- **Acceptance:** UNIT `outputs::tests::new_window_uses_stored_height`; SMOKE `outputAutoHeight`.

### B-099 · P2 · windows · Cascaded windows stay 12 px from the bar
- **Sources:** parity-features #17 (NEW-05).
- **Files:** `src-tauri/src/windows/layout.rs`.
- **Fix:** Cascade new outputs along the bar edge (x only, 24 px steps, wrapping within the work area) so every new window keeps the 12 px `OUTPUT_GAP` and the glob can reach (sheet's F25 fallback covers dragged windows).
- **Acceptance:** UNIT `layout::tests::cascade_keeps_the_gap`.

### B-100 · S2 · windows · Channels for the keyboard layer
- **Sources:** design-ux U19 (Rust half), U37.
- **Files:** `src-tauri/src/ipc.rs`, `preload.js`, `src-tauri/src/windows/outputs.rs`, `src-tauri/src/windows/mod.rs`.
- **Fix:** `outputs::close_all(app)` (C1, part of K-04; the tray's `Close all answer windows` calls it, B-080). `output:close {label?}` closes that window (default: caller or target); `output:close-all`; `output:last-answer {label?}` returns `{text}` of the target's last answer; `output:cycle-target` (B-021); `output:blur` (B-018); `bar:prefill-open {text}` → `windows::open_bar(app, None, Some(text))` (K-23).
- **Acceptance:** UNIT `outputs::tests::close_all_and_last_answer`; SMOKE+ `keyboardChannels`.

### B-101 · S2 · windows · Windows can be moved and resized from the keyboard
- **Sources:** design-ux U30 (Rust half).
- **Files:** `src-tauri/src/ipc.rs`, `preload.js`, `src-tauri/src/windows/mod.rs`.
- **Fix:** Invoke `window:nudge {dx, dy, dw, dh}` (DIP) moves or resizes the calling window within its display's work area, respecting minimums; outputs store the result like a drag. Check whether Win+Arrow snapping works on the runner and say so in `entries/windows.md`.
- **Acceptance:** SMOKE+ `windowNudge` (nudge an output by 16 px: bounds move 16 DIP).

### B-102 · S2 · windows · Esc clears a half-typed answer to the task before it stops the task
- **Sources:** design-ux U17 (Rust half), C-16.
- **Files:** `src-tauri/src/windows/mod.rs`, `src-tauri/src/ipc.rs`, `preload.js`.
- **Fix:** Invoke `bar:dirty {dirty}` stores the flag. `on_escape`: if the agent state is `asking`, `plan` or `paused`, the overlay is focused and the bar is dirty, emit listen `bar:clear` to the overlay and announce `Cleared. Press Esc again to stop the task.`; otherwise stop as today.
- **Acceptance:** UNIT `windows::tests::escape_clears_dirty_bar_while_asking`; SMOKE+ `escClearsBeforeStop`.

### B-103 · S2 · windows · The front output is marked
- **Sources:** design-visual F19 (Rust half).
- **Files:** `src-tauri/src/windows/outputs.rs`.
- **Fix:** When `touched` changes, send `output:meta.front` (K-06) to every output (true only for the most recently touched).
- **Acceptance:** UNIT `outputs::tests::front_follows_touch`.

### B-104 · S2 · windows · Hotkey-to-field latency is measured
- **Sources:** design-ux U25 (Rust half).
- **Files:** `src-tauri/src/windows/mod.rs`, `src-tauri/src/ipc.rs`, `preload.js`.
- **Fix:** Record `t0` in the hotkey handler and `t1` on a new invoke `bar:ready` sent after the field is focused; keep the last 20 in memory; the windows probe opens the bar 20 times and logs `barOpenMs=<p50>/<p95>` with a must of p95 < 100 ms on the runner.
- **Acceptance:** SMOKE+ `barOpenMs`.

### B-105 · S2 · windows · "Start over" keeps the old conversation for 10 s and can be undone
- **Sources:** design-ux U22 (Rust half).
- **Files:** `src-tauri/src/windows/outputs.rs`, `src-tauri/src/ipc.rs`, `preload.js`.
- **Fix:** `outputs::reset` keeps the previous `Chat` for 10 s (`undo_reset`); `chat:reset` keeps the sheet visible; new invoke `output:undo-reset {label?}` restores the chat and replays `chat:sync`.
- **Acceptance:** UNIT `outputs::tests::undo_reset_within_10s`; SMOKE+ `startOverUndo`.

### B-106 · S2 · windows · `PANEL_WIDTH` 600
- **Sources:** design-visual F27 (layout half).
- **Files:** `src-tauri/src/windows/layout.rs`.
- **Fix:** `PANEL_WIDTH = 600` to match the store default (B-087); constants test updated.
- **Acceptance:** UNIT `layout::tests::constants_match_main_js`; SMOKE `widthDefaults`.

### B-107 · S2 · windows · Delete core rows superseded by `host::settings::route`
- **Sources:** coordination (K-15).
- **Files:** `src-tauri/src/ipc.rs`.
- **Fix:** After llm's C1 is on rust-rewrite, delete the core `route` arms for the channels listed in `.claude/rust-rewrite/entries/llm.md` "Routed by host::settings" (expected: `settings:save-key`, `settings:clear-key`, `settings:save-model`, `settings:save-brain`, `settings:save-find-model`, `settings:save-agent-model`, `settings:clip-key`, `settings:use-clip-key`). `every_invoke_channel_has_a_route` must still pass.
- **Acceptance:** UNIT `every_invoke_channel_has_a_route`; grep in Progress showing each channel string appears in exactly one router file.

### B-178 · S3 · windows · Windows do not pile up past six
- **Sources:** design-ux U38 (Rust half).
- **Files:** `src-tauri/src/windows/outputs.rs`.
- **Fix:** When a seventh output would open, close the least recently touched idle one (never a running one) and emit `bar:note {text: 'Closed "<title>". It is in History.'}` (K-23).
- **Reconciled:** Decision D-32.
- **Acceptance:** UNIT `outputs::tests::seventh_closes_least_recent_idle`; SMOKE+ `windowCap`.

### B-179 · S3 · windows · Settings knows the bar position; History knows whether saving is on
- **Sources:** design-ux U47 (Rust half), U32 (Rust half).
- **Files:** `src-tauri/src/ipc.rs`.
- **Fix:** `settings:get` gains `overlayPos`; `history:list` returns `{rows, enabled}` (K-24).
- **Acceptance:** UNIT `ipc::tests::settings_get_key_set` (B-096) includes `overlayPos`; UNIT `ipc::tests::history_list_has_enabled`.

## Rules

The coordinator's rules, verbatim: "LF endings; never run the app locally; never weaken or skip a test or gate to get green; commit messages in sentence case describing the effect like the existing log, each ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; push only its own branch".

What each one means here:

- **LF endings** in every file you write. Before each commit run `git diff --cached --check` and `git ls-files --eol <changed files>` (expect `i/lf w/lf`). The repo has no `.gitattributes`, so your editor's endings are what gets committed.
- **Never run the app locally.** No `cargo tauri dev`, no launching a built exe, no `npm start` of the Electron app. This machine has no Rust toolchain and nine lanes share it; CI is the build and the test bed. Local checks are limited to `node --check <file>` for JS and reading code.
- **Never weaken or skip a test or gate to get green.** No `#[ignore]`, no deleted or loosened assertions, no raised thresholds, no `continue-on-error`, no label removed from a label file, no scene removed from ui-shots. An expected value changes only where one of your items specifies the new value (for example B-049's gate tables or B-087's defaults test).
- **Commits** in sentence case describing the effect, like `git log --oneline -15` on `rust-rewrite` (for example "Keep the first-run tour out of the capture and desktop checks"). Item IDs may go in the body. Every message ends with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Push only your own branch**, `rust/windows`: `git push origin HEAD:rust/windows`. Never push `rust-rewrite`, `main`, `ci-logs`, `shots` or another lane's branch; never force-push.
- **Never poll the GitHub API (60 req/hr shared by the whole machine).** No `gh run`, `gh api`, `gh pr`, or `curl` to api.github.com. CI results come only from `tools/ci-wait.mjs`, which reads the `ci-logs` branch with git.
- Edit only the files listed under "Files you own". If you need a change elsewhere, write it under Blockers (file, the change, which item needs it) and carry on with other items. Never create the per-lane files that harness C1 creates (`selftest/probes/<lane>.rs`, `tools/smoke-labels/<lane>.txt`, `tools/ui-shots/lanes/<lane>.mjs`); merge `origin/rust-rewrite` once they exist.
- Do not edit `DESIGN.md`, `PARITY.md`, `ARCHITECTURE.md`, `REQUESTS.md`, `BACKLOG.md` (only B-149 appends to its "Post-merge findings" section), the audits, other lanes' briefs, the Electron reference (`main.js`, `lib/**` except files you own, `package*.json`, `electron-builder.yml`) or `README.md`.
- Never commit a real API key. Tests and probes use fake keys and the fake LLM.

## Verification loop

Every cycle:

1. Make a batch of changes: one or more items, each with its unit tests, probe labels, ui-shots scenes and checks, and its entries.
2. `git diff --cached --check`, `node --check` on changed `.js` and `.mjs` files, re-read the diff against the item specs.
3. Commit, then push once: `git push origin HEAD:rust/windows` (first push: `git push -u origin HEAD:rust/windows`).
4. Wait for CI with the script below. If the push touched `renderer/`, `settings/`, `history/`, `src-tauri/bridge/` or `tools/ui-shots/`, wait on `ui-shots` as well as `rust`.
5. Exit 0: record the items as green in Progress with the sha. Exit 1: read what it printed, fix, and go to step 1. Exit 2: follow the printed sha, or wait again.

The coordinator's CI recipe, verbatim:

- Usage: `node tools/ci-wait.mjs <sha> [workflow=rust|ui-shots] [--timeout-min 30]` (sha can be HEAD/short). It `git fetch`es the `ci-logs` branch every 45 s; no GitHub API.
- Exit 0 = green; 1 = failure (prints summary.md, errors.txt, last 80 lines of the failing gate's log, and the `git show origin/ci-logs:<dir>/fmt.patch | git apply` command when rustfmt fails); 2 = timeout or cancelled (prints the newer sha to wait on if a later push superseded it).
- Full logs: `origin/ci-logs:<ref with / as ->/<sha>/<workflow>/` holds summary.md, meta.json, errors.txt, and prebuild/fmt/clippy/test/build/smoke/capture/desktop .log (ui-shots: bridge/playwright/shots .log). `<ref>/latest/<workflow>/` = newest run.
- ui-shots only runs when renderer/settings/history/bridge/tools/ui-shots paths change, so `ci-wait <sha> ui-shots` exits 2 on other pushes; that is expected.
- Pushing a new commit to the same branch cancels the running CI for the older sha (concurrency cancel-in-progress), so batch changes and push once per cycle.

For this lane the log directory is `origin/ci-logs:rust-windows/<sha>/<workflow>/` and the newest run is `origin/ci-logs:rust-windows/latest/<workflow>/`. Read a file with `git show "origin/ci-logs:rust-windows/latest/rust/smoke.log"` (use PowerShell or set `MSYS_NO_PATHCONV=1` in Git Bash, which otherwise mangles the `ref:path` argument). The rust workflow takes up to an hour on a cold cache; use `--timeout-min 70` when the build is cold. When the same sha is the tip of two branches (right after merging `origin/rust-rewrite` with nothing new on top), add `--ref rust/windows`.

## Starting and staying current

1. Your worktree starts at `rust-rewrite` HEAD. Create the branch with `git switch -c rust/windows` if it does not exist.
2. Read this brief, the cross-lane tables in `.claude/rust-rewrite/BACKLOG.md` (lanes, ownership, hotspots, contracts), `DESIGN.md`, and for each item the audit sections and PARITY rows it cites. The code is the ground truth for current behaviour; `main.js` on `main` is the ground truth for Electron behaviour.
3. Push your C1 (if "Contracts you provide" lists any) before anything else, with a commit message starting `C1 contracts:`. When it is green, write `C1 green at <sha>` in Progress.
4. Each cycle, `git fetch origin`. When a contract you need has landed (`git grep -n "<symbol>" origin/rust-rewrite -- <file>`) or `rust-rewrite` has moved, `git merge origin/rust-rewrite` (a merge commit; never rebase or force-push). A conflict in a file you own: resolve it. A conflict in a file you do not own means you edited it by mistake: take `origin/rust-rewrite`'s side and note it in Progress.
5. If CI fails for a reason entirely outside your files (a broken merge on `rust-rewrite`, a runner outage), record it under Blockers with the log line and keep working on what you can verify.

## Shared documents

Never edit `DESIGN.md`, `PARITY.md` or `ARCHITECTURE.md`. In the same commit as the change, append to `.claude/rust-rewrite/entries/windows.md`:

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

- C1 green at 3127827 (`1397118` plus its rustfmt fix). Format, Clippy, Unit tests, Capture check and Desktop selfcheck passed; Build installer failed only on the NSIS download 504, which the coordinator's standing rule in INTEGRATION.md counts as green. Contents: lane marker blocks in both allowlists; K-03 `AppState.queries` with `query_started`/`query_ended`/`query_running`/`query_forget` and `Hooks.abort_query_for`; K-04 `outputs::fresh`, `outputs::close_all`; K-05 `overlay:state.target`/`visibleOutputs`; K-06 `output:meta`; K-07 `Hooks.overlay_extra`; K-22 `ui:text-scale`, `settings:set-text-scale`, `settings:get.textScale`/`osTextScale`; K-23 window channels (shapes in doc comments on each route in ipc.rs); K-24 `history:list.enabled`, `settings:get.overlayPos`. `host/settings.rs` is in the route scan.
- Unit level done (each item's UNIT acceptance test is in the tree and passed CI at c8c2a37 unless noted): B-018, B-019, B-020, B-021, B-022, B-023, B-024, B-025, B-026, B-089, B-090, B-091, B-092, B-093, B-094, B-095, B-096, B-097, B-098, B-099, B-100, B-101, B-102, B-103, B-104, B-105, B-106, B-107 (bbb2d19), B-178, B-179. B-027 (sheet-sized hit-testing, fixed glass, `content_growth_never_sets_bounds`) and K-24 `history:list` rows' `question` land in the commit after c8c2a37. Their SMOKE labels wait on the standing item.
- styleStable (bar's probe, `pill-again WS_EX_NOACTIVATE=false`): fixed in 3c071b0 (the subclass keeps WS_EX_NOACTIVATE and WS_EX_TOOLWINDOW through tao's WM_STYLECHANGING rewrite; UNIT `flags::tests::style_rewrite_keeps_our_bits` uses the probe's sampled values). rust/bar goes green once rust/windows is merged into rust-rewrite.
- B-027 and K-24 `history:list` rows' `question`: green at 8886c02 (rust: every step success, smoke included), which also merges rust-rewrite 075577b. Next step: the standing item (probe labels, see Blockers).
- Note for llm: until the Ask calls `query_started(label, gen)` / `query_ended(label, gen)` and registers `abort_query_for`, `outputs::running_in` treats the single `busy` flag as the target window's Ask (close and reset of another window no longer abort it).
- Note for the integrator: `hud:docked`, `buddy:move` and `settings:data` left both allowlists (B-095; `every_listen_channel_has_a_sender`); main.js still sends `hud:docked` to its own output window, which no page listens to.

## Blockers

(Format: `- B-xxx: <what blocks it>; <why>; <what would unblock it>`.)

- Standing item (TODO, not started; stopped at the coordinator's request so Andy can test): `selftest/probes/windows.rs` still prints nothing and `tools/smoke-labels/windows.txt` lists nothing. Every SMOKE and SMOKE+ label in this brief is still to port (the PARITY list under "Smoke labels you port" plus the 15 new ones). Unit acceptance for every item is in place.
- escKeepsAnswer, perWindowAbort (when the probes are written): the smoke run starts with no `AEVAL_FAKE_LLM` and no key, so a real fake-LLM stream needs rust.yml (not windows-owned) to set `AEVAL_FAKE_LLM=1` and a fake key; without that the probe can only check the window side (query_started, Esc or close, query_running still true).
- B-018 (cleanup, not blocking): `src-tauri/src/host/agent.rs:184` (agent lane) passes `true` to `windows::collapse_panel`, whose `keep_output` argument does nothing since D-01; agent drops the argument, then windows removes the parameter.
