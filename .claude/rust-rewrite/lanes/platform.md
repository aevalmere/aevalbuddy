# Lane brief: platform

Branch `rust/platform`, cut from `rust-rewrite`. 25 items. Cross-lane tables (ownership, hotspots, contracts, decisions, merge order) are in `.claude/rust-rewrite/BACKLOG.md`; everything you need to do your items is in this file.

## Goal

The shell never loses data and never fails silently: settings and history writes are atomic and recoverable, a second launch focuses the running bar, panics and dead renderers are survived and reported, hotkeys, tray, updater, installer and autostart behave like Electron or better, and the security surface is narrow. You also own Cargo.toml and lay it out so eight other lanes can add dependencies without conflicts.

## Files you own

`src-tauri/src/{lib,main,paths,instance,crash,hotkeys,tray,notify,autostart,updater,history}.rs`, `src-tauri/src/store/**`, `src-tauri/src/host/menu.rs`, `src-tauri/bridge/**`, `src-tauri/capabilities/**`, `src-tauri/Cargo.toml`, `src-tauri/build.rs`, `src-tauri/tauri.conf.json`, `src-tauri/installer-hooks.nsh`, `src-tauri/installer.nsi` (new), `src-tauri/app.manifest`, `src-tauri/icons/**`, `src-tauri/.gitignore`, and (after harness C1) `src-tauri/src/selftest/probes/platform.rs`, `tools/smoke-labels/platform.txt`.

## Contracts you provide (C1 first)

- **Cargo.toml layout:** per-lane comment blocks (`# ---- lane: <lane> ----` / `# ---- end ----`) for dependencies and inside the `windows` / `windows-sys` feature arrays (one entry per line), with every pre-enumerated dependency and feature listed under "Hotspots" in BACKLOG.md, each placed in the block of the lane that needs it. CI must stay green with them (unused features are fine).
- **K-01** `crash::guard`, `crash::guard_async`; **K-02** `crash::guard_webview` (a working minimum in C1; B-013 and B-014 finish them).
- **K-08** `Store::set_raw(key, Value)`; **K-09** `Store::key_storage()`.
- **B-087** default `inputWidth` 600 for new profiles.
- **Capabilities for frame.js:** the window permissions `frame.js` uses (is-maximized, toggle-maximize, start-dragging, start-resize-dragging, and event listen/unlisten for resize events) so bar's K-41 works; B-078 narrows everything else later.

## Contracts you consume

- **K-04** `outputs::close_all` (windows) for the tray item in B-080.
- **K-26** fake LLM fixtures (llm) for B-086's probe.
- **K-30** selftest API (harness).

## Smoke labels you port

`tray`, `aumid`, `singleInstance`, `quitOnLastClose`, `rendererReload`, `crashLogged`, `shortcut.*`, `shortcuts`, `shortcutsOneChord`, `noGlobalEnter`, `noGlobalEscWithBar`, `escapeSuspend`, `captureMode`, `shortcutSet`, `shortcutReset`, `trayOrder`, `trayClick`, `trayAccel`, `trayCheckStates`, `trayTooltip`, `updateConfig`, `updateClassify`, `updateNotes`, `update.*`, `updateFeed`, `updateQuiet`, `feedOverrideStrict`, `historySaved`, `historyTitle`, `historySearch`, `historyRemove`, `historyRespectsSwitch`, `historyCleared`, `historyFlushOnQuit`, `contextMenus`, `contextMenuEditable`, `launchAtLogin`. Exact names and Electron semantics: `.claude/rust-rewrite/audit/parity-ipc.md` §3 and `main.js` on `main`.

## Shared-document entries you write

D-03, D-07, D-11, D-12, D-16 (store half), D-37; PARITY evidence for STORE, HIST, LIFE, TRAY, UPD, SYS, PKG-01, WIN-14 rows your items close, and the UPD-04 correction (B-079).

## Lane notes

- `lib.rs` is the crate root: any lane that needs a crate-level change asks you through its Blockers; check the Blockers sections of the other briefs on `origin/rust-rewrite` and in their branches (`git show origin/rust/<lane>:.claude/rust-rewrite/lanes/<lane>.md`) once per cycle.
- `src-tauri/bridge/aeval-bridge.js` is shared by every page; keep its key handling order (pages see keys first) intact, since bar and sheet rely on handling Ctrl+=, Ctrl+R and Ctrl+W themselves.

## Standing item: port your smoke labels

Every PARITY label under "Smoke labels you port" and every new label below is printed by `src-tauri/src/selftest/probes/platform.rs` through the K-30 API, with the Electron semantics from `.claude/rust-rewrite/audit/parity-ipc.md` §3 and `main.js` on `main`, and listed in `tools/smoke-labels/platform.txt` in the same commit that makes it print `=ok`. Acceptance: the rust workflow's smoke log shows each label `=ok` and the labels gate passes.

