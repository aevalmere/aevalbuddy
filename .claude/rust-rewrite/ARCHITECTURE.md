# AevalBuddy on Tauri v2: architecture and porting contract

Status: phase 1 (scaffold) done on branch `rust-rewrite`. This file is the contract the porting agents
work from. PARITY.md is the definition of done; this file says how the pieces fit and who owns which file.
Change either only by editing it in the same commit as the code that needed the change.

## 1. Shape

```
renderer pages (unchanged HTML/CSS/JS)          src-tauri (Rust)
  renderer/index.html  overlay (pill + bar)       lib.rs        builder, setup, state wiring
  renderer/output.html answer + trail             ipc.rs        one command, channel router, allowlists
  settings/settings.html                          windows/      window manager, stage margin, motion, HUD
  history/history.html                            store/        electron-store compatible settings + key crypto
  renderer/region.html  area selector             history.rs    history.json
  renderer/lens.html    Find highlights           hotkeys.rs    global shortcuts (Electron accelerator syntax)
  renderer/buddy.html   tour + pointer            tray.rs       tray + menu-as-data
        |                                         updater.rs    latest.yml feed, sha512, NSIS handoff
  window.aeval  <- bridge/aeval-bridge.js         crash.rs      panic hook, errors.log, renderer reload
        |   invoke(channel, payload)              notify.rs     WinRT toasts with click handlers
        |   on(channel, cb)                       llm/          OpenRouter client, catalogue, prompts, points
        v                                         ask.rs        the query flow (handleQuerySubmit)
  Tauri command `aeval_invoke`                    find.rs       Smart Find
  Tauri events (emit_to window label)             agent/        desktop agent loop + safety gate
                                                  desktop/      JSON-lines client for the C# host sidecar
                                                  capture.rs    screen capture (DXGI) + JPEG pipeline
                                                  region.rs     area selector window
                                                  lens.rs       lens windows
                                                  buddy.rs      tour + pointer window
                                                  backdrop.rs   glass frames + tier
                                                  selftest/     smoke, shots, contrast (AEVAL_SMOKE/AEVAL_SHOT)
```

One process. The Electron main process becomes the Rust core; each `lib/*.js` responsibility becomes
one Rust module (section 3). The pages stay as they are and talk to Rust through `window.aeval`, which
keeps exactly the shape preload.js gives them today.

## 2. Decisions

### D1. The C# desktop host stays, as a sidecar. Capture moves to native Rust.

**Decision.** `lib/desktop/host.cs` + `host.ps1` keep running in `powershell.exe` exactly as today
(source sent over stdin, prebuilt DLL loaded only on a SHA-256 match, compile-and-cache fallback). The
Rust `desktop` module is a port of `lib/desktop/index.js` only: the JSON-lines client, timeouts, respawn
rules, own-window registry, and the local fuzzy `find` scoring.

**Why.** host.cs is 2630 lines of UI Automation, Windows.Media.Ocr, SendInput, focus-stealing
workarounds, occlusion checks and DPI handling, each tuned against real apps and proven by
`lib/desktop/selfcheck.js` and `smoke-v2-desktop.js` on the runner. Rewriting it in windows-rs buys
nothing a user can see (the helper is already lazy, off the boot path, and isolated from crashes), and
every behaviour it would have to re-earn is a parity item (DESK-01..12, AGENT-05/07/10/14) where a
regression is a safety regression: the gate reads names, roles, states and password flags through it.
Keeping the process boundary also keeps the crash isolation and the "three deaths in 60 s" rule.

