# Platform lane entries

Folded into DESIGN.md, PARITY.md and ARCHITECTURE.md by the merge agent.

## PARITY · STORE-14 · D-16, B-087
Replace "| STORE-14 | `inputWidth` | 420 | Clamped 320..900. | store.js:79,302-318 |" with:
| STORE-14 | `inputWidth` | 600 on a new profile (D-16; Electron 420). A stored width is kept. | Clamped 320..900. | store.js:79,302-318 |
status: done; evidence: UNIT store::tests::fresh_file_gets_every_default_in_order, store::tests::stored_input_width_is_kept

## DESIGN · Spacing and radius · D-16
Add:
New profiles start with a 600 px bar (the `inputWidth` default; windows' `PANEL_WIDTH` matches). At 420 px the close button was hidden and a question had about 200 px after the token and the icons; Spotlight is about 680 px and Raycast 750. A width a person already stored is kept.

## PARITY · STORE-44 · B-001, B-002
Replace "| STORE-44 | Write is atomic (electron-store uses write-file-atomic). | electron-store | UNIT |" with:
| STORE-44 | Write is atomic and durable: one save at a time with the snapshot taken under the save lock, a unique temp name, fsync, then `MoveFileExW(REPLACE_EXISTING \| WRITE_THROUGH)`; the previous good file is kept as `aevalbuddy.json.bak`. A file that does not parse is copied to `.corrupt` and `.bak` is used in its place; a file that exists but cannot be read (after three retries) is never written over that session; invalid UTF-8 is decoded lossily, as Node did. | electron-store | UNIT |
status: done; evidence: UNIT store::tests::concurrent_sets_never_lose_or_corrupt, store::tests::parse_failure_recovers_from_bak, store::tests::invalid_utf8_is_read_lossy_not_reset, store::tests::unreadable_file_disables_saving, durable::tests::replaces_and_leaves_no_temp

## PARITY · STORE-40 · B-004
Replace "Decrypt failure -> '' (user re-enters), never a crash." with:
Decrypt failure -> '' (user re-enters), never a crash. A new key is minted only when `Local State` is missing or is a JSON object with no `os_crypt.encrypted_key`; a file that cannot be read or parsed is left alone (copied to `Local State.corrupt`) and keys are stored in plain text that session. A UTF-8 BOM is skipped. The file is rewritten durably with every other key kept.
status: done; evidence: UNIT crypto::tests::unreadable_local_state_is_left_alone, crypto::tests::bom_local_state_keeps_its_key, crypto::tests::local_state_is_created_then_reused

## PARITY · FILE-03 · B-003, D-12
Replace "written via `.tmp` then rename; 400 ms debounce." with:
written durably (unique temp, fsync, write-through replace) 400 ms after the first change by one writer thread (a throttle, as history.js:55-63 did; later changes ride along); a file that does not parse is renamed to `history.json.corrupt-<unix ms>` first.
status: done; evidence: UNIT history::tests::corrupt_file_is_kept_aside_not_overwritten, history::tests::steady_saves_still_write_within_400ms, history::tests::flush_now_and_timer_never_overlap

## PARITY · HIST-05 · B-003
Replace "| HIST-05 | Unreadable/missing file = empty list, never a failed start. Write failures ignored. |" with:
| HIST-05 | Missing file = empty list; a file that does not parse is renamed aside, then an empty list; a file that exists but cannot be read = empty list and no writes this session. Never a failed start. A failed write is retried on the next change and on quit. |
status: done; evidence: UNIT history::tests::corrupt_file_is_kept_aside_not_overwritten, history::tests::unreadable_file_is_never_written

## PARITY · HIST-06 · D-12
Add (note to the row):
D-12: history writes throttle from the first change (400 ms, never postponed by later changes) on one long-lived writer thread; `flush_now` on quit takes the same write lock, so the two never overlap. Reason: Electron's history.js throttled; the Rust port had become a debounce that a steady stream of saves could postpone forever, with a thread per change.

## PARITY · KEY-09 · B-011
Replace "While the engine injects a synthetic Escape, the global Escape is unregistered and retaken 60 ms after (RegisterHotKey fires on injected input)." with:
While the engine injects a synthetic Escape, the global Escape is unregistered and retaken 60 ms after (RegisterHotKey fires on injected input). Suspensions nest: Escape comes back only when the last overlapping synthetic key is done. Every registration change runs on the main thread, one at a time, and an apply that changes nothing touches nothing (opening the bar registers Escape alone).
status: done; evidence: UNIT hotkeys::tests::escape_depth_counter, hotkeys::tests::apply_skips_unchanged_set, hotkeys::tests::a_taken_chord_is_retried_alone, hotkeys::tests::capture_mode_lets_go_and_keeps_results

## PARITY · KEY-12 · B-177
Replace "probe with ours unregistered, "That shortcut is already in use - try another." Returns shortcuts and active map." with:
probe with ours unregistered; a taken chord answers `{ok: false, error: "Another app already uses that shortcut. Try another." (C-56), free: [...]}`, where `free` lists those of Ctrl+Alt+A, Super+Shift+Space, Ctrl+Shift+Space that no other app holds and none of ours uses (U48). Returns shortcuts and active map.
status: done (Rust half); evidence: UNIT hotkeys::tests::taken_returns_free_alternatives, hotkeys::tests::validation_matches_main_js

## PARITY · LIFE-06 · B-011
Add (evidence for "warn if any shortcut failed"):
After the boot apply, each action whose chord could not be registered is written to stderr (`hotkeys::report_failures`).

## PARITY · LIFE-01 · B-012
Add (note to the row):
The second copy calls AllowSetForegroundWindow(ASFW_ANY) before it writes `open`, and keeps trying for 15 s. The first copy serves the pipe from before the app is built (an `open` during boot is answered when setup is done), reads one line with a 2 s limit on its own task (a client that says nothing holds up nobody), and retries a failed pipe creation, 100 ms doubling to 5 s.
status: UNIT instance::tests::stalled_client_does_not_block_server, instance::tests::an_open_during_boot_is_answered_once_ready; SMOKE+ singleInstance pending the platform probe (K-30)

## PARITY · LIFE-05 · B-013
Add (note to the row):
Every main-thread entry point the platform owns runs inside `crash::guard` (page load, window events, global shortcuts, tray and context-menu clicks, main-thread tasks, exit), so a panic there is logged and survived. A panic on the main thread outside a guard cannot be survived (it would unwind into a window procedure): the toast says "AevalBuddy hit a problem and has to restart." and is shown before the process ends.
status: UNIT crash::tests::guard_catches_and_logs, crash::tests::toast_follows_the_thread; SMOKE crashLogged and SMOKE+ crashMainThread pending the platform probe (K-30)

## PARITY · LIFE-03 · B-015
Add (evidence):
`run()` appends `--force-device-scale-factor=<AEVAL_SHOT_SCALE>` to WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS (keeping any value already there) before any webview exists. The window-maths half is B-025 (windows).

## PARITY · UPD-15 · B-016
Add (note to the row):
The installer finds the Electron entry by the first ten characters of DisplayName ("AevalBuddy <version>") and its folder from UninstallString, falling back to InstallLocation; it still requires "Uninstall AevalBuddy.exe" in that folder, runs it silently and waits.

## PARITY · UPD-02 · B-017
Add (note to the row):
Every failed check or download is one attempt: it counts toward the backoff and schedules the next timer even when its message matches the last failure's (the port deduped by message and could stop rescheduling, leaving a manual check at `checking`).
status: UNIT updater::tests::identical_failures_both_reschedule

## PARITY · UPD-04 · B-079
Replace "| UPD-04 | Backoff 2, 5, 15, 30, 60, 120 min then 4 h, +0..10 % jitter. |" with:
| UPD-04 | Backoff 2, 5, 15, 30, 60, then 120 min for every later failure (it stays at 120 min plus jitter), +0..10 % jitter. |
status: done; evidence: UNIT updater::tests::backoff_table

## PARITY · UPD-07 · B-079
Add (note to the row):
A latest.yml without a sha512 is refused before the download (nothing unverified is installed). `in_flight` is cleared by a drop guard, so a check that panics never blocks the next one. An empty AEVAL_OFFLINE means online, as in Electron (LIFE-13, LIFE-17).
status: UNIT updater::tests::missing_sha512_refuses, updater::tests::in_flight_clears_on_panic, updater::tests::empty_offline_env_is_online

## PARITY · WIN-14 · B-077
Add (note to the row):
The bridge shows the menu wherever a page did not handle `contextmenu`: `ui:context-menu {kind:'edit'}` on a text field, `{kind:'selection'}` on a non-empty selection; `menu::edit_template` is the template as data (Electron roles). The context menu's pending actions are swapped in after the items are made and taken out with one lock, never held across a main-thread round trip (B-076).
status: UNIT menu::tests::edit_and_selection_templates_match_main_js, menu::tests::pending_lock_not_held_while_building; SMOKE+ contextMenuEditable pending the platform probe (K-30)

## PARITY · LIFE-10 · B-086
Add (note to the row):
`lib.rs` `stop_for_exit()` stops a running task through the agent service with "Stopped because AevalBuddy quit." on RunEvent::Exit (the port used the reasonless stop hook).
status: code done; SMOKE+ quitStopReason pending the platform probe (K-26, K-30)

## PARITY · STORE-01 · B-082
Add (note to the row):
The v1 fields are the OpenRouter key's source of truth in both builds: `get_provider_key("openrouter")` reads `apiKeyEncrypted` / `apiKeyPlain`, and every load rewrites `apiKeys.openrouter` from them when they differ, so a key the Electron build changed or cleared after a rollback stays changed or cleared after the upgrade back.
status: UNIT store::tests::v1_fields_win_after_rollback

## PARITY · STORE-26 · B-083
Add (note to the row):
An entry is cut at 500 UTF-16 units, as JavaScript's `slice(0, 500)`; a surrogate pair is kept whole or dropped, never split.
status: UNIT store::tests::input_history

## PARITY · HIST-01 · B-083
Add (note to the row):
The 4 MB budget counts UTF-16 units of the serialized JSON, as `JSON.stringify(list).length` did (the port counted UTF-8 bytes and dropped CJK history early). Each entry is measured once per trim, so trimming is linear. Titles cut at 89 UTF-16 units plus `…` without splitting a surrogate pair. `history:list` rows carry only what history.js reads (id, at, title, asked, reply).
status: UNIT history::tests::cjk_budget_matches_electron, history::tests::trim_result_serializes_under_4mb, history::tests::emoji_title_cut_on_unit_boundary; SMOKE historyTitle, historySearch pending the platform probe

## PARITY · SYS-03 · B-081, D-11
Replace "StartupApproved respected." with:
StartupApproved respected at boot; turning the setting on in Settings marks it enabled (D-11).

Add (note to the row):
The Settings switch writes `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run` value `com.aevalbuddy.app` = REG_BINARY `02` and eleven zero bytes (Task Manager's "Enabled"), as Electron's `setLoginItemSettings` did. Boot (`autostart::sync`) writes only the Run value. D-11: an explicit toggle means the person wants it to run, while a Task Manager disable must survive every restart (Electron rewrote StartupApproved on every launch).
status: SMOKE+ launchAtLogin pending the platform probe

## PARITY · TRAY-01 · B-080
Replace "Tray icon is the pointer arrow, 16 px and 32 px PNGs embedded (base64 in tray.js)." with:
Tray icon is the pointer arrow: 16 px and 32 px PNGs from tray.js plus 20 px and 24 px resampled from the 32 px one; the smallest at least 16 px times the primary display's scale is used, so Windows only scales it down.
status: UNIT tray::tests::icon_matches_scale, tray::tests::icons_decode

## PARITY · TRAY-02 · B-080, D-07
Replace "Left click and double click: trail if the agent runs, else the bar." with:
Left click: trail if the agent runs, else the bar. A double click opens once (D-07).
Add (note to the row):
D-07: the DoubleClick arm is dropped, and the second click of a double click (within the system double-click time of the last open) is ignored. Reason: Windows reports one double click as two clicks and a double click; Electron opened on all three, which flickered open, closed, open.
status: UNIT tray::tests::double_click_opens_once; SMOKE+ trayClick pending the platform probe

## PARITY · TRAY-03 · B-080
Add (note to the row):
Wording follows tasks and conversations (C-83, C-84): `Stop the task` (Esc) replaces `Stop agent`, `New conversation` replaces `New chat`, and `Close all answer windows` (C-85, U38) follows it and calls `outputs::close_all` (K-04). The idle order is now `Ask, find, or do…`, `Ask about an area…`, `Find on screen…`, `Do a task…`, `-`, `Hide the overlay`, `New conversation`, `Close all answer windows`, `History…`, `-`, `Settings…`, `Help and feedback…`, check item, `-`, `Quit AevalBuddy`.
status: UNIT tray::tests::idle_order, tray::tests::ready_running_hidden; SMOKE trayOrder pending the platform probe

## PARITY · TRAY-05 · B-080
Add (note to the row):
Each check's 10 s "Up to date" is cleared only by its own timer, so a second check inside the 10 s keeps its full 10 s.
status: UNIT tray::tests::second_check_keeps_its_ten_seconds; SMOKE+ trayCheckStates pending the platform probe

## PARITY · TRAY-07 · B-080
Add (note to the row):
The menu is made once and edited in place on the main thread: only rows whose text or enabled state changed are touched, rows are replaced inside the same menu when the set changes (never `set_menu`, so an open menu is not swapped), download progress redraws at most once a second, and a refresh never blocks its caller.
status: UNIT tray::tests::refresh_only_touches_changed_items, tray::tests::progress_is_throttled_but_other_edits_are_not

## ARCHITECTURE · 10.3 Behaviour Rust must keep for the pages · B-078, D-37
Add (after the "Drag and resize" bullet):
- **Security surface (D-37).** `capabilities/default.json` grants the pages only event listen/unlisten and the window calls `frame.js` makes; `core:default` and every menu, tray, path, image and app permission are gone. `app.security.csp` repeats the pages' own meta CSP, so the header and the meta tag agree and frames still load over both forms of `aeval-frame`. An internal `aeval-navigation` plugin refuses any navigation away from `tauri.localhost` in every webview. Under a test run (`quiet_run`) one runtime capability adds `core:event:allow-emit`, because `Cx::eval` answers through an event; a shipped run never has it. Reason: a page that is ever made to run hostile content can only listen, move its own window and call the allowlisted channels.
status: UNIT tests::navigation_stays_in_the_app; SMOKE+ navigationBlocked pending the platform probe