New labels your items add, beyond the PARITY list above (you print these): `crashMainThread`, `navigationBlocked`, `quitStopReason`.

## Items

| ID | Severity | Title |
|---|---|---|
| B-001 | P0 | Serialize, fsync and back up every settings save |
| B-002 | P0 | A read error never overwrites the settings file |
| B-003 | P0 | History never overwrites a file it could not read; one writer, throttled and fsynced |
| B-004 | P0 | A damaged Local State never mints a new encryption key |
| B-011 | P1 | Hotkey registration is serialized; Escape resumes synchronously; no-op applies are skipped |
| B-012 | P1 | A second launch brings the bar forward with keyboard focus, even during a slow boot |
| B-013 | P1 | Panics on the main thread are survived and reported honestly |
| B-014 | P1 | A dead WebView2 renderer is reloaded, not left as a blank click-eating rectangle |
| B-015 | P1 | The forced shot scale reaches WebView2 |
| B-016 | P1 | The installer removes the Electron install it replaces |
| B-017 | P1 | The updater reschedules after every failed attempt |
| B-076 | P2 | The context menu never holds its lock across a main-thread round trip |
| B-077 | P2 | Fields get Cut/Copy/Paste/Select all and a selection gets Copy/Select all |
| B-078 | P2 | Defense in depth: narrow capability, a Tauri CSP, no navigation away from the app |
| B-079 | P2 | Updater integrity and liveness, plus the small updater nits |
| B-080 | P2 | The tray updates in place, opens once per double click, and speaks in tasks and conversations |
| B-081 | P2 | Turning on "Launch at login" really launches at login |
| B-082 | P2 | After a rollback and upgrade, a cleared or changed OpenRouter key stays cleared or changed |
| B-083 | P2 | History counts its budget like Electron and trims in linear time |
| B-084 | P2 | Keys saved without encryption are logged and visible |
| B-085 | P2 | An interactive install is one click, like Electron's |
| B-086 | P2 | Quitting tells the running task why it stopped |
| B-087 | S2 | New profiles start with a 600 px bar |
| B-088 | P2 | The update smoke retries like Electron and honours `AEVAL_UPDATE_EXPECT` |
| B-177 | S3 | A taken shortcut comes with free alternatives |

Work in severity order (P0, then S1 and P1, then S2 and P2, then S3), after your C1 if you have one.

### B-001 · P0 · platform · Serialize, fsync and back up every settings save
- **Sources:** review-core P0-1; parity-core STORE-44.
- **Files:** `src-tauri/src/store/mod.rs`.
- **Fix:** Add a dedicated `static SAVE: parking_lot::Mutex<()>` held across snapshot, serialize, write and rename, and take the snapshot inside it, so the last writer always writes the newest state and two writes never overlap. Write to a unique temp name (`aevalbuddy.json.<pid>.<n>.tmp`), call `File::sync_all()` before replacing, and replace with `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)` (feature `Win32_Storage_FileSystem`, pre-enumerated in platform C1). Before each replace, keep the previous good file as `aevalbuddy.json.bak` (copy, not rename). On a parse failure at open, try `.bak` before falling back to defaults, and keep the `.corrupt` copy as today.
- **Reconciled:** review-core asks for save_lock plus unique tmp plus fsync plus `.bak`; parity-core asks for the lock and optional unique tmp. Both are adopted in full.
- **Acceptance:** UNIT `store::tests::concurrent_sets_never_lose_or_corrupt` (8 threads x 100 `set` calls on distinct keys, reopen, the JSON parses and every key is present); UNIT `store::tests::parse_failure_recovers_from_bak`.

### B-002 · P0 · platform · A read error never overwrites the settings file
- **Sources:** review-core P0-2.
- **Files:** `src-tauri/src/store/mod.rs` (`open_with`).
- **Fix:** Start fresh only on `ErrorKind::NotFound`. On `InvalidData` read the bytes and decode with `String::from_utf8_lossy` (what Node did). On any other error retry the read 3 times 100 ms apart; if it still fails copy the file to `.corrupt` when possible, run with defaults in memory and set a `saving_disabled` flag so nothing is ever written over the original for this session (log `[store] read failed: <kind>; saving disabled`).
- **Acceptance:** UNIT `store::tests::invalid_utf8_is_read_lossy_not_reset`; UNIT `store::tests::unreadable_file_disables_saving` (simulate with an injected reader returning `PermissionDenied`, assert the file bytes are unchanged after `set`).