**What changes.** host.ps1/host.cs/prebuilt DLL ship as Tauri resources (plain files beside the exe; no
asar, so PowerShell could read them directly, but the stdin bootstrap is kept byte-identical so the
`desktop-<hash>.dll` cache key is unchanged). `tools/build-desktop.js` still makes the prebuilt DLL in CI.
The own-window list now includes the WebView2 browser/renderer pids (they replace Electron's child pids).

**Later, optional.** Individual ops can move to windows-rs behind the same op names if a measured reason
appears (for example `foregroundWindow` latency). The JSON contract is the seam. Not part of this port.

**Capture is different.** Electron's `desktopCapturer` disappears with Electron, so screen capture must
be native anyway. `capture.rs` uses DXGI Desktop Duplication (windows-rs; no yellow border, per output,
honours WDA_EXCLUDEFROMCAPTURE) with a GDI BitBlt fallback, `fast_image_resize` (Lanczos3) for the
downscale from the display's physical pixels to a 1600 px long edge, and a JPEG encoder at 4:4:4 chroma
with the same quality ladder (88, -8 steps, floor 62, 1.6 MB budget). The whole screen is the active
display (D-09): the foreground window's monitor when that window is not ours, else the display under the
pointer, else the nearest one. `windows::screen::latch_active()` records it when the bar opens;
`windows::screen::latched()` and `capture::screen::latched_display()` read the same latch. The
pipeline numbers are PARITY CAP-01..13. CAP-13 adds a fidelity test against the Electron capture of the
same fixture, because "no blurrier than Chromium's thumbnail scaler" is the bar.

a failed DXGI open keeps that output on GDI for 10 s (cleared when the display layout changes), and
outputs on one adapter share one D3D11 device, keyed by adapter LUID.
evidence: UNIT `dxgi::tests::open_failure_backs_off`; capture check `dxgiOpensPerTick` (≤ 1 over a forced-failure burst)

an HDR output (`IDXGIOutput6` colour space G2084/P2020) is duplicated with `DuplicateOutput1` in
`R16G16B16A16_FLOAT`; each linear scRGB channel is divided by the output's SDR white level
(`DISPLAYCONFIG_SDR_WHITE_LEVEL`, 200 nits when unreadable), clamped and sRGB-encoded through a 64K-entry
lookup table. SDR outputs keep the BGRA path. Reason: an HDR desktop read as 8-bit comes out washed out or
clipped, and the model reads tone as content. Not verified on HDR hardware (lane Blockers).
evidence: UNIT `dxgi::tests::scrgb_to_srgb_at_sdr_white`; capture check `hdr=off` on the runner

### D2. One generic command, a JS bridge that keeps `window.aeval` identical

Every renderer call goes through one Tauri command, `aeval_invoke(channel, payload)`, routed in Rust by
channel name. The alternative (one Tauri command per channel) would force every renderer file to change
its calls and would scatter the allowlist. With one command:

- the renderer files keep `window.aeval.invoke('settings:get')` and `window.aeval.on('overlay:state', cb)`
  unchanged;
- the allowlist is one Rust table, unit-tested against preload.js so the two cannot drift
  (`ipc::tests::invoke_allowlist_matches_preload`, `listen_allowlist_matches_preload`);
- Rust knows which window called (`WebviewWindow::label()`), which replaces `e.sender` /
  `BrowserWindow.fromWebContents` in `glass:passthrough`, `glass:hello` and `ui:context-menu`.

The bridge (`src-tauri/bridge/aeval-bridge.js`) is injected as an initialization script into every
webview before page scripts run. It defines `window.aeval = { invoke, on }`, rejects/throws
`Blocked IPC channel: <x>` for channels outside the same allowlists, and listens per window
(`getCurrentWebviewWindow().listen`) so an event sent to one window never reaches another. `on()` returns
an unsubscribe function synchronously, as preload.js does.

### D3. Events are emitted per window label

Rust sends with `app.emit_to(label, channel, payload)`. Labels: `overlay`, `output-<n>` (one per
floating output window, REQUESTS.md 11; the phase-1 scaffold has a single `output`), `settings`,
`history`, `region`, `buddy`, `lens-<displayId>`. Payloads are the exact JSON objects main.js sends
today (PARITY section 8). Channel names keep their colons; Tauri v2 event names allow `:`.

### D4. Binary payloads (glass frames, loupe) go through a custom protocol, not JSON

`glass:frame.img` and `lens:loupe.img` are JPEG Buffers today. Serialising 2 Hz frames as JSON number
arrays would blow the 25 ms main-thread budget (GLASS-06). Rust keeps the newest frame per display in
memory and serves it at `aeval-frame://localhost/<display>/<seq>.jpg` (registered with
`register_asynchronous_uri_scheme_protocol`); the event carries `{ img: <url>, ... }`. The one renderer
change this needs is in `glass.js`/`lens.js`: accept a URL string where a Buffer arrived (owned by agent
d, recorded in DESIGN.md as an implementation note, not a departure). Frames still never touch disk.

### D5. Settings store is electron-store compatible, read and write

`store/` reads and writes `%APPDATA%\aevalbuddy\aevalbuddy.json` with the same keys, defaults, sanitisers,
chosen-flag rules and the v2 shortcut migration (PARITY STORE-01..44). Unknown keys are preserved. Writes
are atomic (temp + rename). The user-data folder is `%APPDATA%\aevalbuddy` (or `AEVAL_USERDATA`), never
Tauri's `app_data_dir()` (which would be `%APPDATA%\com.aevalbuddy.app`).

API key: Electron `safeStorage` on Windows is Chromium OSCrypt. `store/crypto.rs` reads
`os_crypt.encrypted_key` from `%APPDATA%\aevalbuddy\Local State`, strips the `DPAPI` prefix,
`CryptUnprotectData` gives the AES-256 key, and blobs are `v10 | nonce(12) | ciphertext+tag(16)`
AES-256-GCM. Rust encrypts the same way, so a rollback to the Electron build still reads the key. If
`Local State` is missing (fresh install), Rust creates it with a new DPAPI-wrapped key in the same format.
WebView2's own user data goes to `%LOCALAPPDATA%\aevalbuddy\WebView2` so nothing rewrites `Local State`.

### D6. Updater: our own, on the existing GitHub feed

`tauri-plugin-updater` needs its own `latest.json`, a minisign key pair and a different install flow;
it would change the feed, the release workflow, the update-check test and the transition path at once.
Instead `updater.rs` ports `lib/updater.js` and speaks electron-builder's feed: the releases atom feed for
the version and notes, `latest.yml` for the file name and sha512, a background download into
`%LOCALAPPDATA%\aevalbuddy\pending\`, sha512 check, and on restart-or-quit the NSIS installer run with
`/S` then the app relaunched (`--updated` handling in our NSIS hook). Every schedule, backoff, sentence
and log line in PARITY section 21 is kept, and `.github/workflows/update-check.yml` keeps working with
`AEVAL_UPDATE_FEED`.

Transition (UPD-15): the release workflow keeps publishing `AevalBuddy-Setup.exe` + `latest.yml`, so
installed Electron copies (electron-updater) download the Tauri NSIS installer and run it silently. The
Tauri NSIS installer gets a hook (`installer-hooks.nsh`, NSIS_HOOK_PREINSTALL) that finds the
electron-builder uninstall key, runs the old uninstaller silently with AppData kept, and leaves
`%APPDATA%\aevalbuddy` untouched. Proven by the update-check workflow starting from the last Electron
release.

### D7. Global shortcuts keep Electron's accelerator syntax

Stored chords are Electron strings (`Ctrl+Alt+Space`, `CommandOrControl+Shift+A`, `Super+...`). `hotkeys.rs`
parses that syntax itself into RegisterHotKey modifiers and virtual keys (through
`tauri-plugin-global-shortcut`'s `Shortcut::new(mods, code)`), never by string round-trip, and never
rewrites what is stored. Probe-by-registering (KEY-12) and the conditional global Escape (KEY-07/09) are
kept. Enter is never registered.

### D8. Single instance is per user-data folder

`tauri-plugin-single-instance` keys on the app identifier, which would make every `AEVAL_USERDATA` run
collide with a real session (the reason that variable exists). `lib.rs` takes a named mutex
`Local\aevalbuddy-<sha256(userData)[..16]>` and forwards a second launch over a named pipe of the same
name, which opens the bar (LIFE-01).

### D9. Notifications through WinRT directly

Toasts need click handlers (presence toast opens Settings or the bar, the update toast restarts). The
notification plugin has no click callback on Windows, so `notify.rs` uses `tauri-winrt-notification`
with the app's AUMID `com.aevalbuddy.app` (LIFE-04) and an `on_activated` callback.

### D10. Window flags the Tauri builder does not cover are set with windows-rs

`windows/flags.rs` applies after creation: `WS_EX_TOOLWINDOW` (off taskbar and Alt-Tab), `WS_EX_NOACTIVATE`
for never-focusable windows (pill while idle, lens, buddy), `SetWindowPos(HWND_TOPMOST)` re-assertion,
`SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` re-applied on every show (Tauri's `content_protected`
covers creation; verify it uses EXCLUDEFROMCAPTURE, not WDA_MONITOR, and set it ourselves if not),
show-without-activate (`SW_SHOWNOACTIVATE`), and click-through with forwarding (Tauri's
`set_ignore_cursor_events` has no `forward`; the renderer's hover logic needs mouse-move while
click-through, so `flags.rs` keeps a low-cost cursor poll at 60 Hz while any window is click-through and
emits the synthetic `mousemove` the page used to get). Never `SetLayeredWindowAttributes` / opacity
(WIN-15).

### D11. Webview APIs Electron had and Tauri lacks

| Electron | Rust replacement | Owner |
|---|---|---|
| `webContents.executeJavaScript` with a result (smoke, shots, contrast, reduced-motion read) | `selftest/eval.rs`: `webview.eval()` wraps the code so the page calls `aeval_eval_result(id, value)` back; awaited with a timeout | e |
| `webContents.capturePage` (shots, contrast gate) | WebView2 `ICoreWebView2::CapturePreview` through `with_webview` | e |
| `sendInputEvent` mouseWheel (transcriptWheel) | `SendInput` wheel at the window centre while the window is under the pointer | e |
| `webContents.isOffscreen()` fixture page | a hidden webview rendering `lib/backdrop-fixture.html`, captured with CapturePreview | c |
| `matchMedia('(prefers-reduced-motion)')` read at boot | same, through eval-with-result, plus `SPI_GETCLIENTAREAANIMATION` | a |
| `-webkit-app-region: drag` (Settings, History, HUD) | `data-tauri-drag-region` on the same elements | d |
| `render-process-gone` | WebView2 `ProcessFailed` -> reload | a |
| `systemPreferences.getAnimationSettings` | `SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION)` | a |
| `nativeTheme.shouldUseHighContrastColors` / `updated` | `SPI_GETHIGHCONTRAST` + `WM_SETTINGCHANGE` | c |
| `screen.*` DIP/physical conversions | `capture/screen.rs` enumerates (`EnumDisplayMonitors` + `GetDpiForMonitor`) and builds Chromium's DIP
layout once (`dip_origins`): the primary keeps (0, 0); every other display is placed edge to edge with the
display it touches physically, walking out from the primary (nearest first, then device name), so no two
DIP rects overlap and a DIP rect names one display. The offset along the shared edge is Chromium's
ScaleOffset: measured on the parent's scale when the display starts after the parent's start, on its own
scale when it starts before. `windows/screen.rs` keeps its API over the same layout. Reason: WebView2
pages report `window.screenX` in Chromium's layout, so any other layout puts page and Rust coordinates on
different displays; the old origin/scale rule overlapped at mixed DPI (a 150% display at x=1920 started
at DIP 1280, inside the 100% primary). | a |
| `powerMonitor.on('resume')` | `WM_POWERBROADCAST` / `PBT_APMRESUMEAUTOMATIC` | a |
| `net.isOnline()` | `InternetGetConnectedState` | a |

## 3. Module map (one module per lib/*.js responsibility)

| Electron source | Rust module | Notes |
|---|---|---|
| main.js lifecycle, single instance, boot order | `lib.rs`, `instance.rs` | LIFE-* |
| main.js windows, positioning, expand/collapse, HUD, docking; lib/stage.js; lib/motion.js | `windows/{mod,layout,stage,motion,hud,flags,screen}.rs` | POS-*, WIN-* |
| main.js `registerIpc` | `ipc.rs` (router) | handlers live in the owning module |
| preload.js | `bridge/aeval-bridge.js` + `ipc.rs` allowlists | IPC-*, EVT-* |
| lib/store.js | `store/{mod,keys,migrate,crypto}.rs` | STORE-* |
| lib/history.js | `history.rs` | HIST-* |
| lib/tray.js | `tray.rs` | TRAY-* |
| main.js `applyRegistrations`, shortcuts:* | `hotkeys.rs` | KEY-* |
| lib/updater.js | `updater.rs` | UPD-* |
| lib/crash.js | `crash.rs` | LIFE-05, WIN-16 |
| Notification use in main/updater/crash | `notify.rs` | SYS-01, UPD-08/11 |
| main.js applyLaunchAtLogin | `autostart.rs` | SYS-03 |
| lib/openrouter.js | `llm/{client,sse,catalogue,prompt,points,elements,errors}.rs` | MODEL-*, FAIL-*, PROMPT-*, POINT-*, ERR-* |
| lib/fake-llm.js | `llm/fake.rs` | LIFE-15 |
| lib/skills.js | `skills.rs` | FILE-11 |
| main.js handleQuerySubmit, withFailover, liveModels, showModelPoints, readExactText | `ask.rs` | ASK-* |
| lib/find.js | `find.rs` | FIND-* |
| lib/agent.js | `agent/{mod,tools,gate,observe,json_turn}.rs` | AGENT-* |
| lib/desktop/index.js | `desktop/{mod,protocol,scoring}.rs` + resources host.cs/host.ps1 | DESK-* |
| main.js captureScreen, activeDisplay | `capture.rs` | CAP-* |
| lib/region.js | `region.rs` | CAP-10/11 |
| lib/lens.js | `lens.rs` | LENS-* |
| lib/cursor-buddy.js + walkthrough | `buddy.rs` | TOUR-* |
| lib/backdrop.js | `backdrop.rs` (+ `aeval-frame://` protocol) | GLASS-* |
| lib/contrast.js, lib/shots*.js, runSmoke, lib/smoke*.js | `selftest/{mod,smoke,smoke_v2,smoke_v21,smoke_glass,smoke_desktop,shots,contrast,eval}.rs` | SMK, SHOT-* |
| renderer/intent.js | stays JS; `ask.rs` copies only the `VISUAL` regex (unit test compares it to the JS source) | ASK-08 |

