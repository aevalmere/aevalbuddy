# Parity audit: core (LIFE, WIN, POS, KEY, RKEY, TRAY, STORE, FILE)

Branch `rust-rewrite` @ 544f982 against Electron main@069a4b8 (v1.1.1). Read-only audit; nothing was run.

**Summary (150 rows):** DONE 120 · PARTIAL 16 · BUG 4 · MISSING 1 · STUBBED 0 · DEPARTED 9 · UNSURE 0

**Runtime proof, all rows:** none of the in-scope `SMOKE:` / `SMOKE+:` labels is emitted by the Rust build yet.
`src-tauri/src/selftest.rs` (57 lines) is the Phase-1 scaffold: it prints `version`, `userData`, `webviewData`,
`scaffold.pages`, `scaffold.bridge`, then exits. The "Runtime proof needed" column below is therefore outstanding
for every row. The UNIT tests named in the table do exist.

**Marker grep (in-scope modules: lib, instance, paths, crash, windows/*, state, hotkeys, ipc, tray, store/*,
history, notify, autostart, updater, bridge):**
- No `todo!`, `unimplemented!`, `TODO`, `FIXME`, `XXX`, `stub` or `not yet`.
- `ipc.rs:623/631` `not_ported` is the fallback for unrouted channels. The test at `ipc.rs:667` asserts that no allowlisted channel reaches it.
- `#[allow(dead_code)]` appears once, on `store/mod.rs:833` `clear_input_history`. It is unused in Electron too.
- Non-test `expect`, none on IO or locks (the locks are parking_lot, so they cannot poison):
  - `lib.rs:178` is Builder::build.
  - `store/mod.rs:243` and `history.rs:42` are OnceLock values that boot sets before any read.
  - The rest are static regexes: `ipc.rs:278` and `updater.rs:55, 73, 74, 106, 109, 111`.
- Dead state: `state.rs:193` `reduced_motion` is never set. Only `windows/mod.rs:294` reads it.
- `crash.rs:71-73`: the comment says a panic on Tauri's main thread leaves the process running. That is not true (see LIFE-05).

---

## Blocking

These are all the BUG, MISSING and PARTIAL rows, grouped by the Rust file a fix touches. Within each group the most severe row comes first. Groups are ordered by their worst row.

Overall ranking, most severe first:
1. STORE-44
2. POS-09
3. KEY-09
4. LIFE-03
5. WIN-16 / LIFE-12
6. LIFE-01
7. LIFE-05
8. WIN-14
9. POS-11
10. LIFE-10
11. STORE-01/02/40
12. POS-20 / STORE-16
13. POS-02 / POS-05
14. LIFE-16
15. FILE-03
16. LIFE-06

### store/mod.rs

**STORE-44: BUG.** Saves are not serialised, and every save uses the same tmp file.
- Rust: `store/mod.rs:290-302` (`save` clones under the data lock, then writes with no lock held) and `store/mod.rs:943-957` (`write_atomic` always writes `aevalbuddy.json.tmp`).
- Electron: electron-store with write-file-atomic. It is synchronous on a single-threaded main process.
- Failure:
  1. On the first Ask, `renderer/bar.js` fires `bar:history-push` and then `query:submit` without awaiting either.
  2. `aeval_invoke` is async, so both run at once on the tokio pool. `push_input_history` calls `set` → `save`, and `ipc.rs:217-218` `mark_first_request_sent` calls `set` → `save`.
  3. Both `fs::write` the same tmp file through separate handles, each truncating it and writing from offset 0. The result is a lost update, or one snapshot followed by the tail of the longer one: invalid JSON.
  4. The bad file is then renamed over `aevalbuddy.json`. On the next boot `open_with` moves it to `.corrupt` and starts from defaults. All API keys, shortcuts and positions are gone.
  5. The debounced width and size saves (`windows/mod.rs:72` `debounce` runs them on tokio threads) open the same race at any time.
- Fix:
  - Add a `static WRITE: Mutex<()>`. Hold it across snapshot, serialise, write and rename, taking the snapshot inside the lock, so the last writer always writes the newest state and two writes never overlap.
  - Optionally use a per-write tmp name (`aevalbuddy.json.<pid>.<n>.tmp`).
  - UNIT: 8 threads × 100 `set` calls on distinct keys, then reload. The JSON must parse and every key must be present.

**STORE-01, STORE-02, STORE-40: PARTIAL.** After a rollback, a stale OpenRouter key wins.
- Rust: `store/mod.rs:381-398` (`get_provider_key` reads `apiKeys.openrouter` before the v1 fields) and `store/mod.rs:409-436` (mirroring).
- Electron: store.js:145-168.
- Failure:
  1. Rust saves key A. That writes `apiKeys.openrouter` and mirrors A into `apiKeyEncrypted`.
  2. The user rolls back, and Electron clears the key or changes it to B. Electron writes only the v1 fields; `apiKeys` survives as an unknown key.
  3. The user upgrades again. Rust returns A: the cleared key comes back, or the changed key is stale.
- Fix:
  - For `openrouter`, treat the v1 fields as the source of truth: return `legacy_api_key()`.
  - In `migrate()`, rewrite `apiKeys.openrouter` from the v1 fields whenever they differ.
  - UNIT: `apiKeys.openrouter`=enc(A) with `apiKeyEncrypted`=enc(B) returns B. With both v1 fields empty it returns ''.

**STORE-16: PARTIAL.** See POS-20. `outputHeight` is written (`windows/outputs.rs:492`) but never read back.

### windows/mod.rs (+ windows/flags.rs)

**POS-09: BUG.** Collapsing never gives the foreground back.
- Rust: `windows/mod.rs:505-508` calls `set_focusable(false)`, pin, click-through and `show_inactive`, with no blur. Nothing in the tree calls `GetForegroundWindow` or `GW_HWNDNEXT` for this.
- Electron: main.js:635 `overlayWin.blur()`, plus `setFocusable(false)`, whose Windows path calls `Focus(false)` → `Deactivate()` and activates the next visible window.
- Failure:
  1. The user is typing in Notepad, presses Ctrl+Alt+Space, then Escape.
  2. The bar folds to the pill, but the overlay stays the foreground window: WS_EX_NOACTIVATE does not deactivate a window that is already active.
  3. Keystrokes go to the hidden bar until the user clicks Notepad.
  4. The same happens on `exit_hud` → collapse.
- Fix:
  - Add `flags::blur(hwnd)`. If `GetForegroundWindow()==hwnd`, walk `GetWindow(hwnd, GW_HWNDNEXT)` to the first visible, non-tool, non-own-process window and call `SetForegroundWindow` on it.
  - Alternatively, record the foreground window in `trigger_capture` before `show_focused` and restore it if `IsWindow`.
  - Call it before `set_focusable(false)`.
  - Proof: a smoke check that Notepad is foreground after `overlay:collapse` (a new label, e.g. `SMOKE+:collapseReturnsFocus`), alongside `pillWidthHeld` and `collapseClickable`.

**WIN-16: MISSING.** Nothing recovers a dead renderer.
- Rust: no `ProcessFailed` or `with_webview` anywhere in `src-tauri`. Pages are built at `windows/mod.rs:127` (`create_page`), `region.rs:108`, `lens.rs:95` and `buddy.rs:262`.
- Electron: crash.js:88-102 (`render-process-gone` → log, announce once, reload). DESIGN.md:559 says "A renderer that dies is reloaded".
- Failure: a WebView2 renderer crash leaves a blank, always-on-top, click-eating rectangle (pill or bar) for the rest of the session.
- Fix:
  - In `create_page` and each split-c builder, after build: `w.with_webview(|wv| wv.controller().CoreWebView2()?.add_ProcessFailed(..))`.
  - For RENDER_PROCESS_EXITED, RENDER_PROCESS_UNRESPONSIVE or FRAME_RENDER_PROCESS_EXITED: `crash::write("renderer", kind+reason)`, then `crash::announce()`, then `CoreWebView2.Reload()`.
  - For BROWSER_PROCESS_EXITED: log it and recreate the window.
  - Proof: SMOKE+:rendererReload.

**LIFE-12: PARTIAL.** Same gap as WIN-16, for every window.
- Content protection and re-protection on show are present: `flags::show_inactive` / `show_focused`, `desktop/win.rs::show_inactive`, `region.rs:119/129`.
- Missing: no window is crash-guarded. The split-c windows (`region.rs`, `lens.rs`, `buddy.rs`) bypass `create_page` / `flags::adopt`, so the WIN-16 hook has to be added to each builder.
- Electron: main.js:2927-2937.

### hotkeys.rs (+ desktop/client.rs)

**KEY-09: BUG.** Escape is retaken twice as late as it should be, from a detached task with no guard.
- Rust: `desktop/client.rs:1007-1011` already sleeps 60 ms before calling `after_synthetic_key`. Then `hotkeys.rs:402-409` `resume_escape` spawns a task that sleeps another 60 ms, clears `escape_suspended` and re-applies, with no generation check.
- Electron: main.js:2782-2794. lib/desktop/index.js:964-968 awaits `afterSyntheticKey`, so Escape is back before `key()` returns, and the next key's `beforeSyntheticKey` always comes after.
- Failure:
  1. The agent sends two Escape-bearing keys within 60 ms of the first `key()` returning.
  2. The second `suspend_escape` finds nothing to unregister.
  3. At about 120 ms the first resume task re-registers the global Escape while the second injection is in flight.
  4. RegisterHotKey fires on the injected Escape, `on_escape` runs, and the agent stops itself.
- Fix:
  - Make `resume_escape` synchronous: no spawn and no sleep, because the client already waited 60 ms.
  - Replace the bool with a depth counter. Suspend increments it; resume decrements it and calls `apply` only when it reaches 0.
  - Proof: SMOKE+:escapeSuspend.

### lib.rs (+ windows/flags.rs, windows/screen.rs)

**LIFE-03: BUG.** The forced shot scale reaches the window maths but not WebView2.
- Rust: `windows/screen.rs:85` `forced_scale()` is the only consumer. `windows/flags.rs:312` `scale_of` uses the real `GetDpiForWindow`. There is no `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` and no `additionalBrowserArgs` in src-tauri, .github or tools.
- Electron: main.js:36-38 (`--force-device-scale-factor`).
- Failure: in the shots125 and shots150 runs, windows are sized for 1.25 or 1.5 while the page renders at the runner's real 1.0. The hit-test bands, WM_GETMINMAXINFO limits and forwarded mousemove client coordinates also use 1.0. The scale matrix compares mis-scaled pages, and the exit code can still pass.
- Fix:
  - First thing in `run()`: if `AEVAL_SHOT_SCALE` parses to a value above 0, append `--force-device-scale-factor=<s>` to `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` (keeping any existing value) before any webview exists.
  - Make `screen::forced_scale` pub, and have `flags::scale_of` return it when set.
  - Proof: INSTALL:shots125.exitCode, shots150.exitCode, plus an eye check of the SHOT set.

**LIFE-05: PARTIAL.** A panic on the main thread ends the app.
- Rust: `crash.rs:74` installs the panic hook first (correct). But nothing catches a panic in a main-thread callback: `lib.rs:122-131` (`on_page_load`), `lib.rs:131` (`on_window_event`), the `run_on_main_thread` closures, and the tray and menu handlers.
- Rust 1.81+ aborts when a panic unwinds out of an `extern "system"` wndproc. tao's runner instead catches the panic and resumes it out of `run`. Either way the process ends, after the hook has logged it.
- Electron: main.js:40-41 and crash.js:72-81. Errors are survived (DESIGN.md:555-556).
- Fix:
  - Add `crash::guard(scope, f)` = `catch_unwind(AssertUnwindSafe(f))`, logging and announcing on Err.
  - Wrap every main-thread entry point with it, and correct the comment at `crash.rs:71-73`.
  - Proof: SMOKE:crashLogged, plus a main-thread variant.

**LIFE-06: PARTIAL (trivial).** The failed-shortcut warning is missing.
- Rust: `lib.rs:150`.
- Electron: main.js:2761-2922. After `applyRegistrations` it does `console.warn` when any shortcut failed.
- Fix: after `hotkeys::apply`, `eprintln!` the actions whose result is `Some(false)`.
- The rest of the boot order matches. The backdrop starts earlier, in `desktop::setup`, which is harmless.

### instance.rs (+ lib.rs)

**LIFE-01: PARTIAL.** A second launch opens the bar without keyboard focus.
- Rust: `instance.rs:57-68` `signal` never calls `AllowSetForegroundWindow`. The first instance's `show_focused` → `SetForegroundWindow` (`flags.rs:~262`) is then refused by the foreground lock: the bar appears, but typing goes to the previous app.
- Electron: main.js:2753-2759, through Chromium's ProcessSingleton, which calls `AllowSetForegroundWindow` for the running process before signalling.
- Second gap: `signal` retries for only 2 s (20 × 100 ms). The pipe is served only in `setup` (`lib.rs:140`, after `windows::boot`), so a second launch while the first is still booting is dropped.
- Fix:
  - Call `AllowSetForegroundWindow(ASFW_ANY)` in `signal` before writing.
  - Retry for as long as the bootMs gate allows (15 s), or create the pipe server before `Builder::build` and queue the "open" until setup is done.
  - Proof: SMOKE+:singleInstance. It should also assert that the overlay is foreground.

### bridge/aeval-bridge.js (+ ipc.rs, host/menu.rs)

**WIN-14: PARTIAL.** The editing context menu does not match.
- Rust: `bridge/aeval-bridge.js:89-91` prevents every context menu except on editable targets, which get WebView2's native menu. A selection outside a field gets no menu at all.
- Electron: main.js:384-404. Fields get Cut/Copy/Paste/—/Select all; a selection gets Copy/Select all.
- Fix:
  - Bridge: if the page did not handle the event, always call preventDefault. Then send `invoke('ui:context-menu', {kind:'edit'})` for an editable target, or `{kind:'selection'}` when the trimmed `getSelection()` is non-empty.
  - `host/menu.rs:130` `items_for`: map those kinds to `PredefinedMenuItem` cut/copy/paste/separator/select_all.
  - Expose the template as a data function for SMOKE+:contextMenuEditable.
  - `ipc.rs:235` already routes `ui:context-menu`.

### windows/outputs.rs (+ windows/layout.rs)

**POS-11: PARTIAL.** A resize mid-glide starts from the live rect, not the target.
- Rust: `windows/outputs.rs:223-238` reads the live mid-glide glass rect and returns early if the height is within 1 px. Otherwise it animates from the mid-glide x and y.
- Electron: main.js:736-748.
- Failure:
  - A height report arriving while the glide passes through that same height is dropped, and the window lands on the old target (the "stopped growing" bug class).
  - A streaming answer's `output:resize` during a dodge (`host/agent.rs:370`) freezes x mid-dodge, so the window stays partly under the agent's target (POS-23).
- Fix: when `motion::is_animating(h)`, take `have` (x, y, height) from the stored in-flight target `o.rect`, not from `motion::glass(h)`.

**POS-20 + STORE-16: PARTIAL.** The last dragged height is not used for the next new window.
- Rust: `windows/outputs.rs:140-150` (`size_for` reuses the stored width but never the height) and `outputs.rs:485-492` (`on_user_resize` stores both). New windows start with `user_height = 0`.
- Electron: main.js:981-994 and store.js:331-340. PARITY §26 says "the last size is used for the next new window".
- Failure: an Electron user's saved `outputHeight` is ignored after the upgrade. A height dragged in one window is not applied to the next.
- Fix: in `create` and `show`, seed `user_height` from `store::get().get_output_height()` when it is above 0. New chat (`reset`) keeps zeroing it.

**POS-02 + POS-05: PARTIAL.** The output minimum height is 160, not 90, and DESIGN.md does not record it.
- Rust: `windows/layout.rs:15` `OUTPUT_MIN_HEIGHT = 160`, used at `layout.rs:99-101` and in `outputs.rs` meta.
- Electron: main.js:96-151 and 323-336 (90).
- Status of the departure: ARCHITECTURE.md:447 states "min size 320x160", but DESIGN.md has no departure paragraph. `OUTPUT_GAP` 12 (instead of DOCK_GAP 8) is recorded in DESIGN.md "The glob".
- Missing test: there is no constants-table UNIT test for POS-02.
- Fix: either restore 90, or add a DESIGN.md departure with its reason and strike POS-02/05. Add `#[test] fn constants_match_main_js()`.

### host/mod.rs

**LIFE-10: PARTIAL.** Quitting stops the agent with no reason.
- Rust: `lib.rs:188-189` calls the `stop_agent` hook, which is `host/mod.rs:362` → `services().agent.stop(None)`. The string "Stopped because AevalBuddy quit." appears nowhere.
- Electron: main.js:2945-2949.
- What already works: Find closes and the helper is killed (`lib.rs:191, 195`).
- Fix: give the hook a reason argument, or add a `stop_agent_quit` hook that calls `agent.stop(Some("Stopped because AevalBuddy quit."))`.
- Proof: UPDATE.

### desktop/checks.rs

**LIFE-16: PARTIAL.** The Notepad checks run by default.
- Rust: `desktop/checks.rs:529-531` sets `notepad = AEVAL_DESKTOP_NOTEPAD != "0"` and defaults to **true** when the variable is unset.
- Electron: main.js:3837 and 3910 run them only when `GITHUB_ACTIONS` or `AEVAL_DESKTOP_NOTEPAD` is set.
- Failure: `AEVAL_DESKTOP_CHECK=1` on a real desktop opens Notepad and types into it.
- Fix: `notepad = GITHUB_ACTIONS is set || (AEVAL_DESKTOP_NOTEPAD is set && != "0")`.
- Proof: RUNNER.

### history.rs

**FILE-03: PARTIAL (low).** The write timing differs from Electron and the writes can overlap.
- Rust: `history.rs:109-121` debounces on the **last** change; history.js throttles from the first. A steady stream of saves less than 400 ms apart postpones the write indefinitely, so a crash loses more.
- Rust: `history.rs:125-142`. `flush` from a timer thread and `flush_now` at Exit can write `history.json.tmp` at the same moment (the STORE-44 pattern, at quit only).
- Electron: history.js:34-84.
- Fix: schedule a write only if none is pending, which is a throttle. Serialise `flush` with a write mutex.

---

## Full table

### 1. LIFE

| ID | Status | Rust location | Evidence/notes | Runtime proof needed |
|---|---|---|---|---|
| LIFE-01 | PARTIAL | instance.rs:33 acquire, :57 signal, :71 serve; lib.rs:101,140 | Mutex per sha(path), pipe "open". UNIT lock_is_per_folder_and_case_blind. No AllowSetForegroundWindow; 2 s retry window | SMOKE+:singleInstance |
| LIFE-02 | DONE | paths.rs user_data_dir | UNIT user_data_is_electrons_folder, aeval_userdata_overrides, blank_override_is_ignored | UNIT + installer-check |
| LIFE-03 | BUG | windows/screen.rs:85; flags.rs:312 | Window maths only; WebView2 not forced; scale_of real DPI | INSTALL:shots125/150.exitCode + SHOT |
| LIFE-04 | DONE | notify.rs set_app_user_model_id; lib.rs:96 | SetCurrentProcessExplicitAppUserModelID("com.aevalbuddy.app") first in run() | SMOKE+:aumid |
| LIFE-05 | PARTIAL | crash.rs:74; lib.rs:99 | Hook first, process-wide. UNIT log_lines_and_trimming. Main-thread panic ends process | SMOKE:crashLogged |
| LIFE-06 | PARTIAL | lib.rs:132-175 | Order matches; failed-shortcut warn missing (trivial) | SMOKE:windows, tray, bootMs |
| LIFE-07 | DONE | desktop/mod.rs:32; host/ask.rs:151 | setup does not start helper; first prefetch starts it | SMOKE:helperNotAtBoot |
| LIFE-08 | DONE | lib.rs:95 mark_start; windows/mod.rs:158 | boot_ms recorded on first overlay load, then show_inactive | SMOKE:bootMs |
| LIFE-09 | DONE | lib.rs:182 (Tauri default) | Last window closing exits; no prevent_exit | SMOKE+:quitOnLastClose |
| LIFE-10 | PARTIAL | lib.rs:188-195; host/mod.rs:362 | Stop reason missing; find closed, helper killed | UPDATE |
| LIFE-11 | DONE | lib.rs:192-198 | unregister_all, history flush_now, updater destroy, tray destroy | SMOKE+:historyFlushOnQuit |
| LIFE-12 | PARTIAL | windows/mod.rs:127; region.rs:108; lens.rs:95; buddy.rs:262 | Protected + re-protect on show; no crash guard; split-c bypass create_page | SMOKE contentProtection, windowsChecked |
| LIFE-13 | DONE | host/mod.rs:61; updater.rs:471; llm/client.rs:271 | Offline forced. Nit: updater.rs:472 treats AEVAL_OFFLINE="" as offline (Electron: online) | SHOT:27, SMOKE:findOffline |
| LIFE-14 | DONE | host/ask.rs:79,95,135 | Exact text and bar context gated (main.js:1810,2027). No unit test | UNIT (missing) |
| LIFE-15 | DONE | llm/fake.rs:23-26,117,190 | Embedded agent/find fixtures, {{var}}, delayMs abortable, request log; unit tests present | DIFF + smokes |
| LIFE-16 | PARTIAL | desktop/checks.rs:529 | Notepad on by default when unset | RUNNER |
| LIFE-17 | DONE | updater.rs:471 | InternetGetConnectedState (row allows it); no error path to map to true. No unit test | UNIT (missing) |
| LIFE-18 | DONE | flags.rs:190-202; ipc.rs settings_data canHide/winBuild | Registry CurrentBuildNumber >= 19041 | SMOKE:captureProtection |
| LIFE-19 | DONE | windows/mod.rs:358-383; lib.rs:168 | 4 s re-pin of overlay, settings, outputs, lens-*; skips lowered; off under quiet_run | SMOKE:topmostHolds |
| LIFE-20 | DONE | windows/motion.rs reduced() | SPI_GETCLIENTAREAANIMATION read per move. That is the same OS bit Chromium maps prefers-reduced-motion from, so no separate renderer read is needed. state.reduced_motion is dead | SMOKE+:reducedMotion |

### 2. WIN

| ID | Status | Rust location | Evidence/notes | Runtime proof needed |
|---|---|---|---|---|
| WIN-01 | DONE | flags.rs pin_on_top/show_inactive/show_focused; windows/mod.rs:358 | HWND_TOPMOST after focus changes and every 4 s | SMOKE:overlayOnTop, outputOnTop, topmostHolds |
| WIN-02 | DONE | flags.rs protect; windows/mod.rs:127 content_protected(true) | WDA_EXCLUDEFROMCAPTURE when build >= 19041 | SMOKE:captureProtection, contentProtection |
| WIN-03 | DONE | flags.rs adopt/set_tool_window | WS_EX_TOOLWINDOW. Smoke value must stay `true:toolbar` (no DESIGN.md entry) | SMOKE:taskbarHidden |
| WIN-04 | DONE | windows/mod.rs:337 boot | Pill+margin, Resize::None, click-through + poll, shown inactive on load. Menu: see WIN-14 | SHOT:01, SMOKE:collapseClickable |
| WIN-05 | DEPARTED | windows/outputs.rs create/create_spare/on_loaded | Several `output-<n>` windows: REQUESTS 11, DESIGN.md "Output windows", NEW-01/02. chat:sync on reload: UNIT chat_sync_sends_the_whole_transcript | SHOT:04, SMOKE+:outputMulti |
| WIN-06 | DONE | windows/mod.rs:883-930, 992 (160 ms), 1060 | Width min/max 380-900, height 360..wa-40, section jump, capture mode off + re-apply + tour on close. Width unclamped, as in Electron | SHOT:08-10, SMOKE:settingsWindow |
| WIN-07 | DONE | windows/mod.rs:930, 992 | Same frame; saved 50 ms debounced (Electron: every resize) | SHOT:16 |
| WIN-08 | DONE | region.rs:108-145 | Per pick, display rect, focusable, blur cancels. No backgroundThrottling flag; window is foreground, so no effect | SMOKE:regionSelectorCancels, SHOT:17 |
| WIN-09 | DONE | lens.rs:95-126 | Display-sized, focusable(false) + WS_EX_NOACTIVATE, ignore cursor, protected on show | SMOKE:lensHits |
| WIN-10 | DONE | buddy.rs:262-322 | Not focusable, click-through + forwarding, show_inactive (protects) | SHOT:11, SMOKE:buddyPoints |
| WIN-11 | DONE | windows/stage.rs | inflate/deflate/limit (0 stays 0). UNIT margin_round_trip | SMOKE:answerFollowsTranscript, panelWidthRange |
| WIN-12 | DONE | ipc.rs route glass:passthrough | Outputs only, never in HUD | SMOKE:collapseClickable |
| WIN-13 | DONE | flags.rs set_limits + subclass WM_GETMINMAXINFO | Resizable never toggled | SMOKE:pillPinned, panelHeightPinned |
| WIN-14 | PARTIAL | bridge/aeval-bridge.js:89-91; host/menu.rs:130 | No selection menu; fields get WebView2's native menu | SMOKE+:contextMenuEditable |
| WIN-15 | DONE | (grep) | No SetLayeredWindowAttributes or opacity anywhere | SMOKE:noSetOpacity |
| WIN-16 | MISSING | none | No ProcessFailed handler | SMOKE+:rendererReload |
| WIN-17 | DONE | windows/mod.rs:858, 978, 374 | Settings/History not topmost after open_url; re-pinned on focus; keep_on_top skips lowered | SMOKE+:lowerForBrowser |

### 3. POS

| ID | Status | Rust location | Evidence/notes | Runtime proof needed |
|---|---|---|---|---|
| POS-01 | DONE | windows/layout.rs PILL | 44×44 | SHOT:01 |
| POS-02 | PARTIAL | windows/layout.rs:1-40 | All match except OUTPUT_MIN_HEIGHT 160 (not in DESIGN.md). Gap 12 is in DESIGN.md. No constants test | UNIT (missing) |
| POS-03 | DONE | layout.rs bounds_for_size | UNIT bounds_hug_and_clamp | UNIT |
| POS-04 | DEPARTED | layout.rs docked_bounds (unused for layout) | NEW-01 / REQUESTS 11 | SMOKE+:outputFloating |
| POS-05 | PARTIAL | layout.rs:95-102 | Per window; min 160 instead of 90 | SMOKE:answerHeightUnderLongChat, outputAutoHeight |
| POS-06 | DONE | layout.rs pos_from_rect | UNIT pos_from_rect_and_snap | UNIT |
| POS-07 | DONE | layout.rs find_snap | UNIT pos_from_rect_and_snap | SMOKE:hudSnapFound |
| POS-08 | DONE | windows/mod.rs:445 expand_panel | State first, bar size, click-through off, focusable, pin, show_focused, re-sync after T_EXPAND, apply | SMOKE:mode, SHOT:03 |
| POS-09 | BUG | windows/mod.rs:481-539 | No blur: foreground stays on the collapsed bar. T_COLLAPSE+30 / 0 and generation guard are right | SMOKE:pillWidthHeld, collapseClickable + focus check |
| POS-10 | DONE | windows/outputs.rs show | First show sets glass and shows inactive; visible windows glide; hide immediate. Glob per DESIGN.md | SHOT:04 |
| POS-11 | PARTIAL | windows/outputs.rs:207-239 | Takes the mid-glide rect: drops reports and cuts dodges short | SMOKE:answerFollowsTranscript |
| POS-12 | DEPARTED | n/a | NEW-01 (no docking) | SMOKE+:outputFloating |
| POS-13 | DONE | windows/mod.rs:250-323 | Every field plus glassIntensity, hasSentFirstRequest etc. hotkeyFailed is false before first apply (Electron: false). No serde snapshot test | UNIT (missing), SHOT |
| POS-14 | DEPARTED | windows/mod.rs:740 enter_hud, :792 exit_hud | Output ghost removed (NEW-04, REQUESTS 11); bar half implemented | SMOKE:hudMode, hudExitMode, storedPos |
| POS-15 | DONE | windows/mod.rs:814 settle_hud_drag | 160 ms settle after the last move; mark_app guard. Output ghost follow departed | SMOKE+:hudDragSettle |
| POS-16 | DEPARTED | n/a | NEW-04 | SMOKE+:noOutputLayout |
| POS-17 | DEPARTED | ipc.rs hud:set-dock no-op | NEW-04; PARITY §26 keeps the channel as a no-op | SMOKE+:noOutputLayout |
| POS-18 | DONE | windows/mod.rs:836 hud_nudge; layout.rs nudge | UNIT nudges_step_the_grid | SMOKE+:hudNudge |
| POS-19 | DONE | windows/mod.rs:992 on_resized (overlay) | 160 ms; skipped if the app moved it or a glide runs; width only | SMOKE:panelWidthRange, panelWidthHeld, widthAfterCycle |
| POS-20 | PARTIAL | windows/outputs.rs:140-150, 485-492 | Per-window height works; stored height never seeds new windows | SMOKE:outputAutoHeight |
| POS-21 | DONE | windows/mod.rs:337, 387 apply_input_limits | Pill min=max=PILL; width saved only in Panel | SMOKE:pillWidthHeld, pillPinned, widthDefaults |
| POS-22 | DONE | windows/motion.rs | Bezier, one animation per window, takeover, reduced jumps. UNIT glide_curve_matches_the_browser, lerp_and_shrink | SMOKE:motionAnchor |
| POS-23 | DONE | host/agent.rs:370; layout.rs dodge_x | Matches dodgeAnswer for the run's window. UNIT dodge. A resize can cut it short (POS-11) | SMOKE+:dodge |
| POS-24 | DONE | store sanitize_pos; screen.rs | Stored values stay DIP; fractional DIP work area | MIGRATE |

### 4. KEY

| ID | Status | Rust location | Evidence/notes | Runtime proof needed |
|---|---|---|---|---|
| KEY-01 | DONE | store/mod.rs defaults() | UNIT fresh_file_gets_every_default_in_order | SMOKE:shortcuts, shortcut.captureQuery |
| KEY-02 | DONE | windows/mod.rs:684 on_hotkey → open_bar(None) | | SMOKE:barOpensUnlocked |
| KEY-03 | DONE | windows/mod.rs:684 | open_bar(Some("find")) | SMOKE:barKeys |
| KEY-04 | DONE | windows/mod.rs:684 | Trail (unfocused) while running | SMOKE+:agentHotkeyTrail |
| KEY-05 | DONE | windows/mod.rs:684; host start_region hook | | SMOKE+:regionHotkey |
| KEY-06 | DONE | windows/mod.rs:604 toggle_visibility | Hides overlay and outputs, cancels tour, tray refresh | SMOKE+:toggleVisibility |
| KEY-07 | DONE | hotkeys.rs:309-342 apply | Escape only for agent/buddy/region/HUD, never Enter | SMOKE:noGlobalEnter, noGlobalEscWithBar, keySwallow |
| KEY-08 | DONE | windows/mod.rs:640 on_escape | Priority order matches main.js:1311-1356 | SMOKE:regionEscape |
| KEY-09 | BUG | hotkeys.rs:402-409; desktop/client.rs:1007-1011 | 120 ms detached resume, no generation guard | SMOKE+:escapeSuspend |
| KEY-10 | DONE | hotkeys.rs register/apply/main_chord_failed | true/false/null; parse error counts as false. UNIT unparseable_is_an_error_not_a_panic | SMOKE:shortcut.* |
| KEY-11 | DONE | hotkeys.rs set_capture_mode | apply skips everything in capture mode | SMOKE+:captureMode |
| KEY-12 | DONE | hotkeys.rs:418-464 | Messages verbatim. UNIT validation_matches_main_js | UNIT + SMOKE+:shortcutSet |
| KEY-13 | DONE | hotkeys.rs reset | | SMOKE+:shortcutReset |
| KEY-14 | DONE | hotkeys.rs parse/modifier/key_code | Electron syntax, AltGr = Ctrl+Alt, stored strings untouched. UNIT electron_forms | UNIT + MIGRATE |
| KEY-15 | DONE | hotkeys.rs action_label | UNIT labels | UNIT |

### 5. RKEY

| ID | Status | Rust location | Evidence/notes | Runtime proof needed |
|---|---|---|---|---|
| RKEY-01 | DONE | renderer/bar.js; bridge:93-107 | bar.js diff has no key changes beyond waiting-run Enter (NEW-06). Bridge blocks only browser chords; editing, Ctrl+1..3, F6, Alt+Down pass | SMOKE:barKeys |
| RKEY-02 | DONE | renderer/intent.js | Unchanged | SMOKE:barKeys |
| RKEY-03 | DONE | renderer/output.js | Key handlers unchanged | SMOKE:barKeys |
| RKEY-04 | DONE | renderer/overlay.js | Unchanged except HUD mousemove passthrough | SMOKE:barKeys |
| RKEY-05 | DONE | renderer/region.js:107-140 | Unchanged | SMOKE:regionEscape |
| RKEY-06 | DEPARTED | renderer/trail.js:197-200; bar.js onEnter | Trail input removed, answered in the bar: NEW-06, REQUESTS 9, DESIGN.md "Output windows" | SMOKE:agentTakeover |
| RKEY-07 | DONE | settings/settings.js:817-900 | Recorder unchanged; runs in capture phase, before the bridge | SMOKE+:shortcutSet |
| RKEY-08 | DONE | history/history.js | No key changes in the diff | SHOT:16 |

### 6. TRAY

| ID | Status | Rust location | Evidence/notes | Runtime proof needed |
|---|---|---|---|---|
| TRAY-01 | DONE | tray.rs ICON_16/ICON_32 | Same base64. One image picked at primary scale >= 1.5 instead of multi-rep, so slightly soft at 125%. UNIT icons_decode | SMOKE:tray |
| TRAY-02 | DONE | tray.rs init | Left-up and double-click call trigger_or_trail | SMOKE+:trayClick |
| TRAY-03 | DONE | tray.rs template/labels | UNIT idle_order, ready_running_hidden | SMOKE tray order |
| TRAY-04 | DONE | tray.rs labels | Accelerator after a tab, not registered | SMOKE+:trayAccel |
| TRAY-05 | DONE | tray.rs check_item, :305 run_check | UNIT check_states. Nit: the 10 s reset has no timer id, so a second check can shorten "Up to date" | SMOKE+:trayCheckStates |
| TRAY-06 | DONE | tray.rs tooltip | UNIT tooltips | SMOKE+:trayTooltip |
| TRAY-07 | DONE | hotkeys apply; windows/mod.rs:604; updater.rs:452; host/agent.rs:199,300,498 | | covered |
| TRAY-08 | DONE | tray.rs on_menu → windows/mod.rs:878 open_help | Lowers Settings/History first | UNIT |

### 9. STORE

Every row also needs MIGRATE (round trip), which does not exist yet.

| ID | Status | Rust location | Evidence/notes | Runtime proof needed |
|---|---|---|---|---|
| STORE-01 | PARTIAL | store/mod.rs:381-436 | Mirrored on write; after a rollback the stale apiKeys.openrouter wins. UNIT provider_keys_encrypt_and_mirror_openrouter, electron_key_is_migrated | MIGRATE |
| STORE-02 | PARTIAL | store/mod.rs:401-436 | Plain fallback and mirror OK; same rollback issue | MIGRATE |
| STORE-03 | DONE | store chosen() | UNIT chosen_flag_rules | MIGRATE+UNIT |
| STORE-04 | DONE | store defaults | | MIGRATE |
| STORE-05 | DONE | store brain_enabled | UNIT chosen_flag_rules | MIGRATE+UNIT |
| STORE-06 | DONE | store defaults | | MIGRATE |
| STORE-07 | DONE | store chosen("brain") | | MIGRATE+UNIT |
| STORE-08 | DONE | store defaults | | MIGRATE |
| STORE-09 | DONE | store show_paid | | MIGRATE |
| STORE-10 | DONE | store skill | auto or an existing id | MIGRATE+UNIT |
| STORE-11 | DONE | store shortcuts | Merged over defaults | MIGRATE+UNIT |
| STORE-12 | DONE | store sanitize_pos, overlay_pos | UNIT sanitisers_match_lib_store | MIGRATE+UNIT |
| STORE-13 | DEPARTED | store output_pos / set_output_pos | PARITY §26: read for migration only (NEW-01). Writes `docked:false`, so a rollback shows Electron's output undocked at that position | MIGRATE |
| STORE-14 | DONE | store clamp_w | 320..900 | MIGRATE+UNIT |
| STORE-15 | DONE | store clamp_w | | MIGRATE+UNIT |
| STORE-16 | PARTIAL | store output_height; outputs.rs:492 | Written but never read (POS-20) | MIGRATE |
| STORE-17 | DONE | store settings_size | Clamps; 460×470 fallback kept | MIGRATE+UNIT |
| STORE-18 | DONE | store history_size | | MIGRATE+UNIT |
| STORE-19 | DONE | store toggles; lib.rs:105 | | MIGRATE |
| STORE-20 | DONE | store decide_presence; autostart.rs | UNIT presence_and_glass | MIGRATE |
| STORE-21 | DONE | store surface | solid → glassIntensity 0 (NEW-12). UNIT solid_profiles_start_with_no_glass | MIGRATE |
| STORE-22 | DONE | store decide_presence | UNIT presence_and_glass | MIGRATE |
| STORE-23 | DONE | store take_presence_notice | | MIGRATE |
| STORE-24 | DONE | store walkthrough; lib.rs:154 | | MIGRATE |
| STORE-25 | DONE | store migrate | | MIGRATE |
| STORE-26 | DONE | store input history | Strings only, last 50. UNIT input_history. Item length counted in chars, not UTF-16 (nit) | MIGRATE+UNIT |
| STORE-27 | DONE | store find model | UNIT find_and_agent_settings | MIGRATE+UNIT |
| STORE-28 | DONE | store defaults | | MIGRATE |
| STORE-29 | DONE | store agent model | | MIGRATE+UNIT |
| STORE-30 | DONE | store defaults | | MIGRATE |
| STORE-31 | DONE | store find settings | UNIT find_and_agent_settings | MIGRATE+UNIT |
| STORE-32 | DONE | store find settings | true → auto, false → off | MIGRATE+UNIT |
| STORE-33 | DONE | store find settings | Not-false is true | MIGRATE+UNIT |
| STORE-34 | DONE | store agent settings | | MIGRATE+UNIT |
| STORE-35 | DONE | store agent settings | 10..50 | MIGRATE+UNIT |
| STORE-36 | DONE | store agent settings | | MIGRATE |
| STORE-37 | DONE | store agent settings | | MIGRATE |
| STORE-38 | DONE | store agent settings | | MIGRATE |
| STORE-39 | DONE | store/mod.rs:313-360 migrate | UNIT v2_shortcut_migration | MIGRATE (v1.0 fixture) |
| STORE-40 | PARTIAL | store/crypto.rs; store/mod.rs:381 | OSCrypt v10/GCM/Local State exact, byte-identical to Chromium (UNIT opens_a_blob_sealed_by_node, seals_byte_for_byte_like_chromium, wrong_key_or_garbage_is_none_not_a_panic, local_state_is_created_then_reused). Partial only for the rollback precedence | MIGRATE |
| STORE-41 | DEPARTED | store/mod.rs:471 | True when any provider has a key: REQUESTS 2, ARCHITECTURE 11.4 (no DESIGN.md paragraph yet) | UNIT |
| STORE-42 | DONE | store heal_*; host/settings.rs:34-46 | Chosen → rewritten; default → in memory | SMOKE:healRetired |
| STORE-43 | DONE | store open_with/save | Whole map written. UNIT unknown_keys_survive_and_values_win_over_defaults | MIGRATE |
| STORE-44 | BUG | store/mod.rs:290-302, 943-957 | Concurrent saves share one tmp; can corrupt the file | UNIT (concurrency, missing) |

### 10. FILE

| ID | Status | Rust location | Evidence/notes | Runtime proof needed |
|---|---|---|---|---|
| FILE-01 | DONE | paths.rs user_data_dir | %APPDATA%\aevalbuddy. UNIT user_data_is_electrons_folder | MIGRATE |
| FILE-02 | DONE | store open_with/write_atomic | Tab JSON, BOM strip. Corrupt → `.corrupt` and continue (Electron threw). UNIT corrupt_file_is_kept_aside | MIGRATE |
| FILE-03 | PARTIAL | history.rs:109-142 | Format and tmp+rename right; debounce instead of throttle; flush/flush_now tmp overlap | SMOKE:historySaved + MIGRATE |
| FILE-04 | DONE | crash.rs write_to | `[ISO] scope\ndetail\n\n`, 256 KB → 128 KB. UNIT log_lines_and_trimming | SMOKE:crashLogged |
| FILE-05 | DONE | updater.rs:35, 415-433 | `[ISO] level message`, same trim | SMOKE:update.cause |
| FILE-06 | DONE | updater.rs:493-511 | {lastChecked, lastRunVersion} | UPDATE |
| FILE-07 | DONE | store/crypto.rs load_or_create; paths.rs webview_data_dir | Key only created when missing, never deleted; WebView2 elsewhere | MIGRATE |
| FILE-08 | DONE | desktop/client.rs:132-145 | Cache only in debug or AEVAL_DESKTOP_CACHE (Electron: unpackaged) | SMOKE:desktopReady |
| FILE-09 | DONE | tauri.conf.json:21-22; desktop/client.rs:151-178 | DLL bundled to `desktop/`; SHA compiled in from AEVAL_DESKTOP_DLL_SHA (release.yml:99, rust.yml:78). Unset → compile in memory | SMOKE:desktopReadyMs |
| FILE-10 | DONE | updater.rs:39-43 feed_config | github/aevalmere/aevalbuddy/private false. UNIT asserts owner | SMOKE:updateConfig |
| FILE-11 | DONE | skills.rs:32 ORDER, :178 load; host/mod.rs:289 | Built-ins compiled in; drop-in folder `<exe>\skills`. UNIT order (skills.rs:267) | SMOKE:skills |
| FILE-12 | DONE | llm/fake.rs:23-26 | agent/find jsonl embedded (catalogue stays real, as in Electron) | SMOKE (v2desktop) |
| FILE-13 | DONE | paths.rs webview_data_dir | %LOCALAPPDATA%\aevalbuddy\WebView2. UNIT webview_profile_is_not_in_the_userdata_root | RUNNER |
| FILE-14 | DONE | (grep of fs writes) | Only store, history, logs, updater.json, Local State, pending updates (LOCALAPPDATA), WebView2 profile, CI-only temp file. Frames served no-store | RUNNER (profile diff) |