### B-003 · P0 · platform · History never overwrites a file it could not read; one writer, throttled and fsynced
- **Sources:** review-core P0-3, P2-14; parity-core FILE-03; parity-ipc HIST-06.
- **Files:** `src-tauri/src/history.rs`.
- **Fix:** On a parse error rename the file to `history.json.corrupt-<unix ms>` before caching an empty list. On a read error other than `NotFound` keep the cache empty and set `writable = false` so the file is never overwritten this session. Replace the thread-per-debounce with one long-lived writer thread fed by a channel. Write timing is a throttle from the first change (Electron history.js:34-84): the first change schedules a write 400 ms later; changes inside that window ride along; nothing postpones it. `flush` and `flush_now` (RunEvent::Exit) go through the same writer and a write mutex held across write and rename, use a unique temp name, `sync_all`, then `MoveFileExW(... WRITE_THROUGH)`.
- **Reconciled:** parity-core wants Electron's throttle, review-core wants a single writer; both are kept (Decision D-12).
- **Acceptance:** UNIT `history::tests::corrupt_file_is_kept_aside_not_overwritten`; UNIT `history::tests::steady_saves_still_write_within_400ms`; UNIT `history::tests::flush_now_and_timer_never_overlap`; SMOKE `historySaved`, SMOKE+ `historyFlushOnQuit`.

### B-004 · P0 · platform · A damaged Local State never mints a new encryption key
- **Sources:** review-core P0-4.
- **Files:** `src-tauri/src/store/crypto.rs`.
- **Fix:** Create a key only when `Local State` is `NotFound`, or parses as a JSON object without `os_crypt.encrypted_key`. On any other read or parse error return `OsCrypt { key: None }` and leave the file alone (copying it to `Local State.corrupt` is fine). Strip a UTF-8 BOM before parsing. `write_json` writes a unique temp, `sync_all`, replaces with write-through, and preserves every other key already in the file.
- **Acceptance:** UNIT `crypto::tests::unreadable_local_state_is_left_alone`; UNIT `crypto::tests::bom_local_state_keeps_its_key`; existing `local_state_is_created_then_reused` still passes.

### B-011 · P1 · platform · Hotkey registration is serialized; Escape resumes synchronously; no-op applies are skipped
- **Sources:** review-core P1-2; parity-core KEY-09, LIFE-06; design-ux U25 (registration half).
- **Files:** `src-tauri/src/hotkeys.rs`, `src-tauri/src/lib.rs`.
- **Fix:** Funnel every `apply` through `app.run_on_main_thread` (inline when already on main), so calls are serialized without a lock held across plugin round trips. Re-read `want_escape` immediately before registering Escape and unregister it if no longer wanted. Replace the `escape_suspended` bool with a depth counter: `suspend_escape` increments and unregisters at 0→1; `resume_escape` is synchronous (no spawn, no sleep: desktop/client.rs already waited 60 ms) and decrements, calling `apply` only at 0. `apply` computes the wanted set `(action, chord)+escape` and returns early when it equals the registered set, so opening the bar never re-registers everything. After the boot `apply`, `eprintln!` each action whose result is `Some(false)` (Electron main.js:2761-2922).
- **Acceptance:** UNIT `hotkeys::tests::escape_depth_counter` (suspend, suspend, resume leaves Escape off; second resume re-applies); UNIT `hotkeys::tests::apply_skips_unchanged_set`; SMOKE+ `escapeSuspend` (two Escape-bearing synthetic keys 30 ms apart never trigger `on_escape`); SMOKE `noGlobalEscWithBar`.

### B-012 · P1 · platform · A second launch brings the bar forward with keyboard focus, even during a slow boot
- **Sources:** parity-core LIFE-01; review-core P2-8.
- **Files:** `src-tauri/src/instance.rs`, `src-tauri/src/lib.rs`.
- **Fix:** In `signal`, call `AllowSetForegroundWindow(ASFW_ANY)` before writing, and retry for 15 s (the bootMs gate). Create the pipe server before `Builder::build` and queue "open" until setup finishes. Read one line with `tokio::time::timeout(2 s)` instead of `read_to_end`. On a `create` error retry with backoff (100 ms doubling to 5 s) instead of returning.
- **Acceptance:** UNIT `instance::tests::stalled_client_does_not_block_server`; SMOKE+ `singleInstance` (a second process signals; the overlay is the foreground window within 2 s).

### B-013 · P1 · platform · Panics on the main thread are survived and reported honestly
- **Sources:** parity-core LIFE-05; review-core P2-5 (hook half).
- **Files:** `src-tauri/src/crash.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/tray.rs`, `src-tauri/src/host/menu.rs`.
- **Fix:** Add `crash::guard(scope, f)` = `catch_unwind(AssertUnwindSafe(f))` that logs to errors.log and announces once on Err, and `crash::guard_async(scope, fut)` for async bodies (C1, contract K-01). Wrap every main-thread entry point platform owns: `on_page_load`, `on_window_event`, every `run_on_main_thread` closure in platform files, tray and menu handlers. In the panic hook, choose the toast by thread: a panic on the main thread or during boot says the app has to restart (`AevalBuddy hit a problem and has to restart.`), others keep "It is still running". Correct the comment at crash.rs:71-73.
- **Acceptance:** UNIT `crash::tests::guard_catches_and_logs`; SMOKE `crashLogged` plus SMOKE+ `crashMainThread` (a panic inside a guarded `run_on_main_thread` closure is logged and the app keeps running).