Shared state: `state.rs` holds `AppState` (overlay mode, chat, chatId, region, query generation, busy,
bar mode, pill error, output view, pending agent, last exact) behind one `parking_lot::Mutex`, plus an
internal broadcast bus (`tokio::sync::broadcast`) that mirrors main.js's `bus` so smoke can wait on the
same events the windows render. Async runtime: Tauri's tokio. No blocking work on the main thread: the
event loop thread only touches windows.

## 4. IPC contract

### 4.1 Command

```rust
#[tauri::command]
async fn aeval_invoke(window: WebviewWindow, channel: String, payload: Option<Value>)
    -> Result<Value, String>
```

- Not in `INVOKE_CHANNELS` -> `Err("Blocked IPC channel: <channel>")`.
- Each channel maps to one handler `fn(&AppHandle, &WebviewWindow, Value) -> Result<Value, String>`
  (async where it does I/O). The return value is the exact JSON main.js returns today (PARITY section 7),
  `null` where main.js returns undefined.
- A handler panic is caught, logged to errors.log, and rejects the promise; it never takes the app down.

### 4.2 Bridge (renderer side)

```js
window.aeval.invoke(channel, payload)  // Promise, same rejections as preload.js
window.aeval.on(channel, cb)           // returns unsubscribe(), throws on a blocked channel
```

Implemented over `window.__TAURI_INTERNALS__` / `window.__TAURI__` (`app.withGlobalTauri: true`), looked up
lazily at call time so the bridge can be injected before Tauri's own globals.

### 4.3 Binary payloads

See D4. Event payload `img` becomes a URL string under `aeval-frame://`. Nothing else changes shape.

### 4.4 Handler ownership

`ipc.rs` (agent a) owns the table; the handler bodies live in the owning module. The table in phase 1 maps
every channel to `not_ported`, so each agent replaces only the rows for its own channels:

| Channels | Owner |
|---|---|
| overlay:*, output:resize, hud:*, ui:focus-*, settings:* (except key test and models), shortcuts:*, history:*, update:*, app:*, open-external, clipboard:write, bar:history*, tour:replay, settings:set-surface | a |
| query:*, chat:new, models:list, settings:test-key, find:*, agent:*, bar:mode, ui:context-menu | b |
| region:*, buddy:*, glass:* | c |

Agent d owns no Rust channels; agent e owns none but adds `aeval_eval_result` (a second command, not
exposed through `window.aeval`, used only by `selftest/eval.rs`).

## 5. Crates

