# Parity audit: IPC, events, history, updates, system, packaging, smoke, shots

Scope: PARITY.md sections 7 (IPC-01..84), 8 (EVT-01..38), 20 (HIST), 21 (UPD), 22 (SYS), 23 (PKG), 24 (SMK), 25 (SHOT).
Rust: branch `rust-rewrite` at 544f982. Electron reference: `main` (v1.1.1, 069a4b8). On the working tree, main.js, lib/ and preload.js are still the Electron files; preload.js differs from main only by `output:bar`, `output:close` and `settings:set-glass`.
Method: read-only. I read each row against the Electron source it cites, then against the Rust route or emitter. I did not run anything. Line numbers refer to the working tree at 544f982.

## 1. Summary

**172 rows: DONE 135 · PARTIAL 10 · BUG 5 · MISSING 11 · STUBBED 0 · DEPARTED 6 · UNSURE 5**

| Section | DONE | PARTIAL | BUG | MISSING | DEPARTED | UNSURE | Rows |
|---|---|---|---|---|---|---|---|
| §7 IPC | 74 | 4 | 3 | 0 | 3 | 0 | 84 |
| §8 EVT | 36 | 0 | 0 | 0 | 2 | 0 | 38 |
| §20 HIST | 5 | 1 | 0 | 0 | 0 | 0 | 6 |
| §21 UPD | 11 | 1 | 2 | 1 | 0 | 0 | 15 |
| §22 SYS | 4 | 1 | 0 | 0 | 0 | 0 | 5 |
| §23 PKG | 4 | 1 | 0 | 0 | 0 | 5 | 10 |
| §24 SMK (grouped rows) | 1 | 1 | 0 | 5 | 1 | 0 | 8 |
| §25 SHOT | 0 | 1 | 0 | 5 | 0 | 0 | 6 |

Smoke labels (§24 plus the SMOKE+ labels named in §7, §8, §22 and §26):
- 209 labels in total. 3 are superseded by §26 (hudDockedDefault, hudDockToggle, storedOutputPos), which leaves **206 required**.
- **2 are emitted** under `[SMOKE]` today: `version` and `done`.
- **204 are missing.** 33 of the missing ones (the desktop selfcheck) are already implemented, but they only print as `[DESKTOP]` under `AEVAL_DESKTOP_CHECK`.

Marker grep over src-tauri/src:
- No `todo!`, `unimplemented!`, `TODO`, `FIXME` or "not yet".
- 13 `#[allow(dead_code)]` sites say "Called by the smoke and shot harnesses (agent e), not merged yet": agent/run.rs:642,805; backdrop.rs:602,613,619; backdrop/contrast.rs:10; desktop/client.rs:252,1050,1059,1072,1090; lens.rs:280,296,306.
- `ipc::not_ported` (ipc.rs:628-640) cannot be reached for an allowlisted channel.
- No allowlisted channel falls through to a default value where Electron returned data. The route's default `Ok(Value::Null)` (ipc.rs:625) only answers channels whose Electron handler returns `undefined`. The one no-op, `hud:set-dock` (ipc.rs:457), is DEPARTED under §26.

## 2. Blocking

Ordered by severity. Each item gives: what is wrong, where it is in Rust, the Electron source, and the fix.

### B1. MISSING: smoke harness (SMK-01, 03, 04, 05, 06, 07; SMK-02 PARTIAL)
- **Problem:** `AEVAL_SMOKE` runs only the scaffold. It prints `version`, `userData`, `webviewData`, `scaffold.pages` and `scaffold.bridge`, then `[SMOKE] done`, and exits 0. None of the 204 other required labels is printed.
  - installer-check `smoke.exitCode` and `smoke.printed` still pass, but only because they accept any `[SMOKE]` line.
  - rust.yml only checks `scaffold.bridge=ok`.
- **Rust:** src-tauri/src/selftest.rs:1-57 (`scaffold`). desktop/checks.rs:27-52 runs the selfcheck only under AEVAL_DESKTOP_CHECK/AEVAL_CAPTURE_CHECK.
- **Electron:** main.js:3213-3952 (runSmoke), lib/desktop/selfcheck.js, lib/smoke-v2.js, lib/smoke-v21.js, lib/smoke-glass.js, lib/smoke-v2-desktop.js.
- **Fix:** see section 3 for the per-label worklist. Harness capabilities to build first:
  1. **Eval with a result** in a named webview. Tauri's `eval` returns nothing, so use `WebviewWindow::with_webview` with `ICoreWebView2::ExecuteScript` and its completion handler (webview2-com). Needed for markdown, transcriptScroll, chatCeiling, scrollThumb, moreIndicator, noOutputInput, styleSamples and intentTable.
  2. **Real input injection** with `SendInput` (wheel, drag, keys). Needed for transcriptWheel, outputDrag, resizeEdges, keySwallow, noGlobalEnter, regionEscape and barKeys.
  3. **Run the desktop selfcheck inside the smoke** with `Report::new("SMOKE")` (desktop/checks.rs:37 already takes a prefix).
     - Pass `ownPoint`: the pill centre, converted with `windows::screen::dip_to_px`.
     - Pass `ownHandles`: every webview window's HWND.
     - Run the Notepad half only when `GITHUB_ACTIONS` or `AEVAL_DESKTOP_NOTEPAD` is set (main.js:3833-3842).
  4. **Fake LLM** for failover, Ask and agent probes: `AEVAL_FAKE_LLM` already exists (llm/client.rs:237-239, llm/fake.rs).
  5. **Must/log reporting:**
     - `must(label, ok, why)` prints `label=FAIL:why`.
     - The final line is `[SMOKE] failed: a, b` with exit 1, or `[SMOKE] done` with exit 0.
     - An uncaught panic prints `[SMOKE] error=...` with exit 1.
     - Each block (v2basics, v21basics, glassGates, v2desktop) gets a catch that prints `must(<block>, false, msg)` (main.js:3881-3914).
  6. **Exit code** must follow the musts, and stdout must be flushed before `app.exit`.
- **Also edit rust.yml:116-138.** The "Scaffold smoke" step should fail on any `=FAIL` line and on a missing `[SMOKE] done`, not only on `scaffold.bridge`. Raise the 90 s timeout to the 360 s installer-check allows.

### B2. MISSING: visual self-check, `AEVAL_SHOT` (SHOT-01..05)
- **Problem:** nothing reads `AEVAL_SHOT`, `AEVAL_SHOT_MATRIX_ONLY` or `AEVAL_SHOT_SCALE`. lib.rs:39-50 (`quiet_run`) and updater.rs:616 only use them to stay quiet.
  - Under installer-check the exe boots normally and never exits.
  - `shots.exitCode` therefore ends in `timeout` after 600 s and `shots.written` gets 0 PNGs.
  - `shots125.*` and `shots150.*` time out after 400 s each.
  - That is 6 failed Checks and about 23 minutes of runner time.
- **Rust:**
  - No handler exists.
  - The contrast gate exists as dead code (backdrop/contrast.rs:10).
  - Nothing implements CapturePreview. A grep finds only a comment in contrast.rs:5.
- **Electron:** lib/shots.js (frames 01-17, `[SHOT] fit.<name>=window<h>:surface<h>` at main.js:3080), lib/shots-v2.js (18-30), lib/shots-matrix.js (`m-<surface>-<backdrop>-<scale>.png`, `[SHOT] matrix scale=.. contrastMin=.. failedFrames=..`).
- **Fix:**
  1. Add `selftest::shots(app, dir)`, selected when `AEVAL_SHOT` is set.
     - Drive each surface into the named state through the same hooks the IPC uses.
     - Photograph each window with `ICoreWebView2::CapturePreview` (PNG) through `with_webview`.
     - Use a real screen grab (capture/screen.rs) for `12-live-overlay`, with content protection lifted for one frame.
     - Write `<dir>/<name>.png` and print `[SHOT] <name>=<w>x<h>`.
  2. When `AEVAL_SHOT_MATRIX_ONLY=1`, apply `AEVAL_SHOT_SCALE`.
     - Electron uses `--force-device-scale-factor`. The WebView2 equivalent is the `--force-device-scale-factor` browser argument through `additional_browser_args`, set before the webviews are created.
     - Render the 13 matrix surfaces over the white, black and wallpaper fixtures.
     - Send every frame through `backdrop::contrast`.
     - Print the summary line and exit 1 if `failedFrames>0` (SHOT-04).
  3. Exit 0 on success, and always exit within 600 s.