### B-014 · P1 · platform · A dead WebView2 renderer is reloaded, not left as a blank click-eating rectangle
- **Sources:** parity-core WIN-16, LIFE-12.
- **Files:** `src-tauri/src/crash.rs`.
- **Fix:** Add `crash::guard_webview(&WebviewWindow, on_browser_exit: impl Fn(&AppHandle, &str) + Send + Sync + 'static)` (C1, K-02): `with_webview` → `controller().CoreWebView2()?.add_ProcessFailed(..)`. For RENDER_PROCESS_EXITED, RENDER_PROCESS_UNRESPONSIVE and FRAME_RENDER_PROCESS_EXITED: `crash::write("renderer", "<label> <kind> <reason>")`, `crash::announce()` once per session, `CoreWebView2.Reload()`. For BROWSER_PROCESS_EXITED: log and call `on_browser_exit(app, label)` so the owner recreates the window. windows lane calls it from `create_page` (B-090) and capture from the region, lens and buddy builders (B-114).
- **Acceptance:** SMOKE+ `rendererReload` (platform probe crashes the overlay renderer with DevTools `Page.crash`, then `Cx::eval` answers within 5 s).

### B-015 · P1 · platform · The forced shot scale reaches WebView2
- **Sources:** parity-core LIFE-03 (lib.rs half).
- **Files:** `src-tauri/src/lib.rs`.
- **Fix:** First thing in `run()`: if `AEVAL_SHOT_SCALE` parses to a value > 0, append `--force-device-scale-factor=<s>` to `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`, keeping any existing value, before any webview exists. The window maths half is B-025 (windows: `flags::scale_of` returns `screen::forced_scale()` when set; capture makes `forced_scale` pub in C1, K-11).
- **Acceptance:** shots matrix logs from B-010 at 1.25 and 1.5 show `devicePixelRatio` 1.25 / 1.5 in `[SHOT] fit.*` lines; installer job `shots125.exitCode`, `shots150.exitCode` pass.

### B-016 · P1 · platform · The installer removes the Electron install it replaces
- **Sources:** review-core P1-3; parity-ipc B4, UPD-15.
- **Files:** `src-tauri/installer-hooks.nsh`.
- **Fix:** Match `DisplayName` by prefix (`StrCpy $4 $2 10` then `StrCmp $4 "AevalBuddy"`), and get the folder from `UninstallString` (`"<dir>\Uninstall AevalBuddy.exe" /currentuser`: strip quotes, arguments and the file name). Keep the `IfFileExists "$3\Uninstall AevalBuddy.exe"` guard. Run the old uninstaller silently and wait for it.
- **Acceptance:** installer job (B-052) step "Electron upgrade": install Electron v1.1.1 from its GitHub release, then the Tauri installer `/S`; log lines `[INSTALL] electronRemoved=ok` and `[INSTALL] uninstallEntries=1`.

### B-017 · P1 · platform · The updater reschedules after every failed attempt
- **Sources:** parity-ipc B3, UPD-02, IPC-34.
- **Files:** `src-tauri/src/updater.rs`.
- **Fix:** Dedupe failures per attempt, not per message: `check()` and `download()` take a fresh attempt id and pass it to `fail(id, ..)`, which returns early only when `id == last_failed_attempt`. Every attempt counts toward the backoff and schedules the next timer; a manual check that fails ends in `retrying` or `error`, never `checking`.
- **Acceptance:** UNIT `updater::tests::identical_failures_both_reschedule` (two `fail` calls with the same text from two attempts give `failures == 2` and a scheduled timer); SMOKE `updateFeed`.

### B-076 · P2 · platform · The context menu never holds its lock across a main-thread round trip
- **Sources:** review-core P2-1.
- **Files:** `src-tauri/src/host/menu.rs`.
- **Fix:** Build the items into a local `Vec<(id, action, item)>` without holding `pending`, then lock `pending` only to swap the map in; `on_menu_event` locks only to take the action.
- **Acceptance:** UNIT `menu::tests::pending_lock_not_held_while_building` (lock is free inside the item factory callback); SMOKE+ `contextMenus`.