| Need | Crate | Why |
|---|---|---|
| App shell, windows, webviews, tray, menus | `tauri` 2 (`tray-icon` feature) | the target |
| Global shortcuts | `tauri-plugin-global-shortcut` 2 | RegisterHotKey, same as Electron |
| Clipboard | `tauri-plugin-clipboard-manager` 2 | |
| Open URLs | `tauri-plugin-opener` 2 | |
| Win32 / WinRT (flags, DPI, DXGI capture, DPAPI, registry, SendInput for tests) | `windows` (windows-rs) | first-party bindings |
| Toasts with click | `tauri-winrt-notification` | D9 |
| HTTP + SSE | `reqwest` (rustls, stream) + hand-written SSE line parser (port of readSse) | parity of the parser, no EventSource quirks |
| Async | `tokio` (Tauri's runtime) | |
| JSON | `serde`, `serde_json` (`preserve_order` so the settings file keeps key order) | |
| YAML (latest.yml) | `serde_yaml` | |
| Regex | `regex`; `fancy-regex` only where JS uses lookbehind (find.js tokenizer) | |
| Images | `fast_image_resize` (Lanczos3), `jpeg-encoder`, `image` (decode/crop in tests) | CAP-04/05 |
| Crypto | `aes-gcm`, `sha2` (sha512/sha256), `base64` | STORE-40, UPD-07, DESK-12 |
| Locks | `parking_lot` | |
| Errors | `thiserror` for `DesktopError`/`ApiError` with stable codes | |

Each crate is added by the agent whose module first uses it; the phase-1 manifest carries only what the
scaffold compiles.

## 6. Renderer changes (all of them)

The pages stay. The complete list of edits allowed in renderer/, settings/, history/:

1. `data-tauri-drag-region` beside each `-webkit-app-region: drag` element (overlay.css:1039, settings.css:86,
   history.css:84 regions) and the no-drag children excluded.
2. `glass.js` and `lens.js` accept a frame URL string (D4).
3. The REQUESTS.md 7-11 UI changes (PARITY section 26), owned by agent d.
   Agent d also added `renderer/frame.js` (drag regions and resize zones through Tauri's window API),
   `renderer/glob.js` (the glob morph), the Liquid glass slider (REQUESTS.md 5) and one-time suggestions
   (REQUESTS.md 3); the Rust half is section 10.
4. Nothing else. `window.aeval` keeps its shape, so bar.js, output.js, trail.js, overlay.js, intent.js,
   markdown.js, region.js, buddy.js, settings.js, history.js are untouched. If a page relies on an
   Electron-only behaviour found later, the fix goes in the bridge or in Rust first.

`frontendDist` is `src-tauri/ui`, staged by `build.rs` from `renderer/`, `settings/`, `history/` and
`lib/backdrop-fixture.html`, so relative paths like `../renderer/glass.js` resolve exactly as under
file://.

## 7. Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` on every push
  (`.github/workflows/rust.yml`).
- Results without the API: every `rust.yml` and `ui-shots.yml` run on `rust-rewrite` or `rust/**`
  ends, pass, fail or cancel, by committing to the orphan branch `ci-logs` (`tools/ci-publish.mjs`).
  `<ref>/<sha>/<workflow>/` (`<ref>` is the branch with `/` as `-`) holds `summary.md` (run link,
  conclusion, every step's outcome), `errors.txt` (error and warning lines) and one log per gate
  (`fmt.log` and `fmt.patch`, `clippy.log`, `test.log`, `build.log`, `smoke.log`, `capture.log`,
  `desktop.log`; `bridge.log`, `shots.log`). `<ref>/latest/<workflow>/` is the newest run; 30 kept.
- After a push, `node tools/ci-wait.mjs <sha> [rust|ui-shots] [--timeout-min 30]` fetches `ci-logs`
  every 45 s and exits 0 (green), 1 (red; prints errors.txt and the failing gate's log tail) or 2 (no
  verdict: timed out, or cancelled by a newer push to the same ref). Agents never poll the GitHub API
  for run status: unauthenticated, it allows 60 requests an hour for the whole machine.
- `tauri build` produces the NSIS installer as an artifact, and the Smoke step runs the built exe with
  `AEVAL_SMOKE=1` (`src-tauri/src/selftest/`). Each lane proves its behaviour inside the real exe in
  its own probe, `selftest/probes/<lane>.rs` (`pub fn run(cx: &mut Cx)`), through one API documented
  with a worked example at the top of `selftest/mod.rs`: `cx.must(label, ok, why)` prints
  `[SMOKE] label=ok` or `=FAIL:<why>`; `cx.log(label, value)` prints `label=<value>`;
  `cx.eval(window, js)` returns the JSON value of an expression in that window's page (a Promise is
  awaited, 5 s timeout); `cx.input(Input::..)` sends real `SendInput` mouse and keyboard input at
  physical pixels (move, click, double and right click, drag, wheel, key chord, text);
  `cx.wait_for(timeout, cond)`, `cx.block_on(fut)` and `cx.on_main(f)` cover waiting, async code and
  the main thread. Probes run in a fixed lane order, each in its own `catch_unwind`
  (`probe.<lane>=ok|FAIL`), and roll up into Electron's block labels `v2basics`, `v21basics`,
  `glassGates` and `v2desktop` (`selftest/probes/mod.rs` BLOCKS). The run ends `[SMOKE] done` (exit
  0) or `[SMOKE] failed: a, b` (exit 1); a panic outside the probes prints `[SMOKE] error=<msg>`.
  `tools/smoke-gate.mjs` fails the step on any `=FAIL`, a missing `done`, or any label listed in
  `tools/smoke-labels/<lane>.txt` (one file per lane, owned by that lane) that never printed.
- When the port is functional, the existing `installer.yml` / `tools/installer-check.ps1` run unchanged
  against the Tauri installer: same `[SMOKE]` labels, same `[SHOT]` output, same install/uninstall
  checks. A port is not done until that workflow is green with no smoke label missing (agent e adds a
  "labels present" check that diffs the label set against PARITY section 24).
- DIFF tests: the Electron-free JS modules (openrouter parsing, points, elements, find matchers, agent
  gate, intent, updater classify/notes) run under node on recorded inputs; the Rust port must produce
  equal JSON. Harness: `tools/diff/` (agent e), inputs in `tools/fixtures/diff/`.
- MIGRATE: a profile (settings, encrypted key, history, Local State) written by the Electron v1.1.1
  installed exe on the runner is opened by the Rust exe; every key, the key, and history must read back.
- UPDATE: `update-check.yml` ported, starting from the last Electron release.

- ui-shots (`tools/ui-shots/shots.mjs`): each lane owns `tools/ui-shots/lanes/<lane>.mjs`, exporting
  `{ scenes, mocks, checks, cssLintEnforce }` in the shape documented at the top of `shots.mjs`
  (shared fixtures in `tools/ui-shots/kit.mjs`, the lane list in `tools/ui-shots/lanes.mjs`). Every
  scene runs at device scale 1 and 1.5 (`-150` suffix), and once more at `--text-scale: 2` (`-ts200`)
  when flagged; a scene may run on Playwright's virtual clock. Lane `mocks` merge over the common
  answers; `bridge.test.mjs` checks every lane file's shape. Pictures are published per branch to
  `shots:rust-d-ui/<ref with / as ->/latest/` and `/<sha>/`.

## 8. Work split (disjoint file ownership)

Every path is owned by exactly one agent. Touching another agent's file means asking that agent (or the
coordinator) instead.

### (a) Core: lifecycle, windows, tray, hotkeys, store, settings, updater
- `src-tauri/src/lib.rs`, `main.rs`, `state.rs`, `instance.rs`, `ipc.rs`, `paths.rs`
- `src-tauri/src/windows/**`, `store/**`, `history.rs`, `tray.rs`, `hotkeys.rs`, `updater.rs`, `crash.rs`,
  `notify.rs`, `autostart.rs`
- `src-tauri/tauri.conf.json`, `src-tauri/capabilities/**`, `src-tauri/Cargo.toml` (others request deps
  through the coordinator or add them in a clearly separated block in the same PR), `src-tauri/build.rs`,
  `src-tauri/icons/**`, `src-tauri/installer-hooks.nsh`
- PARITY: LIFE, WIN, POS, KEY, TRAY, STORE, FILE, HIST, UPD, SYS, PKG-01..08

- Window model per REQUESTS.md 8-11 (PARITY section 26): output windows are created on demand with
  labels `output-<n>`, any number at once, free-floating (no docking, no output ghost in the HUD
  editor), each owning its conversation or run trail in `state.rs` (`conversations: HashMap<label,
  Chat>`, `focused_output`); the bar routes `query:submit` / `agent:continue` to the focused one. Native
  title-bar drag and WM_NCHITTEST edge/corner resize zones on the stage margin for every resizable
  window (`windows/flags.rs`). The glob-out morph is driven by Rust bounds glides (setBounds only) in
  step with the CSS morph agent d builds; the bar's rect is the morph origin.

### (b) LLM providers, Ask, Find, Agent
- `src-tauri/src/llm/**`, `ask.rs`, `find.rs`, `agent/**`, `skills.rs`
- `tools/fixtures/llm/**` (shared fixtures; additive only)
- PARITY: ASK, MODEL, FAIL, PROMPT, POINT, ERR, FIND, AGENT

### (c) Desktop automation, capture, region, lens, pointer, glass backdrop
- `src-tauri/src/desktop/**`, `capture.rs`, `region.rs`, `lens.rs`, `buddy.rs`, `backdrop.rs`
- `src-tauri/resources/desktop/**` (host.cs, host.ps1 copies or references), `lib/desktop/host.cs` and
  `host.ps1` stay the source of truth and are bundled from there
- PARITY: DESK, CAP, GLASS, LENS, TOUR

### (d) Renderer bridge and UI
- `src-tauri/bridge/**`, `renderer/**`, `settings/**`, `history/**`, `DESIGN.md`
- PARITY: RKEY, EVT (renderer side), section 6 of this file, NEW-03..09 (renderer side)
- REQUESTS.md 7-11: one formatting system for output windows and the closing bar; a title bar on every
  output window; no input in output pages (ask_user / plan answers move to the bar); the glob-out
  liquid-glass morph in CSS; removal of the dock/undock UI and the answer ghost from the layout editor.
  These widen section 6's "allowed renderer edits" list; each lands with its DESIGN.md entry.

### (e) CI, installer, self-test
- `.github/workflows/**`, `tools/**` (except `tools/fixtures/llm`), `src-tauri/src/selftest/**`,
  `tools/diff/**`
- PARITY: SMK (section 24), SHOT, PKG-09/10, the MIGRATE and UPDATE harnesses

Order: (a) and (d) first (windows and bridge are what everything renders through), (b) and (c) in
parallel once `state.rs` and the window labels are stable, (e) continuously; the smoke module grows with
each port.

## 9. Risks

1. **safeStorage format.** STORE-40 assumes Electron 38's safeStorage on Windows is OSCrypt v10 with the
   key in `Local State`. If a profile was written by `DPAPI`-only legacy code the blob has no `v10` prefix;
   handle both. Losing the key means every upgraded user re-enters it: the MIGRATE test is mandatory
   before any release.
2. **Content protection semantics.** Tauri's `content_protected` must map to WDA_EXCLUDEFROMCAPTURE (not
   WDA_MONITOR, which shows a black box in captures and to screen shares). D10 sets it ourselves.