### B3. BUG: updater stops retrying after two identical failures (UPD-02, IPC-34)
- **Problem:** `fail()` drops any failure whose message equals the previous one (updater.rs:561-568). `last_error` is cleared only when a check succeeds (updater.rs:760-761).
  - `check()` has already called `clear_timer()` and set `status:"checking"` (updater.rs:753-754).
  - A second identical failure therefore returns early: no retry is scheduled and the status stays `checking`.
- **Failure scenario:**
  1. GitHub answers 503 (or DNS fails while `InternetGetConnectedState` says online) on the 20 s first check. The status becomes `retrying`, with a retry in 2 min.
  2. The retry fails with the same text. `fail()` returns at line 565.
  3. The status is stuck at `checking` with a stale `nextCheck`. No background check runs again until resume or restart.
  4. A manual "Check for updates" (IPC-34) that hits the same error returns `{status:"checking"}`, so Settings shows "Checking…" forever.
- **Electron:** lib/updater.js:302-304 dedupes by object identity (`err === lastError`). That only collapses the 'error' event and the rejection of the same attempt, so every new attempt is counted and rescheduled.
- **Fix (updater.rs):** dedupe per attempt, not per message.
  - Give `check()` and `download()` an attempt id and pass it to `fail(id, …)`.
  - Return early only when `id == last_failed_attempt`.
  - Simpler alternative: drop the dedupe, since the Rust port has no second error channel per attempt.
  - Add a unit test: two `fail("x")` calls in a row give `failures == 2` and a scheduled timer.

### B4. BUG: the installer hook never removes the Electron copy (UPD-15, PKG)
- **Problem:** NSIS_HOOK_PREINSTALL has two checks that can never pass for an Electron install:
  - It reads `InstallLocation` from `HKCU\…\Uninstall\<key>` (installer-hooks.nsh:25). electron-builder writes InstallLocation under `HKCU\Software\{APP_GUID}` (app-builder-lib templates/nsis/multiUser.nsh:8, include/installer.nsh:104). The Uninstall key only gets DisplayName, UninstallString, QuietUninstallString, DisplayVersion, DisplayIcon and Publisher (installer.nsh:119-133).
  - It also requires `DisplayName == "AevalBuddy"` (line 31). electron-builder writes `"${productName} ${version}"`, i.e. `AevalBuddy 1.1.1` (NsisTarget.js:473).
  - So `StrCmp $3 ""` always loops, and the Electron copy is never uninstalled.
- **Failure scenario:** electron-updater runs the Tauri installer.
  - Two installs remain: `%LOCALAPPDATA%\Programs\aevalbuddy` (Electron) and Tauri's `%LOCALAPPDATA%\AevalBuddy`.
  - Add/Remove Programs shows two "AevalBuddy*" entries.
  - Uninstalling the stale Electron entry later deletes the shared `AevalBuddy.lnk` Start Menu and Desktop shortcuts (uninstaller.nsh, `WinShell::UninstShortcut` / `Delete $oldStartMenuLink`), which the Rust install also owns.
- **Fix (installer-hooks.nsh):**
  - Match `DisplayName` by prefix: `StrCpy $2 $2 10` then compare with `AevalBuddy`.
  - Get the folder from `UninstallString`, which is `"<dir>\Uninstall AevalBuddy.exe" /currentuser`: strip the quotes and `\Uninstall AevalBuddy.exe`. Alternatively read `HKCU\Software\<APP_GUID>` InstallLocation, where APP_GUID is electron-builder's UUIDv5 of appId `com.aevalbuddy.app`.
  - Keep the `IfFileExists "$3\Uninstall AevalBuddy.exe"` guard.
  - Proof needed: an old-Electron → Rust A → Rust B run on a runner (the UPD-15 verify), plus `reg query` showing one entry.

### B5. MISSING proof: installer.yml still builds and checks the Electron installer (PKG-03..07, SHOT-06)
- **Problem:** installer.yml runs on `main` only, builds with `npm run build` (electron-builder) and passes `dist\AevalBuddy-Setup.exe` to installer-check.ps1. No workflow runs installer-check against the Tauri NSIS output.
  - rust.yml builds the NSIS installer but never installs it.
  - Every INSTALL:* row is therefore unproven for Tauri.
- **Rust:** .github/workflows/rust.yml:113-114 builds it; .github/workflows/installer.yml:39-42 and 85 build and check Electron.
- **Fix:** add a rust.yml job, or a `rust-rewrite` trigger in installer.yml, that:
  1. downloads the `aevalbuddy-tauri-nsis` artifact;
  2. reads the version from package.json;
  3. runs `tools/installer-check.ps1 -Installer <tauri setup.exe> -Version <v> -Out out`;
  4. publishes the shots the way installer.yml does (SHOT-06: `shots` branch, newest 15).

### B6. BUG: `history:open` keeps the global picked area (IPC-18)
- **Problem:** Reopening a stored conversation does not clear `region::ACTIVE`. `outputs::open_conversation` clears only the per-output `chat.region` and `last_question` (outputs.rs:415-416). `submit_to` reads the global `super::active_region()` for every question (host/ask.rs:392).
- **Failure scenario:**
  1. Pick an area with Ctrl+Shift+Space, ask, then open History and reopen an older text-only conversation.
  2. Type a follow-up. Rust captures only the old area. Electron captured the whole screen.
- **Electron:** main.js:1921-1942 (`activeRegion = null; lastQuestion = ''; store.setOutputHeight(0)`).
- **Fix (ipc.rs:436-447):** call `crate::region::clear_active()` before `outputs::open_conversation`, as the `chat:new` arm does at ipc.rs:428. The per-output `user_height = 0.0` already covers `setOutputHeight(0)`.

### B7. BUG: `buddy:card` "try" opens an empty bar (IPC-69, EVT-31)
- **Problem:** `on_try` calls `windows::open_bar(&a2, Some("agent"), Some(String::new()))` (host/mod.rs:339).
  - The tour's "Try it" card opens the Agent bar with no task.
  - It also emits `bar:prefill {text:""}`, which Electron never sent.
- **Electron:** main.js:2430-2433, `openBar('agent', { prefill: 'open Notepad and write a haiku' })`.
- **Fix (host/mod.rs:339):** `Some("open Notepad and write a haiku".to_string())`.

### B8. PARTIAL: `region:pick` from the bar skips Electron's guards (IPC-12)
- **Problem:** the IPC arm calls `region::pick` directly (desktop/channels.rs:33-39).
  - It has no HUD guard. Electron returns when `overlayMode === 'hud'` (main.js:1233).
  - It does not bracket the pick with `apply_registrations`. The start_region hook does (host/mod.rs:369-376). Escape is only claimed by the `on_region_change` task that `region::changed()` spawns before `select` sets the active flag (region.rs:219-220). That is a race, so a global Escape may not cancel a selector the bar's button opened.
- **Fix (desktop/channels.rs:33-39):**
  - Return early when `shared(app).lock().mode == Mode::Hud`.
  - Otherwise route through `shared(app).call(app, |h| h.start_region.as_ref())`, as the hotkey path does (windows/mod.rs:698-700), instead of calling `region::pick` itself.

### B9. MISSING/PARTIAL: smoke update-feed retries and `AEVAL_UPDATE_EXPECT` (UPD-13, UPD-12)
- **Problem:** nothing reads `AEVAL_UPDATE_EXPECT`, and there is no 45 s per-attempt limit with retries after 10 s and 30 s on 5xx or timeouts.
  - The feed override (updater.rs:174, test `feed_override_rules`) is done.
  - `AEVAL_UPDATE_QUIT` (updater.rs:801-807) is done.