### B-077 · P2 · platform · Fields get Cut/Copy/Paste/Select all and a selection gets Copy/Select all
- **Sources:** parity-core WIN-14.
- **Files:** `src-tauri/bridge/aeval-bridge.js`, `src-tauri/src/host/menu.rs`.
- **Fix:** Bridge: when the page did not handle `contextmenu`, always `preventDefault`, then invoke `ui:context-menu {kind:'edit'}` for an editable target or `{kind:'selection'}` when the trimmed `getSelection()` is non-empty. `items_for` maps those kinds to `PredefinedMenuItem` cut, copy, paste, separator, select_all (Electron main.js:384-404). Expose the template as a pure function for the probe.
- **Acceptance:** UNIT `menu::tests::edit_and_selection_templates_match_main_js`; SMOKE+ `contextMenuEditable`.

### B-078 · P2 · platform · Defense in depth: narrow capability, a Tauri CSP, no navigation away from the app
- **Sources:** review-core P2-2 (capability, CSP, navigation half).
- **Files:** `src-tauri/capabilities/default.json`, `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`.
- **Fix:** Narrow the capability to the permissions the pages use: event listen/unlisten, window start-dragging, start-resize-dragging, is-maximized, toggle-maximize, on-resized (K-41 needs), plus anything `cargo test`/the smoke shows missing; drop menu and tray permissions from webviews. Set `app.security.csp` to the strictest policy the pages work under (`default-src 'self'; img-src 'self' aeval-frame: data: blob:; connect-src 'self' ipc: http://ipc.localhost aeval-frame:; style-src 'self' 'unsafe-inline'; script-src 'self'`) and keep each page's meta CSP. Register a tiny internal plugin (`tauri::plugin::Builder::on_navigation`) that refuses any navigation away from the app origin for every webview.
- **Acceptance:** SMOKE+ `navigationBlocked` (platform probe evals `location.href='https://example.com'`; the page URL is unchanged 1 s later); SMOKE `glassLoaded` and `scaffold.pages` still pass (CSP did not break frames).

### B-079 · P2 · platform · Updater integrity and liveness, plus the small updater nits
- **Sources:** review-core P2-6; parity-core LIFE-13, LIFE-17; parity-ipc UPD-04.
- **Files:** `src-tauri/src/updater.rs`.
- **Fix:** An empty or missing `sha512` is an error (no unverified install). Clear `in_flight` with a drop guard. `AEVAL_OFFLINE=""` means online (Electron). Add UNIT for `online()` mapping (LIFE-17). Correct PARITY UPD-04's "then 4 h" to "stays at 120 min plus jitter" via `entries/platform.md`.
- **Acceptance:** UNIT `updater::tests::missing_sha512_refuses`; UNIT `updater::tests::in_flight_clears_on_panic`; UNIT `updater::tests::empty_offline_env_is_online`.

### B-080 · P2 · platform · The tray updates in place, opens once per double click, and speaks in tasks and conversations
- **Sources:** review-core P2-9; parity-features #12 (polish 7f520b8); parity-core TRAY-05, TRAY-01; design-ux U38 (tray half), C-83, C-84, C-85.
- **Files:** `src-tauri/src/tray.rs`.
- **Fix:** Keep item handles and call `set_text`/`set_enabled` only on changes; throttle progress refreshes to 1 Hz; never swap the menu while it is open. Drop the `DoubleClick` arm (a Windows double click also produces `Click(Up)`). The 10 s "Up to date" reset gets a timer id so a second check does not shorten it. Use a multi-size icon (16, 20, 24, 32) picked per DPI. Labels: `Stop the task` (C-83), `New conversation` (C-84), new item `Close all answer windows` (C-85) calling `outputs::close_all` (K-04). Update the tray label tables and the `trayOrder` lists.
- **Reconciled:** Electron opened on both events; dropping DoubleClick removes the open/close/open flicker. Decision D-07.
- **Acceptance:** UNIT `tray::tests::idle_order` and `ready_running_hidden` updated; UNIT `tray::tests::refresh_only_touches_changed_items`; SMOKE `trayOrder`, SMOKE+ `trayClick` (one synthetic double click opens the bar once), SMOKE+ `trayCheckStates`.

### B-081 · P2 · platform · Turning on "Launch at login" really launches at login
- **Sources:** parity-ipc B12 (SYS-03).
- **Files:** `src-tauri/src/autostart.rs`.
- **Fix:** In the function the `settings:set-launch-at-login` route calls (not the boot `apply` path), when turning on also write `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run` value `com.aevalbuddy.app` as REG_BINARY `02 00 00 00 00 00 00 00 00 00 00 00`. At boot leave StartupApproved alone.
- **Reconciled:** Electron's `setLoginItemSettings` writes StartupApproved on every call; Rust writes it only on an explicit toggle so a Task Manager disable survives restarts. Decision D-11.
- **Acceptance:** SMOKE+ `launchAtLogin` (toggle off/on: Run value present and StartupApproved enabled; `[INSTALL] startupApproved=02...` in the installer job).