3. **Capture fidelity.** DXGI duplication returns full-resolution BGRA; our downscale replaces Chromium's.
   CAP-13 measures it. Never ship without it green.
4. **Click-through with forwarding.** Electron's `setIgnoreMouseEvents(true, {forward:true})` has no
   Tauri equivalent; D10's poll must not cost measurable CPU (glassCost smoke).
5. **Transition update.** electron-updater runs the new installer with its own flags; the Tauri NSIS
   installer must accept them, remove the electron-builder install, keep AppData, and relaunch. If this
   fails, every installed user is stranded on Electron. UPDATE must pass from the last Electron release.
6. **WebView2 differences.** WebView2 is evergreen Chromium, so CSS (`animation-timeline`,
   `feDisplacementMap`, filters) should match; GPU compositing of transparent windows differs from
   Electron's. The shots matrix and contrast gate at 100/125/150 % are the check.
7. **Toolchain only in CI.** No Rust toolchain on Andy's machine by rule; every compile is a push.
   Keep pushes small and read results through the public API.
8. **Stdout from a GUI-subsystem exe.** Smoke lines are read from redirected stdout; Rust's `println!`
   works when the parent redirects handles (PKG-09). If WebView2 or Tauri reattaches a console, the
   installer-check would lose them; the scaffold smoke checks this from day one.
9. **Startup and memory.** Tauri should beat Electron on both; bootMs < 15 s is the gate, and a new
   SMOKE+ line should log working set for the record.

## 10. Split (d): the renderer's contract with Rust

Written by agent (d) with the renderer changes for REQUESTS.md 3-11. Agent (a) implements the Rust half
against it; until then each call below fails or returns `null` and the page falls back as noted, so nothing
breaks while the ports land.

### 10.1 New channels (both allowlists, preload.js and `ipc.rs`, appended in this order)

| Channel | Kind | Payload | Returns | Rust behaviour | Page fallback |
|---|---|---|---|---|---|
| `output:bar` | invoke | none | `{x, y, width, height}` or `null` | The bar's **glass** rect (not the window, no stage margin) in screen DIPs, or `null` while the bar is collapsed or hidden. Answered for any output window. | The glob grows from the sheet's top edge; closing fades. |
| `output:close` | invoke | none | `null` | Closes the calling output window (label from `WebviewWindow::label()`), drops its conversation from `state.rs`, aborts its query or run as `overlay:collapse` would for that conversation, and makes the most recently focused remaining output the follow-up target. The page has already played the close glob before calling; destroy without any further animation. | `overlay:collapse` (the Electron behaviour). |
| `settings:set-glass` | invoke | `{glassIntensity: number}` | `{glassIntensity, glass: {tier, reasons}}` | Clamp to 0..1 (two decimals), store as `glassIntensity` in aevalbuddy.json (new key, default 0.5; on first read, a profile with `surface: 'solid'` and no `glassIntensity` reads as 0), then broadcast `glass:tier {tier, reasons, intensity}` to every glass page. `surface` stays in the file for rollback. | Pages already share the value through localStorage, so it is live without Rust; it just does not reach the settings file. |

No new listen channels.

The per-lane blocks (BACKLOG "Hotspots") follow the split (d) channels in both allowlists, in the order windows, capture, llm, agent, each between `// ---- lane: <name> ----` and `// ---- end: <name> ----`. `ipc::tests::lane_blocks_match_preload` keeps the two files' markers identical.
Windows lane, invoke: `settings:set-text-scale`, `output:blur`, `output:cycle-target`, `output:close-all`, `output:last-answer`, `output:undo-reset`, `output:glass`, `window:nudge`, `bar:dirty`, `bar:ready`, `bar:prefill-open`, `a11y:announce`. Listen: `output:meta`, `ui:text-scale`, `bar:clear`, `bar:note`, `a11y:say`.

### 10.2 Extended payloads (fields added, none renamed)

- `overlay:state.hasSentFirstRequest: bool` (REQUESTS.md 3). New store key `hasSentFirstRequest`, default
  false, set true the first time any of `query:submit`, `find:step`, `agent:start` arrives with text. The bar
  shows suggestions only while it is false. The renderer also keeps `localStorage['aeval.hasSentFirstRequest']`
  so the chips go at once.
- `glass:tier` and `glass:frame` gain `intensity: number` (the stored `glassIntensity`); `glass:hello` returns
  `{intensity}` (today it returns `null`; glass.js accepts both).
- `overlay:state.agent.state` must include `'asking'` while a run waits on `ask_user`, `'plan'` while it
  waits on plan approval and `'paused'` while paused (it already does in Electron). The bar turns Enter
  into `agent:continue {text}` (asking) or `agent:continue {}` (plan/paused, empty field) for the run in the
  focused output window.
- `settings:get` gains `glassIntensity`.

- `overlay:state.target`: `{label, title, kind}` of the output the bar's next question continues, or null. `kind` is `chat`, `find` or `trail`; `title` is the first question (whitespace collapsed, 90 characters), else `New conversation`, `Find` or `Task`. `overlay:state.visibleOutputs`: how many output windows are open and not hidden with the app (K-05).
- `overlay:state` also carries whatever the `overlay_extra` hook returns, merged over the core fields; llm adds `recipients` (K-07).
- `output:meta {grow, front, target}` to each output when one of its values changes: `grow` is `down`, or `up` when the window sits above the bar, so the sheet grows away from it; `front` is true for the most recently touched open window; `target` for the window the bar continues (K-06).
- `ui:text-scale {scale}` to every window when the Windows text size or the person's factor changes, and to a page when it says `glass:hello`. `scale` = Windows factor (1.0 to 2.25) times the person's (1.0 to 2.0), at most 2.5, three decimals (K-22).
- `settings:get` gains `textScale` (the person's factor), `osTextScale` (the Windows factor) and `overlayPos` (the bar's anchor `{h, v, offX, offY}`) (K-22, K-24).
- `settings:set-text-scale {scale}` -> `{ok, textScale, osTextScale, scale}`; the factor is kept to one decimal between 1.0 and 2.0 (K-22).
- `history:list {query}` -> `{rows, enabled}`; `enabled` is whether history saving is on (K-24).
- `output:close {label?}` closes that window, default the calling output, else the bar's target; with no window at all the bar folds as before. `output:close-all` closes every window with a conversation; the warm spare stays (K-04).
- `output:last-answer {label?}` -> `{text}`: the window's last answer, '' if none. `output:undo-reset {label?}` -> `{ok}`. `output:cycle-target {dir: 1|-1}` -> `{label}`: the new target, or null. `output:blur`: the calling window gives the foreground back. `output:glass {top, height}`: the visible sheet inside the glass, in glass DIPs (K-23).
- `window:nudge {dx, dy, dw, dh}` -> `{ok}`: the calling window moves or resizes by DIPs inside its work area and size limits; false for the bar, which moves with `hud:nudge` (K-23).
- `bar:dirty {dirty}`, `bar:ready` (the bar's field has focus after a hotkey), `bar:prefill-open {text}` (K-23).
- `a11y:announce {text, priority}` -> `{to}`: relayed as `a11y:say {text, priority}` to the focused AevalBuddy window, else the open bar; `priority` is `polite` or `assertive`, text at most 300 characters (K-23).
- `bar:clear {}`: the bar empties its field. `bar:note {text}`: a one-line note under the bar.
- `chat:reset {undoMs}`: how long Undo is offered, 0 when there was nothing to undo.

- `query:error {gen, message, question, code, provider, model, modelName, tone, actions, detail}` (`llm::payloads::QueryError`). `code` is one of `llm::errors::codes`; `tone` is `error` or `note`; `actions` are the buttons in order, `{id, label, provider?, model?, url?, section?}`, with each id's meaning on `payloads::Action`; `detail` is the Copy details text (provider, model id, status, models tried, the provider's message). `retryInMs` joins with B-120.
- `query:done.usage` gains `modelId`, `modelName`, `provider` and `providerName`; `model` stays the short id for older pages.
- `query:stage` names a stand-in by its display name: `That model is busy. Trying <model name>…`, `… Reading with <model name>…`, `… Thinking with <model name>…` (C-21..C-23).