- **Electron:** main.js:3287-3320.
- **Fix:** in the smoke harness (selftest.rs):
  - Call `updater::check` up to 3 times, each under `tokio::time::timeout(45 s)`.
  - Between attempts, sleep 10 s then 30 s when the cause is 5xx or a timeout.
  - Print `update.retryN`, `update.attempts`, `update.status`, `update.version`, `update.problem` and `updateFeed`, with expect = `AEVAL_UPDATE_EXPECT || "current,downloading,ready"` (`any` passes everything).
  - Print `updateQuiet` and `update.cause`.

### B10. PARTIAL: the installer is a wizard, not one-click (PKG-01)
- **Problem:** Tauri's NSIS template shows its pages (welcome, folder, install, finish) on an interactive run. electron-builder's `oneClick` installed with no pages. Per-user and `/S` are right (tauri.conf.json:27-30). There is no DESIGN.md entry for this departure.
- **Fix:** either record the departure in DESIGN.md, or set `bundle.windows.nsis.template` to a copy of Tauri's installer.nsi with the pages removed (`!insertmacro MUI_PAGE_*` dropped, `SetSilent` behaviour kept).

### B11. PARTIAL: `settings:get` has no key-set unit test, and its keys differ (IPC-22)
- **Problem:** `settings_data` (ipc.rs:321-363) returns every Electron key plus `keys`, `glassIntensity` and `hasSentFirstRequest`. The extras are DEPARTED: multi-provider keys, glass intensity and the first-request flag, per REQUESTS 2, 3, 5 and NEW-10. The row's required `UNIT (key set)` test does not exist.
- **Fix (ipc.rs tests):** split the key list out of `settings_data` into a pure function and assert it equals the 28 Electron keys from main.js:2525-2562 plus the 3 documented extras.

### B12. PARTIAL: launch at login ignores StartupApproved even on an explicit toggle (SYS-03)
- **Problem:** autostart.rs:28 writes or removes only the Run value. Electron's `app.setLoginItemSettings({openAtLogin})` (main.js:1946-1958, Electron 38, default `enabled:true`) also writes `StartupApproved\Run` as enabled.
  - Rust matches the PARITY wording ("StartupApproved respected") but not the cited source.
- **Failure scenario:** the user disables AevalBuddy in Task Manager > Startup, then switches "Launch at login" off and on in Settings. Settings shows on, but Windows never starts the app.
- **Fix (autostart.rs):**
  - When the switch turns on (`settings:set-launch-at-login {on:true}` only, not at boot), write `HKCU\…\Explorer\StartupApproved\Run` value `com.aevalbuddy.app` as REG_BINARY `02 00 00 00 00 00 00 00 00 00 00 00`.
  - At boot, leave it alone.
  - Record the boot-time departure in DESIGN.md.

### B13. PARTIAL: the history budget counts UTF-8 bytes, not UTF-16 units (HIST-01)
- **Problem:** `MAX_BYTES = 4*1024*1024` is compared with `json_len` in bytes (history.rs:17, 286). Titles are cut by `char` (history.rs:48). Electron compares the JSON string's `.length`, which counts UTF-16 code units, and cuts with `slice`.
- **Failure scenario:** Chinese or Japanese text costs 3 bytes per unit. Rust trims a history.json that Electron kept, so an upgraded user loses old conversations on the first save. Emoji titles also differ by one character.
- **Fix (history.rs):** `json_len` should return `serde_json::to_string(..).encode_utf16().count()`. Build titles from `encode_utf16()`, taking 89 units plus `…`, and do not split a surrogate pair. Add a CJK unit test.

### B14. PARTIAL: `settings:use-clip-key` advances the tour only after the key passes a test (IPC-78)
- **Problem:** ipc.rs:498-510 calls `key_saved` (host/mod.rs:227-269), which tests the key first. Electron advanced tour step 0 at once (main.js:2575-2582). With a rejected key, Rust shows the test message and stays on step 0.
- **Fix:** match Electron by advancing when `stepIndex == 0` before testing, or record the stricter behaviour in DESIGN.md. Low severity.

### B15. PARTIAL: `output:resize` also applies during a glide (IPC-06)
- **Problem:** `outputs::resize` sets bounds even while the window is animating. Electron stores the height and returns while `motion.isAnimating(outputWin)` (main.js:2446-2451).
- **Fix (windows/outputs.rs resize):** store `content_height`, then return when `motion::is_animating(label)`. The glide's landing re-syncs.

## 3. Smoke worklist

**Status key:** E = emitted under `[SMOKE]` today. M = missing. D = implemented, but printed as `[DESKTOP]` only. S = superseded (§26). **Bold** = `must`.
"Line" is the main.js line that holds the exact expression and format; copy it from there.

### 3.1 installer-check.ps1: every Check, judged against the Tauri build

| Check | Requires | Tauri today |
|---|---|---|
| install.exitCode | `/S` install exits 0 | UNSURE, likely passes (Tauri NSIS, currentUser). Not yet run on a runner (B5) |
| install.registered | HKCU/HKLM Uninstall entry with DisplayName `AevalBuddy*` | UNSURE, likely passes (Tauri writes `Uninstall\AevalBuddy`, DisplayName = productName) |
| install.version | DisplayVersion = package version | UNSURE, likely passes (version comes from ../package.json) |
| install.exe | `AevalBuddy.exe` beside the uninstaller named in UninstallString | UNSURE, likely passes (mainBinaryName AevalBuddy; uninstall.exe in $INSTDIR) |
| install.exeVersion | exe version is `V` or `V.0` | UNSURE (tauri-build writes version resources from 1.1.1) |
| install.startMenuShortcut | `%APPDATA%\…\Start Menu\Programs\AevalBuddy.lnk` | UNSURE. Tauri puts it in a subfolder when `startMenuFolder` is set; it is not set here |
| install.desktopShortcut | `Desktop\AevalBuddy.lnk` | UNSURE. Tauri creates it in silent mode unless `/NS` |
| install.notLaunched | no AevalBuddy process after `/S` | UNSURE, likely passes (Tauri relaunches only with `/R`; the hook runs the app only with `--force-run`) |
| smoke.exitCode | `AEVAL_SMOKE=1` exits 0 within 360 s | Passes, with the scaffold only (selftest.rs:28-57) |
| smoke.printed | any `^\[SMOKE\]` line on stdout | Passes. PKG-09 is satisfied, because a GUI-subsystem println reaches redirected handles (rust.yml:116-138 depends on it) |
| shots.exitCode | `AEVAL_SHOT=dir` exits 0 within 600 s | **FAILS**: unhandled, ends in timeout (B2) |
| shots.written | at least one `*.png` | **FAILS** (B2) |
| shots125.exitCode / shots125.written | matrix at 1.25 writes `m-*-125.png` | **FAILS** (B2) |
| shots150.exitCode / shots150.written | matrix at 1.5 writes `m-*-150.png` | **FAILS** (B2) |
| uninstall.exe / uninstall.registry / uninstall.startMenuShortcut / uninstall.desktopShortcut | QuietUninstallString (else UninstallString `/S`) removes the exe, the entry and both shortcuts | UNSURE, likely passes. Tauri's uninstaller removes them; user data in `%APPDATA%\aevalbuddy` is outside Tauri's delete-app-data paths |

installer-check does not look for any particular `[SMOKE]` label; `=FAIL` lines become annotations only. The §24 labels below are needed for PARITY, not for installer-check.

### 3.2 main.js runSmoke (109 labels: 2 E, 104 M, 3 S)