### B-082 · P2 · platform · After a rollback and upgrade, a cleared or changed OpenRouter key stays cleared or changed
- **Sources:** parity-core STORE-01, STORE-02, STORE-40.
- **Files:** `src-tauri/src/store/mod.rs`.
- **Fix:** For `openrouter`, the v1 fields are the source of truth: `get_provider_key("openrouter")` returns `legacy_api_key()`. In `migrate()`, rewrite `apiKeys.openrouter` from the v1 fields whenever they differ.
- **Acceptance:** UNIT `store::tests::v1_fields_win_after_rollback` (`apiKeys.openrouter`=enc(A), `apiKeyEncrypted`=enc(B) → B; both v1 empty → '').

### B-083 · P2 · platform · History counts its budget like Electron and trims in linear time
- **Sources:** parity-ipc B13 (HIST-01); parity-features #18 (polish 3de320f); review-core P2-12; parity-core STORE-26 (nit).
- **Files:** `src-tauri/src/history.rs`, `src-tauri/src/store/mod.rs` (input-history length nit).
- **Fix:** The 4 MB budget counts UTF-16 code units (`encode_utf16().count()` of the serialized JSON); titles take 89 UTF-16 units plus `…` without splitting a surrogate pair. Trim by summing string lengths with a deliberately high estimate and popping until it fits (polish-loop algorithm), serializing once per write. `list()` stops cloning messages and returns exactly the fields `history/history.js` reads (check the page before trimming; the `history:list` route in ipc.rs keeps its shape). Input-history item length counts UTF-16 units.
- **Acceptance:** UNIT `history::tests::cjk_budget_matches_electron`; UNIT `history::tests::trim_result_serializes_under_4mb`; UNIT `history::tests::emoji_title_cut_on_unit_boundary`; SMOKE `historyTitle`, `historySearch`.

### B-084 · P2 · platform · Keys saved without encryption are logged and visible
- **Sources:** review-core P2-13; design-ux U21 (storage half).
- **Files:** `src-tauri/src/store/crypto.rs`, `src-tauri/src/store/mod.rs`.
- **Fix:** `Store::key_storage() -> &'static str` returning `"encrypted"` or `"plain"` (C1, K-09); log once when a key is saved in plain text. llm surfaces it in `settings:get` (B-038) and settings renders C-58 (B-069).
- **Acceptance:** UNIT `store::tests::key_storage_reports_plain_without_os_key`.