### 10.3 Behaviour Rust must keep for the pages

- **Event before listener.** `window.aeval.invoke` waits for every `on()` registration still in flight
  (bridge). Rust must send a page's first state only after that page's first invoke (`glass:hello` for
  glass pages), never on `PageLoadEvent::Finished` alone.
- **Frames** (D4): `aeval-frame://` responses carry `Access-Control-Allow-Origin: *` and
  `Cache-Control: no-store`. glass.js fetches the URL to sample its pixels for the scrim; without the
  header the fetch fails and the glass stays solid. On Windows the page sees the URL as
  `http://aeval-frame.localhost/...`; both forms are in every page's CSP.
- **Windows transparency off no longer means solid** (REQUESTS.md 5). backdrop.rs keeps capturing and
  sending frames when `EnableTransparency=0`; the reason may still be reported, and the renderer ignores
  `transparency-off` and `reduced-transparency`. High contrast, `capture-visible` and `capture-failed`
  still make the tier solid.
- **Output windows** (`output-<n>`): created with the stage margin, `resizable(true)`, min size 320x160,
  visible, then the page globs itself in on its first `chat:user` / `query:start` / `agent:begin` /
  `view:set find`. Rust places a new one so its glass is 12 px from the bar's glass on the bar's free
  side (that gap is inside both 20 px margins, so the glob can draw across it); later ones cascade
  24 px. The window's height follows `output:resize` until the person resizes it (POS-05/20, per window).
  `chat:new` from an output window resets that window's conversation; it does not open another window.
  Every event for a conversation goes only to its window's label.
- **Drag and resize** go through Tauri's window API from `renderer/frame.js`: `startDragging()` on
  `[data-drag]` regions, `toggleMaximize()` on double-click of `[data-drag="max"]` (Settings, History),
  `startResizeDragging(dir)` on the edge and corner zones. Capabilities grant
  `core:window:allow-start-dragging`, `allow-start-resize-dragging`, `allow-toggle-maximize`,
  `allow-internal-toggle-maximize`. A Rust `WM_NCHITTEST` hit-test (split a) may be added on top; the two
  do not conflict because the zones are page elements that start the move themselves. The bar's zones
  are left and right only (its height is pinned by equal min/max).
- **HUD editor** places the bar only: output windows ignore `hud:*`; `hud:set-dock` and `hud:docked`
  have no caller left in the pages (they stay in the allowlists until the parity tests drop them).
- **Browser behaviour** the bridge takes back (no Rust needed, listed so nobody re-adds it in Rust): the
  WebView2 context menu outside text fields, F5/Ctrl+R reload, Ctrl+F find bar, Ctrl+P print, Ctrl+S/U/J/H/
  O/N/T/W/L/D/E, F3/F7/F11/F12, Alt+Left/Right/Home, Ctrl+wheel zoom and drop-to-navigate.

- **Security surface (D-37).** `capabilities/default.json` grants the pages only event listen/unlisten and the window calls `frame.js` makes; `core:default` and every menu, tray, path, image and app permission are gone. `app.security.csp` repeats the pages' own meta CSP, so the header and the meta tag agree and frames still load over both forms of `aeval-frame`. An internal `aeval-navigation` plugin refuses any navigation away from `tauri.localhost` in every webview. Under a test run (`quiet_run`) one runtime capability adds `core:event:allow-emit`, because `Cx::eval` answers through an event; a shipped run never has it. Reason: a page that is ever made to run hostile content can only listen, move its own window and call the allowlisted channels.
Status (B-078, D-37): UNIT tests::navigation_stays_in_the_app; SMOKE+ navigationBlocked pending the platform probe

### 10.4 Verification

`.github/workflows/ui-shots.yml` runs `tools/ui-shots/bridge.test.mjs` (the bridge under node with a fake
`__TAURI__`) and `tools/ui-shots/shots.mjs`, which loads the real pages in the runner's Edge, one frame per
window on a 1600x1000 fixture desktop, with `tools/ui-shots/mock-bridge.js` for `window.aeval`. It prints
`[UI] <check>=ok|FAIL` and publishes the photographs to `shots/rust-d-ui/latest/`. Once the Rust side of
section 10 is ported, the same checks move into the installed-exe self-test (agent e).

## 11. Split (a): the core API the other splits call

Written by agent (a). Everything below is `pub` in `src-tauri/src` and stable for the merge.

### 11.1 Shared state and hooks (`state.rs`)

- `state::shared(app)` gives `Shared`: `.lock()` for `AppState`, `.bus` for the smoke mirror. Rule: take
  the lock, copy, drop it, then touch windows. Never hold it across a window call or an `.await`.
- Flags other splits set: `busy`, `query_gen`, `capture_ok`, `find_open`, `agent {running, step, state}`,
  `buddy_active`, `region_active`, `pill_error`, `bar_mode`. Call `windows::send_overlay_state(app)` after.
- Hooks the core calls (`Shared::set_hooks(|h| ...)` at setup); unset, the core does what main.js did with
  that module absent: `stop_agent`, `close_find`, `cancel_region`, `cancel_tour`, `start_region`,
  `start_tour`, `abort_query`, `prefetch_bar_context`, `warm_catalogue`, `show_trail`, `tour_notify(event)`,
  `set_bar_mode(mode)`, `key_saved(provider)` (unset: the tour hears `key-saved`; b/c put main.js's
  test-then-advance logic here), `settings_extra() -> Value` (merged over `settings:get`; its `glass`
  field is also what `settings:set-glass` returns and broadcasts as the tier).

`AppState.queries` maps an output label to the generation of the Ask streaming into it; `busy` is "any entry". The Ask calls `query_started(label, gen)` and `query_ended(label, gen)` (a stale end for an older generation leaves a newer Ask alone); a closed window's entry goes with `query_forget`. `Hooks.abort_query_for(label)` stops that window's Ask only; closing or resetting a window calls it only when that window's own Ask is running. Until llm reports per window, the single `busy` flag counts for the target window. `Hooks.overlay_extra` returns extra `overlay:state` fields; it is called before the state lock is taken, so it may read the state itself.

### 11.2 Windows (`windows/`)

- Labels: `overlay`, `output-<n>` (any number), `settings`, `history`. `windows::emit(app, label, channel,
  payload)` sends to one window and mirrors on the bus. `windows::window(app, label)`.
- `windows::create_page(app, label, page, outer_rect_dip, flags::Meta)` builds any further window (lens,
  region, buddy) with the shared treatment: frameless, transparent, topmost, tool window, hidden from
  capture, bridge injected, Win32 subclass for resize and limits. `Meta { margin, resize: None |
  Horizontal | All, min, max }` (glass DIPs; 0 = no max).
- Bar: `open_bar(app, mode, prefill)`, `trigger_capture`, `trigger_or_trail`, `expand_panel`,
  `collapse_panel(app, keep_output)`, `toggle_visibility`, `on_escape`, `set_bar_mode`, `bar_rect(app)`
  (glass DIPs), `enter_hud` / `exit_hud` / `hud_nudge`.
- Frames: `open_settings(app, section)`, `close_settings`, `open_history`, `close_history`,
  `notify_history_changed`, `open_url` (http(s) only, lowers Settings/History first), `open_help`.
- `windows::on_hello(app, label)`: the first state for a page (overlay state, an output's `view:set` +
  `chat:sync` + state, the pending Settings section). **Whoever answers `glass:hello` calls it** (agent c's
  `desktop::channels::route` answers that channel first on its branch).