| Label | Today | Pass condition or value (line) | How the Rust smoke produces it |
|---|---|---|---|
| **windows** | M | overlay and output both exist (3227) | `windows::window(app, OVERLAY)` and `outputs::spare(app)` present |
| version | E | `app.getVersion()` (3228) | selftest.rs:41 |
| shortcut.captureQuery | M | lastRegisterResults (3229) | `hotkeys::active(app)["captureQuery"]` (hotkeys.rs:345) |
| shortcut.captureRegion | M | (3230) | same, `captureRegion` |
| **tray** | M | `tray.isReady()` (3233) | `tray::is_ready(app)` (tray.rs:285) |
| **updateConfig** | M | app-update.yml says provider github, owner aevalmere, repo aevalbuddy (3244-3250) | No app-update.yml in Tauri. Assert the compiled feed constants in updater.rs (release base / owner / repo) |
| **updateClassify** | M | classify table (3255-3262) | `updater::classify` on the same inputs (the unit test exists; print its verdict) |
| **updateNotes** | M | `=== 'Faster area capture · Fix & tidy'` (3267) | `updater::notes_snippet` on the same fixture HTML |
| update.retry1 / update.retry2 | M | lastCause per retry (3305) | B9 retry loop |
| update.attempts | M | causes joined by `,` (3308) | B9 |
| update.status / update.version / update.problem | M | from the state after the check (3309-3311) | `updater::get()` fields |
| **updateFeed** | M | expect `any`, or the status is in `AEVAL_UPDATE_EXPECT` (default `current,downloading,ready`) (3313-3317) | B9 |
| **updateQuiet** | M | problem does not match `/error\|exception\|stack\|http/i` (3318) | regex on `updater::get()["problem"]` |
| update.cause | M | only when retrying/error (3320) | B9 |
| shortcut.toggleVisibility | M | (3322) | `hotkeys::active` |
| model | M | `store.getModel()` (3323) | `store::get().get_model()` |
| modelList | M | `<tiers>:live\|fallback` (3326) | `host::ask::models_list(kind vision)` (host/ask.rs:719) |
| modelListPaidHidden | M | every group is free (3329) | same, showPaid off |
| **modelDefaultOffered** | M | DEFAULT_MODEL is in the live free group (3331-3335) | catalogue `list_models` plus the default constant |
| modelListWithPaid | M | `<tier><n>` joined by `,` (3337) | `list_models(..., true)` |
| **brainListDefault** | M | DEFAULT_BRAIN is in the live free text group (3340-3344) | as above, Kind::Text |
| healBrain / healModel | M | the healed ids (3351-3352) | heal the retired ids `nvidia/nemotron-3-super-120b-a12b:free` and `nvidia/nemotron-nano-12b-v2-vl:free` through `live_models` |
| **healRetired** | M | the healed ids are in the live free lists (3353-3357) | same |
| brainPrompt | M | `ok` when it contains `cannot see the screen` and `[[point` (3368) | llm/prompt.rs brain prompt |
| brainDefault / brainEnabledDefault | M | store values (3374-3375) | store getters |
| skills | M | `<n>:<ids joined by +>` (3379) | `skills.list()` (skills.rs:216) |
| skillInPrompt | M | `ok` when it contains `Screen assistant` and `0 to 1000 grid` (3385) | `make_system_prompt` (llm/prompt.rs:193) |
| promptShape | M | `nosize:lines` (3395) | same, 1280x800 |
| gridShared | M | `ok`, viaReader includes `point ` (3403) | same |
| skillFallback | M | `screen-assist` (3409) | `skills.get("no-such-skill")` (skills.rs:226) |
| skillDefault | M | (3410) | store skill id |
| skillRoute | M | `debug,explain,rewrite,screen-assist` (3414) | `skills.route` (skills.rs:240) on the same 4 questions |
| skillRouteFromScreen | M | route({question:'x', task:'rewrite'}) (3415) | same |
| widthDefaults / pillWidthHeld / pillPinned / panelWidthRange / panelHeightPinned / panelWidthHeld / widthAfterCycle | M | 3418-3434 | `windows::layout` and the overlay min/max sizes through Win32 `GetWindowRect`; cycle pill → panel → pill |
| outputAutoHeight | M | `store.getOutputHeight() === 0` (3435) | `store::get()` output height |
| motionAnchor | M | `top-right-held` (3439) | `windows::motion` grow/shrink (motion.rs:176) |
| capture | M | `ok:<bytes>bytes:<w>x<h>` (3446) | `capture::capture_screen_async(None)` |
| markdown | M | `ok`: `<li>`, `<strong>`, `md-h` (3453) | **eval-with-result** in the output webview |
| transcriptScroll | M | 3477 | eval-with-result |
| transcriptWheel | M | `moved:N` (3501) | **SendInput** wheel over the output, then eval scrollTop |
| chatCeiling / scrollThumb / answerHeightUnderLongChat | M | 3507-3513 | eval-with-result plus window bounds |
| answerFollowsTranscript | M | `exact` (3520) | the window height equals the eval'd surface height |
| moreIndicator | M | 3531 | eval-with-result |
| pointExtract | M | `<n>:clean:tight` (3541) | ask.rs point extraction |
| pointShapes | M | `6/6` (3565) | same, the DRIFT table |
| pointNonDirective | M | `kept` (3570) | same |
| mode / hasKey | M | 3572-3573 | `shared.mode`, `store.has_api_key()` |
| taskbarHidden / overlayOnTop / outputOnTop | M | 3577-3579 | Win32 `WS_EX_TOOLWINDOW` / `WS_EX_TOPMOST` on the HWNDs |
| settingsWindow | M | `onTop<bool>` (3582) | `windows::open_settings`, then the topmost style |
| captureProtection | M | `on` or `unsupported:build<N>` (3587) | `GetWindowDisplayAffinity` |
| readerElements / pointByName / readerRowShapes (`6/6`) / readerPixelRow / pointAmbiguous / pointNamedExtract | M | 3600-3650 | ask.rs `parse_elements` / `match_element` ports |
| pointResolved | M | buddy active (3657) | `buddy::is_active` |
| pointGridMapping | M | `{"fx":0.5,"fy":0.5}` (3662) | `ask::point_fraction` (ask.rs:65) |
| pointPixelRescue / pointBottomEdge / pointOffScreen (`null`) | M | 3664-3667 | same |
| regionCapture | M | `<w>x<h>:tagged` (3677) | `capture_screen_async(Some(rect))` |
| regionSharper / regionPointTarget | M | 3686-3697 | same |
| regionSelectorCancels | M | `true` (3701) | `region::select` then `region::cancel` |
| regionEscape | M | `cancelled:panel-kept` (3708) | **SendInput** Escape during `region::pick` |
| regionPrompt | M | `ok`, includes `one area` (3714) | make_system_prompt viaRegion |
| failoverRateLimit / failoverKeepsAuthError (`threw:401`) / failoverCapped (`<n><=3:true`) | M | 3734-3759 | llm/failover.rs with `AEVAL_FAKE_LLM` fixtures; `MAX_MODEL_ATTEMPTS` (failover.rs:11) |
| historySaved (`2:3`) / historyTitle (`ok`) / historySearch / historyRemove (`1`) / historyRespectsSwitch (`1:skipped`) / historyCleared (`0`) / historyRetryGuard (`nothing-to-retry`) | M | 3787-3809 | `history::get()` on a scratch dir; retry guard via the output's `last_question` |
| queryWithoutKey | M | `needs-key:chat0` (3811) | `host::ask` submit with no key; chat length of the target output |
| hudMode / hudSnapFound (`left-top`) / hudExitMode / storedPos | M | 3813-3827 | `windows::enter_hud` / layout snap / `exit_hud`; store overlay pos |
| hudDockedDefault / hudDockToggle / storedOutputPos | S | §26 | replaced by outputFloating, outputMulti, noOutputLayout (3.7) |
| v2desktop (skip note) | M | `skipped (...)` when not on CI (3917) | print when neither GITHUB_ACTIONS nor AEVAL_DESKTOP_NOTEPAD is set |
| buddyPoints / buddyPointsDone / walkthrough | M | 3922-3926 | buddy.rs start points, advance, start_walkthrough |
| **crashLogged** | M | an unhandled rejection reaches errors.log (3937) | panic in a spawned task, then read `crash::file()` (crash.rs:54) |
| **v2basics** / **v21basics** / **glassGates** / **v2desktop** | M | FAIL only when the block throws (3881-3914) | catch per block |
| done / failed / error | E (hollow) | 3945 | selftest.rs:50-54; must follow the musts |

### 3.3 lib/desktop/selfcheck.js (33 labels: all D)

All are implemented in desktop/selfcheck.rs and printed by `AEVAL_DESKTOP_CHECK` as `[DESKTOP]` (rust.yml:172-197). To emit them, call the same `run` from the smoke with `Report::new("SMOKE")`, `ownPoint` set to the pill centre in px, and `ownHandles` set to every window HWND. Pass `notepad = GITHUB_ACTIONS || AEVAL_DESKTOP_NOTEPAD`.