### B-085 · P2 · platform · An interactive install is one click, like Electron's
- **Sources:** parity-ipc B10 (PKG-01).
- **Files:** `src-tauri/tauri.conf.json`, `src-tauri/installer.nsi` (new; a copy of Tauri's NSIS template for the pinned tauri-bundler version).
- **Fix:** Set `bundle.windows.nsis.template` to a copy of Tauri's `installer.nsi` with every `!insertmacro MUI_PAGE_*` removed (no welcome, folder or finish page), `SetSilent`-equivalent behaviour for interactive runs, per-user install unchanged, and the post-install launch kept. Keep `installer-hooks.nsh` wired.
- **Reconciled:** parity-ipc offered "record a departure or remove the pages"; one-click keeps Electron parity. Decision D-03.
- **Acceptance:** installer job: an interactive (no `/S`) install run with a 60 s timeout completes without input and `install.*` Checks pass (`[INSTALL] oneClick=ok`).

### B-086 · P2 · platform · Quitting tells the running task why it stopped
- **Sources:** parity-core LIFE-10.
- **Files:** `src-tauri/src/lib.rs`.
- **Fix:** In `RunEvent::Exit`, call `host::services().agent.stop(Some("Stopped because AevalBuddy quit."))` instead of the reasonless `stop_agent` hook.
- **Acceptance:** extract the exit handling into `fn stop_for_exit()`; SMOKE+ `quitStopReason` in `selftest/probes/platform.rs` starts a fake-LLM run through the agent service's public start API, calls `stop_for_exit()`, and checks the trail's final text equals the reason.

### B-087 · S2 · platform · New profiles start with a 600 px bar
- **Sources:** design-visual F27 (store half).
- **Files:** `src-tauri/src/store/mod.rs`.
- **Fix:** Default `inputWidth` 600 for profiles without a stored value (C1, so windows' `PANEL_WIDTH` in B-106 matches). Existing stored widths are untouched. Update `fresh_file_gets_every_default_in_order`.
- **Reconciled:** Decision D-16.
- **Acceptance:** UNIT `store::tests::fresh_file_gets_every_default_in_order` (600); SMOKE `widthDefaults`.

### B-088 · P2 · platform · The update smoke retries like Electron and honours `AEVAL_UPDATE_EXPECT`
- **Sources:** parity-ipc B9 (UPD-12, UPD-13), §3.2 update rows.
- **Files:** `src-tauri/src/selftest/probes/platform.rs`.
- **Fix:** Call `updater::check` up to 3 times, each under `tokio::time::timeout(45 s)`; between attempts sleep 10 s then 30 s when the cause is 5xx or a timeout. Print `update.retry1`, `update.retry2`, `update.attempts`, `update.status`, `update.version`, `update.problem`, `updateFeed` (must: status in `AEVAL_UPDATE_EXPECT` or `current,downloading,ready`; `any` passes), `updateQuiet` (problem does not match `/error|exception|stack|http/i`), `update.cause`, `updateConfig`, `updateClassify`, `updateNotes`, `feedOverrideStrict` (main.js:3244-3320).
- **Acceptance:** SMOKE `updateFeed`, `updateQuiet`, `update.attempts` present in the smoke log.

### B-177 · S3 · platform · A taken shortcut comes with free alternatives
- **Sources:** design-ux U48 (Rust half), C-56.
- **Files:** `src-tauri/src/hotkeys.rs`.
- **Fix:** When a chord is taken, probe `Ctrl+Alt+A`, `Win+Shift+Space`, `Ctrl+Shift+Space` and return the free ones in the `shortcuts:set` error payload (`{ok:false, error, free: [..]}`), error text C-56 `Another app already uses that shortcut. Try another.`
- **Acceptance:** UNIT `hotkeys::tests::taken_returns_free_alternatives` (with an injected registrar); UNIT `validation_matches_main_js` updated.

## Rules

The coordinator's rules, verbatim: "LF endings; never run the app locally; never weaken or skip a test or gate to get green; commit messages in sentence case describing the effect like the existing log, each ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; push only its own branch".

What each one means here:

- **LF endings** in every file you write. Before each commit run `git diff --cached --check` and `git ls-files --eol <changed files>` (expect `i/lf w/lf`). The repo has no `.gitattributes`, so your editor's endings are what gets committed.
- **Never run the app locally.** No `cargo tauri dev`, no launching a built exe, no `npm start` of the Electron app. This machine has no Rust toolchain and nine lanes share it; CI is the build and the test bed. Local checks are limited to `node --check <file>` for JS and reading code.
- **Never weaken or skip a test or gate to get green.** No `#[ignore]`, no deleted or loosened assertions, no raised thresholds, no `continue-on-error`, no label removed from a label file, no scene removed from ui-shots. An expected value changes only where one of your items specifies the new value (for example B-049's gate tables or B-087's defaults test).
- **Commits** in sentence case describing the effect, like `git log --oneline -15` on `rust-rewrite` (for example "Keep the first-run tour out of the capture and desktop checks"). Item IDs may go in the body. Every message ends with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Push only your own branch**, `rust/platform`: `git push origin HEAD:rust/platform`. Never push `rust-rewrite`, `main`, `ci-logs`, `shots` or another lane's branch; never force-push.
- **Never poll the GitHub API (60 req/hr shared by the whole machine).** No `gh run`, `gh api`, `gh pr`, or `curl` to api.github.com. CI results come only from `tools/ci-wait.mjs`, which reads the `ci-logs` branch with git.
- Edit only the files listed under "Files you own". If you need a change elsewhere, write it under Blockers (file, the change, which item needs it) and carry on with other items. Never create the per-lane files that harness C1 creates (`selftest/probes/<lane>.rs`, `tools/smoke-labels/<lane>.txt`, `tools/ui-shots/lanes/<lane>.mjs`); merge `origin/rust-rewrite` once they exist.
- Do not edit `DESIGN.md`, `PARITY.md`, `ARCHITECTURE.md`, `REQUESTS.md`, `BACKLOG.md` (only B-149 appends to its "Post-merge findings" section), the audits, other lanes' briefs, the Electron reference (`main.js`, `lib/**` except files you own, `package*.json`, `electron-builder.yml`) or `README.md`.
- Never commit a real API key. Tests and probes use fake keys and the fake LLM.

## Verification loop

Every cycle:

1. Make a batch of changes: one or more items, each with its unit tests, probe labels, ui-shots scenes and checks, and its entries.
2. `git diff --cached --check`, `node --check` on changed `.js` and `.mjs` files, re-read the diff against the item specs.
3. Commit, then push once: `git push origin HEAD:rust/platform` (first push: `git push -u origin HEAD:rust/platform`).
4. Wait for CI with the script below. If the push touched `renderer/`, `settings/`, `history/`, `src-tauri/bridge/` or `tools/ui-shots/`, wait on `ui-shots` as well as `rust`.
5. Exit 0: record the items as green in Progress with the sha. Exit 1: read what it printed, fix, and go to step 1. Exit 2: follow the printed sha, or wait again.

The coordinator's CI recipe, verbatim:

- Usage: `node tools/ci-wait.mjs <sha> [workflow=rust|ui-shots] [--timeout-min 30]` (sha can be HEAD/short). It `git fetch`es the `ci-logs` branch every 45 s; no GitHub API.
- Exit 0 = green; 1 = failure (prints summary.md, errors.txt, last 80 lines of the failing gate's log, and the `git show origin/ci-logs:<dir>/fmt.patch | git apply` command when rustfmt fails); 2 = timeout or cancelled (prints the newer sha to wait on if a later push superseded it).
- Full logs: `origin/ci-logs:<ref with / as ->/<sha>/<workflow>/` holds summary.md, meta.json, errors.txt, and prebuild/fmt/clippy/test/build/smoke/capture/desktop .log (ui-shots: bridge/playwright/shots .log). `<ref>/latest/<workflow>/` = newest run.
- ui-shots only runs when renderer/settings/history/bridge/tools/ui-shots paths change, so `ci-wait <sha> ui-shots` exits 2 on other pushes; that is expected.
- Pushing a new commit to the same branch cancels the running CI for the older sha (concurrency cancel-in-progress), so batch changes and push once per cycle.

For this lane the log directory is `origin/ci-logs:rust-platform/<sha>/<workflow>/` and the newest run is `origin/ci-logs:rust-platform/latest/<workflow>/`. Read a file with `git show "origin/ci-logs:rust-platform/latest/rust/smoke.log"` (use PowerShell or set `MSYS_NO_PATHCONV=1` in Git Bash, which otherwise mangles the `ref:path` argument). The rust workflow takes up to an hour on a cold cache; use `--timeout-min 70` when the build is cold. When the same sha is the tip of two branches (right after merging `origin/rust-rewrite` with nothing new on top), add `--ref rust/platform`.

## Starting and staying current

1. Your worktree starts at `rust-rewrite` HEAD. Create the branch with `git switch -c rust/platform` if it does not exist.
2. Read this brief, the cross-lane tables in `.claude/rust-rewrite/BACKLOG.md` (lanes, ownership, hotspots, contracts), `DESIGN.md`, and for each item the audit sections and PARITY rows it cites. The code is the ground truth for current behaviour; `main.js` on `main` is the ground truth for Electron behaviour.
3. Push your C1 (if "Contracts you provide" lists any) before anything else, with a commit message starting `C1 contracts:`. When it is green, write `C1 green at <sha>` in Progress.
4. Each cycle, `git fetch origin`. When a contract you need has landed (`git grep -n "<symbol>" origin/rust-rewrite -- <file>`) or `rust-rewrite` has moved, `git merge origin/rust-rewrite` (a merge commit; never rebase or force-push). A conflict in a file you own: resolve it. A conflict in a file you do not own means you edited it by mistake: take `origin/rust-rewrite`'s side and note it in Progress.
5. If CI fails for a reason entirely outside your files (a broken merge on `rust-rewrite`, a runner outage), record it under Blockers with the log line and keep working on what you can verify.

## Shared documents

Never edit `DESIGN.md`, `PARITY.md` or `ARCHITECTURE.md`. In the same commit as the change, append to `.claude/rust-rewrite/entries/platform.md`:

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

- C1 (K-01, K-02, K-08, K-09, B-087, capabilities, Cargo.toml lane blocks): at 3e74cba Format, Clippy, Unit tests, Capture check and Desktop selfcheck passed; Build installer failed on `http status: 504` downloading nsis_tauri_utils.dll (a network error, not the code). Re-run at caadb51 (same code, this file only): the same gates passed and Build installer failed on the same 504.
- C1 green at caadb51 (the coordinator's standing rule: a run whose only red step is a bundler download 504 in Build installer counts as green); merged into rust-rewrite as 17e55bb.
- d7e5e5f (P0 and P1 batch): Unit tests passed; Format failed on one assert in hotkeys.rs and Clippy on `fetch_update` (deprecated in Rust 1.99, renamed `try_update`) and a `manual_contains` in a test; Capture check and Desktop selfcheck only failed because no exe was built behind Clippy. Fixed in the next push.
- In progress (pushed, awaiting CI): B-001, B-002, B-003, B-004 (P0); B-011, B-012, B-013, B-015, B-016, B-017; B-076, B-077, B-079, B-086 (code; its probe waits on K-26, K-30); B-177; B-081, B-082, B-083.

## Blockers

(Format: `- B-xxx: <what blocks it>; <why>; <what would unblock it>`.)

- Every SMOKE acceptance (C1 counts as green under the coordinator's rule): the rust workflow's Build installer step fails with `failed to bundle project: http status: 504` while the Tauri bundler downloads nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll (runs for 3e74cba and caadb51), and Scaffold smoke is skipped behind it; a GitHub release download outside my files; unblocked when the download succeeds again, or by caching %LOCALAPPDATA%\tauri\NSIS in rust.yml (not mine).