- Geometry helpers: `screen::{all, primary, work_area, matching, active, dip_to_px, px_to_dip}`,
  `motion::{glass, set_glass, animate, stop, is_animating, reduced}`, `flags::{set_click_through,
  set_focusable, show_inactive, show_focused, hide, pin_on_top, protect}`.

### 11.3 Output windows (`windows/outputs.rs`)

- `outputs::target(app) -> Option<label>`: the window a new question goes to. The focused output if it is
  open, else the warm spare, which is placed (12 px off the bar's glass on its free side, cascading 24 px)
  and shown before this returns, so the page's glob runs on the first `chat:user` / `query:start` /
  `agent:begin` / `view:set`. Agent b calls this on `query:submit`, `agent:start` and Find, and sends
  every event of that conversation to that label only.
- `outputs::with(app, label, |o: &mut Output| ...)`: the window's conversation (`o.chat.messages`,
  `o.chat.region`, `o.chat.last_question`, `o.chat.id`), `o.kind` (Chat / Trail), `o.view`.
- `outputs::show(app, label)`, `outputs::resize(app, label, height)` (`output:resize`, grows away from
  the bar, stops following once the person resizes), `outputs::focused(app)`, `outputs::reset(app, label)`
  (`chat:new` from that window), `outputs::close(app, label)` (`output:close`), `outputs::detach_focus`
  (the bar's `chat:new`: the next question opens a new window), `outputs::open_conversation(app, entry)`
  (history), `outputs::chat_payload(&o)` (`chat:sync`).

`outputs::fresh(app)` is always a new or spare idle window, never the focused chat (Find results and task trails open in it). `outputs::close_all(app)` closes every window with a conversation; each stops only its own Ask.

### 11.4 Store (`store/`)

- `store::get()` after `store::init(user_data)`. electron-store compatible file, safeStorage (OSCrypt v10
  AES-256-GCM, key from `Local State`) compatible keys.
- Provider keys (REQUESTS.md 2): `get/set/clear/has_provider_key(provider)` for `anthropic`, `openai`,
  `gemini`, `openrouter`; `provider_keys_present()`; the legacy OpenRouter key migrates into
  `apiKeys.openrouter` (and `set_api_key` still mirrors into `apiKeyEncrypted` for a rollback).
  `has_api_key()` is true when any provider has a key.
- `get/set_glass_intensity` (0..1, two decimals, default 0.5; a `surface: 'solid'` profile with no value
  reads 0), `get_has_sent_first_request` / `mark_first_request_sent` (set by `ipc.rs` on the first
  `query:submit`, `agent:start` with text, or `find:step`).

### 11.5 Channels in `ipc.rs` that split (a) routes outside its 4.4 rows

`chat:new` (bar: `new_chat`; output: `outputs::reset`), `bar:mode` (through the `set_bar_mode` hook),
`glass:hello` (`on_hello`, returns `{intensity}`), `glass:passthrough` (output windows' margin). When b
and c replace these rows they keep those calls.

## 12. Split (b): seams to the store, the desktop and the windows

Split (b) reaches the rest of the app only through these traits, so its modules build and test without
a window, a store or the desktop helper. Whoever wires `ipc.rs` implements them; none of (b)'s files
import `state.rs`, `store/**` or `desktop/**`.

| Trait | File | Implemented by | What it carries |
|---|---|---|---|
| `LlmSettings` | `llm/settings.rs` | (a), over `store/**` | `api_key(provider)`, `provider_override()`, `model(kind)` (the stored OpenRouter id, default included), `heal_model(kind, id)` (Electron's healModel/healBrain/healAgentModel), `native_model(provider, kind)`, `show_paid()` |
| `DesktopOps` | `agent/seams.rs` | (c), a match over `method` onto `desktop::Desktop` | lib/desktop/index.js as JSON: export name plus its one options object, reply as (c) returns it; errors as `DeskError {code, message}` |
| `Emitter` | `agent/seams.rs` | (a) | `emit(channel, payload)` to the windows that render it |
| `FindHost` | `find.rs` | (a) | Find settings (`related` off/auto/ask, `ocr`), `online()`, `active()`, the Find model id, `emit`, `on_lens`, `notify` |
| `AgentHost` (: `Emitter`) | `agent/run.rs` | (a) | `getAgentSettings()` (`every`, `max_steps`, `screenshots`, `return_cursor`), `active()`, the agent model, `online()`, `point_at`, `frame`, `show_trail`, `dodge`, `open_target`, `own_focused`, the clipboard, `on_finish(RunReport)` (main.js `finishAgentRun`; `trail_text(rows)` is in run.rs), `on_state` |

Wiring onto §11 at the merge (this section was §10 on rust/b-llm) (glue in `ipc.rs`/`state.rs` setup, nothing inside (b)'s files):
`LlmSettings::api_key(p)` is `store::get().get_provider_key(p.id())` (the migrated OpenRouter key
included). `AgentHost::show_trail` calls the `show_trail` hook, which owns the trail and the answer
dodge. `settings_extra()` returns the healed ids (`Llm::resolve_known` per `Kind`) and
`Skills::list()`. `chat:new` and `bar:mode` stay with (a); if (b) routes them, it keeps the
§11.5 calls (`new_chat` / `outputs::reset`, the `set_bar_mode` hook). A run's events go to the
label `outputs::target(app)` gave at `agent:start`.

Providers (REQUESTS 2). `llm/provider.rs` names four: OpenRouter, Anthropic, OpenAI, Gemini. A key is
recognised by prefix (`sk-or-`, `sk-ant-`, `sk-`/`sk-proj-`, `AIza`); `provider_override()` wins when
that provider has a key. Order with no override: OpenRouter (the free models stay the zero-cost
default), then Anthropic, OpenAI, Gemini; no key at all is Electron's needs-key state. Every caller
builds one OpenAI-shaped body; `llm/wire.rs` maps it to the native API (Anthropic Messages, OpenAI
Chat Completions, Gemini generateContent/streamGenerateContent) and maps replies, streams, tool calls
and errors back. Provider-only state a later turn must replay (Anthropic thinking blocks, Gemini
thought signatures) travels on the assistant message under `_native` and is stripped for OpenRouter.
Native models never fail over (failover is for `:free` ids only). `AEVAL_FAKE_LLM` answers every
provider in OpenRouter's dialect from `tools/fixtures/llm`, which are compiled into the binary;
`AEVAL_FAKE_LLM_DIR` adds more at runtime. Settings stores per-provider keys and model picks; the
store keys are (a)'s to name.

**Routed by host::settings.**
`host::settings::route` (K-15) runs first from `host::ask::route`, so each channel below is answered there and the matching core row in `ipc.rs` is never reached. windows deletes those rows under B-107.

- `settings:save-key`
- `settings:clear-key`
- `settings:save-model`
- `settings:save-brain`
- `settings:save-find-model`
- `settings:save-agent-model`
- `settings:clip-key`
- `settings:use-clip-key`
- `settings:test-key` (its arm moved here from `host::ask::route` once windows C1 added `host/settings.rs` to the `every_invoke_channel_has_a_route` scan)
- `settings:set-provider-order` (B-038)
- `settings:set-failover` (B-038)
- `auth:openrouter-connect` (B-039)
- `skills:open-folder` (B-181)

Listen `provider:status {id, status, statusText, busyUntil, notice?}` goes to Settings and the overlay after every key test, save, clear, and any answer that changes a provider's state.

The llm lane's C1 contracts, for the lanes that consume them:

- K-15 `host::settings::route(app, label, channel, args) -> Option<Result<Value, String>>`: the Settings channels about providers, keys and models (list above).
- K-16 `llm::settings::candidates(&dyn LlmSettings) -> Vec<Candidate>`: the providers a request may go to, in the person's order (`providerOrder`, with the legacy `provider` override migrated to the top), skipping providers without a key, keys whose prefix names another provider, and providers in a cooldown (`llm::status`). `Candidate { provider, key }` with `model(Kind)`; `llm::settings::model_for(s, provider, kind)` is the model id a provider receives.
- K-17 `ApiError { retry_after_ms: Option<u64>, attempts: u32, code: Option<&'static str> }`, with the codes in `llm::errors::codes`. Defaults (`None`, 1, `None`) until B-042 and B-120 fill them.
- K-18 `llm::recipients(&dyn LlmSettings) -> Vec<Recipient>`: who receives the screen, first first, as `{id, name, model, modelName, free}`. `settings:get` carries it as `recipients`; the overlay gets it through `Hooks.overlay_extra` once windows C1 has that hook.
- K-19 the renderer payloads as serde types in `llm::payloads`: `QueryError` (`query:error`), `Sent` (`output:sent`), `Usage` (`output:usage`), `ProviderRow` (`settings_extra.providers`), `FailoverPrefs`, `ProviderStatusEvent` (`provider:status`), `ProviderModels`, `SaveKeyReply`, `TestKeyReply`, `ClipKeyReply`, `Action` and `Pin`.
- K-26 `Fake::push_fixture(Value)` puts a fixture ahead of all others; `Fake::clear_pushed()` goes back to the embedded set. Besides the lib/fake-llm.js fields a fixture may carry `provider` (matches only that provider id), `model` (only that model id), `status` (answers with that HTTP status and `{"error":{"message":…}}`), `retryAfter` (the `Retry-After` header, seconds), `stallAfterHeadersMs` (headers, then silence for that long before the body) and `times` (matches that many requests, then drops out). Each logged `FakeRequest` records `provider` and the `url` the real request would have gone to.

Timeouts: the HTTP client connects within 10 s and sends TCP keep-alives every 30 s. A stream that sends no bytes for 45 s (keep-alive comments count) ends with `ApiError::http(408)`, which `with_failover` treats like a busy model; a non-streamed call (the reader, agent turns) is capped at 90 s from request to last byte. `AEVAL_LLM_IDLE_MS` shortens the stall timeout for the self-test.

`Llm` remembers the model ids that answered 404 this session (`failover::BadModels`): `first_model` skips a retired free id as the first attempt and `next_free_model` skips it as a stand-in. Once the catalogue is cached, a stand-in must be listed and able to do the job (images for Vision, tool calls for Agent); a cold cache means curated order. An `ApiError` carries `attempts`, the number of models tried, and its text mentions a backup only when that is above 1.

Wire limits: a 200 reply whose body is an `error` object is that error; an SSE line over 1 MB ends the stream with a 502; the history sent with a question starts at a user turn (`ask::sendable_history`); a skill body is cut at 8 KB and `skills.list()` marks it `truncated`. A model that declines (Anthropic `stop_reason: refusal`, a Gemini safety block) is an error with code `refused`, which never fails over. Provider messages pass through `errors::redact_keys`, so a key a provider quotes back never reaches a page or a log.

Gemini requests carry no `temperature`, like OpenAI and Anthropic requests.
Reason: Google's Gemini 3 guide recommends keeping temperature at its default because lower values can cause looping and worse reasoning, and the 0.2 the answer stream sends was chosen for OpenRouter's free models.

Keys are tested before they are stored: `settings:save-key {provider, key}` answers `{ok, status, message, keyHint, provider, credit}`, or `{ok:false, error, detected}` for a key whose prefix names another provider; a rejected key is not stored, a key that could not be checked is kept as `unchecked`. `settings:test-key {provider}` answers `{ok, status, message, credit, actions}`; for OpenRouter it also sends one `max_tokens: 1` request to the default free vision model, and a data-policy refusal reads `blocked` (C-65) with `Open OpenRouter privacy settings`. `settings:get` carries `providers` (rows in try order), `failover`, `keyHints` and `keyStorage`. Statuses live in `llm::status` (in memory, per key fingerprint) and change on key tests and on answers: 401 rejected, 402 no credit (native), a native 429 busy for Retry-After or 60 s; an answer clears them. `auth:openrouter-connect` runs OAuth PKCE S256 through a loopback listener on 127.0.0.1 (`llm::auth`), 5 min timeout, then stores and tests the key. Checked against openrouter.ai/docs (OAuth PKCE guide) on 2026-11-08: `/auth?callback_url&code_challenge&code_challenge_method=S256`, then `POST /api/v1/auth/keys {code, code_verifier, code_challenge_method}` returning `{key}`.

Native model lists come from the providers (`GET /v1/models` for Anthropic and OpenAI, `GET /v1beta/models` for Gemini), cached 5 minutes and filtered to chat models; the curated table stays the fallback and the source of defaults. A stored native id the cached list lacks goes out as the provider's default for the job, in memory only.

Each output window has its own Ask: a new question cancels only the one running in its window, `query:cancel` stops the caller's window (or the focused output), and `Hooks.abort_query_for` stops one window's Ask. Busy follows `AppState::query_started/ended`.

## 13. The merge: `host/` glues the splits together

Written at the merge of rust/a-core, rust/c-desktop, rust/b-llm and rust/d-ui into `rust-rewrite`.
`src-tauri/src/host/` holds everything that sat between modules in main.js; none of the split-owned
files import it except `ipc.rs` and `lib.rs`.

| File | main.js it ports | Notes |
|---|---|---|
| `host/mod.rs` | the `init({...})` blocks, liveModels, startWalkthrough, rememberChat | `host::setup` builds `Services` (Llm, Skills, Find, Agent, the desktop adapter) and registers every `state::Hooks` entry, `buddy::init` deps, `backdrop::set_hooks` + `set_intensity`, and the `region::on_change` hook. `refresh_flags` copies buddy/region/Find/agent state into `AppState` before the core reads it. |
| `host/settings.rs` | `store.get*` for the model layer | `LlmSettings` over the store; `api_key(p)` is `get_provider_key(p.id())`. `provider` and `nativeModels.<provider>.<kind>` are optional store keys (no UI writes them yet). |
| `host/desktop_ops.rs` | `desktop.*` calls | `DesktopOps` onto (c)'s `Desktop`; `{t, opts}` is spread as index.js did; integral floats become integers. Also `desktop.init` deps (takeover input, Find nudge, synthetic-Escape suspension). |
| `host/ask.rs` | handleQuerySubmit, abortCurrentQuery, prefetchBarContext, freshBarText, readExactText, showModelPoints, resolveUiaPoint, models:list, settings:test-key | A question goes to `outputs::target`; a retry goes back to the window it came from. Native providers skip the catalogue and failover. |
| `host/find.rs` | setBarMode, openFind, closeFind, findEmit, findToLens, findToAsk, find:* | Find's results go to one output window plus the overlay. |
| `host/agent.rs` | startAgentRun (consent card), agentEmit, onAgentState, showTrail, setControlFrame, agentPointAt, dodgeAnswer, openTarget, finishAgentRun, agent:* | A run's events go to the label `outputs::target` gave at start. |
| `host/menu.rs` | contextMenu | Tauri popup menus, ids `ctx:<n>`. |

Routing order in `aeval_invoke`: (c)'s `desktop::channels`, then `host::{ask, find, agent}::route` and
`ui:context-menu`, then the core rows. `ipc::tests::every_invoke_channel_has_a_route` fails if an
allowlisted channel has no row anywhere.

OpenAI: the GPT-6 family takes tool calls only through the Responses API, so `Llm::supports_tools`
answers false for OpenAI and the agent uses the one-JSON-object fallback there (checked 2026-11-07).

Two rules the merge added. `state::Hooks::refresh_flags` copies buddy, region, Find and agent state into
`AppState`; the core calls `state::refresh(app)` at the top of every function that reads those flags
(`hotkeys::apply`, `overlay_state`, `on_escape`, `toggle_visibility`, ...), never with the state lock held.
`backdrop`'s pace hook runs with the backdrop lock released and reads visibility through Win32: a Tauri
window getter off the main thread waits for the main thread, and holding the lock meanwhile deadlocked boot.