desktopError (D), **desktopReady** (D), desktopReadyMs (D), desktopDpi (D), **desktopWindows** (D), desktopWindowCount (D), **desktopOwnExcluded** (D), **ownWindowBlocked** (D), desktopNotepad (D), **desktopNotepadIncluded** (D), notepadWindow (D), focusMethod (D), uiaTreeNodes (D), uiaTreeMs (D), uiaTreeRoles (D), editor (D), ocrMs (D), **ocrWords** (D), ocrDetail (D), typeFocus (D), typeReadText (D), **uiaTypeRead** (D), uiaSetValueMs (D), uiaSetValueVia (D), **uiaSetValue** (D), getTextSpans (D), **findEditor** (D), findTop (D), elementAt (D), **notepadClosed** (D), tempFileLeft (D), closeError (D), dontSave (D).

### 3.4 lib/smoke-v2.js (9 labels: all M)

| Label | Pass condition | How |
|---|---|---|
| **helperNotAtBoot** | the desktop helper process is not running after boot | `desktop::client()` not started; no child process |
| bootMs (+**bootMs** < 15000) | process start to first overlay paint | `Instant` at `main` to the overlay's PageLoadEvent::Finished |
| **trayOrder** | exact tray label lists, idle and running | `tray::labels(&Input{..})` (tray.rs:179) for both states, compared with smoke-v2.js's lists |
| shortcuts | registered chords | `hotkeys::active` |
| **shortcutsOneChord** | one chord per action, no duplicates | store shortcuts |
| barKeys (+**barKeys**) | Ctrl+1..3 mode keys and Enter in the bar | SendInput into the focused bar, then read `bar:mode` |
| **onboardingSteps** | `0>1>2` | drive the tour with `buddy:skip` and `settings:save-key` (fake key) |
| windowsChecked | the windows examined | count |
| **contentProtectedAll** | every window `WDA_EXCLUDEFROMCAPTURE` when canHide | `GetWindowDisplayAffinity` on every webview HWND |

### 3.5 lib/smoke-v21.js (15 labels: all M)

| Label | Pass condition | How |
|---|---|---|
| gateBypasses (+**gateBypasses**) | the agent gate's bypass table | agent/gate.rs (gate.rs:308 test seam) |
| **gatePassesPlainInput** / **gateFailsClosed** / **gatePlanFirst** / **gateChords** | the gate verdicts | agent/gate.rs |
| intentTable (+**intentTable** ≥ 40 rows, 0 wrong) | renderer/intent.js table | **eval-with-result** in the overlay webview (intent.js stays JS) |
| **intentPrefixes** | prefix routing | same |
| intentWorstMs / **intentFast** (< 5 ms) | timing | same, `performance.now()` |
| **intentDoNeedsMargin** | | same |
| keySwallow | the bar swallows keys it handles | SendInput |
| **noGlobalEnter** | Enter is not registered globally | `hotkeys` bound set has no Enter |
| **noGlobalEscWithBar** | Escape is not global while only the bar is open | the hotkeys `escape_registered` flag |
| **barOpensUnlocked** | `false:ask` | `windows::open_bar(None)`, then read the lock and mode |
| **feedOverrideStrict** | feed override rules | `updater::feed_override_from` (unit test exists; print its verdict) |

### 3.6 lib/smoke-glass.js (9 labels: all M)

| Label | Pass condition | How |
|---|---|---|
| glassTier | tier and reasons | backdrop tier |
| **glassLoaded** | `glass:(live\|solid)` | every glass page has called `glass:hello` (ipc::Seen), plus the tier |
| **noSetOpacity** | no window opacity animation | assert that no `SetLayeredWindowAttributes` alpha is in use (flags) |
| snapCost (+**snapCost** blockP95 < 25 ms) | main-thread block per frame | `backdrop::measure(n)` (backdrop.rs:767) |
| glassCost | | same |
| styleSamples / **styleStable** | computed styles stable across frames | eval-with-result |
| collapseClickable (+**collapseClickable**) | pill clickable after collapse | `flags::is_click_through(OVERLAY)` false over the pill; SendInput click |
| **topmostHolds** | ≥ 15 late samples, never buried | sample `GetWindow(GW_HWNDPREV)` / topmost over 60 s, with keep_on_top (windows/mod.rs:358) |

### 3.7 lib/smoke-v2-desktop.js, CI only (23 labels: all M)

| Label | Pass condition | How |
|---|---|---|
| **v2NotepadOpen** | Notepad opened with fixture text | desktop client helpers (client.rs:1050-1090 dead-code exports) |
| lensFirstDraw (+**lensFirstDraw**) | `3:true:true` | lens.rs:280-306 exports plus `lens:draw` on `lens-<id>` |
| lensWhyClear (+**lensWhyClear**) | covers=0 | same |
| findLiteral / findLiteralMs (< 50) | literal hit | `host::find` query, literal |
| **lensHits** / **findStep** / **findRelated** / findRelatedWhy / **findConcept** / findConceptHit | | host/find.rs with the fake LLM |
| **findOffline** | offline status | `AEVAL_OFFLINE=1` path (host/mod.rs `online`) |
| **askHasUiaText** | the Ask prompt carries UIA text | `bar_context` (host/ask.rs:135-150) |
| **pointFromUia** | ≤ 4 px off | ask point resolution against Notepad's UIA rect |
| agentRun / **agentFocusKept** / **agentJsonMode** / agentStopMs (< 300) / **agentTakeover** / **agentMaxSteps** | | agent/run.rs with the fake LLM; `dispatched()` (run.rs:642) |
| v2NotepadClosed | | desktop client |
| **agentConfirmGate** | the confirm id gate | `agent.confirm` with a wrong id returns false (run.rs:761) |
| **passwordRedacted** | | agent redact |

### 3.8 SMOKE+ labels named by PARITY (11 labels: all M)

| Label | Row | Pass condition | How |
|---|---|---|---|
| eventTargeting | §8 | the output window never receives `bar:open` | the internal bus in `windows::emit` (windows/mod.rs:54): record (label, channel) and assert none is (output-*, bar:open) |
| contextMenus | IPC-63 | templates as data | host/menu.rs templates (unit test `menus_match_main_js` exists; print it) |
| launchAtLogin | IPC-32 / SYS-03 | the Run value after a toggle | `reg` read of `HKCU\…\Run\com.aevalbuddy.app` |
| outputFloating | NEW-01 | the output is not docked and opens beside the bar | output bounds vs bar bounds |
| outputMulti | NEW-02 | 3 open, all visible, distinct labels | `outputs::*` |
| outputDrag | NEW-03 | a SendInput drag on the title bar moves the window | SendInput |
| noOutputLayout | NEW-04 | HUD shows no output ghost | `enter_hud`; outputs hidden |
| globMorphMs | NEW-05 | morph duration | motion timing |
| noOutputInput | NEW-06 | no input/textarea in the output DOM | eval-with-result |
| resizeEdges | NEW-07 | each edge or corner drag changes the right dimension | SendInput on 8 edges |
| suggestionsOnce | NEW-10 | suggestions show until the first request, then never again across restarts | submit once, then read `hasSentFirstRequest` from settings_data (ipc.rs:321-363) and eval-with-result for the suggestion DOM after a restart |

## 4. Full table

The "Runtime proof needed" column names the PARITY verify that has not yet run against the Tauri build.

### §7 IPC

