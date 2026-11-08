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
downscale, and a JPEG encoder with the same quality ladder (88, -8 steps, floor 62, 1.6 MB budget). The
pipeline numbers are PARITY CAP-01..13. CAP-13 adds a fidelity test against the Electron capture of the
same fixture, because "no blurrier than Chromium's thumbnail scaler" is the bar.

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
| `screen.*` DIP/physical conversions | `windows/screen.rs` over `EnumDisplayMonitors` + `GetDpiForMonitor`, same rounding as Electron | a |
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
4. Nothing else. `window.aeval` keeps its shape, so bar.js, output.js, trail.js, overlay.js, intent.js,
   markdown.js, region.js, buddy.js, settings.js, history.js are untouched. If a page relies on an
   Electron-only behaviour found later, the fix goes in the bridge or in Rust first.

`frontendDist` is `src-tauri/ui`, staged by `build.rs` from `renderer/`, `settings/`, `history/` and
`lib/backdrop-fixture.html`, so relative paths like `../renderer/glass.js` resolve exactly as under
file://.

## 7. Verification

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` on every push
  (`.github/workflows/rust.yml`).
- `tauri build` produces the NSIS installer as an artifact; a scaffold smoke runs the built exe with
  `AEVAL_SMOKE=1` and requires `[SMOKE] scaffold.bridge=ok` (every page loaded and called through
  `window.aeval`).
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

## 10. Split (b): seams to the store, the desktop and the windows

Split (b) reaches the rest of the app only through these traits, so its modules build and test without
a window, a store or the desktop helper. Whoever wires `ipc.rs` implements them; none of (b)'s files
import `state.rs`, `store/**` or `desktop/**`.

| Trait | File | Implemented by | What it carries |
|---|---|---|---|
| `LlmSettings` | `llm/settings.rs` | (a), over `store/**` | `api_key(provider)`, `provider_override()`, `model(kind)` (the stored OpenRouter id, default included), `heal_model(kind, id)` (Electron's healModel/healBrain/healAgentModel), `native_model(provider, kind)`, `show_paid()` |
| `DesktopOps` | `agent/seams.rs` | (c), a match over `method` onto `desktop::Desktop` | lib/desktop/index.js as JSON: export name plus its one options object, reply as (c) returns it; errors as `DeskError {code, message}` |
| `Emitter` | `agent/seams.rs` | (a) | `emit(channel, payload)` to the windows that render it |
| `FindHost` | `find.rs` | (a) | Find settings (`related` off/auto/ask, `ocr`), `online()`, `active()`, the Find model id, `emit`, `on_lens`, `notify` |

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
