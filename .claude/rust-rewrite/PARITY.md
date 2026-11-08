# AevalBuddy Electron -> Tauri v2 parity checklist

This file is the definition of "no lost functionality and no lost quality" for the Rust rewrite. Every
item below is something the Electron build (main @ 069a4b8, v1.1.1) does today. The Rust build is done
when every item is checked off by the method in its **Verify** column, on the installed exe on a GitHub
`windows-latest` runner (never on Andy's desktop; see memory `test-in-cloud`).

An item may only be dropped or changed if the departure is written into DESIGN.md with its reason, and
the item here is struck through with a link to that paragraph. "Rust does it differently but the user
cannot tell" is fine and needs no entry; anything a user, a test, or an existing install could notice is
a departure.

Sources are `file:line` in the Electron tree at 069a4b8. `main.js` is the Electron main process.

## Verify legend

| Code | Meaning |
|---|---|
| `SMOKE:x` | The existing `[SMOKE] x=` line, emitted by the Rust app under `AEVAL_SMOKE=1` with the same label and the same pass condition, read by the unchanged `tools/installer-check.ps1`. |
| `SMOKE+:x` | A new smoke line the Rust port must add (the behaviour was untested in Electron, or the old test reached into Electron internals). |
| `UNIT` | `cargo test` in `src-tauri`, run by `.github/workflows/rust.yml`. |
| `DIFF` | Differential test: the JS module runs under plain node (it is Electron-free) and the Rust port runs the same inputs; outputs must be equal. Harness owned by agent (e), inputs in `tools/fixtures/`. |
| `SHOT:n` | The screenshot `n` from the visual self-check (`AEVAL_SHOT`), compared by eye against the Electron set on the `shots` branch, plus the contrast gate where the matrix applies. |
| `INSTALL:x` | The `Check 'x'` in `tools/installer-check.ps1`. |
| `MIGRATE` | The upgrade test (new, agent (e)): a profile written by the Electron v1.1.1 build is opened by the Rust build, which must read every key, the API key, and history unchanged. |
| `UPDATE` | `.github/workflows/update-check.yml` (old release updates to A, A updates to B), ported. |
| `RUNNER` | A scripted check on the runner that is not a smoke line (named in the item). |

---

## 1. Process lifecycle and environment (LIFE)

| ID | What | Source | Verify |
|---|---|---|---|
| LIFE-01 | Single instance. A second launch does not start a second app; it opens the bar (`triggerCapture`) in the running one. The lock is per user-data directory, so `AEVAL_USERDATA` runs get their own lock and never collide with a real session. | main.js:2753-2759 | SMOKE+:singleInstance (spawn a second copy with the same AEVAL_USERDATA; it must exit and the first must expand) |
| LIFE-02 | `AEVAL_USERDATA` overrides the user-data directory before anything reads it (store, history, logs, lock). | main.js:30-32 | UNIT + used by every installer-check run |
| LIFE-03 | `AEVAL_SHOT_SCALE` forces the device scale factor (1, 1.25, 1.5) for the shots matrix. Tauri has no Chromium switch; the port must reach the same rendering scale (WebView2 `--force-device-scale-factor` via `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`, plus Rust-side window maths at that scale). | main.js:36-38 | INSTALL:shots125.exitCode, shots150.exitCode |
| LIFE-04 | `app.setAppUserModelId('com.aevalbuddy.app')` so toasts carry the app's name and clicks reach the app. | main.js:2766 | SMOKE+:aumid (read back with GetCurrentProcessExplicitAppUserModelID) |
| LIFE-05 | Crash handler installed before anything else (uncaught exception, unhandled rejection; in Rust: panic hook on every thread + task). | main.js:40-41, crash.js:72-81 | SMOKE:crashLogged |
| LIFE-06 | Boot order: IPC registered, region IPC, history enabled flag, overlay window, output window, `decidePresence`, launch-at-login applied, tray, hotkeys, updater init, warn if any shortcut failed, presence toast, first-run tour, smoke/shots/catalogue warm-up, backdrop init, 4 s keep-on-top interval (not under smoke/shots). | main.js:2761-2922 | SMOKE:windows, SMOKE:tray, SMOKE:bootMs |
| LIFE-07 | The desktop helper is NOT started at boot (boot time is a gate); it starts on first use. | main.js:52-53, smoke-v2.js:33 | SMOKE:helperNotAtBoot |
| LIFE-08 | Boot time: pill visible within 15 s of process start (`bootMs` measured from process start to overlay ready-to-show). | main.js:21-24,456-460; smoke-v2.js:34-35 | SMOKE:bootMs |
| LIFE-09 | `window-all-closed` quits the app. | main.js:2939-2941 | SMOKE+:quitOnLastClose (unit-level: closing every window ends the process) |
| LIFE-10 | `before-quit`: a running agent is stopped with "Stopped because AevalBuddy quit.", Find closed, desktop helper killed (so an update's installer never finds it running under the install folder). | main.js:2945-2949 | UPDATE (install-on-quit leaves no helper) |
| LIFE-11 | `will-quit`: helper stopped, all global shortcuts unregistered, history flushed now (skips the 400 ms debounce), updater timers cleared, tray destroyed. | main.js:2951-2958 | SMOKE+:historyFlushOnQuit |
| LIFE-12 | Every window created anywhere (including lens, region, buddy) is crash-guarded, content-protected, registered with the backdrop (unless offscreen), and re-protected on every `show`. | main.js:2927-2937 | SMOKE (contentProtection, windowsChecked) |
| LIFE-13 | `AEVAL_OFFLINE=1` makes `online()` false and every model/catalogue call fail like a dropped connection. | main.js:218-225, openrouter.js:14-16,29 | SHOT:27-find-offline, SMOKE:findOffline |
| LIFE-14 | `AEVAL_NO_UIA` disables all UI Automation reads (exact text, bar context, UIA pointing). | main.js:1810,2027 | UNIT |
| LIFE-15 | `AEVAL_FAKE_LLM=1` answers every chat request from `tools/fixtures/llm/*.jsonl` (catalogue fetch stays real); request bodies are kept for assertions; `{{var}}` substitution; per-turn `delayMs` honouring abort. | fake-llm.js:1-198 | DIFF + every SMOKE that uses it |
| LIFE-16 | `AEVAL_DESKTOP_NOTEPAD` / `GITHUB_ACTIONS` gate the Notepad-driving checks (never on a real desktop). | main.js:3837,3910 | RUNNER |
| LIFE-17 | Online check: `net.isOnline()`, true on error. Rust: `InternetGetConnectedState`/NLM `IsConnectedToInternet`, true on error. | main.js:218-225 | UNIT |
| LIFE-18 | Windows build detection: `CAN_HIDE_FROM_CAPTURE` is true only on build >= 19041 (WDA_EXCLUDEFROMCAPTURE); below it the app says so instead of pretending. | main.js:73-74 | SMOKE:captureProtection |
| LIFE-19 | The 4 s `keepOnTop` interval re-pins every visible window (overlay, output, settings, lens). | main.js:692-696,2919-2921 | SMOKE:topmostHolds |
| LIFE-20 | Reduced motion: read from the renderer at boot (`prefers-reduced-motion`) and re-read from the OS (`SPI_GETCLIENTAREAANIMATION`) before every window move. | main.js:461-468,584-591 | SMOKE+:reducedMotion |

## 2. Windows (WIN)

Common to every window: frameless, transparent, no shadow from the OS, skip taskbar, WS_EX_TOOLWINDOW
(Electron `type:'toolbar'`; nothing in Alt-Tab), always on top at the `screen-saver` level +1 (in Win32
terms HWND_TOPMOST re-asserted), visible on all workspaces, content protected (WDA_EXCLUDEFROMCAPTURE),
`contextIsolation` with only the `window.aeval` bridge, no Node.

| ID | What | Source | Verify |
|---|---|---|---|
| WIN-01 | `pinOnTop`: always-on-top at the top band, re-asserted after `setFocusable` (Windows drops it) and every 4 s. | main.js:83-87,603,637 | SMOKE:overlayOnTop, outputOnTop, topmostHolds |
| WIN-02 | `hideFromCapture` on every window when build >= 19041. | main.js:89-92 | SMOKE:captureProtection, SMOKE contentProtection (smoke-v2 `contentProtection`) |
| WIN-03 | Tool window: off taskbar and Alt-Tab. Smoke prints `taskbarHidden=true:toolbar`. | main.js:414-438, 3576-3577 | SMOKE:taskbarHidden (label kept; value becomes `true:toolwindow` only if DESIGN.md records it, else keep `true:toolbar`) |
| WIN-04 | Overlay window (`renderer/index.html`): starts at pill bounds plus the 20 px stage margin, not focusable, click-through with mouse-move forwarding while idle, shown inactive (never steals focus), context menu attached. | main.js:440-477 | SHOT:01-pill, SMOKE:collapseClickable |
| WIN-05 | Output (answer) window (`renderer/output.html`): focusable, stage margin, hidden until an answer, rebuilds the transcript (`chat:sync`) after a reload. | main.js:479-503 | SHOT:04-answer |
| WIN-06 | Settings window (`settings/settings.html`): 380-900 wide, min 360 tall, height clamped to work area - 40, opens at saved size, saved 160 ms after a resize, not in taskbar, on top, protected; reopening focuses and jumps to a section; closing ends shortcut capture mode, re-applies hotkeys, notifies the tour. | main.js:505-574 | SHOT:08..10, SMOKE:settingsWindow |
| WIN-07 | History window (`history/history.html`): same frame rules as Settings, saved size, protected. | main.js:1869-1918 | SHOT:16-history |
| WIN-08 | Region selector window: one per pick, covers the chosen display, focusable (keyboard and mouse), blur cancels, `backgroundThrottling:false`. | region.js:43-96 | SMOKE:regionSelectorCancels, SHOT:17-region |
| WIN-09 | Lens windows: one per display with something to draw, display-sized, click-through, never focusable, protected, `aevalLens` (desktop engine may sit under them). | lens.js:39-86 | SMOKE:lensHits (`isFocusable()===false`, click lands on Notepad) |
| WIN-10 | Buddy (pointer/tour) window: display-sized, not focusable, click-through with forwarding except over its card, protected, glass. | cursor-buddy.js:371-415 | SHOT:11-buddy, SMOKE:buddyPoints |
| WIN-11 | Stage margin: overlay and output windows are 20 px larger than their glass on every side; all bounds code speaks glass rectangles; min/max sizes offset by the margin; 0 max means unlimited. | stage.js:1-69 | SMOKE:answerFollowsTranscript, SMOKE:panelWidthRange |
| WIN-12 | Margin is click-through: `glass:passthrough` toggles ignore-mouse on the output window only, never in HUD mode. | main.js:2458-2462, renderer/glass.js | SMOKE:collapseClickable |
| WIN-13 | Resizability only through min/max sizes, never by toggling the resizable flag (toggling strips WS_THICKFRAME for good). | main.js:892-911 | SMOKE:pillPinned, panelHeightPinned |
| WIN-14 | Editing context menu in every window: Cut/Copy/Paste/Select all in editable fields, Copy/Select all on a selection. | main.js:384-404 | SMOKE+:contextMenuEditable (template as data) |
| WIN-15 | No window ever calls setOpacity (it toggles WS_EX_LAYERED and leaves desktop artifacts). | DESIGN.md, smoke-glass.js:90-100 | SMOKE:noSetOpacity |
| WIN-16 | A dead renderer is reloaded (`render-process-gone` except clean exit), logged, announced once. Rust: WebView2 `ProcessFailed` -> reload. | crash.js:88-102 | SMOKE+:rendererReload (kill the WebView2 renderer pid, page comes back) |
| WIN-17 | Links opened from Settings/History lower those windows from topmost until they are focused again, so the browser is not underneath. | main.js:2339-2349 | SMOKE+:lowerForBrowser |

## 3. Overlay modes, layout and motion (POS)

| ID | What | Source | Verify |
|---|---|---|---|
| POS-01 | Pill is a 44x44 glass orb; state shown by rim and badge only. | main.js:94-96 | SHOT:01-pill |
| POS-02 | Constants: panel width 420; input height 62-280 (default 76); output min 90; output auto max = max(380, 66% of work area); default output 120; dock gap 8; snap margin 16; snap distance 110; dock distance 150; drop settle 160 ms; chat history limit 12 turns; T_EXPAND 260, T_COLLAPSE 170, T_OUT_IN 300, T_OUT_OUT 150, T_OUT_GROW 200 ms. | main.js:96-151 | UNIT (constants table) |
| POS-03 | `boundsForSize`: each axis hugs left/right/top/bottom with offset or centres; clamped into the work area of the primary display. | main.js:282-296 | UNIT |
| POS-04 | Docked output sits below a top-anchored bar or above a bottom-anchored one, clamped into the work area. | main.js:309-321 | UNIT, SMOKE:storedOutputPos |
| POS-05 | Output height: a user-dragged height wins (clamped 90..workArea-40); otherwise transcript height clamped 90..outputMaxHeight. | main.js:323-336 | SMOKE:answerHeightUnderLongChat, outputAutoHeight |
| POS-06 | `posFromRect`: a free drop hugs the nearer edge on each axis, keeping the offset. | main.js:340-350 | UNIT |
| POS-07 | Nine HUD snap anchors (corners, edge midpoints, centre) at margin 16; snap only within 110 px of centre distance, nearest wins. | main.js:353-380 | SMOKE:hudSnapFound |
| POS-08 | Expand: mode reaches the renderer before the window grows; window takes bar size at once (orb grows in-page); click-through off; focusable on; re-pin; show + focus; re-sync bounds after T_EXPAND; hotkeys re-applied. | main.js:593-619 | SMOKE:mode, SHOT:03-panel |
| POS-09 | Collapse (D-01): the answer keeps streaming and output windows stay; closes Find; the foreground goes back to the app that had it, then not focusable, click-through with forwarding, show inactive; window shrinks to the pill only after T_COLLAPSE+30 ms (0 with reduced motion) so the fold plays inside the bar-sized window. | main.js:623-663 | SMOKE:pillWidthHeld, SMOKE:collapseClickable |
| POS-10 | Show output: first open sets final bounds and shows inactive, then syncs after T_OUT_IN; later opens glide (`resizeOutput`). Hide is immediate, never animated. | main.js:667-732 | SHOT:04-answer |
| POS-11 | Output growth glides over T_OUT_GROW and re-applies a height reported mid-glide when it lands (the "window stopped growing" bug). | main.js:736-748 | SMOKE:answerFollowsTranscript |
| POS-12 | Docked output follows the bar immediately, but never cuts short a glide in flight. | main.js:750-760 | SMOKE+:dockFollows |
| POS-13 | `overlay:state` payload, sent to overlay and output: mode, orbAt, reducedMotion, hasKey, captureOk, inputAnchor, outputPos, pillState (idle/chat/busy/agent), inputWidth, outputWidth, outputFixedHeight, chatMax (= outputMaxHeight-56), region size, busy, barMode, agent {running, step, state}, pillError, offline, findOpen, shortcuts, hotkeyFailed. Field names and meanings unchanged. | main.js:762-812 | UNIT (serde snapshot), SHOT |
| POS-14 | HUD layout editor: enter aborts query, cancels the tour, stops motion, shows both ghosts (input 140 tall at the user width; output 200 tall), both movable, sizes locked; exit stores the pending anchors, restores limits, hides output, collapses, refocuses Settings. | main.js:816-876 | SMOKE:hudMode, hudExitMode, storedPos, SHOT:06,07 |
| POS-15 | HUD drag: snap decision runs once 160 ms after movement stops (never mid-drag); a docked output ghost follows the bar live; snap guard suppresses the move events the app itself causes. | main.js:878-890,996-1034 | SMOKE+:hudDragSettle |
| POS-16 | HUD output drag: docking (within 150 px of the dock slot) beats anchor snapping; dragging clear detaches; `hud:docked` and `hud:snapped` sent. | main.js:1036-1064 | SMOKE:hudDockToggle |
| POS-17 | Dock toggle button: docking snaps the ghost; undocking nudges it 170 px away in its growth direction. | main.js:1066-1084 | SMOKE:hudDockToggle, hudDockedDefault |
| POS-18 | Keyboard HUD: arrows step the bar across the 3x3 grid of the work area, then settle as a drop. | main.js:1006-1018 | SMOKE+:hudNudge |
| POS-19 | User resize of the open bar: width only (min/max height equal), stored 160 ms after the drag unless the app moved it in the last 500 ms or a glide is running. | main.js:919-979 | SMOKE:panelWidthRange, panelWidthHeld, widthAfterCycle |
| POS-20 | User resize of the answer window stores width and height; a stored height switches it from auto-sizing to scrolling until New chat. | main.js:981-994 | SMOKE:outputAutoHeight |
| POS-21 | The pill never inherits the panel minimum width (a widened pill used to be stored as a chosen width). | main.js:919-933, 3416-3424 | SMOKE:pillWidthHeld, pillPinned, widthDefaults |
| POS-22 | Window motion: 60 Hz setBounds on one curve `cubic-bezier(0.32,0.72,0,1)`; one animation per window; a new one takes over from the current rect; reduced motion jumps to the end. | motion.js:1-163 | SMOKE:motionAnchor, UNIT (bezier) |
| POS-23 | Answer window dodges the agent: if the agent is about to act under it, it glides to the opposite side of its display. | main.js:2198-2210 | SMOKE+:dodge |
| POS-24 | Positions are DIP; Tauri speaks physical pixels. Every stored anchor/offset/width stays DIP so existing values mean the same thing on every monitor scale. | store.js:270-340 | MIGRATE |

## 4. Global hotkeys (KEY)

| ID | What | Source | Verify |
|---|---|---|---|
| KEY-01 | Default chords: captureQuery `Ctrl+Alt+Space`; captureRegion, findOnScreen, agentTask, toggleVisibility unbound (''). | store.js:14-20 | SMOKE:shortcuts, SMOKE:shortcut.captureQuery |
| KEY-02 | captureQuery opens the bar unlocked (one input that infers Ask/Find/Do). | main.js:1264-1266,2006-2018 | SMOKE:barOpensUnlocked |
| KEY-03 | findOnScreen opens the bar locked to Find. | main.js:1268-1270 | SMOKE:barKeys |
| KEY-04 | agentTask opens the bar locked to Agent, or shows the trail (unfocused) while a run is going. | main.js:1272-1279 | SMOKE+:agentHotkeyTrail |
| KEY-05 | captureRegion starts the area selector. | main.js:1281-1283 | SMOKE+:regionHotkey |
| KEY-06 | toggleVisibility hides/shows overlay and (if active) output, cancels the tour, refreshes the tray label. | main.js:1285-1309 | SMOKE+:toggleVisibility |
| KEY-07 | Global Escape is registered ONLY while the agent runs, the tour or pointer is active, the area selector is open, or the HUD editor is open; never while idle or with only the bar open. Enter is never global. | main.js:1385-1397 | SMOKE:noGlobalEnter, noGlobalEscWithBar, keySwallow |
| KEY-08 | Escape priority: stop agent > close Find > cancel region > cancel tour > exit HUD > close focused Settings > close focused History > collapse panel. | main.js:1311-1356 | SMOKE:regionEscape |
| KEY-09 | While the engine injects a synthetic Escape, the global Escape is unregistered and retaken 60 ms after (RegisterHotKey fires on injected input). Suspensions nest: Escape comes back only when the last overlapping synthetic key is done. Every registration change runs on the main thread, one at a time, and an apply that changes nothing touches nothing (opening the bar registers Escape alone). | main.js:2782-2794 | SMOKE+:escapeSuspend |
| KEY-10 | Registration result per action: true, false (owned by another app), null (unbound). A failed main chord shows in the bar (`hotkeyFailed`) and triggers the toast. An unparseable stored accelerator counts as false, never a crash. | main.js:1362-1384,804 | SMOKE:shortcut.* |
| KEY-11 | Capture mode (Settings recording a combo) suspends every global shortcut. | main.js:1365,2689-2692 | SMOKE+:captureMode |
| KEY-12 | `shortcuts:set` validation: action must be known ("Unknown action."); must include Ctrl/Alt/Super plus a non-modifier key ("Include Ctrl, Alt, or Win plus a key, for example Ctrl+Shift+A."); no clash with another action ("Already used by "<label>". Pick a different combo."); probe with ours unregistered; a taken chord answers `{ok: false, error: "Another app already uses that shortcut. Try another." (C-56), free: [...]}`, where `free` lists those of Ctrl+Alt+A, Super+Shift+Space, Ctrl+Shift+Space that no other app holds and none of ours uses (U48). Returns shortcuts and active map. | main.js:1399-1407,2694-2731 | UNIT + SMOKE+:shortcutSet |
| KEY-13 | `shortcuts:reset` restores defaults and re-registers. | main.js:2733-2737 | SMOKE+:shortcutReset |
| KEY-14 | Accelerator strings are stored in Electron syntax (`Ctrl+Alt+Space`, `CommandOrControl`, `Super`, `AltGr`...). Rust parses that syntax into RegisterHotKey modifiers/VK; stored strings are never rewritten. | store.js:255-268 | UNIT (every Electron accelerator form) + MIGRATE |
| KEY-15 | Action labels: captureQuery "Ask, find, or do", captureRegion "Ask about an area", findOnScreen "Find on screen", agentTask "Do a task", toggleVisibility "Hide or show the overlay". | main.js:129-135 | UNIT |

## 5. Keyboard handled inside pages (RKEY)

These live in renderer JS and must keep working unchanged under WebView2. Verify: SMOKE:barKeys plus the
renderer files are byte-identical apart from the changes listed in ARCHITECTURE.md section 6.

| ID | What | Source |
|---|---|---|
| RKEY-01 | Bar: Tab/Ctrl+1..3 switch mode, typed text survives; Backspace at start unlocks a locked bar; Up/Down recall input history; Alt+Down opens the guess menu; Alt on a Do guess; Escape collapses; F6 to the answer. | renderer/bar.js:180-505 |
| RKEY-02 | Prefixes `/` (find), `?` (ask), `>` (do), `//` literal. | renderer/intent.js |
| RKEY-03 | Answer: arrow/page scrolling keys, Escape, F6 back to the bar. | renderer/output.js:53-60,551-560 |
| RKEY-04 | Overlay pill: Enter/Space opens; HUD arrows nudge, Enter finishes. | renderer/overlay.js:280-320 |
| RKEY-05 | Region selector: Escape cancels, Enter confirms, arrows move/resize (Alt resizes). | renderer/region.js:107-140 |
| RKEY-06 | Trail: Enter on inputs continues / answers ask_user. | renderer/trail.js:198 |
| RKEY-07 | Settings: Enter saves key, listbox arrows/Home/End/Enter/Escape/Tab, shortcut recorder (Escape cancels, needs a modifier). | settings/settings.js:152-293,817-870 |
| RKEY-08 | History: Escape clears search then closes. | history/history.js:180,242 |

## 6. Tray (TRAY)

| ID | What | Source | Verify |
|---|---|---|---|
| TRAY-01 | Tray icon is the pointer arrow: 16 px and 32 px PNGs from tray.js plus 20 px and 24 px resampled from the 32 px one; the smallest at least 16 px times the primary display's scale is used, so Windows only scales it down. | tray.js:19-48 | SMOKE:tray |
| TRAY-02 | Left click: trail if the agent runs, else the bar. A double click opens once (D-07).
Add (note to the row):
D-07: the DoubleClick arm is dropped, and the second click of a double click (within the system double-click time of the last open) is ignored. Reason: Windows reports one double click as two clicks and a double click; Electron opened on all three, which flickered open, closed, open. | tray.js:69-78, main.js:2854 | SMOKE+:trayClick |
| TRAY-03 | Menu order (as labels, `-` separators): [`Restarting after this answer…` disabled, `-`] or [`Restart to update to X`, `-`] when an update is ready; [`Stop agent` (Esc), `-`] while running; `Ask, find, or do…`, `Ask about an area…`, `Find on screen…`, `Do a task…`, `-`, `Hide the overlay`/`Show the overlay`, `New chat`, `History…`, `-`, `Settings…`, `Help and feedback…`, check item, `-`, `Quit AevalBuddy`. | tray.js:122-165 | SMOKE (smoke-v2 tray order check via `labels()`), keep the data function |
| TRAY-04 | Each item shows its current accelerator without registering it. | tray.js:127-132 | SMOKE+:trayAccel |
| TRAY-05 | Check item states: `Checking…` (disabled), `Downloading X… N%` (disabled), `Up to date (X)` for 10 s, `Update failed, try again`, `Check for updates`. | tray.js:80-116 | SMOKE+:trayCheckStates |
| TRAY-06 | Tooltip: `AevalBuddy · X is ready, restart to update` / `AevalBuddy is using your computer · Esc to stop` / `AevalBuddy · <chord>` / `AevalBuddy`. | tray.js:168-183 | SMOKE+:trayTooltip |
| TRAY-07 | Refreshed whenever hotkeys, hidden state, agent state, or update state change. | main.js:1307,1384,2152,2870 | covered by above |
| TRAY-08 | Help opens `https://github.com/aevalmere/aevalbuddy/issues/new/choose` (lowering Settings/History first). | main.js:2337,2370-2374 | UNIT |

## 7. IPC: invoke channels (renderer -> main)

The renderer API is `window.aeval.invoke(channel, payload) -> Promise`. Unknown channels reject with
`Blocked IPC channel: <channel>` (preload.js:139-142). The Rust bridge keeps the same allowlist and the
same rejection text; the allowlist is unit-tested against preload.js (`ipc::tests`).

| ID | Channel | Behaviour and return shape | Source | Verify |
|---|---|---|---|---|
| IPC-01 | `overlay:trigger` | Trail if agent runs, else open bar unlocked. | main.js:2378-2384 | SMOKE+ |
| IPC-02 | `overlay:collapse` | Exit HUD if in HUD, else collapse (POS-09 under D-01: no abort, no output hidden). | 2434-2437 | SMOKE+ |
| IPC-03 | `overlay:resize` {height} | Panel only; remember height; skip while gliding; set bounds; follow docked output. | 2438-2445 | SMOKE:answerFollowsTranscript |
| IPC-04 | `overlay:set-ignore` {ignore} | Pill/panel only: click-through with forwarding. | 2452-2455 | SMOKE:collapseClickable |
| IPC-05 | `overlay:open-settings` | Open Settings. | 2469 | SMOKE:settingsWindow |
| IPC-06 | `output:resize` {height} | Not in HUD, only while active; glide to new height. | 2446-2451 | SMOKE:answerFollowsTranscript |
| IPC-07 | `open-external` {url} | http(s) only; lowers Settings/History; opens default browser. | 2499-2504 | UNIT |
| IPC-08 | `query:submit` {question, inferred} | Runs one turn (section 11); returns {status: walkthrough/needs-key/done/error}. | 2471 | SMOKE:queryWithoutKey, askHasUiaText |
| IPC-09 | `query:cancel` | Abort current query. | 2472 | SMOKE+ |
| IPC-10 | `query:retry` | Re-send last question as a retry, or {status:'nothing-to-retry'}. | 2476-2479 | SMOKE:historyRetryGuard |
| IPC-11 | `chat:new` | Reset chat (section 11). | 2480 | SMOKE+ |
| IPC-12 | `region:pick` | Start area selection. | 2482 | SMOKE+ |
| IPC-13 | `region:clear` | Back to whole screen. | 2483 | SMOKE+ |
| IPC-14 | `region:done` rect | Finish selection (window coords + display origin; < 12 px either axis cancels). | region.js:127-149 | SMOKE+ |
| IPC-15 | `region:cancel` | Cancel selection. | region.js:144-147 | SMOKE:regionSelectorCancels |
| IPC-16 | `overlay:open-history` | Open History. | 2485 | SHOT:16 |
| IPC-17 | `history:list` {query} | {rows:[{id, at, title, model, count, asked, reply}]} newest first, query matches title and every message. | 2486-2488, history.js:146-172 | SMOKE:historySearch |
| IPC-18 | `history:open` {id} | Reopen conversation as live chat, about the whole screen: the picked area is cleared, and only the window it lands in stops its Ask; {ok}. | 2489, 1921-1942 | SMOKE+ |
| IPC-19 | `history:remove` {id} | {ok}. | 2490 | SMOKE:historyRemove |
| IPC-20 | `history:clear` | {ok}. | 2491 | SMOKE:historyCleared |
| IPC-21 | `history:close` | Close window. | 2492 | SMOKE+ |
| IPC-22 | `settings:get` | Object with exactly: findModel, defaultFindModel, agentModel, defaultAgentModel, find, agent, winBuild, hasKey, model, defaultModel, brain, brainEnabled, defaultBrain, showPaid, skills, skill, shortcuts, active, defaults, version, update, canHide, saveHistory, historyCount, launchAtLogin, surface, glass{tier, reasons}, launchAtLoginLive. Models resolved from the cached catalogue without waiting. | 2525-2562 | UNIT (key set) + SHOT:08 |
| IPC-23 | `settings:save-key` {key} | Empty -> {ok:false, error:'Enter a key before saving.'}; saves; updates overlay state; tour step 0 advances after a passing test, or after "could not reach" with the message `Saved; it could not be tested offline. <step 2 text>`, else shows the test message. | 2592-2613 | SMOKE:onboardingSteps |
| IPC-24 | `settings:clear-key` | {ok:true}. | 2615-2619 | SMOKE+ |
| IPC-25 | `settings:save-model` {model} | {ok, model}. Sets modelChosen. | 2621-2624 | UNIT |
| IPC-26 | `settings:save-skill` {skill} | {ok, skill} ('auto' or a known id, else 'auto'). | 2587-2590 | UNIT |
| IPC-27 | `settings:save-brain` {model} | {ok, brain}. | 2646-2649 | UNIT |
| IPC-28 | `settings:set-brain-enabled` {on} | {ok, brainEnabled}. | 2651-2654 | UNIT |
| IPC-29 | `settings:set-show-paid` {on} | {ok, showPaid}. | 2675-2678 | UNIT |
| IPC-30 | `settings:set-save-history` {on} | {ok, saveHistory, historyCount}; off stops writes, does not delete. | 2656-2662 | SMOKE:historyRespectsSwitch |
| IPC-31 | `settings:clear-history` | Clears history, notifies History window; {ok, historyCount}. Input history is NOT cleared here (store.clearInputHistory exists but is unused; keep as-is). | 2664-2668 | SMOKE+ |
| IPC-32 | `settings:set-launch-at-login` {on} | Stores, applies (packaged only); {ok, launchAtLogin}. | 2670-2673 | SMOKE+:launchAtLogin |
| IPC-33 | `settings:test-key` | {ok, message} (section 12). | 2564 | UNIT (mock HTTP) |
| IPC-34 | `update:check` | Updater state after the check settles. | 2584 | SMOKE:updateFeed |
| IPC-35 | `update:restart` | {ok}. | 2585 | UPDATE |
| IPC-36 | `settings:close` | Close Settings. | 2685-2687 | SMOKE+ |
| IPC-37 | `models:list` {kind} | kind in text/find/agent/vision; {groups, live, kind, current}; current resolved live. | 2626-2640 | SMOKE:modelList |
| IPC-38 | `clipboard:write` {text} | {ok:true}. | 2680-2683 | UNIT |
| IPC-39 | `hud:enter` | Enter HUD editor. | 2494 | SMOKE:hudMode |
| IPC-40 | `hud:exit` | Exit HUD editor. | 2495 | SMOKE:hudExitMode |
| IPC-41 | `hud:set-dock` {docked} | Toggle dock. | 2497 | SMOKE:hudDockToggle |
| IPC-42 | `shortcuts:set` {action, accelerator} | See KEY-12. | 2694-2731 | SMOKE+ |
| IPC-43 | `shortcuts:reset` | See KEY-13. | 2733-2737 | SMOKE+ |
| IPC-44 | `shortcuts:capture-mode` {on} | See KEY-11. | 2689-2692 | SMOKE+ |
| IPC-45 | `buddy:resolve-result` payload | Tour: page reports the target rect. | 2739-2741 | SMOKE:walkthrough |
| IPC-46 | `buddy:dismiss` | Cancels the tour/pointer. | 2742 | SMOKE+ |
| IPC-47 | `buddy:advance` | Next point / step. | 2743 | SMOKE:buddyPointsDone |
| IPC-48 | `buddy:set-ignore` {ignore} | Pointer window click-through toggle (card hover). | 2744-2746 | SMOKE+ |
| IPC-49 | `app:quit` | Quit. | 2748 | SMOKE+ |
| IPC-50 | `bar:mode` {mode} | Sets ask/find/agent (find opens Find; leaving find closes it); returns {mode}. | 2385-2388 | SMOKE:barKeys |
| IPC-51 | `find:open` {prefill?} | With prefill (<=120 chars): open bar locked to Find with that text, {ok}. Else switch to Find and return find state. | 2390-2397 | SMOKE+ |
| IPC-52 | `find:query` {query, literal} | Run Find (section 14). | 2398 | SMOKE:findLiteral |
| IPC-53 | `find:step` {by} | Step current hit (number or {index}). | 2400 | SMOKE:findStep |
| IPC-54 | `find:scope` {scope?} | Toggle/set window vs screen, persisted; {scope}. | 2401-2405 | SMOKE+ |
| IPC-55 | `find:to-ask` {index?} | Hit becomes an Ask with a padded region (120 DIP) and withImage. | 2406, 2127-2143 | SMOKE+ |
| IPC-56 | `find:close` | Close Find. | 2407 | SMOKE+ |
| IPC-57 | `agent:start` {task, context} | See AGENT section; returns {status: empty/needs-key/unsupported/busy/consent/offline/started, runId}. | 2408, 2268-2307 | SMOKE:agentJsonMode |
| IPC-58 | `agent:stop` | {ok}. | 2409 | SMOKE+ |
| IPC-59 | `agent:confirm` {id, allow} | {ok} only for the current random id. | 2410, agent.js:734-741 | SMOKE:agentConfirmGate |
| IPC-60 | `agent:continue` {text} | Resume after pause / answer ask_user / approve plan; {ok}. | 2411 | SMOKE:agentTakeover |
| IPC-61 | `agent:consent` {allow} | First-run consent; stores consent and starts the pending run; {ok, started}. | 2412-2419 | SMOKE+ |
| IPC-62 | `agent:do-it` {question, answer} | Starts a plan-gated run with the answer as context. | 2420-2422 | SMOKE+ |
| IPC-63 | `ui:context-menu` {kind,...} | Native menus: answer (Copy, Find this on screen, Do it), hit (Go to, Copy text, Ask about this), row (Copy step, Retry from here when ended). | 2310-2333 | SMOKE+:contextMenus (templates as data) |
| IPC-64 | `settings:save-find-model` {model} | {ok, model}. | 2641 | UNIT |
| IPC-65 | `settings:save-agent-model` {model} | {ok, model}. | 2642 | UNIT |
| IPC-66 | `settings:set-find` patch | {ok, find}. | 2643 | UNIT |
| IPC-67 | `settings:set-agent` patch | {ok, agent}. | 2644 | UNIT |
| IPC-68 | `tour:replay` | Close Settings, start the tour. | 2424-2427 | SMOKE:walkthrough |
| IPC-69 | `buddy:card` {id} | Skip the card; `try` opens the bar in Agent with "open Notepad and write a haiku". | 2430-2433 | SMOKE+ |
| IPC-70 | `buddy:skip` | Skip the current step. | 2428 | SMOKE:onboardingSteps |
| IPC-71 | `find:related` | Run related-words stage now; {ok}. | 2399 | SMOKE:findRelated |
| IPC-72 | `bar:history` | Input history (<=50 strings, newest last). | 2506 | UNIT |
| IPC-73 | `bar:history-push` {text} | Trim, cap 500 chars, dedupe, only when saveHistory on. | 2507, store.js:382-389 | UNIT |
| IPC-74 | `ui:focus-answer` | Show+focus the answer if active; {ok}. | 2510-2515 | SMOKE+ |
| IPC-75 | `ui:focus-bar` | Focus bar, or open it; {ok:true}. | 2516-2523 | SMOKE+ |
| IPC-76 | `app:help` | Opens the help URL; {ok:true}. | 2505 | UNIT |
| IPC-77 | `settings:clip-key` | If no key saved and clipboard holds `^sk-or-[A-Za-z0-9_-]{20,200}$`: {found:true, masked:'sk-or-xxx…abcd'} (first 9 + last 4). Key never leaves Rust. | 2567-2574 | UNIT |
| IPC-78 | `settings:use-clip-key` | Saves the clipboard key or {ok:false, error:'The clipboard no longer holds an OpenRouter key.'}; advances tour step 0. | 2575-2582 | UNIT |
| IPC-79 | `hud:nudge` {dx, dy} | See POS-18. | 2496 | SMOKE+ |
| IPC-80 | `buddy:focus` | Focus the tour card. | 2429 | SMOKE+ |
| IPC-81 | `glass:hello` | Marks the window as a glass page and schedules a frame now. | 2463, backdrop.js:235-239 | SMOKE:glassLoaded |
| IPC-82 | `glass:passthrough` {ignore} | See WIN-12. | 2458-2462 | SMOKE:collapseClickable |
| IPC-83 | `settings:set-surface` {surface} | auto/solid; rebroadcast tier; {surface, tier, reasons}. | 2464-2468 | SMOKE+ |
| IPC-84 | Allowlist integrity | Exactly the 83 invoke channels in preload.js:8-96, no more. | preload.js | UNIT (`ipc::tests::invoke_allowlist_matches_preload`) |

## 8. IPC: events (main -> renderer)

`window.aeval.on(channel, cb) -> unsubscribe`. Unknown channels throw `Blocked IPC channel: <channel>`
synchronously. Payload shapes unchanged. Allowlist: preload.js:98-136, unit-tested.

| ID | Event | Sent to | Source |
|---|---|---|---|
| EVT-01 | `overlay:state` | overlay, output | main.js:807-811 |
| EVT-02 | `hud:snapped` {} | overlay or output | main.js:1027,1052,1059,1083 |
| EVT-03 | `hud:docked` {docked} | removed from both allowlists: no page listens and the Tauri port never sends it (output windows float on their own, REQUESTS #11) | main.js:851,1051,1063,1082 |
| EVT-04 | `query:start` {gen} | output | main.js:1540 |
| EVT-05 | `query:stage` {gen,label}: `Reading your screen`, `Busy — reading with X`, `Thinking`, `Busy — thinking with X`, `Answering directly`, `Busy — asking X` | output | main.js:1589-1669 |
| EVT-06 | `query:delta` {gen,text} (directive-free) | output | main.js:1572 |
| EVT-07 | `query:done` {gen, usage{model, ttftMs, ms, prompt, completion, cost, textOnly}, inferred, question} or {gen} | output | main.js:1681-1694,1716 |
| EVT-08 | `query:error` {gen, message, question} | output | main.js:1714 |
| EVT-09 | `chat:user` {gen,text} | output | main.js:1539,2288 |
| EVT-10 | `chat:reset` {} | output | main.js:1972 |
| EVT-11 | `chat:sync` {messages} | output | main.js:493,1938 |
| EVT-12 | `history:changed` {} | history | main.js:1865 |
| EVT-13 | `buddy:resolve` {selector} | overlay/settings | cursor-buddy.js:251 |
| EVT-14 | `buddy:watch` {selector|null} | overlay/settings | cursor-buddy.js:239,366 |
| EVT-15 | `buddy:show` payload | buddy | cursor-buddy.js:84,143,189,280 |
| EVT-16 | `buddy:hide` | buddy | cursor-buddy.js:297 |
| EVT-17 | `buddy:done` | buddy | cursor-buddy.js:347 |
| EVT-18 | `buddy:move` | removed from both allowlists: never sent; `ipc::tests::every_listen_channel_has_a_sender` keeps unsent channels out | preload.js:116 |
| EVT-19 | `settings:data` | removed from both allowlists: never sent; settings read `settings:get` | preload.js:117 |
| EVT-20 | `update:state` state | settings | main.js:2872 |
| EVT-21 | `find:hits` {gen, query, hits, current} | output, overlay | find.js:305 |
| EVT-22 | `find:state` {gen, open, status, lines, ...} | output, overlay | find.js:306,785 |
| EVT-23 | `agent:begin` {task} | output | main.js:2289 |
| EVT-24 | `agent:step` row | output | agent.js:256-266 |
| EVT-25 | `agent:state` {state, step, message...} | output | agent.js:248-252 |
| EVT-26 | `agent:confirm-request` {id, text, reason, why, url{host,rest,full}} | output | agent.js:721-727 |
| EVT-27 | `agent:done` {state, text, rows, steps, usage} | output | agent.js:1121 |
| EVT-28 | `agent:consent-request` {task} | output | main.js:2282 |
| EVT-29 | `lens:draw` {hits, current, frame} | lens | lens.js:99,122 |
| EVT-30 | `view:set` {view} chat/find | output | main.js:2075 |
| EVT-31 | `bar:prefill` {text, mode} | overlay | main.js:2016 |
| EVT-32 | `settings:section` {id} | settings | main.js:510,551 |
| EVT-33 | `bar:open` {lock} | overlay | main.js:2012 |
| EVT-34 | `bar:context` {screen<=16000, title, process, chars} | overlay | main.js:2043-2048 |
| EVT-35 | `glass:frame` {img, full, display, tier, reasons, fixture} | glass pages | backdrop.js:127-136 |
| EVT-36 | `glass:tier` {tier, reasons} | glass pages | backdrop.js:87,224 |
| EVT-37 | `lens:loupe` {i, img} | lens | lens.js:153 |
| EVT-38 | Allowlist integrity: exactly the 37 listen channels in preload.js:98-136. | preload.js | UNIT (`ipc::tests::listen_allowlist_matches_preload`) |

Every event must reach only the window(s) named above (Electron sends per-webContents). Verify:
SMOKE+:eventTargeting (the output window never receives `bar:open`).

`glass:frame.img` and `lens:loupe.img` are binary JPEG Buffers in Electron (structured clone). The Rust
port must deliver an equivalent the unchanged `glass.js`/`lens.js` can draw (see ARCHITECTURE.md 4.3);
verify SMOKE:glassLoaded, snapCost (main-thread block p95 < 25 ms), SHOT matrix.

## 9. Persisted settings: `%APPDATA%\aevalbuddy\aevalbuddy.json` (STORE)

electron-store file, name `aevalbuddy`, in userData. JSON, keys at top level. Defaults are merged on
read; electron-store also writes defaults back to disk. The Rust store must read a file written by
Electron v1.1.1 and write one that Electron v1.1.1 can still read (rollback safety). Verify for every row:
MIGRATE (round trip) + UNIT (getter semantics).

| ID | Key | Default | Semantics | Source |
|---|---|---|---|---|
| STORE-01 | `apiKeyEncrypted` | '' | base64 of Electron `safeStorage.encryptString` output (see STORE-40). | store.js:47,145-168 |
| STORE-02 | `apiKeyPlain` | '' | Fallback when encryption unavailable; cleared when encrypted is set. | store.js:48 |
| STORE-03 | `model` | DEFAULT_MODEL | Used only when `modelChosen`. Empty -> default. | store.js:49,179-190 |
| STORE-04 | `modelChosen` | false | | store.js:53 |
| STORE-05 | `brainEnabled` | false | Used only when `brainEnabledChosen`. | store.js:60,220-229 |
| STORE-06 | `brainEnabledChosen` | false | | store.js:64 |
| STORE-07 | `brain` | DEFAULT_BRAIN | Used only when `brainChosen`. | store.js:65,192-203 |
| STORE-08 | `brainChosen` | false | | store.js:66 |
| STORE-09 | `showPaidModels` | false | | store.js:68 |
| STORE-10 | `skill` | 'auto' | 'auto' or a skill id that exists, else 'auto'. | store.js:72,243-253 |
| STORE-11 | `shortcuts` | DEFAULT_SHORTCUTS | Merged over defaults on read. | store.js:73,255-268 |
| STORE-12 | `overlayPos` | {h:'right', v:'top', offX:16, offY:16} | Sanitised: h in left/center/right, v in top/center/bottom, offsets rounded and clamped 0..8000 else 16. | store.js:74,270-287 |
| STORE-13 | `outputPos` | {docked:true, h:'right', v:'top', offX:16, offY:240} | Same sanitising; docked unless explicitly false. | store.js:75,289-300 |
| STORE-14 | `inputWidth` | 600 on a new profile (D-16; Electron 420). A stored width is kept. | Clamped 320..900. | store.js:79,302-318 |
| STORE-15 | `outputWidth` | 420 | Clamped 320..900. | store.js:80 |
| STORE-16 | `outputHeight` | 0 | 0 = auto; positive = user-dragged height. | store.js:81,331-340 |
| STORE-17 | `settingsSize` | {width:660, height:560} | Width 380..900, height 360..1400; note setter falls back to 460x470 on bad input (keep). | store.js:82,409-428 |
| STORE-18 | `historySize` | {width:460, height:560} | Same clamps. | store.js:83,430-445 |
| STORE-19 | `saveHistory` | true | | store.js:86 |
| STORE-20 | `launchAtLogin` | false | Fresh installs flip it on via decidePresence. | store.js:92 |
| STORE-21 | `surface` | 'auto' | 'solid' or 'auto'. | store.js:95,395-402 |
| STORE-22 | `presenceDecided` | false | | store.js:96,360-366 |
| STORE-23 | `presenceNoticePending` | false | Taken once by the toast. | store.js:97,368-372 |
| STORE-24 | `walkthroughAutoShown` | false | | store.js:98 |
| STORE-25 | `shortcutsVersion` | 0 | Migration marker; current 2. | store.js:99 |
| STORE-26 | `inputHistory` | [] | Strings only, last 50. | store.js:100,376-393 |
| STORE-27 | `findModel` | DEFAULT_FIND | Chosen-flag rule. | store.js:103,460-461 |
| STORE-28 | `findModelChosen` | false | | store.js:104 |
| STORE-29 | `agentModel` | DEFAULT_AGENT | Chosen-flag rule. | store.js:105,462-463 |
| STORE-30 | `agentModelChosen` | false | | store.js:106 |
| STORE-31 | `findScope` | 'window' | 'screen' or 'window'. | store.js:107,473-491 |
| STORE-32 | `findRelatedMode` | 'ask' | ask/auto/off; setter maps true->auto, false->off. | store.js:110,484-488 |
| STORE-33 | `findOcr` | true | Not-false = true. | store.js:111 |
| STORE-34 | `agentPermission` | 'risky' | 'every' or 'risky'. | store.js:113,493-512 |
| STORE-35 | `agentMaxSteps` | 25 | Clamped 10..50. | store.js:114 |
| STORE-36 | `agentScreenshots` | true | | store.js:115 |
| STORE-37 | `agentReturnCursor` | true | | store.js:116 |
| STORE-38 | `agentConsent` | false | | store.js:117 |
| STORE-39 | Migration v2 | If shortcutsVersion < 2: any shortcut equal to its old default (Ctrl+Shift+Space/A/G/U/H) or missing moves to the new default; `findRelated:false` becomes `findRelatedMode:'off'`; `findRelated` and `lastMode` deleted; version set to 2. | store.js:121-134 | MIGRATE (v1.0 profile fixture) |
| STORE-40 | API key encryption compatibility | Electron `safeStorage` on Windows is Chromium OSCrypt: blob = `v10` + 12-byte nonce + AES-256-GCM ciphertext+tag, key = `os_crypt.encrypted_key` in `%APPDATA%\aevalbuddy\Local State` (base64, `DPAPI` prefix, then CryptUnprotectData). Rust decrypts exactly that and encrypts the same way (so a rollback to Electron still reads it). Decrypt failure -> '' (user re-enters), never a crash. A new key is minted only when `Local State` is missing or is a JSON object with no `os_crypt.encrypted_key`; a file that cannot be read or parsed is left alone (copied to `Local State.corrupt`) and keys are stored in plain text that session. A UTF-8 BOM is skipped. The file is rewritten durably with every other key kept. | store.js:136-168 | MIGRATE (key set by Electron is read by Rust and vice versa) |
| STORE-41 | `hasApiKey` = decrypted key non-empty. | store.js:175-177 | UNIT |
| STORE-42 | heal*: a chosen model that left the catalogue is rewritten in place; a non-chosen default is swapped in memory only. Same for brain, find, agent. | store.js:205-218,465-471 | SMOKE:healRetired |
| STORE-43 | Unknown keys in the file are preserved on write (Electron writes the whole object). | electron-store | MIGRATE |
| STORE-44 | Write is atomic and durable: one save at a time with the snapshot taken under the save lock, a unique temp name, fsync, then `MoveFileExW(REPLACE_EXISTING \| WRITE_THROUGH)`; the previous good file is kept as `aevalbuddy.json.bak`. A file that does not parse is copied to `.corrupt` and `.bak` is used in its place; a file that exists but cannot be read (after three retries) is never written over that session; invalid UTF-8 is decoded lossily, as Node did. | electron-store | UNIT |

## 10. Files on disk (FILE)

| ID | Path | What | Source | Verify |
|---|---|---|---|---|
| FILE-01 | `%APPDATA%\aevalbuddy\` | userData (Electron default for package name `aevalbuddy`). The Rust app must use this exact folder, NOT Tauri's `%APPDATA%\com.aevalbuddy.app`. | Electron | MIGRATE |
| FILE-02 | `...\aevalbuddy.json` | Settings (section 9). | store.js:42-44 | MIGRATE |
| FILE-03 | `...\history.json` | `{version:1, conversations:[{id, at, title, model, messages:[{role, text, kind?}]}]}`; written durably (unique temp, fsync, write-through replace) 400 ms after the first change by one writer thread (a throttle, as history.js:55-63 did; later changes ride along); a file that does not parse is renamed to `history.json.corrupt-<unix ms>` first. | history.js:34-84 | SMOKE:historySaved + MIGRATE |
| FILE-04 | `...\errors.log` | `[ISO] scope\nstack\n\n`; when > 256 KB keep the last 128 KB. | crash.js:25-52 | SMOKE:crashLogged |
| FILE-05 | `...\updater.log` | `[ISO] level message`; > 256 KB keeps the last 128 KB; smoke reads its last warn/error line. | updater.js:85-103 | SMOKE:update.cause |
| FILE-06 | `...\updater.json` | {lastChecked, lastRunVersion}. | updater.js:112-133 | UPDATE |
| FILE-07 | `...\Local State` | Chromium file holding the OSCrypt key (STORE-40). Must not be deleted by the Rust app; WebView2 must not be pointed at this folder as its user data folder (it would rewrite it). | Electron/Chromium | MIGRATE |
| FILE-08 | `%LOCALAPPDATA%\aevalbuddy\desktop-<hash>.dll` | Compiled desktop helper cache (dev / no-prebuilt path). | desktop/index.js:214-218 | SMOKE:desktopReady |
| FILE-09 | `<install>\resources\desktop-<hash>.dll` + `prebuilt.json` sha256 | Prebuilt helper, loaded only if its SHA-256 matches. Rust ships it as a Tauri resource with the same check. | electron-builder.yml extraResources, host.ps1 | SMOKE:desktopReadyMs < 5000 |
| FILE-10 | `<install>\resources\app-update.yml` | provider github / owner aevalmere / repo aevalbuddy / not private. Rust replaces this with a compiled-in feed config; smoke `updateConfig` checks that config instead. | electron-builder.yml publish | SMOKE:updateConfig |
| FILE-11 | `skills/*.md` | Front matter name/summary + body; order screen-assist, debug, explain, rewrite, then alphabetical; loaded once; drop-in. | skills.js | SMOKE:skills |
| FILE-12 | `tools/fixtures/llm/*.jsonl` | Fake-LLM fixtures, shipped in the app (inert unless AEVAL_FAKE_LLM). | electron-builder.yml files | SMOKE (v2desktop) |
| FILE-13 | WebView2 user data | Must live outside `%APPDATA%\aevalbuddy` root files (use `%LOCALAPPDATA%\aevalbuddy\WebView2`) so Electron's `Local State` is untouched. | new | RUNNER |
| FILE-14 | Nothing else is written: no screenshots, no screen readings, no window text to disk. | history.js header | RUNNER (diff the profile folder after smoke) |

## 11. Ask: the query flow (ASK)

| ID | What | Source | Verify |
|---|---|---|---|
| ASK-01 | `help me` (optionally with . or !) starts the tour instead of asking. | main.js:1511-1514 | SMOKE+:helpMe |
| ASK-02 | No key -> {status:'needs-key'}, overlay state refreshed, nothing sent. | main.js:1515-1518 | SMOKE:queryWithoutKey |
| ASK-03 | Each submit aborts the previous one; generation counter; empty input sends `What's on my screen and what should I do?`. | main.js:1519-1522,127 | SMOKE+ |
| ASK-04 | Retry resumes the unanswered last user turn without duplicating it; otherwise it is a new message. | main.js:1527-1530 | SMOKE+:retryNoDup |
| ASK-05 | Up to 12 prior turns travel as text; only the newest message carries the screen. | main.js:140,1529; openrouter.js:926-946 | DIFF |
| ASK-06 | Echo (`chat:user`) and thinking (`query:start`) are sent before the capture, so sending feels instant; output shown; busy state on. | main.js:1537-1543 | SHOT:05 |
| ASK-07 | Capture, live-model check and exact-text read run in parallel; fresh bar text (< 8 s old, same region) is reused. | main.js:1548-1554,2057-2061 | SMOKE+ |
| ASK-08 | Text-only answer when: shot taken, brain off, not withImage, exact text >= 800 chars, question not VISUAL (`renderer/intent.js` VISUAL regex). | main.js:1563-1565,205 | SMOKE+:textOnly |
| ASK-09 | Brain mode: reader (vision, non-streamed) with failover -> brain (text, streamed) with failover; reader failure falls back to one stage; brain failure before any text falls back to the vision model with the image ("Answering directly"). | main.js:1587-1678 | DIFF + SMOKE+ (fake LLM) |
| ASK-10 | Skill: pinned or routed (`auto`: reader TASK word outranks question cues). | main.js:1618-1623, skills.js | SMOKE:skillRoute, skillRouteFromScreen |
| ASK-11 | Point directives stripped from the stream and collected; pointer shown after the answer (max 3), on the display the shot came from. | main.js:1572,1698-1702,1765-1802 | SMOKE:pointResolved |
| ASK-12 | Done: assistant turn appended, `query:done` with usage meter, history saved (after every answer), pill error cleared, tour notified `first-answer`. | main.js:1679-1703 | SMOKE+ |
| ASK-13 | Error: partial text kept in chat and history; friendly message (naming the reader model if the reader failed) with the question for Retry; abort sends `query:done` with no message. | main.js:1704-1718 | SHOT:14-answer-error |
| ASK-14 | Only the newest generation clears busy and calls `updater.idle()` (a held restart never cuts off a newer answer). | main.js:1719-1730 | UPDATE |
| ASK-15 | Live models: stored vision/brain ids checked against the cached (5 min) catalogue; a retired id is replaced and a chosen one healed on disk; warning logged. | main.js:1477-1491 | SMOKE:healRetired |
| ASK-16 | Exact text: foreground window (never ours) via UI Automation, region converted DIP->physical, maxChars 16000, no OCR, timeout 1500 ms warm / 4000 ms cold, null on any failure. | main.js:1809-1821 | SMOKE:askHasUiaText |
| ASK-17 | Bar context prefetch on open: starts the helper and the catalogue, reads foreground process/title/text, sends `bar:context` (only if still the latest). | main.js:2025-2053 | SMOKE+ |
| ASK-18 | New chat: abort, clear chat, new id, drop region and last question, output height back to auto, `chat:reset`, hide output, refocus bar. | main.js:1960-1978 | SMOKE+ |
| ASK-19 | Reopen from history: chat restored (trail kind kept), region cleared, height auto, History closed, bar expanded, output shown, `chat:sync`. | main.js:1921-1942 | SMOKE+ |
| ASK-20 | Chat id format `c<base36 time><6 random>`. | main.js:179-181 | UNIT |

## 12. Models, catalogue, failover, prompts (MODEL / FAIL / PROMPT / POINT)

| ID | What | Source | Verify |
|---|---|---|---|
| MODEL-01 | Endpoints: chat `https://openrouter.ai/api/v1/chat/completions`, models `/api/v1/models`, key `/api/v1/key`. Headers: Bearer key, JSON, `X-Title: AevalBuddy`; body gets `usage:{include:true}`. | openrouter.js:7-43 | DIFF (request bodies) |
| MODEL-02 | Defaults: vision `nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free`, brain `nvidia/nemotron-3.5-super-vl:free`, find `nvidia/nemotron-3.5-lightning:free`, agent `nvidia/nemotron-3.5-super-vl:free`. Both vision and brain defaults must be in the live free list or smoke fails. | openrouter.js:53-80 | SMOKE:modelDefaultOffered, brainListDefault |
| MODEL-03 | Curated vision list (10, order, tiers, notes) and brain list (5); agent pool = brains + curated, deduped; find pool = brains. | openrouter.js:92-166,222-229 | UNIT (table) |
| MODEL-04 | Tier labels `Free` / `Paid, a fraction of a cent per question`; offline fallback list from the curated table so the picker is never empty. | openrouter.js:168-179 | SMOKE:modelList (`:fallback`) |
| MODEL-05 | Catalogue cached 5 min, 8 s timeout, null on failure or empty. | openrouter.js:284-298 | UNIT |
| MODEL-06 | listModels: paid hidden unless asked; retired or image-less (vision) models dropped; detail line `$X in / $Y out per million tokens · NK context` or `Free · NM context`; agent entries tagged `Tools`/`JSON mode`. | openrouter.js:186-280 | SMOKE:modelList, modelListPaidHidden, modelListWithPaid |
| MODEL-07 | pickLive: keep if usable (vision needs image input; others any listed), else kind default, else first free curated usable, else keep id; never moves a paid choice to another paid one. | openrouter.js:318-346 | SMOKE:healRetired + UNIT |
| MODEL-08 | testKey messages: `No key saved yet.`, `OpenRouter rejected this key. Check it and save again.`, `OpenRouter replied HTTP N.`, `Key works. About $X.XX of credit left.`, `Key works, but the credit is used up. Add credit, or pick a Free model.`, `Key works.`, `Could not reach OpenRouter. Check your connection.` | openrouter.js:366-398 | UNIT (mock) |
| FAIL-01 | Failover statuses: 404, 408, 429, 500, 502, 503, 504, 529 (Anthropic's overloaded); never on abort, a refusal, or an account-wide limit (the daily free cap, an empty balance), which every model of the account shares. | openrouter.js:840-847 | SMOKE:failoverRateLimit |
| FAIL-02 | Only `:free` models fail over; paid never switches. Auth (401) and other errors throw immediately. | main.js:1456 | SMOKE:failoverKeepsAuthError |
| FAIL-03 | At most 3 models per request. | main.js:1414,1457 | SMOKE:failoverCapped |
| FAIL-04 | No failover once text reached the user (`canRetry`). | main.js:1455,1648,1668 | DIFF |
| FAIL-05 | nextFreeModel: first untried free in the kind's pool (find uses brains), skipping ids that answered 404 this session; when the catalogue is known, only listed stand-ins, and for Vision only ones that take images, for Agent only ones with tool calls. | openrouter.js:857-864 | UNIT |
| FAIL-06 | Stage labels name the stand-in by its catalogue display name (C-21..C-23). The short name (last path part, `:free` stripped, 28 chars max with ellipsis) stays in `query:done.usage.model` for older pages. | main.js:1418-1421 | UNIT |
| PROMPT-01 | Reader prompt verbatim (SCREEN/STATE/FOCUS/TASK/TEXT/ELEMENTS, grid rule, copy exactly) + REGION_NOTE for areas; max_tokens 1600, temperature 0, non-streamed; report unfenced; rejected unless >= 40 chars with a SCREEN line (status 204, stage reader). | openrouter.js:423-457,631-752 | DIFF (byte-equal prompt) |
| PROMPT-02 | System prompt variants (reader-fed brain, screen, region, no screen, exact-text name pointing) verbatim, joined by newlines; skill appended as `--- <name> ---`. Never quotes the image size. | openrouter.js:760-833 | SMOKE:brainPrompt, skillInPrompt, promptShape, gridShared, regionPrompt |
| PROMPT-03 | Exact-text block wording, 16000-char cap with `[cut]`. | openrouter.js:668-682 | DIFF |
| PROMPT-04 | Answer stream temperature 0.2; Find text calls temperature 0, max_tokens 800 default; agent turns temperature 0, tool_choice auto, parallel_tool_calls false. Gemini requests omit temperature (D-18). | openrouter.js:957,1054-1118 | DIFF |
| PROMPT-05 | SSE parsing: `data:` lines only (keepalive comments skipped), `[DONE]`, error objects -> ApiError(code), usage accumulated, TTFT measured on first content; empty stream = 204 `empty response` unless allowed. A line over 1 MB is a 502; 45 s without a byte is a 408 (B-041); a 200 body holding an `error` object is that error. | openrouter.js:995-1048 | UNIT (recorded streams) |
| POINT-01 | Coordinate directive regex (loose shapes) and named directive regex; malformed `[[point ...]]` swallowed; non-directive `[[...]]` kept; directive held up to 220 chars across deltas; gap whitespace closed. | openrouter.js:1130-1238 | SMOKE:pointExtract, pointShapes, pointNonDirective, pointNamedExtract |
| POINT-02 | ELEMENTS parsing: bullets, coordinate pair anchored at line end, %, fractions, pixel rescue with view size, max 8 rows, any other FIELD: ends the block. | openrouter.js:472-549 | SMOKE:readerElements, readerRowShapes, readerPixelRow |
| POINT-03 | matchElement: exact name wins; containment and word-overlap scoring; min 0.5; margin 0.08 against runner-up. Names compare by Unicode letters and digits, so Cyrillic and CJK labels match. | openrouter.js:551-615 | SMOKE:pointByName, pointAmbiguous |
| POINT-04 | pointFraction: 0-1000 grid; >1000 read as view pixels if inside the view; negative or outside -> dropped. | main.js:1744-1753 | SMOKE:pointGridMapping, pointPixelRescue, pointBottomEdge, pointOffScreen |
| POINT-05 | Points map onto the region rect when the shot was an area, else the display. | main.js:1773-1778 | SMOKE:regionPointTarget |
| POINT-06 | Named point resolved first against the live UI (UIA find, score >= 70, exact unique name or 8-point lead, role words stripped), DIP centre; else against ELEMENTS. | main.js:1838-1853 | SMOKE:pointFromUia |

## 13. Error messages (ERR)

All strings verbatim (they are UI). Verify: UNIT table test against `friendlyError` + DIFF.

| ID | Condition | Text | Source |
|---|---|---|---|
| ERR-01 | data policy / free model publication | `Your OpenRouter account blocks free models. Free models are offered on the condition ...` (full text) | openrouter.js:1256-1258 |
| ERR-02 | 401 | `OpenRouter rejected the API key. Open Settings, check the key, and save it again.` | 1259-1261 |
| ERR-03 | 402 | `<model> bills per token and this account has no credit. Add a dollar at openrouter.ai/credits ...` | 1262-1264 |
| ERR-04 | 403 | `OpenRouter refused the request: <detail>` | 1265-1267 |
| ERR-05 | 404 | `<model> is not available right now, and the free model tried after it was not either. OpenRouter says: <detail>. Pick another model in Settings.` | 1268-1270 |
| ERR-06 | 429 free / paid | `Every free model is busy. ...` / `Rate limited by the provider. Wait a moment, then retry.` | 1271-1275 |
| ERR-07 | 400 image | `The model <m> does not accept images. Pick another Screen model in Settings, or turn on the Answer model so a screen model reads for it.` | 1276-1279 |
| ERR-08 | 400 other | `Request rejected: <detail>` | 1280 |
| ERR-09 | 204 | `The model <m> streamed no answer text. Pick a different model in Settings.` | 1282-1284 |
| ERR-10 | >= 500 | `OpenRouter returned a server error, on this model and on the fallback. Retry in a moment.` | 1285-1287 |
| ERR-11 | network (TypeError) | `No connection. Finding words on your screen still works; answers need the internet.` | 1288-1290 |
| ERR-12 | other / none | `Request failed: <detail>. Retry, or ask again.` / `Request failed for an unknown reason. Retry, or ask again.` | 1250,1291 |
| ERR-13 | abort | no message (null) | 1251 |
| ERR-14 | Agent friendly: own window, elevated target, stale ref, `<CODE>: msg`. | `Refused: that is AevalBuddy's own window.` / `Refused: that window runs as administrator, and AevalBuddy does not.` | agent.js:788-795 |
| ERR-15 | Presence toast | `<chord> is taken by another app` / `Open AevalBuddy from the tray icon by the clock, then pick another shortcut in Settings.`; or `AevalBuddy is running` / `Press <chord> anywhere to ask, find, or do. It starts with Windows; turn that off in Settings.` | main.js:2353-2368 |
| ERR-16 | Crash toast | `AevalBuddy hit an error` / `It is still running. The details were written to errors.log in its settings folder.` | crash.js:56-69 |
| ERR-17 | Update texts | see UPD-09..11 | updater.js |

## 14. Find (FIND)

Port of lib/find.js. Verify by DIFF on recorded snapshots plus the listed smoke lines.

| ID | What | Source | Verify |
|---|---|---|---|
| FIND-01 | Stage A literal on every keystroke: exact substring, token prefix, one edit for words >= 5 letters; NFKD fold, diacritics stripped; camelCase/snake/digit splits. Offline and keyless. | find.js:1-220 | SMOKE:findLiteral |
| FIND-02 | Stage B related: 400 ms after typing stops, one streamed model call, each term matched with stage A as its line arrives. Prompt verbatim. | find.js:510-595 | SMOKE:findRelated |
| FIND-03 | Stage C concept: one call over un-hit numbered spans within a 24000-char budget; lines scoring >= 0.5. | find.js:517-664 | SMOKE:findConcept |
| FIND-04 | Related mode ask/auto/off; `find:related` runs it on demand; the status explains how many characters are sent. | find.js:262-290,754-760 | SHOT:19 |
| FIND-05 | Snapshot via getText (UIA, OCR fallback when enabled); scope window or screen (up to 6 windows); region restricts; re-snapshot poll 2 s; wheel nudges refresh. | find.js:328-425,686-694 | SMOKE+ |
| FIND-06 | Status lines verbatim: `Reading this window…`, `Nothing on screen matches "<q>".`, `Looking for related…`, `No related matches either.`, `Related words send about N characters ...`, `Related matches need a connection.`, `Related matches need an API key. Add one in Settings.`, `Related matches are unavailable right now; exact ones still work.`, `This window shares no text. Try Ask, which looks at the pixels.`, `This window shares no text. Turn on text recognition in Settings, or try Ask.`, `Could not read this window. Using text recognition instead.`, `Desktop reading stopped responding. Restarting it…`. | find.js:262-290,395-420 | SHOT:26,27, SMOKE:findOffline |
| FIND-07 | Step wraps; current hit pinned; hits ordered; `find:hits` + `find:state` emitted with gen; results list appears only once there is something to say. | find.js:220-310,726-752; main.js:2098-2108 | SMOKE:findStep |
| FIND-08 | Hits go to the lens in DIP; `onboarding` notified `find-hit`. | main.js:2110-2124,2805 | SMOKE:lensHits |
| FIND-09 | Find works offline and without a key for literal hits. | find.js | SMOKE:findOffline |

## 15. Agent (AGENT)

Port of lib/agent.js (1646 lines, Electron-free). Every rule below is a quality/safety item. Verify by DIFF
(the gate is pure given a fake desktop) plus smoke.

| ID | What | Source | Verify |
|---|---|---|---|
| AGENT-01 | Constants: 3 consecutive failures, 6 min wall clock, 60 s model timeout (-> 408), observation 12000 chars, thin tree < 15 nodes adds a screenshot, takeover after 24 px of mouse travel, retry backoff 2 s/6 s after acting. | agent.js:21-27 | DIFF |
| AGENT-02 | 26 tools with exact names, descriptions, JSON schemas; JSON-mode rules text generated from the same table. | agent.js:46-160 | DIFF (serialised TOOLS equal) |
| AGENT-03 | Rules prompt verbatim (untrusted screen text, one action per turn, refs over coordinates, never act on our windows, ask_user over guessing, consequential flag, plain-text summary to finish). | agent.js:130-143 | DIFF |
| AGENT-04 | JSON-mode parsing: last balanced object, `{done}` or `{tool|action|name, args|inline}`; unreadable twice -> error state. | agent.js:162-230,1284-1291 | SMOKE:agentJsonMode |
| AGENT-05 | Gate classes read/input/consequential/refused; broken check fails closed (consequential). All rules: clipboard_read asks; read tools free; plan gate refuses until approved; `open` only http/https/mailto or an app name; NAME_VERBS on control name/automationId/helpText; VERBS in reason; model's consequential flag; dialog (#32770) title/body verbs; risky browser pages; mail/chat buttons; terminals (type/key/form_input); Run box; drags; unreadable pointer targets; key chords (HARD_CHORDS, Enter, Delete, F-keys, Win, Alt, Ctrl except FREE_CTRL, non-typing keys, repeats); Enter in address bar / compose fields; type with newline/tab, password field, unknown focus with a password present, address bar; Notepad newline exemption; form_input on password; close_window; permission `every`. | agent.js:470-692 | SMOKE:gateBypasses, gatePassesPlainInput, gateFailsClosed, gatePlanFirst, gateChords |
| AGENT-06 | Confirm card: unguessable id, text from `describe`, reason, why, parsed URL; trail focused; pointer shown at the target. Only the current id resolves. | agent.js:704-741 | SMOKE:agentConfirmGate |
| AGENT-07 | Observation: interactive tree (all for dialogs), password lines redacted twice, cut at 12000 with a hint; screenshot only if screenshots on, model sees images, and (forced or thin tree). | agent.js:744-786 | SMOKE:passwordRedacted |
| AGENT-08 | Context hygiene: only the newest observation and newest image sent in full; older ones collapse to their summary / `[screenshot omitted]`. | agent.js:1015-1033 | DIFF |
| AGENT-09 | Model failover: before acting, a busy free model hands over to the next tools-capable free one (max 3, row `Busy, switching to X`); after acting, the same model is retried with backoff. | agent.js:1035-1103 | DIFF |
| AGENT-10 | Takeover: physical input while running (not while acting/confirming, 150 ms after our own act) pauses with `Paused: you took over. Enter to continue, Esc to stop.` | agent.js:961-980 | SMOKE:agentTakeover |
| AGENT-11 | Resume answers ask_user, approves a plan, or unpauses. | agent.js:982-1002 | SMOKE:agentTakeover |
| AGENT-12 | Finish: one emit of `agent:done` with rows, steps, usage; frame off; pending confirm/ask/plan settled; history gets a collapsed trail message + final text; pill error for error/limit/offline. | agent.js:1105-1131, main.js:2247-2261 | SMOKE+ |
| AGENT-13 | End texts verbatim: `Stopped. Nothing else was done.`, six-minute limit, `Stopped after N steps, the most one task may take. Raise it in Settings > Agent.`, offline after step N, format failure twice, `The agent stopped without saying whether the task was done. Check the app before relying on it.`, `The agent stopped: <friendly>`. | agent.js:1136-1332 | SMOKE:agentMaxSteps |
| AGENT-14 | Focus kept: no step finds an AevalBuddy window in front (`ownFocused` asks the OS, not the webview). | main.js:2822-2832 | SMOKE:agentFocusKept |
| AGENT-15 | Start flow: empty/needs-key/unsupported/busy; first-run consent card; Find closed; chat gets the task; offline -> done offline with no action; catalogue fetched and agent model resolved+healed; bar collapses keeping the trail; plan gate when started with context; region passed in physical px. | main.js:2268-2307 | SMOKE+ |
| AGENT-16 | Control frame on the target display with chip `AevalBuddy is using your computer · Esc to stop`. | main.js:2173-2190 | SHOT:22, SMOKE:lensFirstDraw |
| AGENT-17 | Live cursor glides to each action point with the step label, settling before the action. | main.js:2192-2196, cursor-buddy.js:74-102 | SHOT:22 |
| AGENT-18 | `open` tool: URLs through the default browser, app names via `cmd /d /c start "" <name>` (hidden window) after the name regex; confirmed first. | main.js:2215-2230 | SMOKE+ |
| AGENT-19 | Clipboard read/write tools use the system clipboard (read is consequential). | main.js:2819 | SMOKE+ |
| AGENT-20 | Popups after an action are noticed and reported; rebinding when a ref goes stale. | agent.js:1396-1470 | DIFF |
| AGENT-21 | Trail text for history: plan line, rows with (skipped)/(failed) and errors, numbered. | main.js:2232-2245 | UNIT |
| AGENT-22 | Return cursor setting restores the pointer after the run. | store.js:116, agent.js | SMOKE+ |

## 16. Desktop engine (DESK)

The C# helper (lib/desktop/host.cs, 2630 lines) and its JSON-lines client (lib/desktop/index.js).

| ID | What | Source | Verify |
|---|---|---|---|
| DESK-01 | Process: `powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command -`, bootstrap line sets $AevalCs, $AevalCache, $AevalVersion, $AevalPrebuilt, $AevalDllSha, then host.ps1 over stdin. | desktop/index.js:1-16,259-275; host.ps1 | SMOKE:desktopReady |
| DESK-02 | Protocol: `{id, op, args, timeoutMs}` in; `{id, ok, result}` / `{id, ok:false, error:{code,message}}` out; `{event:'ready'|'fatal'|'user-input'}` lines. | desktop/index.js | UNIT (codec) |
| DESK-03 | Timeouts: 4 s per op, 12 s first UIA read, 15 s OCR / getText with OCR, 25 s start, 8 s capture, type 2 s + 2 ms/char. | desktop/index.js:136-160 | UNIT |
| DESK-04 | Death handling: in-flight calls reject HELPER_DOWN; respawn on next call; 3 deaths in 60 s disables until explicit start. | desktop/index.js | SMOKE+ |
| DESK-05 | Lazy start (LIFE-07); stop kills the whole tree. | desktop/index.js:389-450 | SMOKE:helperNotAtBoot |
| DESK-06 | Own windows excluded from every list and refused as targets (BLOCKED_OWN_WINDOW); lens/buddy registered as `{handle, block:false}` layers; process pid and child pids (WebView2 processes in Rust) included. | main.js:2770-2777; desktop/index.js:472-530 | SMOKE:desktopOwnExcluded, ownWindowBlocked |
| DESK-07 | Low-level input watch on only while agent runs or Find is open; user-input events throttled to ~10/s; key identity never reported. | desktop/index.js:362-375; main.js:2777-2781 | SMOKE:agentTakeover |
| DESK-08 | Physical virtual-screen pixels throughout, per-monitor-v2 DPI aware; conversion to DIP at the edge (`screenToDipRect`/`dipToScreenRect`). | desktop/index.js:17-23 | SMOKE+:dpiRoundTrip at 125/150 % |
| DESK-09 | Ops: listWindows, foregroundWindow (never ours), focusWindow, closeWindow, monitors, readTree (text format `role "name" [ref_N] (states) value=".."`), find (local fuzzy scoring incl. role words, minScore 45), getText (UIA spans + OCR under 200 chars), ocr (Windows.Media.Ocr), capture/screenshot/zoom (maxEdge 1568, 4x cap), elementAt, windowStates, resolve, click (invoke vs SendInput), hover, drag, type (KEYEVENTF_UNICODE, FOCUS_LOST), key, scroll, scrollTo, setValue, invoke, restoreCursor, watchInput, testUserInput, ping. | desktop/index.js:46-131,468-818 | lib/desktop/selfcheck.js (all lines) |
| DESK-10 | Error codes: STALE_REF, NOT_FOUND, BLOCKED_OWN_WINDOW, ELEVATED_TARGET, NO_PATTERN, TIMEOUT, HELPER_DOWN, BAD_ARGS, OCCLUDED, NOT_FOREGROUND, FOCUS_LOST, DISABLED, NO_OCR, UIA_ERROR, INTERNAL, UNSUPPORTED. | desktop/index.js:117-122 | UNIT |
| DESK-11 | Refs `ref_N` stable per RuntimeId within a helper session; generation bumps on structure change. Password values always `•••`. | desktop/index.js:107-114 | SMOKE:passwordRedacted |
| DESK-12 | Prebuilt DLL built at package time (`tools/build-desktop.js`), SHA-256 pinned, loaded from verified bytes only. | tools/build-desktop.js, host.ps1 | SMOKE:desktopReadyMs |

## 17. Screen capture pipeline (CAP)

The most quality-critical section (memory: answer quality is bounded by capture fidelity).

| ID | What | Source | Verify |
|---|---|---|---|
| CAP-01 | Whole screen: the display the bar was opened on (`latched`), defined once (D-09): the foreground window's
monitor when that window is not ours, else the display under the pointer, else the nearest display to
the pointer, else the primary. | main.js:1104-1111 | SMOKE+:captureActiveDisplay (two virtual monitors are not available on the runner; unit-test the selection) |
| CAP-02 | Whole screen is taken at the display's physical pixels and downscaled so the long edge is at most 1600 px
(never upscaled; D-09, Electron used the DIP size); area capture is taken at the display's native pixels (scale factor) and cropped. | main.js:1095,1129-1143 | SMOKE:capture, regionSharper |
| CAP-03 | Crop maps DIP selection -> captured pixels by the size that actually arrived (not the size asked for), clamped inside the image, at least 1 px. | main.js:1156-1170 | SMOKE:regionCapture |
| CAP-04 | Area larger than 1600 on its long edge is resized with best quality (Electron `quality:'best'`; Rust: Lanczos3). No sharpening filter exists today; adding one is a quality change that needs a DESIGN.md entry and before/after shots. | main.js:1173-1182 | SMOKE:regionCapture |
| CAP-05 | JPEG quality 88 with 4:4:4 chroma (D-09; Electron's Skia encoder used 4:2:0), stepping down by 8 while
over 1.6 MB, floor 62. | main.js:1096-1098,1187-1194 | SMOKE:capture (bytes, size) + UNIT |
| CAP-06 | Result: base64, bytes, width, height, display bounds (DIP), region (DIP) or null. Failure -> null (query degrades to text-only), never a crash. | main.js:1198-1211 | SMOKE+:captureFailNull |
| CAP-07 | Display matching on a multi-monitor desk by display id; fallback to the primary; never source order. | main.js:1145-1151 | UNIT |
| CAP-08 | Our windows never appear in a capture (WDA_EXCLUDEFROMCAPTURE honoured by the Rust capture backend). | DESIGN.md Material | SMOKE+:captureExcludesOwn (capture with the bar open over a known colour; the bar's pixels must be absent) |
| CAP-09 | Every message captures fresh (follow-ups see the screen now); the shot taken when an area was picked stands in only while it is under 1 s old. Region sticks to the conversation until cleared or New chat.
Reason: in Electron the pick-time shot answered the first question however long the person took to type it, so a screen that changed meanwhile was described as it had been. | main.js:1544-1551,162-166 | SMOKE+ |
| CAP-10 | Region pick: selector on the active display; Escape claimed while open; cancel leaves the panel as it was; result captured immediately, panel expanded, chip shown. | main.js:1232-1260 | SMOKE:regionEscape, regionSelectorCancels |
| CAP-11 | Region selector: live scrim (not a frozen shot); < 12 px on either axis is a cancel; window coords + display origin; second request replaces the first; blur cancels. | region.js | SMOKE:regionSelectorCancels |
| CAP-12 | DPI: capture sizes at 100/125/150 % produce the same DIP mapping (points land where the model meant). | main.js:1744-1778 | SMOKE+ at each AEVAL_SHOT_SCALE |
| CAP-13 | Capture image quality bar: the Rust whole-screen downscale must be no blurrier than Chromium's thumbnail path. Measured: render a fixture page of 12 px text, capture, compare SSIM/edge energy against the Electron capture of the same fixture (stored on the shots branch). | new | RUNNER:captureFidelity |

## 18. Glass and backdrop (GLASS)

| ID | What | Source | Verify |
|---|---|---|---|
| GLASS-01 | Backdrop: every display captured at 1/6 scale, JPEG q72, held in memory only, sent to visible glass pages (those that said hello); 2 Hz while anything other than the pill is up (fast pace), every 5 s otherwise, none when nothing is visible. | backdrop.js:29-200; main.js:2906-2918 | SMOKE:glassTier, snapCost |
| GLASS-02 | Tier solid when: cannot hide from capture (the stored `surface: 'solid'` is no longer a reason since
B-035: `glassIntensity` 0 is the solid path, so a profile migrated as solid follows the slider; the key
stays in the file for rollback, NEW-12, REQ-5), Windows transparency effects off (`HKCU\...\Themes\Personalize\EnableTransparency`=0), high contrast, >= 3 capture failures (retry every 30 s); renderer adds reduced-transparency and forced-colors. | backdrop.js:45-78 | SMOKE:glassTier |
| GLASS-03 | Tier and reasons broadcast on change, on load, on theme change; Settings shows the reason. | backdrop.js:84-89,202-228 | SHOT:08 |
| GLASS-04 | Pause/resume around the live photograph (no frames while windows are capturable). | backdrop.js:241-247; main.js:3089-3120 | SHOT:12-live-overlay |
| GLASS-05 | Fixture mode for shots: a rendered page (white document, black editor, wallpaper: lib/backdrop-fixture.html) stands in for the desktop; windows also get the full-size picture. | backdrop.js:254-330 | SHOT matrix |
| GLASS-06 | Main-thread cost: capture may not block the main thread > 25 ms p95 (Rust: the event loop thread). | smoke-glass.js:102-113 | SMOKE:snapCost |
| GLASS-07 | Style stability: computed glass styles do not drift across samples. | smoke-glass.js:150-173 | SMOKE:styleStable |
| GLASS-08 | Contrast gate: every frame of the matrix at 100/125/150 % meets WCAG 4.5:1 (3:1 large) measured against the real background pixels. | contrast.js, shots-matrix.js | SHOT matrix (failedFrames=0) |
| GLASS-09 | Lens loupe: magnified cut of the current hit sent to the lens. | lens.js:138-160 | SHOT:20-lens |

## 19. Lens, tour and pointer (LENS / TOUR)

| ID | What | Source | Verify |
|---|---|---|---|
| LENS-01 | Hits drawn per display (literal/related/concept kinds, why text), current ring, agent frame + chip; first draw happens even if hits arrived while loading. | lens.js | SMOKE:lensFirstDraw, lensWhyClear |
| LENS-02 | Hidden/cleared on Find close; frame cleared on agent finish. | main.js:2088-2096,2247-2249 | SMOKE+ |
| TOUR-01 | Four steps (D-06): Find on the orb (`find-hit`), a card asking for a provider with Connect / Later (`key-saved`,
skipped when a key exists), Ask (`first-answer`, skipped without a key), the bar's intent token with Got it.
Only the main chord is quoted, never an empty one. The old Try card is gone (`try` still works, B-129). | main.js:231-267 | SMOKE:onboardingSteps |
| TOUR-02 | Auto-runs once on first launch with no key (1 s after boot), never under smoke/shots; then only on `help me` or Replay. | main.js:2885-2895 | SMOKE+ |
| TOUR-03 | Steps resolve selectors in app windows, poll 150 ms, open prerequisites (Settings section, exit HUD), settings-closed returns to step 0, dismiss cancels. | cursor-buddy.js | SMOKE:walkthrough |
| TOUR-04 | Point mode: up to 3 model points, advanced with the callout; on the display of the shot. | cursor-buddy.js:167-201 | SMOKE:buddyPoints, buddyPointsDone |
| TOUR-05 | The pointer never moves the real OS cursor (except the agent's own SendInput actions). | cursor-buddy.js:1-8 | SMOKE+ |

## 20. History (HIST)

| ID | What | Source | Verify |
|---|---|---|---|
| HIST-01 | Text only; max 200 conversations; trimmed oldest-first while the JSON exceeds 4 MB; title = first user line, whitespace collapsed, 90 chars with ellipsis. | history.js:23-105 | SMOKE:historySaved, historyTitle |
| HIST-02 | Upsert by id (same id = edit, not a new row); only user/assistant turns with text; trail kind kept. | history.js:115-138 | SMOKE:historySaved |
| HIST-03 | Search over title and every message, case-insensitive. | history.js:146-172 | SMOKE:historySearch |
| HIST-04 | Disabled = no writes, nothing deleted. | history.js:86-88,116 | SMOKE:historyRespectsSwitch |
| HIST-05 | Missing file = empty list; a file that does not parse is renamed aside, then an empty list; a file that exists but cannot be read = empty list and no writes this session. Never a failed start. A failed write is retried on the next change and on quit. | history.js:39-75 | UNIT |
| HIST-06 | Saved after every answer and every agent run; flushed on quit. | main.js:1858-1861,2253,2955 | SMOKE+ |

## 21. Updates (UPD)

| ID | What | Source | Verify |
|---|---|---|---|
| UPD-01 | Feed: newest published (non-draft, non-prerelease) GitHub release of aevalmere/aevalbuddy, read without the rate-limited API (atom feed + `releases/download/v<ver>/latest.yml`). | updater.js:1-35; electron-builder.yml publish | SMOKE:updateFeed |
| UPD-02 | Schedule: first check 20 s after launch (2 s with a test feed), every 4 h, 15 s after resume if stale (> 30 min) or failing, on network return; never while an answer streams (defer 60 s). No background check under smoke/shots. | updater.js:41-50,263-300,456-489 | UPDATE |
| UPD-03 | States: dev, idle, checking, current, downloading, ready, offline, retrying; state object fields as listed. | updater.js:66-83 | UNIT |
| UPD-04 | Backoff 2, 5, 15, 30, 60, then 120 min for every later failure (it stays at 120 min plus jitter), +0..10 % jitter. | updater.js:50,254-259 | UNIT |
| UPD-05 | classify: offline (net/DNS/connection error family), limited (429/rate limit), corrupt (checksum), server (else). | updater.js:227-240 | SMOKE:updateClassify |
| UPD-06 | Notes snippet: up to 3 `<li>` items joined ` · `, else first paragraph; entities decoded; 180 chars max cut at a word with ellipsis. | updater.js:190-218 | SMOKE:updateNotes |
| UPD-07 | Download in background; sha512 (base64) verified against latest.yml; re-hash logged with match=; full installer, never web stub. | updater.js:326-411 | UPDATE |
| UPD-08 | Ready: tray item + Settings; one silent toast `AevalBuddy X is ready` / `It installs the next time you quit. Click to restart now.` (click restarts), held until no answer streams. | updater.js:413-428,554-564 | UPDATE |
| UPD-09 | Restart: if busy, `restartPending` and restart when idle; else quit and run the installer silently and relaunch. Install on quit if never restarted. | updater.js:536-551 | UPDATE |
| UPD-10 | Problem texts: `You're offline. AevalBuddy checks again once you're connected.`, `GitHub asked for a pause. Trying again at H:MM.`, `The download did not verify, so it was thrown away. ...`, `The update server did not answer. ...`; never shows error/exception/stack/http. | updater.js:246-252 | SMOKE:updateQuiet |
| UPD-11 | After an update: `AevalBuddy updated to X` / `Click to see what's new.` (opens release page) once, 3 s after launch; `updatedFrom` and releaseUrl in state for the session. | updater.js:433-447,464-469 | UPDATE |
| UPD-12 | Test feed override `AEVAL_UPDATE_FEED`: honoured in a packaged build only under AEVAL_SMOKE or AEVAL_UPDATE_TEST=1; https, or http to loopback; no credentials. `AEVAL_UPDATE_QUIT=1` quits once the verified download is ready. `AEVAL_UPDATE_EXPECT` names the expected smoke result. | updater.js:151-170,378-383; main.js:3312 | SMOKE:feedOverrideStrict, UPDATE |
| UPD-13 | Smoke retries transient feed failures (5xx, timeouts) after 10 s and 30 s; 45 s per attempt. | main.js:3287-3311 | SMOKE:update.attempts |
| UPD-14 | Unpackaged (dev) build: every call is a no-op reporting `dev`. | updater.js:456-461 | UNIT |
| UPD-15 | Transition: installed Electron copies (electron-updater reading latest.yml) must update onto the Rust build, keeping %APPDATA%\aevalbuddy, the Start Menu entry, the Run key and the user's shortcuts. | new | UPDATE (old Electron release -> Rust A -> Rust B) |

## 22. Notifications, startup, shell (SYS)

| ID | What | Source | Verify |
|---|---|---|---|
| SYS-01 | Presence toast once per fresh install, or whenever the main chord is taken; click opens Settings > Shortcuts (taken) or the bar (fresh). Never under smoke. | main.js:2353-2368 | SMOKE+ (toast as data) |
| SYS-02 | decidePresence: the first boot with no key and no tour shown turns on launch at login and queues the notice; upgrades keep their setting. | store.js:358-366 | MIGRATE |
| SYS-03 | Launch at login: HKCU Run entry pointing at the installed exe with no args, written at boot and when toggled; only when packaged (`launchAtLoginLive`). Same value name as Electron (`com.aevalbuddy.app`, Electron's default `name` is the AppUserModelID) so upgrades replace rather than duplicate; StartupApproved respected at boot; turning the setting on in Settings marks it enabled (D-11).

Add (note to the row):
The Settings switch writes `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run` value `com.aevalbuddy.app` = REG_BINARY `02` and eleven zero bytes (Task Manager's "Enabled"), as Electron's `setLoginItemSettings` did. Boot (`autostart::sync`) writes only the Run value. D-11: an explicit toggle means the person wants it to run, while a Task Manager disable must survive every restart (Electron rewrote StartupApproved on every launch). | main.js:1946-1958 | RUNNER (reg query after toggle) |
| SYS-04 | Clipboard read/write (key detection, copy buttons, agent tools). | main.js:2567-2683 | UNIT |
| SYS-05 | Open external URLs in the default browser. | main.js:2499-2504 | UNIT |

## 23. Packaging and installer (PKG)

| ID | What | Source | Verify |
|---|---|---|---|
| PKG-01 | One-click NSIS, per-user (no admin prompt), silent with `/S`. | electron-builder.yml nsis | INSTALL:install.exitCode |
| PKG-02 | Release asset name `AevalBuddy-Setup.exe` (no version) so `releases/latest/download/AevalBuddy-Setup.exe` in the README keeps working; `latest.yml` beside it with sha512. | electron-builder.yml artifactName | RUNNER (release.yml) |
| PKG-03 | Add/Remove Programs entry named `AevalBuddy*` with DisplayVersion = package version. | installer-check.ps1 | INSTALL:install.registered, install.version |
| PKG-04 | Installed exe `AevalBuddy.exe` beside the uninstaller; ProductVersion = version (or version.0). | installer-check.ps1 | INSTALL:install.exe, install.exeVersion |
| PKG-05 | Start Menu `AevalBuddy.lnk` and Desktop `AevalBuddy.lnk`. | installer-check.ps1 | INSTALL:install.startMenuShortcut, install.desktopShortcut |
| PKG-06 | A silent install does not launch the app. | installer-check.ps1 | INSTALL:install.notLaunched |
| PKG-07 | Uninstall removes exe, registry entry, both shortcuts; keeps %APPDATA%\aevalbuddy (`deleteAppDataOnUninstall:false`). | installer-check.ps1, electron-builder.yml | INSTALL:uninstall.* |
| PKG-08 | App icon `build/icon.ico` generated from the pointer arrow. | tools/make-icon.js | INSTALL |
| PKG-09 | Stdout of the GUI exe reaches a redirected parent (smoke lines are read from stdout). | installer-check.ps1 Run-App | INSTALL:smoke.printed |
| PKG-10 | Exe under the installer is not code-signed today; nothing in the port may depend on a signature. | release.yml notes | n/a |

## 24. Smoke assertions (SMK)

Every `[SMOKE]` label below must still be printed by the Rust build, with the same pass condition. `must`
labels fail the run (`=FAIL` -> exit 1); `log` labels are observations read by people and by DEFECTS.md
history. Labels in **bold** are `must`.

main.js runSmoke (3213-3952): **windows**, version, shortcut.captureQuery, shortcut.captureRegion,
**tray**, **updateConfig**, **updateClassify**, **updateNotes**, update.retryN, update.attempts,
update.status, update.version, update.problem, **updateFeed**, **updateQuiet**, update.cause,
shortcut.toggleVisibility, model, modelList, modelListPaidHidden, **modelDefaultOffered**,
modelListWithPaid, **brainListDefault**, healBrain, healModel, **healRetired**, brainPrompt,
brainDefault, brainEnabledDefault, skills, skillInPrompt, promptShape, gridShared, skillFallback,
skillDefault, skillRoute, skillRouteFromScreen, widthDefaults, pillWidthHeld, pillPinned,
panelWidthRange, panelHeightPinned, panelWidthHeld, widthAfterCycle, outputAutoHeight, motionAnchor,
capture, markdown, transcriptScroll, transcriptWheel, chatCeiling, scrollThumb,
answerHeightUnderLongChat, answerFollowsTranscript, moreIndicator, pointExtract, pointShapes,
pointNonDirective, mode, hasKey, taskbarHidden, overlayOnTop, outputOnTop, settingsWindow,
captureProtection, readerElements, pointByName, readerRowShapes, readerPixelRow, pointAmbiguous,
pointNamedExtract, pointResolved, pointGridMapping, pointPixelRescue, pointBottomEdge, pointOffScreen,
regionCapture, regionSharper, regionPointTarget, regionSelectorCancels, regionEscape, regionPrompt,
failoverRateLimit, failoverKeepsAuthError, failoverCapped, historySaved, historyTitle, historySearch,
historyRemove, historyRespectsSwitch, historyCleared, historyRetryGuard, queryWithoutKey, hudMode,
hudDockedDefault, hudDockToggle, hudSnapFound, hudExitMode, storedPos, storedOutputPos, v2desktop
(skip note), buddyPoints, buddyPointsDone, walkthrough, **crashLogged**, **v2basics**, **v21basics**,
**glassGates**, **v2desktop** (as failure label), `[SMOKE] done` / `[SMOKE] failed: ...` / `[SMOKE] error`.

Expected values that people compare by eye (keep them identical): `pointGridMapping={"fx":0.5,"fy":0.5}`,
`pointPixelRescue={"fx":0.9375,"fy":0.9722...}`, `pointOffScreen=null`, `skillRoute=debug,explain,rewrite,screen-assist`,
`skillFallback=screen-assist`, `promptShape=nosize:lines`, `pointShapes=6/6`, `readerRowShapes=6/6`,
`pointNonDirective=kept`, `regionEscape=cancelled:panel-kept`, `failoverKeepsAuthError=threw:401`,
`historySaved=2:3`, `historyRemove=1`, `historyRespectsSwitch=1:skipped`, `historyCleared=0`,
`queryWithoutKey=needs-key:chat0`, `hudDockToggle=true:true`, `hudSnapFound=left-top`,
`transcriptWheel=moved:N`, `answerFollowsTranscript=exact`, `markdown=ok`.

The markdown, transcript, scroll and wheel probes run JavaScript inside the answer page and read a value
back (`executeJavaScript`), and `transcriptWheel` injects a real wheel event (`sendInputEvent`). The Rust
smoke needs an eval-with-result bridge and real input injection (ARCHITECTURE.md 7).

lib/desktop/selfcheck.js: desktopError, **desktopReady**, desktopReadyMs, desktopDpi, **desktopWindows**,
desktopWindowCount, **desktopOwnExcluded**, **ownWindowBlocked**, desktopNotepad, **desktopNotepadIncluded**,
notepadWindow, focusMethod, uiaTreeNodes, uiaTreeMs, uiaTreeRoles, editor, ocrMs, **ocrWords**, ocrDetail,
typeFocus, typeReadText, **uiaTypeRead**, uiaSetValueMs, uiaSetValueVia, **uiaSetValue**, getTextSpans,
**findEditor**, findTop, elementAt, **notepadClosed**, tempFileLeft, closeError, dontSave.

lib/smoke-v2.js: **helperNotAtBoot**, bootMs (+ **bootMs** < 15000), shortcuts, barKeys,
**onboardingSteps**, windowsChecked, plus `contentProtection` (every window protected when canHide) and the
tray order check.

lib/smoke-v21.js: gateBypasses (+ **gateBypasses**), **gatePassesPlainInput**, **gateFailsClosed**,
**gatePlanFirst**, **gateChords**, intentTable (+ **intentTable**, >= 40 rows, 0 wrong), **intentPrefixes**,
intentWorstMs, **intentFast** (< 5 ms), **intentDoNeedsMargin**, keySwallow, **noGlobalEnter**,
**noGlobalEscWithBar**, **barOpensUnlocked**, **feedOverrideStrict**.

lib/smoke-glass.js: glassTier, **glassLoaded**, **noSetOpacity**, snapCost (+ **snapCost** p95 < 25 ms),
glassCost, styleSamples, **styleStable**, collapseClickable (+ **collapseClickable**), **topmostHolds**
(>= 15 late samples, never buried).

lib/smoke-v2-desktop.js (CI only): **v2NotepadOpen**, lensFirstDraw (+ **lensFirstDraw** = 3:true:true),
lensWhyClear (+ **lensWhyClear**), findLiteral, **lensHits**, **findStep**, **findRelated**, findRelatedWhy,
**findConcept**, findConceptHit, **findOffline**, **askHasUiaText**, **pointFromUia** (<= 4 px off),
**agentFocusKept**, **agentJsonMode**, **agentTakeover**, **agentMaxSteps**, v2NotepadClosed,
**agentConfirmGate**, **passwordRedacted**.

The intent table (smoke-v21) runs `renderer/intent.js`, which stays JS; the Rust smoke evaluates it in a
webview (or under node in CI) and prints the same lines.

## 25. Visual self-check (SHOT)

| ID | What | Verify |
|---|---|---|
| SHOT-01 | lib/shots.js frames: 01-pill, 02-panel-nokey, 03-panel, 04-answer, 05-answer-thinking, 06-hud-input, 07-hud-output, 08-settings-top, 09-settings-bottom, 10-settings-models, 11-buddy, 12-live-overlay (content protection off for one frame), 13-panel-region, 14-answer-error, 15-answer-stop, 16-history, 17-region, plus `[SHOT] fit.<name>=window<h>:surface<h>` probes. | INSTALL:shots.written, by-eye diff on the shots branch |
| SHOT-02 | lib/shots-v2.js: 18-panel-find, 19-find-results, 20-lens, 21-panel-agent, 22-agent-running, 23-agent-confirm, 24-agent-done, 25-agent-error, 26-find-empty, 27-find-offline, 29-settings-agent, 30-settings-rail. | same |
| SHOT-03 | lib/shots-matrix.js: pill, bar, answer, thinking, error, find, lens, agent-running, agent-confirm, agent-done, settings, history, tour, each over the white/black/wallpaper fixtures, at 100/125/150 % (`AEVAL_SHOT_MATRIX_ONLY` for the scaled runs), every frame through the contrast gate; `[SHOT] matrix scale=.. contrastMin=.. failedFrames=0`. | INSTALL:shots125.*, shots150.* |
| SHOT-04 | Shots exit 1 when any frame fails contrast. | INSTALL:shots.exitCode |
| SHOT-05 | Per-window page photographs (Electron `capturePage`) need a WebView2 equivalent (`ICoreWebView2::CapturePreview`). | RUNNER |
| SHOT-06 | Screenshots published to the `shots` branch, newest 15 commits. | installer.yml |

---

## 26. Required changes from REQUESTS.md items 7-11 (NEW)

Andy's requests in `.claude/rust-rewrite/REQUESTS.md` win over current behaviour. Where an item below
contradicts an earlier item, the earlier item is **superseded on purpose**: that is an intentional change,
not a regression, and the superseded item is checked off by verifying its replacement. Each change still
gets a DESIGN.md entry (agent d), as the DESIGN.md rule requires, citing REQUESTS.md.

| ID | What | Request | Verify |
|---|---|---|---|
| NEW-01 | Output windows are free-floating: each answer or run trail opens in its own window, not docked to the bar and not placed by a layout setting. A new one opens beside the bar on its free side. | 11 | SMOKE+:outputFloating, SHOT |
| NEW-02 | Several output windows can be open at once; each keeps its own conversation (or run trail), position and size. A new conversation opens a new window; a follow-up typed in the bar goes to the most recently focused output window. Labels `output-<n>`. | 11 | SMOKE+:outputMulti (open 3, all visible, distinct labels) |
| NEW-03 | Every output window has a title bar it is dragged by (`data-tauri-drag-region` / native caption drag), carrying its controls (copy, history, new, close). | 11 | SMOKE+:outputDrag (SendInput drag on the title bar moves the window) |
| NEW-04 | No separate outputs layout: the HUD layout editor no longer places an answer ghost and there is no dock/undock. Positioning the bar stays. | 11 | SMOKE+:noOutputLayout |
| NEW-05 | Output windows "glob out" of the chat bar: on open, a liquid-glass morph grows the window's glass out of the bar's glass (shared rim, stretch and settle on the DESIGN.md glide curve; reduced motion gets a fade). Closing reverses toward the bar when it is open, else fades. Still no setOpacity (WIN-15 holds). | 8 | SHOT (frame sequence), SMOKE:noSetOpacity, SMOKE+:globMorphMs |
| NEW-06 | No follow-up input in any output window. The bar is the only text input. Agent `ask_user` answers and plan approval are typed or confirmed in the bar (buttons in the trail stay), routed to `agent:continue` for the run in the focused output window. | 9 | SMOKE+:noOutputInput (no input/textarea in output DOM), SMOKE:agentTakeover |
| NEW-07 | Edge and corner resize works on every resizable window (bar width, all 8 edges/corners of output windows, Settings, History): resize zones on the transparent stage margin answer WM_NCHITTEST with HTLEFT..HTBOTTOMRIGHT, so the margin's click-through never swallows a resize grab. Min/max limits respected. | 10 | SMOKE+:resizeEdges (SendInput drag on each edge/corner changes the right dimension) |
| NEW-08 | Output windows and the closing bar share one formatting system: title bar height, padding, type scale, markdown and code styles, meta line, buttons, and the empty/thinking/error states. The bar's closing (collapse) state is styled and animated as deliberately as its opening. | 7 | SHOT matrix + contrast gate; review loops (request 12) |
| NEW-09 | Whole-app consistency: every surface uses the DESIGN.md tokens, no one-off styles. | 7 | SHOT, reviews |
| NEW-10 | Suggestions under the bar show only until the first request is sent (question, Find with Enter, or task), then never again, across restarts (`hasSentFirstRequest`, ARCHITECTURE.md 10.2). | 3 | RUNNER:ui-shots suggestionsBeforeFirst, suggestionsGoneAfterFirst, suggestionsNeverAgain; SMOKE+:suggestionsOnce |
| NEW-11 | Full-screen, desktop-oriented layout: Settings and History maximise from their title bar and keep a reading measure when wide; output windows keep a 760 px measure. | 4 | SHOT:07-settings-fullscreen |
| NEW-12 | Liquid glass independent of Windows' transparency setting, with a Settings slider None..Very very (`glassIntensity` 0..1) driving blur, refraction, specular and saturation live in every window; None is solid frosted with full contrast. Supersedes GLASS-02's `transparency-off` reason. | 5 | RUNNER:ui-shots glassLive.*, glassTier.*, glassSaved; SHOT:06-settings-glass-000/050/100 |
| NEW-13 | The Ask/Find/Do shortcut hint lives only on the bar's left button (its tooltip and menu); no legend by the input, no row in Settings, no hint in an output window. | 6 | RUNNER:ui-shots noLegend, hintOnLeftButton, noDuplicateHintInSettings |

### Superseded by section 26 (intentional, not regressions)

| Earlier item | Now |
|---|---|
| WIN-05 (one output window) | NEW-01, NEW-02 |
| POS-04, POS-12 (docked output follows the bar) | NEW-01 |
| POS-05, POS-10, POS-11, POS-20 | still hold, per output window (auto height until the user resizes it; the last size is used for the next new window) |
| POS-14 (HUD shows an output ghost), POS-16, POS-17 (dock toggle) | NEW-04; the HUD edits the bar only |
| POS-23 (answer dodges the agent) | still holds, for the run's output window |
| IPC-41 `hud:set-dock`, EVT-03 `hud:docked` | stay in both allowlists as no-ops until agent d removes their callers, then leave preload-parity tests and lists in one commit |
| STORE-13 `outputPos.docked` | read for migration only; floating windows ignore it; the key is preserved on write for rollback |
| SMOKE hudDockedDefault, hudDockToggle, storedOutputPos | replaced by outputFloating, outputMulti, noOutputLayout |
| POS-13 `overlay:state.outputPos` | field kept for compatibility; no longer drives layout |

Requests 2-6 and 12 in REQUESTS.md (more providers, one-time suggestions, full-screen layout, a glass
slider independent of the Windows transparency setting, de-duplicated hints, polish loops) are also
required and get NEW items when assigned. Request 5 already changes GLASS-02: Windows transparency off
will no longer force the solid tier.

Item count: 457 numbered items in sections 1-23, 25 and 26 (9 NEW, superseding 11 earlier items), plus
about 196 distinct smoke labels in section 24, about 653 checks in all. Any item added later takes the next free number in its section; numbers are
never reused.

## 27. Rust port: status, evidence and notes

Folded from the lanes' entries files at the final merges. Each line names the backlog item (B-xxx) or decision (D-xx) and what it changed or proved for that row; the row above stays the Electron behaviour unless a replacement was folded into it.

### LIFE-01

- B-012: The second copy calls AllowSetForegroundWindow(ASFW_ANY) before it writes `open`, and keeps trying for 15 s. The first copy serves the pipe from before the app is built (an `open` during boot is answered when setup is done), reads one line with a 2 s limit on its own task (a client that says nothing holds up nobody), and retries a failed pipe creation, 100 ms doubling to 5 s.
- B-012: status: UNIT instance::tests::stalled_client_does_not_block_server, instance::tests::an_open_during_boot_is_answered_once_ready; SMOKE+ singleInstance pending the platform probe (K-30)

### LIFE-03

- B-015: `run()` appends `--force-device-scale-factor=<AEVAL_SHOT_SCALE>` to WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS (keeping any value already there) before any webview exists. The window-maths half is B-025 (windows).

### LIFE-05

- B-013: Every main-thread entry point the platform owns runs inside `crash::guard` (page load, window events, global shortcuts, tray and context-menu clicks, main-thread tasks, exit), so a panic there is logged and survived. A panic on the main thread outside a guard cannot be survived (it would unwind into a window procedure): the toast says "AevalBuddy hit a problem and has to restart." and is shown before the process ends.
- B-013: status: UNIT crash::tests::guard_catches_and_logs, crash::tests::toast_follows_the_thread; SMOKE crashLogged and SMOKE+ crashMainThread pending the platform probe (K-30)

### LIFE-06

- B-011: After the boot apply, each action whose chord could not be registered is written to stderr (`hotkeys::report_failures`).

### LIFE-10

- B-086: `lib.rs` `stop_for_exit()` stops a running task through the agent service with "Stopped because AevalBuddy quit." on RunEvent::Exit (the port used the reasonless stop hook).
- B-086: status: code done; SMOKE+ quitStopReason pending the platform probe (K-26, K-30)

### LIFE-12

- B-114: the lens, region and buddy builders call `crash::guard_webview`; a reloaded or crashed buddy page lets the mouse through again, and a page that holds the mouse with no word for 3 s while the pointer is off its bubble has click-through restored.
- B-114: status: done; evidence: UNIT `buddy::tests::watchdog_restores_click_through`; SMOKE+ `buddyReload`

### LIFE-16

- B-115: status: done; evidence: UNIT `desktop::checks::tests::notepad_gating` (Notepad only when `GITHUB_ACTIONS` is set or `AEVAL_DESKTOP_NOTEPAD` is set and not "0"; the Rust port had defaulted to on); desktop selfcheck log `desktopNotepadIncluded=ok`

### WIN-14

- B-077: The bridge shows the menu wherever a page did not handle `contextmenu`: `ui:context-menu {kind:'edit'}` on a text field, `{kind:'selection'}` on a non-empty selection; `menu::edit_template` is the template as data (Electron roles). The context menu's pending actions are swapped in after the items are made and taken out with one lock, never held across a main-thread round trip (B-076).
- B-077: status: UNIT menu::tests::edit_and_selection_templates_match_main_js, menu::tests::pending_lock_not_held_while_building; SMOKE+ contextMenuEditable pending the platform probe (K-30)

### POS-09


### POS-24

- B-005: status: done; evidence: UNIT `windows::screen::tests::overlapping_mixed_dpi_layout_is_adjacent`, `windows::screen::tests::rect_in_old_overlap_maps_to_primary`, `capture::screen::tests::layouts_never_overlap_whatever_the_scales`; SMOKE+ `dpiRoundTrip` follows when the harness probe API lands

### KEY-09

- B-011: status: done; evidence: UNIT hotkeys::tests::escape_depth_counter, hotkeys::tests::apply_skips_unchanged_set, hotkeys::tests::a_taken_chord_is_retried_alone, hotkeys::tests::capture_mode_lets_go_and_keeps_results

### KEY-12

- B-177: status: done (Rust half); evidence: UNIT hotkeys::tests::taken_returns_free_alternatives, hotkeys::tests::validation_matches_main_js

### TRAY-01

- B-080: status: UNIT tray::tests::icon_matches_scale, tray::tests::icons_decode

### TRAY-02

- B-080, D-07: status: UNIT tray::tests::double_click_opens_once; SMOKE+ trayClick pending the platform probe

### TRAY-03

- B-080: Wording follows tasks and conversations (C-83, C-84): `Stop the task` (Esc) replaces `Stop agent`, `New conversation` replaces `New chat`, and `Close all answer windows` (C-85, U38) follows it and calls `outputs::close_all` (K-04). The idle order is now `Ask, find, or do…`, `Ask about an area…`, `Find on screen…`, `Do a task…`, `-`, `Hide the overlay`, `New conversation`, `Close all answer windows`, `History…`, `-`, `Settings…`, `Help and feedback…`, check item, `-`, `Quit AevalBuddy`.
- B-080: status: UNIT tray::tests::idle_order, tray::tests::ready_running_hidden; SMOKE trayOrder pending the platform probe

### TRAY-05

- B-080: Each check's 10 s "Up to date" is cleared only by its own timer, so a second check inside the 10 s keeps its full 10 s.
- B-080: status: UNIT tray::tests::second_check_keeps_its_ten_seconds; SMOKE+ trayCheckStates pending the platform probe

### TRAY-07

- B-080: The menu is made once and edited in place on the main thread: only rows whose text or enabled state changed are touched, rows are replaced inside the same menu when the set changes (never `set_menu`, so an open menu is not swapped), download progress redraws at most once a second, and a refresh never blocks its caller.
- B-080: status: UNIT tray::tests::refresh_only_touches_changed_items, tray::tests::progress_is_throttled_but_other_edits_are_not

### IPC-02


### IPC-18


### EVT-03


### EVT-18


### EVT-19


### STORE-01

- B-082: The v1 fields are the OpenRouter key's source of truth in both builds: `get_provider_key("openrouter")` reads `apiKeyEncrypted` / `apiKeyPlain`, and every load rewrites `apiKeys.openrouter` from them when they differ, so a key the Electron build changed or cleared after a rollback stays changed or cleared after the upgrade back.
- B-082: status: UNIT store::tests::v1_fields_win_after_rollback

### STORE-14

- D-16, B-087: status: done; evidence: UNIT store::tests::fresh_file_gets_every_default_in_order, store::tests::stored_input_width_is_kept

### STORE-26

- B-083: An entry is cut at 500 UTF-16 units, as JavaScript's `slice(0, 500)`; a surrogate pair is kept whole or dropped, never split.
- B-083: status: UNIT store::tests::input_history

### STORE-40

- B-004: status: done; evidence: UNIT crypto::tests::unreadable_local_state_is_left_alone, crypto::tests::bom_local_state_keeps_its_key, crypto::tests::local_state_is_created_then_reused

### STORE-44

- B-001, B-002: status: done; evidence: UNIT store::tests::concurrent_sets_never_lose_or_corrupt, store::tests::parse_failure_recovers_from_bak, store::tests::invalid_utf8_is_read_lossy_not_reset, store::tests::unreadable_file_disables_saving, durable::tests::replaces_and_leaves_no_temp

### FILE-03

- B-003, D-12: status: done; evidence: UNIT history::tests::corrupt_file_is_kept_aside_not_overwritten, history::tests::steady_saves_still_write_within_400ms, history::tests::flush_now_and_timer_never_overlap

### FAIL-01

- B-043: status: changed; evidence: UNIT errors::tests::failover_statuses_are_the_everyday_ones, failover::tests::daily_cap_stays_in_account, errors::tests::daily_cap_is_not_transient

### FAIL-05

- B-042: status: changed; evidence: UNIT catalogue::tests::stand_in_is_live_and_sees_images, failover::tests::bad_model_set_is_skipped, client::tests::retry_after_and_retired_models_are_read

### FAIL-06

- B-119: status: changed; evidence: UNIT failover::tests::short_names

### PROMPT-04

- D-18: status: changed; evidence: UNIT wire::tests::gemini_body_has_no_temperature

### PROMPT-05

- B-125: status: changed; evidence: UNIT sse::tests::buffer_is_capped, client::tests::idle_stream_times_out_as_408, client::tests::error_in_200_body_is_an_error

### POINT-03

- B-127: status: changed; evidence: UNIT elements::tests::cyrillic_and_cjk_names_match

### ERR-01..ERR-12

- B-119: status: replaced by the copy deck (design-ux U14, C-61, C-70..C-74; Decision D-28): each error carries a code, the provider, the model's display name and the actions that fix it, in `tools/fixtures/llm/errors.json`. evidence: UNIT errors::tests::error_table_is_verbatim, errors::tests::codes_and_actions, errors::tests::gemini_invalid_key_is_bad_key, errors::tests::single_attempt_copy_claims_no_stand_in, host::ask::tests::error_rows_name_the_model_and_the_fix

### CAP-01

- B-032: status: changed (D-09); evidence: UNIT `capture::screen::tests::active_prefers_foreground_monitor`, `capture::screen::tests::latch_finds_its_display_only_while_it_exists`

### CAP-02

- B-032: status: changed (D-09); evidence: UNIT `encode::tests::whole_screen_uses_physical_pixels_within_budget` (replaces `encode::tests::whole_screen_matches_electron_thumbnail_sizes`), SMOKE `capture`, capture check `captureSize`

### CAP-05

- B-032: status: changed (D-09); evidence: UNIT `encode::tests::chroma_is_444`, `encode::tests::ladder_starts_at_88_and_stops_at_62`

### CAP-06

- B-110: status: done; evidence: UNIT `capture::tests::hung_worker_is_replaced` (a capture that takes over 3 s fails its call and a fresh capture thread serves the next one); SMOKE+ `captureFailNull` follows when the harness probe API lands

### CAP-07

- B-005: status: done; evidence: UNIT `capture::screen::tests::matching_picks_most_overlap_by_id_not_order`, `capture::screen::tests::ties_go_to_the_primary_then_the_centre`, `capture::tests::area_on_primary_crops_primary`

### CAP-09

- D-08: status: changed; evidence: UNIT host::ask::tests::stale_pick_shot_is_not_reused

### CAP-11

- B-118: the hint reads "Drag to select an area" with an `Esc` keycap and "Arrows move, Alt+arrows resize"; the page is `role="application"` named "Choose an area of the screen", and a polite live region says "Area W by H. Arrows move it, Alt+arrows resize, Enter asks, Esc cancels." on the first key, then the size as it changes (at most every 500 ms).
- B-118: status: done; evidence: SHOT `region-idle-white`, `region-idle-dark`, `region-dragging`, `region-too-small`; check `regionAnnouncesSize`

### GLASS-01

- B-112: frames are readable only by our own pages: the `aeval-frame` protocol sends `Access-Control-Allow-Origin: http://tauri.localhost` (never `*`) and refuses another `Origin` with 403.
- B-112: status: done; evidence: UNIT `backdrop::tests::frame_protocol_cors_is_app_only` (the 403 path is live once lib.rs passes the whole request, see the lane's Blockers); SMOKE `glassLoaded`
- B-113: a window whose display has no frame this tick keeps its last frame; it is never sent another display's frame at the wrong bounds.
- B-113: status: done; evidence: UNIT `backdrop::tests::missing_display_frame_skips_window`
- B-030: only displays hosting a visible glass window are captured; a hidden lens window does not hold the fast pace, and the lens windows go when Find's hits and the agent frame are both gone. A frame DXGI reports unchanged is not converted or encoded again; frames are shrunk while still BGRA.
- B-030: status: done; evidence: SMOKE `snapCost`, SMOKE+ `backdropIdle`; capture check `tickMs` (< 40 ms per 1080p frame), `tickReusesFrame`

### GLASS-02

- B-035: status: changed (NEW-12); evidence: UNIT `backdrop::tests::solid_profile_at_07_is_live`, `backdrop::tests::transparency_setting_is_not_a_reason`; capture check `glassLive`
- B-111: status: done; evidence: UNIT `backdrop::tests::busy_resets_after_panic`, `backdrop::tests::recovery_rebroadcasts_live` (after three capture failures the glass is retried every 30 s, not at the fast pace, and the pages hear `glass:tier live` when a capture succeeds); SMOKE `glassTier`

### LENS-01

- B-034: the lens and pointer pages invoke `lens:ready` / `buddy:ready` after their listeners are registered; Rust flushes the queued draw or show on that invoke, not on the page-load event.
- B-034: status: done; evidence: SMOKE `lensFirstDraw` (`3:true:true`), SMOKE+ `lensReady`, SMOKE `pointResolved`
- B-117: the current hit's loupe is a thin glass rim (inner 1 px white line, 1.5 px bright accent, 2.5 px dark rim, soft 6/16 drop) at radius 8 with no top highlight; the control frame is DESIGN's 2 px `--accent` with a 1 px dark outer line; the why chip is a capsule on one line and r16 once it wraps, padding 8/12. lens.css uses glass.css tokens only (`--skin` and its own accent values are gone).
- B-117: status: done; evidence: SHOT `lens-hits-white`, `lens-hits-dark-editor`; lens.css in `cssLintEnforce`

### TOUR-01

- B-033: status: changed (D-06); evidence: UNIT `buddy::tests::walkthrough_order_and_no_double_spaces` (replaces `buddy::tests::tour_has_four_steps_with_live_chords`), `buddy::tests::steps_apply_by_whether_a_key_exists`; SMOKE `onboardingSteps` (`0>1>2>3`), `walkthrough`

### TOUR-04

- B-031: the pointer window is kept between sessions (hidden, page reset with `buddy:reset`), so a new answer's points show at once and the agent's first cursor after a tour does not wait for a page.
- B-031: status: done; evidence: SMOKE+ `buddyReuse`; SMOKE `buddyPoints`, `buddyPointsDone`

### HIST-01

- B-083: The 4 MB budget counts UTF-16 units of the serialized JSON, as `JSON.stringify(list).length` did (the port counted UTF-8 bytes and dropped CJK history early). Each entry is measured once per trim, so trimming is linear. Titles cut at 89 UTF-16 units plus `…` without splitting a surrogate pair. `history:list` rows carry only what history.js reads (id, at, title, asked, reply).
- B-083: status: UNIT history::tests::cjk_budget_matches_electron, history::tests::trim_result_serializes_under_4mb, history::tests::emoji_title_cut_on_unit_boundary; SMOKE historyTitle, historySearch pending the platform probe

### HIST-05

- B-003: status: done; evidence: UNIT history::tests::corrupt_file_is_kept_aside_not_overwritten, history::tests::unreadable_file_is_never_written

### HIST-06

- D-12: history writes throttle from the first change (400 ms, never postponed by later changes) on one long-lived writer thread; `flush_now` on quit takes the same write lock, so the two never overlap. Reason: Electron's history.js throttled; the Rust port had become a debounce that a steady stream of saves could postpone forever, with a thread per change.

### UPD-02

- B-017: Every failed check or download is one attempt: it counts toward the backoff and schedules the next timer even when its message matches the last failure's (the port deduped by message and could stop rescheduling, leaving a manual check at `checking`).
- B-017: status: UNIT updater::tests::identical_failures_both_reschedule

### UPD-04

- B-079: status: done; evidence: UNIT updater::tests::backoff_table

### UPD-07

- B-079: A latest.yml without a sha512 is refused before the download (nothing unverified is installed). `in_flight` is cleared by a drop guard, so a check that panics never blocks the next one. An empty AEVAL_OFFLINE means online, as in Electron (LIFE-13, LIFE-17).
- B-079: status: UNIT updater::tests::missing_sha512_refuses, updater::tests::in_flight_clears_on_panic, updater::tests::empty_offline_env_is_online

### UPD-15

- B-016: The installer finds the Electron entry by the first ten characters of DisplayName ("AevalBuddy <version>") and its folder from UninstallString, falling back to InstallLocation; it still requires "Uninstall AevalBuddy.exe" in that folder, runs it silently and waits.

### SYS-03

- B-081, D-11: status: SMOKE+ launchAtLogin pending the platform probe

### PKG-01

- B-085: status: ported (one click via the copied template); evidence: installer job `[INSTALL] oneClick=ok` (harness B-052)

### NEW-05

- B-061: status: done; evidence: SMOKE globMorphMs, ui-shots globWordsNeverOutsideShape, globOpenOnClock

### NEW-06

- B-166: status: done; evidence: SMOKE noOutputInput, ui-shots paidHopAsksFirst