| ID | Status | Rust location | Evidence and notes | Runtime proof needed |
|---|---|---|---|---|
| IPC-01 overlay:trigger | DONE | ipc.rs:374 → windows::trigger_or_trail | trail if the agent runs, else open the bar unlocked (main.js:2378-2384) | SMOKE+ |
| IPC-02 overlay:collapse | DONE | ipc.rs:375-381 | HUD → exit_hud, else collapse_panel (2434-2437) | SMOKE+ |
| IPC-03 overlay:resize | DONE | ipc.rs:382 → windows::overlay_resize | panel only; remembers height; skips during the glide (2438-2445). Docked-follow superseded (§26) | answerFollowsTranscript |
| IPC-04 overlay:set-ignore | DONE | ipc.rs:383; windows/flags.rs:444-484 | pill/panel only; forwarding emulated by a 60 Hz poll (no `forward` in Tauri) | collapseClickable |
| IPC-05 overlay:open-settings | DONE | ipc.rs:391-394 | also takes an optional `section` (superset) | settingsWindow |
| IPC-06 output:resize | PARTIAL | ipc.rs:395-399 → outputs::resize | no glide skip (B15) | answerFollowsTranscript |
| IPC-07 open-external | DONE | ipc.rs:419-421 → windows::open_url | http(s) only, lowers Settings/History (2499-2504) | UNIT |
| IPC-08 query:submit | DONE | host/ask.rs:775 → submit_to (~352-712) | walkthrough, needs-key, done, error; stages and failover as handleQuerySubmit (1490-1730) | queryWithoutKey, askHasUiaText |
| IPC-09 query:cancel | DONE | host/ask.rs route | aborts the current query | SMOKE+ |
| IPC-10 query:retry | DONE | host/ask.rs:787 | `nothing-to-retry` when there is no question; targets the calling or focused output (NEW-02) | historyRetryGuard |
| IPC-11 chat:new | DEPARTED | ipc.rs:423-431 | an output resets itself; the bar detaches focus and clears the area (REQUESTS 11, NEW-02) | SMOKE+ |
| IPC-12 region:pick | PARTIAL | desktop/channels.rs:33-39 | no HUD guard; Escape claim racy (B8) | SMOKE+ |
| IPC-13 region:clear | DONE | desktop/channels.rs:40 | | SMOKE+ |
| IPC-14 region:done | DONE | desktop/channels.rs; region.rs (test small_drags_cancel) | < 12 px cancels | SMOKE+ |
| IPC-15 region:cancel | DONE | desktop/channels.rs | | regionSelectorCancels |
| IPC-16 overlay:open-history | DONE | ipc.rs:432 | | SHOT:16 |
| IPC-17 history:list | DONE | ipc.rs:433-435; history.rs (test upsert_filter_and_search) | `{rows}` newest first | historySearch |
| IPC-18 history:open | BUG | ipc.rs:436-447; outputs.rs:383-433 | global area not cleared (B6) | SMOKE+ |
| IPC-19 history:remove | DONE | ipc.rs:448-450 | `{ok}` | historyRemove |
| IPC-20 history:clear | DONE | ipc.rs:451 | `{ok}` | historyCleared |
| IPC-21 history:close | DONE | ipc.rs:452 | | SMOKE+ |
| IPC-22 settings:get | PARTIAL | ipc.rs:321-363 | all Electron keys plus documented extras; key-set unit test missing (B11) | UNIT, SHOT:08 |
| IPC-23 settings:save-key | DONE | ipc.rs:511-535; host/mod.rs:227-269 | empty-key text matches; tour advance after test or "could not reach"; provider detection and `keys` are REQUESTS multi-provider extras | onboardingSteps |
| IPC-24 settings:clear-key | DONE | ipc.rs:536-545 | `{ok:true}` plus `keys` (multi-provider extra) | SMOKE+ |
| IPC-25 settings:save-model | DONE | ipc.rs:546-548 | `{ok, model}` | UNIT |
| IPC-26 settings:save-skill | DONE | ipc.rs:558-561 | `auto` or a known id | UNIT |
| IPC-27 settings:save-brain | DONE | ipc.rs:549-551 | | UNIT |
| IPC-28 settings:set-brain-enabled | DONE | ipc.rs:568-571 | | UNIT |
| IPC-29 settings:set-show-paid | DONE | ipc.rs:572-575 | | UNIT |
| IPC-30 settings:set-save-history | DONE | ipc.rs:576-581 | `{ok, saveHistory, historyCount}`; off keeps rows | historyRespectsSwitch |
| IPC-31 settings:clear-history | DONE | ipc.rs:582-586 | notifies History; input history untouched | SMOKE+ |
| IPC-32 settings:set-launch-at-login | DONE | ipc.rs:587-591; autostart.rs:28 | live only when `!debug_assertions` | SMOKE+:launchAtLogin |
| IPC-33 settings:test-key | DONE | host/ask.rs:808; llm/client.rs:203-205 | `{ok, message}`; optional provider (extra) | UNIT (mock HTTP) |
| IPC-34 update:check | BUG | ipc.rs:610; updater.rs:727-796 | the second identical failure returns `checking` (B3) | updateFeed |
| IPC-35 update:restart | DONE | ipc.rs:611; updater.rs:995 | `{ok}`; restartPending while busy | UPDATE |
| IPC-36 settings:close | DONE | ipc.rs:489 | | SMOKE+ |
| IPC-37 models:list | DONE | host/ask.rs:719-749; llm/catalogue.rs:36-43 | `{groups, live, kind, current}`; kind mapping and current resolution match 2626-2640; optional provider (extra) | modelList |
| IPC-38 clipboard:write | DONE | ipc.rs:612-616 | `{ok:true}` | UNIT |
| IPC-39 hud:enter | DONE | ipc.rs:453 | HUD with no output ghost (NEW-04) | hudMode |
| IPC-40 hud:exit | DONE | ipc.rs:454 | | hudExitMode |
| IPC-41 hud:set-dock | DEPARTED | ipc.rs:457 (no-op) | PARITY §26 superseded table | n/a |
| IPC-42 shortcuts:set | DONE | ipc.rs:618-621; hotkeys.rs:418-465 | all four error texts and the `{ok, shortcuts, active}` shape match 2694-2731; active true/false/null as lastRegisterResults | SMOKE+ |
| IPC-43 shortcuts:reset | DONE | hotkeys.rs:467-471 | | SMOKE+ |
| IPC-44 shortcuts:capture-mode | DONE | ipc.rs:617; hotkeys.rs:412-415 | | SMOKE+ |
| IPC-45 buddy:resolve-result | DONE | desktop/channels.rs | | walkthrough |
| IPC-46 buddy:dismiss | DONE | desktop/channels.rs | | SMOKE+ |
| IPC-47 buddy:advance | DONE | desktop/channels.rs | | buddyPointsDone |
| IPC-48 buddy:set-ignore | DONE | desktop/channels.rs:67 | defaults to true; cursor poll replaces `forward` (D10) | SMOKE+ |
| IPC-49 app:quit | DONE | ipc.rs:487; lib.rs RunEvent::Exit | flush history, stop agent, shutdown helper, install_on_quit | SMOKE+ |
| IPC-50 bar:mode | DONE | host/find.rs:372 | `{mode}`; find opens or closes Find | barKeys |
| IPC-51 find:open | DONE | host/find.rs:377 | prefill capped at 120 | SMOKE+ |
| IPC-52 find:query | DONE | host/find.rs route | | findLiteral |
| IPC-53 find:step | DONE | host/find.rs route | number or `{index}` | findStep |
| IPC-54 find:scope | DONE | host/find.rs route | persisted | SMOKE+ |
| IPC-55 find:to-ask | DONE | host/find.rs:403 | ASK_PAD 120, with_image (unit tests in find.rs) | SMOKE+ |
| IPC-56 find:close | DONE | host/find.rs route | | SMOKE+ |
| IPC-57 agent:start | DONE | host/agent.rs:240-248 | empty, needs-key, busy, consent, offline, started; `unsupported` is non-Windows only (main.js:2276), so unreachable | agentJsonMode |
| IPC-58 agent:stop | DONE | host/agent.rs | `{ok}` | SMOKE+ |
| IPC-59 agent:confirm | DONE | host/agent.rs:249-252; agent/run.rs:761-777 | id-gated, as agent.js:734-740 | agentConfirmGate |
| IPC-60 agent:continue | DONE | host/agent.rs:253; agent/run.rs:780-800 | asking, then planWait, then paused order, as agent.js:982-1001 | agentTakeover |
| IPC-61 agent:consent | DONE | host/agent.rs | `{ok, started}` | SMOKE+ |
| IPC-62 agent:do-it | DONE | host/agent.rs | plan-gated run with the answer as context | SMOKE+ |
| IPC-63 ui:context-menu | DONE | aeval_invoke → host::menu::popup | unit test `menus_match_main_js` | SMOKE+:contextMenus |
| IPC-64 settings:save-find-model | DONE | ipc.rs:552-554 | | UNIT |
| IPC-65 settings:save-agent-model | DONE | ipc.rs:555-557 | | UNIT |
| IPC-66 settings:set-find | DONE | ipc.rs:562-564 | | UNIT |
| IPC-67 settings:set-agent | DONE | ipc.rs:565-567 | | UNIT |
| IPC-68 tour:replay | DONE | ipc.rs:480-486 | closes Settings; starts the tour outside HUD; reapplies hotkeys | walkthrough |
| IPC-69 buddy:card | BUG | desktop/channels.rs:80; host/mod.rs:334-342 | empty prefill (B7) | SMOKE+ |
| IPC-70 buddy:skip | DONE | desktop/channels.rs | | onboardingSteps |
| IPC-71 find:related | DONE | host/find.rs route | `{ok}` | findRelated |
| IPC-72 bar:history | DONE | ipc.rs:458 | | UNIT |
| IPC-73 bar:history-push | DONE | ipc.rs:459 → store push_input_history | | UNIT |
| IPC-74 ui:focus-answer | DONE | ipc.rs:460-471 | `{ok:false}` when no active focused output | SMOKE+ |
| IPC-75 ui:focus-bar | DONE | ipc.rs:472-479 | `{ok:true}` in both branches | SMOKE+ |
| IPC-76 app:help | DONE | ipc.rs:422 | | UNIT |
| IPC-77 settings:clip-key | DONE | ipc.rs:490-497 | `{found, masked}`; the key stays in Rust | UNIT |
| IPC-78 settings:use-clip-key | PARTIAL | ipc.rs:498-510 | tour advances only after a test (B14) | UNIT |
| IPC-79 hud:nudge | DONE | ipc.rs:455; windows/layout.rs:215-226 | 3x3 grid, Math.sign | SMOKE+ |
| IPC-80 buddy:focus | DONE | desktop/channels.rs | | SMOKE+ |
| IPC-81 glass:hello | DONE | desktop/channels.rs:25 | backdrop::hello plus on_hello; returns `{intensity}` (superset) | glassLoaded |
| IPC-82 glass:passthrough | DONE | ipc.rs:384-390; flags.rs:444-484 | outputs only, not in HUD; forwarding poll | collapseClickable |
| IPC-83 settings:set-surface | DONE | ipc.rs:592-597 | `{surface, tier, reasons}`; broadcasts the tier | SMOKE+ |
| IPC-84 allowlist integrity | DEPARTED | ipc.rs:23-153; test `invoke_allowlist_matches_preload` | 86 = the 83 Electron channels plus `output:bar`, `output:close`, `settings:set-glass` (REQUESTS 8-11, glass intensity); the test pins it to the working-tree preload.js; `every_invoke_channel_has_a_route` is a text scan | UNIT |

