# Core review: Rust/Win32 correctness and security

Branch `rust-rewrite` at 544f982. Scope: `src-tauri/src` (lib, main, state, instance, ipc, paths, windows/**, store/**, history, tray, hotkeys, updater, crash, notify, autostart, host/**), `bridge/**`, `tauri.conf.json`, `capabilities/**`, `installer-hooks.nsh`, `build.rs`, `Cargo.toml`. This is a defect hunt in the Rust itself, not a parity check. Nothing was compiled or run. Each finding was traced through the code by hand. **PLAUSIBLE** means the defect is real in the code but the trigger or impact still needs a runtime or registry check.

Counts: **P0 4, P1 3, P2 14**

---

## P0

### P0-1. Concurrent settings saves race on one temp file, and no save is fsynced
- **Where:** `src-tauri/src/store/mod.rs:290-293` (`save`), `src-tauri/src/store/mod.rs:943-955` (`write_atomic`)
- **Defect:** `save()` clones the map under the data lock, then serializes and writes outside it. Every saver writes the same `aevalbuddy.json.tmp` and renames it into place, and nothing calls `sync_all` before the rename.
- **Failure scenario:**
  1. Settings are saved from several threads: IPC commands on tokio workers, debounced position saves, `heal_model` at boot, and main-thread handlers.
  2. Thread A snapshots, then thread B snapshots (which includes B's change), writes and renames. A then writes its older snapshot and renames it over B's file, so B's change is lost. It stays lost if the app exits before the next save, because nothing saves again at exit.
  3. Worse, two `fs::write` calls can overlap on the shared `.tmp`. The second `CREATE_ALWAYS` truncates while the first is still writing. Rust opens files with `FILE_SHARE_DELETE`, so A's rename succeeds while B's handle is still open, and B keeps writing into what is now `aevalbuddy.json`. The result is a file with mixed bytes or a zero-filled gap.
  4. Without a flush, a power cut or BSOD after the rename can leave a zero-filled or zero-length file on NTFS. The Electron build's write path (conf → atomically) fsyncs by default, so this is a regression.
  5. On the next launch the parse fails. The file is copied to `.corrupt`, defaults are saved, and the user loses the API key, consent, shortcuts and every setting.
- **Fix:** Hold a dedicated `save_lock: Mutex<()>` across clone, serialize, write and rename (or serialize under the data lock and write under the save lock). Write to a unique temp name, call `File::sync_all()` before the rename, and replace with `MoveFileExW(MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`. Keep the last good copy as `aevalbuddy.json.bak` and fall back to it when a parse fails.

### P0-2. Any read error other than a parse failure wipes the settings file with no backup
- **Where:** `src-tauri/src/store/mod.rs:256-266` and `:286` (`open_with`)
- **Defect:** `Err(_) => Map::new()` treats every `read_to_string` error like a missing file, and `open_with` then calls `store.save()`. The real file is overwritten with defaults, and no `.corrupt` copy is made (that copy happens only on the parse-failure branch).
- **Failure scenario:** Either of these is enough:
  - The user hand-edits `aevalbuddy.json` in an ANSI editor and adds one non-ASCII character to a custom prompt. `read_to_string` then fails with `InvalidData`. Node read the same file fine, replacing the bad byte with U+FFFD.
  - Another process holds the file without `FILE_SHARE_READ` at boot (a backup tool, a sync client or AV), causing a sharing violation.

  The next start silently replaces the API key blob, consent and all settings with defaults. The data cannot be recovered.
- **Fix:** Start fresh only on `ErrorKind::NotFound`. For `InvalidData`, read the bytes and use `String::from_utf8_lossy`, as Node does. For any other error, copy the file to `.corrupt` if possible and run with defaults in memory but with saving disabled (or retry the read a few times), so the file on disk is never overwritten.

### P0-3. A corrupt or unreadable `history.json` becomes an empty list, and the next save overwrites it
- **Where:** `src-tauri/src/history.rs:91-103` (`load`), `src-tauri/src/history.rs:125-137` (`flush`)
- **Defect:** `load` turns any read or parse failure into an empty cached list and keeps no backup. Then:
  - `flush` writes that cache over the file.
  - `flush` uses one shared `history.json.tmp`.
  - `flush` holds no lock across the write, so the debounce thread and `flush_now` at exit can overlap.
  - `flush` does not fsync.
- **Failure scenario:** `history.json` is damaged. The causes include the same tmp overlap as P0-1 (a debounce thread from `schedule_write` against `flush_now` in `RunEvent::Exit`), a power loss after a non-flushed rename, or a hand edit. On the next launch the list is empty. The first new conversation triggers a save, which writes `{"conversations":[that one]}` over the file, and up to 200 saved conversations are gone with no `.corrupt` copy.
- **Fix:**
  - On a parse error, rename the file to `history.json.corrupt-<ts>` before caching an empty list.
  - On a read error other than `NotFound`, set `enabled = false` (or skip writes) so the file is never overwritten.
  - Serialize `flush` with a write mutex held across write and rename, use a unique temp name, and call `sync_all` before the rename.

### P0-4. A damaged or unreadable `Local State` makes the app mint a new encryption key, which orphans every saved key
- **Where:** `src-tauri/src/store/crypto.rs:34-66` (`load_or_create`), `src-tauri/src/store/crypto.rs:133-142` (`read_json`), `src-tauri/src/store/crypto.rs:144-152` (`write_json`)
- **Defect:** `read_json` returns an empty map for any read or parse error, so `load_or_create` sees no `os_crypt.encrypted_key`. It generates a fresh AES key and rewrites `Local State` with only that key. The careful "leave an existing key alone" branch at `:43-45` only protects a key it managed to read.
- **Failure scenario:** Any of these triggers it:
  - `Local State` is zero-filled after a power cut. `write_json` also skips fsync, and the same tmp pattern is used.
  - The file is briefly locked at boot.
  - The file has a BOM (`read_json` does not strip it, although `store/mod.rs` does).

  A new key replaces the old one in `Local State`. Every `v10` blob encrypted under the old key, including the API keys, can no longer be decrypted on this build or after a rollback to Electron. The user has to re-enter every key, and the rewritten `Local State` also drops whatever else Electron kept there.
- **Fix:**
  - Create a key only when the file is `NotFound`, or when it parses as a valid object that lacks `os_crypt`.
  - On a read or parse error, return `OsCrypt { key: None }` and leave the file alone. Copying it to `Local State.corrupt` is fine.
  - Strip the BOM before parsing.
  - Fsync in `write_json`.

---

## P1

### P1-1. Mixed-DPI DIP layout overlaps, so windows on the primary display get mapped to the secondary
- **Where:**
  - `src-tauri/src/windows/screen.rs:32-52` (`Display::from_px`, DIP origin = `bounds_px / scale`)
  - `src-tauri/src/windows/screen.rs:145-152` (`matching`: `max_by` returns the last element on ties)
  - `src-tauri/src/windows/motion.rs:127-133` (`set_glass`)
  - `src-tauri/src/windows/outputs.rs:228-235` and `:506` (layout against `screen::work_area()`, which is primary only)
- **Defect:** A display to the right of or below the primary that has a higher scale gets a DIP origin inside the primary's DIP range. A rect wholly inside the overlap ties on overlap area, and `max_by` picks the later display (always a secondary, since primary sorts first). `dip_to_px` then places the window on the wrong monitor. The comment says this is what Chromium does, but Chromium keeps displays adjacent in DIP space.
- **Failure scenario:**
  1. Primary is 1920x1080 at 100% (DIP 0..1920). Secondary is 2560x1440 at 150% at physical x=1920 (DIP 1280..2987). The two DIP ranges overlap at 1280..1920.
  2. The default top-right pill or bar on the primary, at DIP x≈1500, ties and resolves to the secondary. It lands at physical x = 1920 + (1500 − 1280) × 1.5 = 2250, on the other monitor.
  3. The round trip is not stable either. `glass()` reads physical coordinates via `matching_px`, which picks the primary correctly, but `set_glass` writes back via `matching`, which picks the secondary. Every expand, collapse or animation of a window in that band jumps it across monitors.
  4. Outputs dragged to the secondary are clamped to the primary work area (`outputs.rs:235`) and stored relative to it (`:506`).
- **Fix:**
  - Build Chromium-style adjacent DIP bounds: place each non-primary display edge-to-edge with its neighbor in DIP space, with no physical/scale origin.
  - Break ties in `matching` toward the primary or the display the window currently occupies, using `first max` instead of `max_by`.
  - For existing windows, pass the `HMONITOR` or physical rect through, so that the read and the write use the same display.
  - Clamp outputs to the work area of the display they are on.

### P1-2. `hotkeys::apply` runs unserialized from several threads, which can leave plain Escape registered system-wide (PLAUSIBLE trigger)
- **Where:** `src-tauri/src/hotkeys.rs:309-342` (`apply`), `:387-399` (`suspend_escape`), `:402-409` (`resume_escape` spawns `apply` on tokio after 60 ms)
- **Defect:** `apply` runs this sequence with no serialization: `unregister_all`, read `want_escape` under the state lock, register each action (each call is a blocking main-thread round trip), then register Escape. It is called concurrently from the main thread, tokio tasks (IPC, `resume_escape`) and the agent's state callbacks.
- **Failure scenario:**
  - **Stale Escape:** Task B (`resume_escape`, or a settings change) reads `want_escape = true` while the agent is still running. The agent then finishes, and task A runs its whole `apply` with `want_escape = false`. B continues and registers `Escape` after A has finished. The app now holds plain Escape as a global hotkey while idle, and Escape stops working in every other app until something calls `apply` again.
  - **False "hotkey taken":** Interleaved `unregister_all` and `register` calls between A and B can make `register` fail with "already registered" for a key this app holds itself. That failure is recorded in `s.hotkeys`, and the user sees a "shortcut taken" banner or toast.
- **Fix:** Funnel every `apply` through `app.run_on_main_thread`, which runs inline when already on main, so calls are serialized without a lock. A mutex held across the plugin calls would deadlock, because `apply` also runs on main and the plugin round-trips to main. Re-read `want_escape` immediately before registering Escape, and unregister Escape if it is no longer wanted.

### P1-3. The NSIS pre-install hook never matches the Electron install, so the old copy is left installed (PLAUSIBLE)
- **Where:** `src-tauri/installer-hooks.nsh:30-31`
- **Defect:** The hook requires `DisplayName` to equal `"AevalBuddy"` exactly. electron-builder's default `uninstallDisplayName` is `${productName} ${version}`, and `electron-builder.yml` on `main` does not override it (its `nsis:` block has only oneClick, perMachine, deleteAppDataOnUninstall and artifactName). The real value is therefore `"AevalBuddy 1.x.y"`, and the `StrCmp` falls through to `aeval_scan`.
- **Failure scenario:** An Electron user updates to the Tauri build, through `electron-updater` or by running the installer. The Electron copy under `%LOCALAPPDATA%\Programs\aevalbuddy` is never uninstalled. The user ends up with two apps, two Apps and Features entries, duplicate or stale shortcuts, and two Run-key autostarts that both try to own the single-instance mutex and the global hotkeys.
- **Fix:** Compare a prefix (`StrCpy $4 $2 10` then `StrCmp $4 "AevalBuddy"`), or match on `InstallLocation` ending in `\Programs\aevalbuddy` together with the existing `Uninstall AevalBuddy.exe` check. Confirm the actual `DisplayName` on a real Electron install, for example in the installer.yml runner job.

---

## P2

### P2-1. The context-menu popup holds the `pending` lock across blocking main-thread menu creation (deadlock window)
- **Where:** `src-tauri/src/host/menu.rs:140-152` (lock held across `MenuItem::with_id`), `:176` (`on_menu_event` on main locks `pending`)
- **Failure scenario:** `ui:context-menu` arrives on a tokio worker, which takes `pending()` and calls `MenuItem::with_id`, a blocking round trip to main. If main is dispatching a menu click at that moment, it blocks on `pending().lock()` at :176 while the worker waits on main. The whole UI freezes for good.
- **Fix:** Build the items into a local `Vec<(id, action, item)>` without holding the lock, then lock `pending` only to swap the map in.

### P2-2. No per-window authorization on IPC, a wide capability and no Tauri CSP (defense in depth)
- **Where:**
  - `src-tauri/src/ipc.rs:204-239` (allowlist only, no label check)
  - `src-tauri/tauri.conf.json` (`withGlobalTauri: true`, `csp: null`)
  - `src-tauri/capabilities/default.json` (`"windows": ["*"]`, `core:default`)
- **Failure scenario:** Script running in any webview can bypass the JS allowlist in `bridge/aeval-bridge.js` by calling `window.__TAURI__.core.invoke('aeval_invoke', …)` directly. That covers the output, find, lens and buddy windows. The script can then:
  - send `agent:start`, `agent:consent` or `agent:confirm` (desktop control)
  - send `settings:save-key` or `shortcuts:set`
  - send `update:restart` or `app:quit`
  - use the `core:default` event, menu and tray permissions

  Today this is mitigated by each page's meta CSP (`script-src 'self'`) and by `renderer/markdown.js` escaping model output, so it is not exploitable as reviewed. One rendering bug in model output would turn it into desktop control.
- **Fix:**
  - Add a channel → allowed-labels table in Rust (for example, agent and settings channels only from `main` and `settings`).
  - Narrow the capability to the event listen/unlisten and window-drag permissions each window needs.
  - Set the CSP in `tauri.conf.json` as well.
  - Block navigation away from the app origin with `on_navigation`.

### P2-3. The click-through poll can re-apply ignore after `set_click_through(false)`, and it polls at 60 Hz forever
- **Where:** `src-tauri/src/windows/flags.rs:488-538` (`poll_once`), `:475-486` (`start_poll`)
- **Failure scenario:** `poll_once` snapshots `clicks` and calls `set_ignore_cursor_events(effective)` with the lock released. If main calls `set_click_through(label, false)` in between, the poll still applies `true` from its stale snapshot and then writes `effective` back. The bar stays click-through, so it cannot be clicked until the next mode change. Separately, the pill is always click-through, so the poll runs at 60 Hz for the whole session and calls `eval` on every mouse move over it.
- **Fix:** Re-check `requested` and `effective` under the lock right before calling `set_ignore_cursor_events`, or route it through main. Slow the poll down, or stop it when the cursor is far from every click-through window.

### P2-4. `flags::forget` by label can wipe a re-created window, and `open_settings` can show a window that is closing
- **Where:** `src-tauri/src/windows/mod.rs:1060-1063` → `src-tauri/src/windows/flags.rs:117-122`; `src-tauri/src/windows/mod.rs:905-921`
- **Failure scenario:** The user closes Settings and reopens it at once. The new `settings` window is adopted (new hwnd under the same label), and then `Destroyed` for the old one calls `forget("settings")`. That removes the new hwnd's `labels`, `metas` and `clicks` entries, so the new window loses its resize band and minimum size. A reopen before `Destroyed` arrives finds the closing window and calls `show()` on it, so nothing appears.
- **Fix:** Have `forget` take the hwnd and remove only matching entries. Track a "closing" set and create a fresh window when the existing one is closing.

### P2-5. A panic in an async IPC command leaves the promise pending and `busy` stuck, and the crash toast says "still running" for fatal panics
- **Where:** `src-tauri/src/ipc.rs:204-239`, `src-tauri/src/crash.rs:66` and `:77-92`, `src-tauri/src/lib.rs:178`
- **Failure scenario:**
  - A panic inside `aeval_invoke` (any `expect` or index in a route) kills the task. The renderer's promise never settles and `busy` stays true, so the overlay is stuck "thinking".
  - A panic in the `extern "system"` subclass proc aborts the process, as does a boot failure at `lib.rs:178`. In both cases the hook still toasts "It is still running".
- **Fix:** Wrap the body of `aeval_invoke` in `AssertUnwindSafe(..).catch_unwind()`, return `Err`, and reset `busy`. In the hook, choose the toast text by thread (main thread or boot means fatal), and wrap the subclass proc body in `catch_unwind`.

### P2-6. The updater's sha512 check fails open, and the in-flight wait can hang forever
- **Where:** `src-tauri/src/updater.rs:885`, `src-tauri/src/updater.rs:742-744`
- **Failure scenario:** A `latest.yml` without `sha512` (hand-made or truncated release) skips verification, and the downloaded exe runs with `/S`. Separately, if the check task that set `in_flight = true` panics, every later `check()` loops on `while in_flight` with no way out. The "Check for updates" button never answers, and the IPC call never returns.
- **Fix:** Treat an empty `sha512` as an error. Clear `in_flight` with a drop guard.

### P2-7. A double submit can leave two Ask streams running into one output (PLAUSIBLE impact)
- **Where:** `src-tauri/src/host/ask.rs:372` (`abort`), `:443-446` (`*current() = Some(..)`)
- **Failure scenario:** Two `query:submit` calls land on different tokio workers. The section between abort and set is synchronous, but it includes `outputs::show`, which makes main-thread round trips. Both calls run `abort()` while `current` is still `None`, and the later set overwrites the earlier token without cancelling it. Both streams then run, costing tokens, and both append assistant turns to `out.chat`. Only the stamped `gen` lets the renderer drop the stale deltas.
- **Fix:** Take the old token and install the new one atomically at the top: `let old = current().lock().replace(new); old.map(|c| c.cancel.cancel())`.

### P2-8. The single-instance pipe can be wedged, and the second-launch signal can miss a slow first boot
- **Where:** `src-tauri/src/instance.rs:56-68` (`signal`, 20 × 100 ms), `src-tauri/src/instance.rs:71-95` (`serve`)
- **Failure scenario:**
  - `read_to_end` has no timeout. One client that connects and never closes blocks the only server loop, and every later launch fails to bring the window forward.
  - A `create` error makes the loop `return`, which stops serving for good.
  - At a cold login with autostart, `serve` starts only after `setup`. A second launch in that window gives up after 2 s, and nothing happens.
- **Fix:** Wrap the read in `tokio::time::timeout`, and read one line instead of waiting for EOF. Retry `create` with backoff instead of returning. Start the server before the slow setup steps, and give `signal` a longer budget.

### P2-9. The tray menu is rebuilt in full on every refresh, with many blocking main-thread round trips
- **Where:** `src-tauri/src/tray.rs:290-299`
- **Failure scenario:** `refresh` runs on every `hotkeys::apply`, agent state change and update-progress tick. Each call builds about 20 items, and from a non-main thread each item creation blocks on main. During a download this stalls the calling tokio worker and main many times a second. Calling `set_menu` while the tray menu is open swaps the menu out from under the user.
- **Fix:** Keep the item handles and call `set_text` or `set_enabled` only on what changed. Throttle progress refreshes to about 1 Hz.

### P2-10. The debounce and animation-generation maps grow without bound
- **Where:** `src-tauri/src/windows/mod.rs:72-88` (`GEN` keyed by strings such as `output-N-resize` and `output-N-move`), `src-tauri/src/windows/motion.rs:94-104` (`generations` keyed by hwnd)
- **Failure scenario:** Every output window ever opened leaves entries behind. Over a long session with hundreds of outputs this is a slow leak, and hwnd reuse inherits a stale generation.
- **Fix:** Remove the entries in `outputs::on_destroyed` and `flags::forget`.

### P2-11. `overlay_state` holds the shared state lock across Win32 and network-state calls
- **Where:** `src-tauri/src/windows/mod.rs:258-322` (the lock is taken at :258; `active_region` is called at :263 and `online` at :309)
- **Failure scenario:** `region::active_region` and `updater::online` (`InternetGetConnectedState`) run with the shared lock held. A slow return stalls every thread that needs the lock, including `on_window_event` on main. This breaks the module's own "copy, drop, then act" rule.
- **Fix:** Gather those values before taking the lock.

### P2-12. History serializes the whole list on every save, and `list()` clones every message
- **Where:** `src-tauri/src/history.rs:197-235`, `src-tauri/src/history.rs:283-289`
- **Failure scenario:** With 200 long conversations, every streamed save and every history search keystroke clones or serializes megabytes while holding the history lock.
- **Fix:** Return summaries from `list()`, and serialize once per debounce window.

### P2-13. Keys silently fall back to plaintext when no OS crypt key is available
- **Where:** `src-tauri/src/store/crypto.rs` (the `key: None` paths)
- **Failure scenario:** This is parity with Electron, but combined with P0-4's `key: None` branch, an API key entered after a `Local State` problem is saved unencrypted with no warning.
- **Fix:** Log it, and show it in settings ("stored without encryption").

### P2-14. Each history debounce spawns a fresh OS thread
- **Where:** `src-tauri/src/history.rs:109-122` (`schedule_write`)
- **Failure scenario:** Streaming an answer triggers many saves, and each one spawns a thread that sleeps out the debounce. This also feeds the P0-3 overlap.
- **Fix:** Use one long-lived writer thread or tokio task with a channel and a deadline.

---

## Checked and found sound

- **Lock discipline:** `windows/*`, `outputs::with` closures, `hotkeys`, `tray`, `buddy`, `find` and `agent` follow "lock, copy, drop, then touch windows", with no lock held across a blocking Tauri getter. The exception is `menu.rs` (P2-1). parking_lot locks do not poison.
- **Win32 code:** `WM_NCHITTEST` sign extension, `WM_GETMINMAXINFO`, `WM_NCDESTROY` subclass removal, the `EnumDisplayMonitors` callback and the saturating f64 → i32 casts are all correct. `layout::clamp` cannot panic.
- **Custom protocol:** `aeval-frame` serves in-memory frames by key, so there is no filesystem path and no traversal.
- **Opening links and apps:** `open_url` allows only http(s). `open_target` validates the app name before `cmd /c start`.
- **Updater:** the installer file name is reduced to `file_name()`, so there is no traversal. reqwest uses rustls with normal certificate checks. The `latest.yml` map keys are always present.
- **Key handling:** keys reach the renderer masked and are never logged. DPAPI is scoped to the current user, by design.