### §8 EVT

| ID | Status | Rust location | Evidence and notes | Runtime proof needed |
|---|---|---|---|---|
| EVT-01 overlay:state | DONE | windows/mod.rs:250-323 (send_overlay_state) | to the overlay and every output; superset of main.js:760-811 | eventTargeting |
| EVT-02 hud:snapped | DEPARTED | windows HUD code | overlay only; HUD has no output (NEW-04) | SMOKE+ |
| EVT-03 hud:docked | DEPARTED | not sent | §26 superseded | n/a |
| EVT-04 query:start | DONE | host/ask.rs submit_to | `{gen}` to the target output | queryWithoutKey |
| EVT-05 query:stage | DONE | host/ask.rs | the six labels | SMOKE+ |
| EVT-06 query:delta | DONE | host/ask.rs | directive-free text | markdown |
| EVT-07 query:done | DONE | host/ask.rs | `{gen, usage{...}, inferred, question}` or `{gen}` | SMOKE+ |
| EVT-08 query:error | DONE | host/ask.rs | `{gen, message, question}` | SMOKE+ |
| EVT-09 chat:user | DONE | host/ask.rs; host/agent.rs | `{gen, text}` | SMOKE+ |
| EVT-10 chat:reset | DONE | outputs.rs reset | `{}` | SMOKE+ |
| EVT-11 chat:sync | DONE | outputs.rs on_loaded, open_conversation | `{id, messages}` (adds `id` for multi-output) | SMOKE+ |
| EVT-12 history:changed | DONE | windows::notify_history_changed | to History only | historyCleared |
| EVT-13 buddy:resolve | DONE | buddy.rs | to overlay/settings | walkthrough |
| EVT-14 buddy:watch | DONE | buddy.rs | | walkthrough |
| EVT-15 buddy:show | DONE | buddy.rs:253 | to `buddy` | buddyPoints |
| EVT-16 buddy:hide | DONE | buddy.rs | | buddyPointsDone |
| EVT-17 buddy:done | DONE | buddy.rs | | buddyPointsDone |
| EVT-18 buddy:move | DONE | ipc.rs LISTEN_CHANNELS | allowlisted, unsent, as Electron | n/a |
| EVT-19 settings:data | DONE | ipc.rs LISTEN_CHANNELS | allowlisted, unsent, as Electron | n/a |
| EVT-20 update:state | DONE | updater.rs set() | to `settings` | updateFeed |
| EVT-21 find:hits | DONE | host/find.rs find_emit | to Find's output plus the overlay | lensHits |
| EVT-22 find:state | DONE | host/find.rs find_emit | | findOffline |
| EVT-23 agent:begin | DONE | host/agent.rs emitter | to the run's output label | agentRun |
| EVT-24 agent:step | DONE | agent/run.rs:861-905 | row plus `runId` | agentRun |
| EVT-25 agent:state | DONE | agent/run.rs:868-879 | `{state, step, ...}` | agentRun |
| EVT-26 agent:confirm-request | DONE | agent/run.rs | `{id, text, reason, why, url{host, rest, full}}` | agentConfirmGate |
| EVT-27 agent:done | DONE | agent/run.rs | `{state, text, rows, steps, usage}` | agentRun |
| EVT-28 agent:consent-request | DONE | host/agent.rs | `{task}` | SMOKE+ |
| EVT-29 lens:draw | DONE | lens.rs (labels `lens-<id>`, lens.rs:55) | `{hits, current, frame}` | lensFirstDraw |
| EVT-30 view:set | DONE | outputs.rs on_loaded; host/find.rs | chat or find | SMOKE+ |
| EVT-31 bar:prefill | DONE | windows::open_bar | `{text, mode}`; see B7 for the empty prefill from the try card | SMOKE+ |
| EVT-32 settings:section | DONE | windows::open_settings | `{id}` | SMOKE+ |
| EVT-33 bar:open | DONE | windows::open_bar | `{lock}` to the overlay | eventTargeting |
| EVT-34 bar:context | DONE | host/ask.rs:135-150 | screen ≤ 16000, title, process, chars | askHasUiaText |
| EVT-35 glass:frame | DONE | backdrop.rs | `aeval-frame://` URL instead of a Buffer, plus `intensity` (ARCHITECTURE 4.3 / DESIGN implementation notes) | glassLoaded, snapCost, SHOT matrix |
| EVT-36 glass:tier | DONE | backdrop.rs; ipc.rs:598-609 | `{tier, reasons, intensity}` | glassTier |
| EVT-37 lens:loupe | DONE | lens.rs | `{i, img: url}` | lensFirstDraw |
| EVT-38 allowlist integrity | DONE | ipc.rs LISTEN_CHANNELS; test `listen_allowlist_matches_preload` | 37 channels | UNIT |

### §20 HIST

| ID | Status | Rust location | Evidence and notes | Runtime proof needed |
|---|---|---|---|---|
| HIST-01 | PARTIAL | history.rs:17, 48, 286 | 200 cap (test caps_at_two_hundred_newest_first) and title test `titles`; 4 MB counted in bytes, titles in chars (B13) | historySaved, historyTitle |
| HIST-02 | DONE | history.rs (test upsert_filter_and_search) | upsert by id; user/assistant text only; trail kind kept | historySaved |
| HIST-03 | DONE | history.rs (same test) | case-insensitive, title and every message | historySearch |
| HIST-04 | DONE | history.rs (test disabled_writes_nothing_and_keeps_rows) | | historyRespectsSwitch |
| HIST-05 | DONE | history.rs (test unreadable_file_is_an_empty_list) | write errors ignored | UNIT |
| HIST-06 | DONE | host/mod.rs:92-100 (remember_chat); agent run save; history.rs:140 flush_now on RunEvent::Exit | debounced write instead of Electron's first-change timer; flushed on quit | SMOKE+ |

### §21 UPD

| ID | Status | Rust location | Evidence and notes | Runtime proof needed |
|---|---|---|---|---|
| UPD-01 | DONE | updater.rs latest_tag / parse_latest_yml / atom_notes (tests latest_yml, atom_and_tag) | releases/latest redirect plus latest.yml plus releases.atom, no API | updateFeed |
| UPD-02 | BUG | updater.rs:561-590 | schedule dies after two identical failures (B3); otherwise 20 s / 2 s first check, 4 h recheck, resume (windows/flags.rs:415), network return, busy defer, quiet under smoke/shots (updater.rs:616) | UPDATE |
| UPD-03 | DONE | updater.rs set()/get() | all 8 states | UNIT |
| UPD-04 | DONE | updater.rs backoff (test backoff_table) | 2, 5, 15, 30, 60, 120, then stays at 120 min plus jitter, which matches updater.js:50,254-259. The PARITY text "then 4 h" is wrong; fix PARITY | UNIT |
| UPD-05 | DONE | updater.rs classify (test classify_matches_lib_updater) | adds reqwest error patterns | updateClassify |
| UPD-06 | DONE | updater.rs notes_snippet (test notes_prefer_list_items) | | updateNotes |
| UPD-07 | DONE | updater.rs download (test versions_and_hashes) | sha512 base64 verify, `match=` log, full installer | UPDATE |
| UPD-08 | DONE | updater.rs announce | toast text, held until idle, click restarts | UPDATE |
| UPD-09 | DONE | updater.rs:995 restart, :1039 install_on_quit, :961 launch_installer | `/S --updated [--force-run]`, detached | UPDATE |
| UPD-10 | DONE | updater.rs problem_text (test problem_texts_are_plain) | | updateQuiet |
| UPD-11 | DONE | updater.rs init / announce_updated | updatedFrom, releaseUrl, 3 s toast | UPDATE |
| UPD-12 | PARTIAL | updater.rs:174 (test feed_override_rules), :801-807 | override and AEVAL_UPDATE_QUIT done; AEVAL_UPDATE_EXPECT not read (B9) | feedOverrideStrict, UPDATE |
| UPD-13 | MISSING | none | smoke retry loop (B9) | update.attempts |
| UPD-14 | DONE | updater.rs init (`packaged()` = `!debug_assertions`) | every call reports `dev` | UNIT |
| UPD-15 | BUG | src-tauri/installer-hooks.nsh:15-44 | the Electron copy is never found (B4). POSTINSTALL `--force-run` relaunch is fine | UPDATE (old Electron → Rust A → Rust B) |

### §22 SYS

| ID | Status | Rust location | Evidence and notes | Runtime proof needed |
|---|---|---|---|---|
| SYS-01 | DONE | lib.rs presence_toast (from :54) | fresh or taken chord; skipped under smoke; click target per case | SMOKE+ |
| SYS-02 | DONE | lib.rs setup: decide_presence then autostart::apply | as store.js:358-366 | MIGRATE |
| SYS-03 | PARTIAL | autostart.rs:12-28 | Run value `com.aevalbuddy.app`, quoted exe; StartupApproved never written (B12) | RUNNER (reg query) |
| SYS-04 | DONE | tauri-plugin-clipboard-manager; ipc.rs:490-510, 612-616 | | UNIT |
| SYS-05 | DONE | windows::open_url | http(s) only | UNIT |

### §23 PKG

| ID | Status | Rust location | Evidence and notes | Runtime proof needed |
|---|---|---|---|---|
| PKG-01 | PARTIAL | tauri.conf.json:27-30 | per-user, `/S`; interactive run is a wizard, not one-click (B10) | INSTALL:install.exitCode |
| PKG-02 | DONE | .github/workflows/release.yml | copies `*-setup.exe` to `dist/AevalBuddy-Setup.exe`; writes and verifies latest.yml (version, files[url, sha512, size], path, sha512, releaseDate); no blockmap (electron-updater falls back to full download) | RUNNER (release) |
| PKG-03 | UNSURE | tauri.conf.json productName/version | Tauri writes `Uninstall\AevalBuddy`; never checked on a runner (B5) | INSTALL:install.registered, install.version |
| PKG-04 | UNSURE | tauri.conf.json mainBinaryName | uninstall.exe beside AevalBuddy.exe; ProductVersion from 1.1.1 | INSTALL:install.exe, install.exeVersion |
| PKG-05 | UNSURE | Tauri NSIS template | Start Menu and Desktop `AevalBuddy.lnk` expected | INSTALL:install.*Shortcut |
| PKG-06 | UNSURE | installer-hooks.nsh:46-55 | runs the app only with `--force-run` | INSTALL:install.notLaunched |
| PKG-07 | UNSURE | Tauri uninstaller | `%APPDATA%\aevalbuddy` is outside Tauri's delete paths, so it is kept | INSTALL:uninstall.* |
| PKG-08 | DONE | src-tauri/icons/icon.ico | byte-identical to build/icon.ico (SHA-256 15ba02c7…); a checked-in copy, so a make-icon.js change must be copied too | INSTALL |
| PKG-09 | DONE | selftest.rs println; rust.yml:116-138 | the scaffold smoke reads `[SMOKE]` from redirected stdout | INSTALL:smoke.printed |
| PKG-10 | DONE | build pipeline | no signature dependency; the desktop DLL is pinned by SHA-256 (rust.yml:73-78) | n/a |

### §24 SMK (grouped; per-label detail in section 3)

| ID | Status | Rust location | Evidence and notes | Runtime proof needed |
|---|---|---|---|---|
| SMK-01 main.js runSmoke | MISSING | selftest.rs:28-57 | 2 of 106 required labels (version, done) | SMOKE |
| SMK-02 desktop selfcheck | PARTIAL | desktop/selfcheck.rs, desktop/checks.rs:27-52 | all 33 labels present as `[DESKTOP]` only; not run by AEVAL_SMOKE | SMOKE |
| SMK-03 smoke-v2 | MISSING | none | 9 labels | SMOKE |
| SMK-04 smoke-v21 | MISSING | none | 15 labels | SMOKE |
| SMK-05 smoke-glass | MISSING | none (backdrop.rs:767 measure is ready) | 9 labels | SMOKE |
| SMK-06 smoke-v2-desktop | MISSING | none (lens/client exports marked dead code) | 23 labels | SMOKE (CI) |
| SMK-07 superseded labels | DEPARTED | n/a | hudDockedDefault, hudDockToggle, storedOutputPos (§26) | via outputFloating/outputMulti/noOutputLayout |
| SMK-08 installer-check smoke.* | DONE | selftest.rs | `smoke.exitCode` / `smoke.printed` pass on the scaffold alone | INSTALL |

The 11 SMOKE+ labels (eventTargeting … suggestionsOnce) are counted under SMK-01..06's harness work in section 3.8.

### §25 SHOT

| ID | Status | Rust location | Evidence and notes | Runtime proof needed |
|---|---|---|---|---|
| SHOT-01 shots.js frames 01-17 + fit probes | MISSING | none | AEVAL_SHOT unhandled (B2) | INSTALL:shots.written |
| SHOT-02 shots-v2 frames 18-30 | MISSING | none | | same |
| SHOT-03 matrix 100/125/150 + contrast | MISSING | backdrop/contrast.rs (dead code) | | INSTALL:shots125.*, shots150.* |
| SHOT-04 exit 1 on contrast fail | MISSING | none | | INSTALL:shots.exitCode |
| SHOT-05 CapturePreview | MISSING | none | | RUNNER |
| SHOT-06 publish to `shots` | PARTIAL | .github/workflows/ui-shots.yml | ui-shots publishes Edge renders of the pages with a stand-in bridge to `rust-d-ui/` (newest 5), not the installed exe; installer.yml still publishes Electron shots (B5) | installer.yml |
