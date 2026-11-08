# Backlog: finishing the Tauri rewrite

This is the single work list for the rest of the Electron to Tauri v2 port on branch `rust-rewrite`. It merges the eight audits in `.claude/rust-rewrite/audit/` into one item per root cause (184 items), resolves where they disagree, and splits the work into nine lanes that own disjoint files so they can run in parallel for hours.

Contract documents, in order of authority: `REQUESTS.md` (all 12 ship), `DESIGN.md` (repo root, the design authority; every departure is recorded there with its reason), `PARITY.md` (469 rows, the no-lost-functionality contract), `ARCHITECTURE.md`. The Electron reference is branch `main`.

Two product constraints outrank everything here: **free models are the default, so 429 and 404 are everyday states**, and **capture fidelity bounds answer quality**. Nothing in this backlog weakens a test or a gate.

Each lane's brief is `.claude/rust-rewrite/lanes/<lane>.md`. It repeats that lane's items in full, so a lane agent needs this file only for the cross-lane tables below.

## How to read an item

`### B-NNN · SEVERITY · lane · Title`, then:

- **Sources:** audit IDs (file and ID, for example `review-core P1-1`, `design-ux U3`, `parity-ipc B5`, `design-visual F13`, copy deck rows `C-nn`). Audit files: `review-core.md`, `review-desktop.md`, `review-llm-agent.md`, `parity-core.md`, `parity-ipc.md`, `parity-features.md`, `design-visual.md`, `design-ux.md`.
- **Files:** what the item touches. Every file is owned by the item's lane unless the line says "(<other> block only)" for the shared allowlists.
- **Fix:** the specification. Where an audit gives exact values (CSS, copy, timings), the fix either quotes them or names the audit section to copy from verbatim.
- **Reconciled:** present when sources disagreed; states the resolution and why.
- **Acceptance:** the named check that proves it. Kinds: `UNIT` (a `cargo test` name in the rust workflow), `SMOKE` (an existing PARITY smoke label that must print `=ok`), `SMOKE+` (a new smoke label this item adds), capture/desktop/installer check lines (rust workflow logs), ui-shots `scene` and `check` names (in the lane's `tools/ui-shots/lanes/<lane>.mjs`).

## Severity

| Code | Meaning |
|---|---|
| P0 | Data loss, a safety bypass, wrong-screen capture, or no runtime proof at all. Do first. |
| P1 | A real defect people will hit, or a missing PARITY behaviour. |
| P2 | A defect with a narrow trigger, robustness, or a test gap. |
| S1 | A design or UX change that decides whether the product feels finished (DESIGN, REQUESTS #7, #12). |
| S2 | A design or UX change that is clearly better and fully specified. |
| S3 | Polish. |

Ranking: P0 > S1 ≈ P1 > S2 ≈ P2 > S3. Inside a lane, work in that order, except that the lane's C1 contracts go first (see "C1 protocol").

## Lanes

| Lane | Branch | Scope | Items | Owned top-level paths |
|---|---|---|---|---|
| harness | `rust/harness` | Smoke runner and probe API, exe shots and contrast matrix, installer job, ui-shots plumbing, CSS lint, MIGRATE, final strict gates, review loops | 9 | `src-tauri/src/selftest*`, `src-tauri/src/backdrop/contrast*`, `.github/workflows/`, `tools/` (except lane files and `tools/fixtures/llm/`), `lib/backdrop-fixture.html` |
| platform | `rust/platform` | Store and history durability, single instance, crash guards, hotkeys, tray, updater, installer, autostart, context menu, CSP and capabilities, Cargo.toml | 25 | `src-tauri/src/{lib,main,paths,instance,crash,hotkeys,tray,notify,autostart,updater,history}.rs`, `src-tauri/src/store/`, `src-tauri/src/host/menu.rs`, `src-tauri/bridge/`, `src-tauri/capabilities/`, `src-tauri/{Cargo.toml,build.rs,tauri.conf.json,installer-hooks.nsh,installer.nsi,app.manifest,.gitignore}`, `src-tauri/icons/` |
| windows | `rust/windows` | Window manager: Esc semantics, per-window queries, target, text scale, a11y relay, maximize, display latch, in-page growth, IPC authorization, keyboard and window channels, the IPC allowlists | 31 | `src-tauri/src/windows/` (except `screen.rs`), `src-tauri/src/state.rs`, `src-tauri/src/ipc.rs`, `preload.js` |
| capture | `rust/capture` | Display layout, capture fidelity, DXGI and HDR, backdrop, region selector, lens, buddy and tour, desktop selfcheck | 21 | `src-tauri/src/{capture,backdrop,region,lens,buddy}.rs`, `src-tauri/src/capture/`, `src-tauri/src/windows/screen.rs`, `src-tauri/src/desktop/{mod,channels,checks,selfcheck,win,scoring}.rs`, `renderer/{lens,buddy,region}.*`, `renderer/cursor-buddy.css` |
| llm | `rust/llm` | Providers (order, keys, PKCE connect, status), failover across providers, per-window Ask, timeouts, errors with codes and actions, busy auto-retry, what was sent, model pin, Find, wire robustness | 23 | `src-tauri/src/llm/`, `src-tauri/src/{ask,find,skills}.rs`, `src-tauri/src/host/{mod,ask,find,settings}.rs`, `skills/`, `tools/fixtures/llm/` |
| agent | `rust/agent` | Agent safety (confirm value, key activation, allowlist gate, takeover), native models, helper protocol and lifecycle, task copy, plan index | 22 | `src-tauri/src/agent/`, `src-tauri/src/host/{agent,desktop_ops}.rs`, `src-tauri/src/desktop/{client,protocol}.rs`, `lib/desktop/host.{cs,ps1}` |
| bar | `rust/bar` | Design system (tokens, shared components, icons, glass levels, light tone), the bar (textarea, token, target chip, keyboard layer, `@` model picker, notices), frame.js window menu, text scale and announcements in pages | 18 | `renderer/{index.html,bar.js,overlay.js,intent.js,glass.css,glass.js,frame.js,icons.svg}`, `renderer/overlay.css` (bar regions) |
| sheet | `rust/sheet` | Output windows: glob, markdown, reading measure, sheet bar, streaming, code blocks, usage and "What was sent", error actions, keyboard, Start over, Find view, agent trail and confirm card | 23 | `renderer/{output.html,output.js,markdown.js,trail.js,glob.js}`, `renderer/overlay.css` (sheet regions) |
| settings | `rust/settings` | Settings and History pages: Providers page, rows and disclosures, slider, models per provider, text size, privacy copy, History rows, undo, keyboard, CSS cleanup | 12 | `settings/`, `history/` |

Every lane also owns `.claude/rust-rewrite/entries/<lane>.md`, its own `tools/ui-shots/lanes/<lane>.mjs`, `tools/smoke-labels/<lane>.txt` and `src-tauri/src/selftest/probes/<lane>.rs` (the last three are created empty by harness C1), and the Progress and Blockers sections of its own brief.

Nobody edits the Electron reference (`main.js`, `lib/**` except the files named above, `package.json`, `package-lock.json`, `electron-builder.yml`), `README.md`, `REQUESTS.md`, `forge.zip`, or the audits. `DESIGN.md`, `PARITY.md` and `ARCHITECTURE.md` change only through entries files (see "Shared documents").

## Decisions

Each decision names the document entry that records it and the lane that writes that entry (in `entries/<lane>.md`).

| ID | Decision | Entry | Owner | Items |
|---|---|---|---|---|
| D-01 | Esc never stops an answer or hides output windows; Esc in the bar clears then closes; Esc in an output moves focus. The foreground still returns to the previous app. | DESIGN Keyboard; PARITY POS-09, IPC-02 notes | windows | B-018, B-060, B-067 |
| D-02 | Output windows keep a 320 x 160 minimum (Electron 90): the 44 px title bar, one row and the stage margins do not fit in 90. | DESIGN Output windows; PARITY POS-02, POS-05 | windows | B-097 |
| D-03 | The NSIS installer is one click, through a copied Tauri template with the MUI pages removed. | PARITY PKG-01; ARCHITECTURE Packaging | platform | B-085 |
| D-04 | Output content grows inside the page; the OS window is sized once. `answerFollowsTranscript` now means "the glass height equals the transcript height"; POS-11 and B15 apply to user moves and dodges only. | DESIGN Output windows; PARITY POS-11, IPC-06, answerFollowsTranscript | windows | B-027, B-164 |
| D-05 | Reading measure is 640 px (DESIGN said 760): about 68 characters at 15/24. | DESIGN Typography | sheet | B-064 |
| D-06 | The tour leads with what works today and never prints a blank shortcut; the Try card becomes "Got it"; the B7 haiku prefill stays for any card that sends `try`. | DESIGN Onboarding; PARITY TOUR rows, IPC-69, EVT-31 | capture | B-033, B-129 |
| D-07 | The tray's DoubleClick arm is dropped (a Windows double click also sends Click(Up), so Electron's behaviour flickered). | PARITY TRAY-05 | platform | B-080 |
| D-08 | The first question after an area pick captures fresh unless the pick-time shot is under 1 s old. | PARITY CAP-09 | llm | B-124 |
| D-09 | Capture sends physical pixels up to a 1600 px long edge with 4:4:4 chroma, from the foreground window's monitor first; one definition of the active display serves capture and window placement. | ARCHITECTURE Capture; PARITY CAP rows | capture | B-032, B-005, B-025 |
| D-10 | `settings:use-clip-key` keeps testing the key before the tour advances. | PARITY (use-clip-key row) | llm | B-038 |
| D-11 | `StartupApproved` is written only on an explicit Launch at login toggle, so a Task Manager disable survives restarts. | PARITY SYS-03 | platform | B-081 |
| D-12 | History writes throttle from the first change, on one writer thread. | PARITY HIST-06 | platform | B-003 |
| D-13 | `providerOrder` replaces the hidden `provider` override; the legacy value migrates to the top of the order. | DESIGN Providers; PARITY STORE-41 | llm | B-037 |
| D-14 | Failover crosses providers, only before any answer text, and asks before switching from free to paid (on by default). The daily free cap never fails over inside the same account, but may hop provider under the same paid rule. | DESIGN Providers; PARITY FAIL rows | llm | B-043, B-046, B-166 |
| D-15 | Each output window has its own query; at most two answers stream at once and a third waits. | ARCHITECTURE Ask; PARITY ASK rows | llm | B-019, B-040 |
| D-16 | New profiles start with a 600 px bar (`inputWidth` default and `PANEL_WIDTH`); stored widths are untouched. | DESIGN Bar; PARITY width rows | platform | B-087, B-106 |
| D-17 | Settings, History and outputs maximize to the full work area and go full bleed. | DESIGN Windows | windows | B-024, B-073 |
| D-18 | Gemini requests omit `temperature`, like OpenAI and Anthropic. | ARCHITECTURE LLM | llm | B-125 |
| D-19 | HTTPS uses the Windows certificate store and the system proxy. | ARCHITECTURE Network | llm | B-126 |
| D-20 | Glass adapts its tone over light content (departure from "dark tint always"), with hysteresis and its own tokens. | DESIGN Material | bar | B-159, B-147 |
| D-21 | The glass slider follows the level table in design-visual §3 "Glass levels". | DESIGN Material | bar | B-055 |
| D-22 | Radius scale 28 / 16 / 12 / 8 / 6 / capsule; inner radius = outer minus inset. | DESIGN Shape | bar | B-054 |
| D-23 | `--accent-text` (#EBC0E9 dark tone, #823A80 light tone) for accent text and icons on glass; `--accent-bright` stays for fills and rims. | DESIGN Palette | bar | B-054, B-159 |
| D-24 | Text follows the Windows text size times a 100-200% setting; pages handle Ctrl+=, Ctrl+- and Ctrl+0. | DESIGN Accessibility | windows | B-022, B-059, B-074 |
| D-25 | CAP-13 fidelity is measured against a lossless ground truth, not an Electron capture. | PARITY CAP-13 | capture | B-116 |
| D-26 | The lens frame is 2 px accent plus a 1 px dark outline, which conforms to DESIGN: no departure entry. | none | capture | B-117 |
| D-27 | The agent safety gate is an allowlist; the gate verdict tables change accordingly. | PARITY AGENT-05; ARCHITECTURE Agent | agent | B-049 |
| D-28 | Errors carry a code and the actions that fix them; busy free models show as a neutral note with a countdown. | DESIGN Errors; PARITY ERR rows | llm | B-119, B-120, B-166 |
| D-29 | The usage line reads `<model name> via <Provider> · <time>`; telemetry moves to the tooltip and Copy details. | DESIGN Answers | sheet | B-162 |
| D-30 | Task copy says "The task stopped", and gives the remedy once. | DESIGN Copy; PARITY AGENT text rows | agent | B-143 |
| D-31 | Start over keeps the window and offers Undo for 10 s. | DESIGN Output windows | windows | B-105, B-168 |
| D-32 | At most six output windows: a seventh closes the least recently touched idle window and says so. | DESIGN Output windows | windows | B-178 |
| D-33 | HDR outputs use a float capture path with SDR tone mapping; hardware verification is recorded as a blocker when no HDR runner exists. | ARCHITECTURE Capture | capture | B-109 |
| D-34 | A keyboard layer for bar and outputs (U19); Ctrl+R becomes "ask about an area" (browser reload is already blocked). | DESIGN Keyboard | bar | B-151, B-167, B-100 |
| D-35 | Alt+Space (or right-click on a title bar) opens a window menu: Move, Size, Maximize, Close. | DESIGN Keyboard | bar | B-158, B-101 |
| D-36 | IPC checks which window is calling against a channel table. | ARCHITECTURE IPC | windows | B-089 |
| D-37 | Narrow capability, a Tauri CSP, and no navigation away from the app. | ARCHITECTURE Security | platform | B-078 |
| D-38 | "What was sent" keeps the last three JPEGs per window in memory only. | DESIGN Privacy; ARCHITECTURE Ask | llm | B-121, B-162 |
| D-39 | Connect OpenRouter uses OAuth PKCE through a loopback callback. | DESIGN Providers; ARCHITECTURE Auth | llm | B-039, B-069 |

## File ownership

One owner per file. A lane that needs a change in a file it does not own writes the request in its brief's Blockers section (file, change, item) and carries on with other items; the coordinator routes it.

| Path | Owner |
|---|---|
| `src-tauri/src/selftest.rs` and `src-tauri/src/selftest/**` | harness, except `selftest/probes/<lane>.rs` (that lane, after harness C1 creates it) |
| `src-tauri/src/backdrop/{contrast.rs,contrast-probe.js,contrast-hide.css}` | harness |
| `.github/workflows/**`, `tools/**`, `lib/backdrop-fixture.html` | harness, except `tools/fixtures/llm/**` (llm), `tools/ui-shots/lanes/<lane>.mjs` and `tools/smoke-labels/<lane>.txt` (that lane) |
| `src-tauri/src/{lib,main,paths,instance,crash,hotkeys,tray,notify,autostart,updater,history}.rs` | platform |
| `src-tauri/src/store/**`, `src-tauri/src/host/menu.rs` | platform |
| `src-tauri/bridge/**`, `src-tauri/capabilities/**` | platform |
| `src-tauri/{Cargo.toml,build.rs,tauri.conf.json,installer-hooks.nsh,installer.nsi,app.manifest,.gitignore}`, `src-tauri/icons/**` | platform (Cargo.toml has per-lane blocks, see Hotspots) |
| `src-tauri/src/windows/{mod,flags,layout,motion,outputs,stage}.rs` and new files under `windows/` | windows |
| `src-tauri/src/state.rs`, `src-tauri/src/ipc.rs`, `preload.js` | windows (allowlists have per-lane blocks, see Hotspots) |
| `src-tauri/src/windows/screen.rs`, `src-tauri/src/capture.rs`, `src-tauri/src/capture/**` | capture |
| `src-tauri/src/backdrop.rs` (not `backdrop/contrast*`), `src-tauri/src/{region,lens,buddy}.rs` | capture |
| `src-tauri/src/desktop/{mod,channels,checks,selfcheck,win,scoring}.rs` | capture |
| `renderer/{lens.html,lens.js,lens.css,buddy.html,buddy.js,cursor-buddy.css,region.html,region.js,region.css}` | capture |
| `src-tauri/src/llm/**`, `src-tauri/src/{ask,find,skills}.rs`, `skills/**` | llm |
| `src-tauri/src/host/{mod,ask,find,settings}.rs` | llm |
| `src-tauri/src/agent/**`, `src-tauri/src/host/{agent,desktop_ops}.rs`, `src-tauri/src/desktop/{client,protocol}.rs`, `lib/desktop/{host.cs,host.ps1}` | agent |
| `renderer/{index.html,bar.js,overlay.js,intent.js,glass.css,glass.js,frame.js}`, new `renderer/icons.svg` | bar |
| `renderer/overlay.css` lines from the top through the "rows under the bar" section; the "layout editor" section; the "reduced motion, contrast" section to the end | bar |
| `renderer/{output.html,output.js,markdown.js,trail.js,glob.js}` | sheet |
| `renderer/overlay.css` sections "answer sheet" and "answers"; sections "find" and "agent trail" | sheet |
| `settings/**`, `history/**` | settings |
| `.claude/rust-rewrite/entries/<lane>.md`; Progress and Blockers of `.claude/rust-rewrite/lanes/<lane>.md` | that lane |
| `DESIGN.md`, `PARITY.md`, `ARCHITECTURE.md`, `BACKLOG.md` (except "Post-merge findings") | merge agent only |

Crate-root modules are declared in `lib.rs` (platform). A lane that needs a new module declares it inside a module it owns (for example `llm/auth.rs` in `llm/mod.rs`, `windows/text_scale.rs` in `windows/mod.rs`, `selftest/migrate.rs` in `selftest/mod.rs`).

## Hotspots and how they are split

**IPC allowlists (`ipc.rs` INVOKE_CHANNELS and LISTEN_CHANNELS, `preload.js` Sets).** They must stay equal and in the same order (`invoke_allowlist_matches_preload`, `listen_allowlist_matches_preload`). Windows C1 adds reserved blocks, identical in both files, in this order at the end of each list:

```
    // ---- lane: windows (do not edit outside your lane) ----
    // ---- end: windows ----
    // ---- lane: capture ----
    // ---- end: capture ----
    // ---- lane: llm ----
    // ---- end: llm ----
    // ---- lane: agent ----
    // ---- end: agent ----
```

(`preload.js` uses the same comment lines.) A lane adds a channel only inside its block, in both files, in the same commit as its route (invoke) or its emitter (listen). The marker lines separate hunks, so parallel additions merge cleanly. Removals of existing channels belong to the lane that owns the channel's route; `buddy:move` and `settings:data` are removed by windows (B-095).

**Invoke routing.** Routers run in order: `desktop::channels` (capture), `host::ask` (llm), `host::find` (llm), `host::agent` (agent), the `ui:context-menu` arm, then the core rows in `ipc.rs` (windows). An earlier router wins. llm adds `host::settings::route` (K-15) to the existing `host/settings.rs` and calls it first from `host::ask::route`; windows C1 adds `include_str!("host/settings.rs")` to the `every_invoke_channel_has_a_route` scan list (the file already exists). After llm C1 is on rust-rewrite, windows deletes the superseded core rows (B-107).

**`renderer/overlay.css`.** Split by its existing section header comments (`/* ---- ... the orb`, `panel positioning`, `bar`, `intent token`, `the menu`, `rows under the bar`, `answer sheet`, `answers`, `layout editor`, `find`, `agent trail`, `reduced motion, contrast`). Bar owns from the top through "rows under the bar", plus "layout editor" and "reduced motion, contrast" to the end. Sheet owns "answer sheet", "answers", "find" and "agent trail". Nobody edits or moves a header line; new sections go inside the owner's range. Sheet's reduced-motion and forced-colors rules go in `@media` blocks inside sheet sections, not in the tail.

**`src-tauri/Cargo.toml`.** Platform C1 regroups dependencies into commented per-lane blocks (`# ---- lane: <lane> ----` / `# ---- end ----`), including inside the `windows` and `windows-sys` `features` arrays (one feature per line), and pre-adds everything the backlog needs: `webview2-com` (the version wry uses), `png` (or `image` with only png), reqwest features for native roots and system proxy, `sha2` and `rand` (PKCE), windows features `UI_ViewManagement`, `Foundation`, `Win32_System_JobObjects`, `Win32_Storage_FileSystem`, `Win32_UI_Accessibility`, `Win32_UI_Input_KeyboardAndMouse` (SendInput), `Win32_Graphics_Dxgi_Common` and `Win32_Graphics_Direct3D11` if not present, `Win32_Networking_WinHttp` (proxy fallback), plus `[profile.release.package.fast_image_resize] opt-level = 3`. A lane that needs one more dependency or feature adds it inside its own block only. `Cargo.lock` is untracked.

**`state::Hooks`.** Windows owns the struct. Windows C1 adds `abort_query_for` (K-03) and `overlay_extra` (K-07). llm registers its hooks in `host/mod.rs`.

**`desktop/channels.rs`** is capture's. It calls `windows::on_hello` (windows-owned), so the text-scale push on hello (B-022) needs no capture edit.

**Smoke and shots outputs.** Each lane writes its probes, labels and ui-shots scenes in its own files (K-30, K-31). The shots branch is published per ref (B-053), so lanes do not overwrite each other's pictures.

## C1 protocol (contracts first)

1. A lane's first push is its **C1**: only the interfaces other lanes consume (listed in its brief under "Contracts you provide"), compiling and tested, behaviour stubbed where the real work is a later item. Commit message starts with `C1 contracts:`.
2. When `node tools/ci-wait.mjs HEAD` exits 0, the lane writes `C1 green at <sha>` in its Progress section. The merge agent merges C1 branches into `rust-rewrite` as they turn green, in this order when several are ready: platform, windows, harness, capture, llm, agent, bar.
3. A lane that consumes a contract checks whether it has landed with `git fetch origin` then `git grep -n "<symbol>" origin/rust-rewrite -- <file>`. When it has, `git merge origin/rust-rewrite` (merge, never rebase or force-push) and continue. Until then, work on items that do not need it.
4. After C1, any lane branch that is green may be merged at a checkpoint; lanes keep merging `origin/rust-rewrite` into their branch to stay current.

## Interface contracts

| ID | Contract | Provider | Consumers | Item |
|---|---|---|---|---|
| K-01 | `crash::guard(name, f)` and `crash::guard_async(name, fut)`: run, catch a panic, log it, toast the right text for the thread | platform | windows, capture | B-013 |
| K-02 | `crash::guard_webview(&WebviewWindow, on_browser_exit)`: reload on renderer `ProcessFailed`, call back on browser-process exit | platform | windows, capture | B-014 |
| K-03 | `AppState::{query_started, query_ended, query_running}`, `Hooks.abort_query_for(label)` | windows | llm | B-019 |
| K-04 | `outputs::fresh(app) -> Option<String>` and `outputs::close_all(app)` | windows | llm, agent, platform | B-020, B-100 |
| K-05 | `overlay:state.target {label,title,kind} \| null` and `overlay:state.visibleOutputs` | windows | bar | B-021 |
| K-06 | listen `output:meta {grow, front, target}` | windows | sheet | B-021 |
| K-07 | `Hooks.overlay_extra() -> Map` merged into `overlay:state` | windows | llm (registers `recipients`) | B-021 |
| K-08 | `Store::set_raw(key, Value)` | platform | windows, llm | B-001 |
| K-09 | `Store::key_storage() -> "encrypted" \| "plain"` | platform | llm | B-084 |
| K-10 | `screen::active()`, `screen::latch_active()`, `screen::latched()`, `screen::work_area_of(display)` in `windows/screen.rs` | capture | windows | B-032 |
| K-11 | `pub screen::forced_scale()` | capture | windows | B-025 |
| K-12 | `region::set_active_on(rect_dip, device)` | capture | llm | B-005 |
| K-13 | `buddy::agent_point_phys(pt_px, label)` | capture | agent | B-005 |
| K-14 | invoke `lens:ready`, `buddy:ready` (capture-internal) | capture | capture renderer | B-034 |
| K-15 | `host::settings::route(app, label, channel, args) -> Option<Result<Value>>`, called first by `host::ask::route`; the list of channels it owns in `entries/llm.md` | llm | windows (B-107) | B-038 |
| K-16 | `llm::settings::candidates(&settings) -> Vec<Candidate>`, `model_for(settings, provider, kind)` | llm | agent | B-037 |
| K-17 | `ApiError { retry_after_ms, attempts, code }` | llm | agent | B-042, B-120 |
| K-18 | `llm::recipients(&settings) -> Vec<Recipient>`; `overlay:state.recipients`; `settings:get.recipients` | llm | agent, bar, settings | B-045 |
| K-19 | Renderer payloads: `query:error {code, provider, model, modelName, retryInMs, tone, actions}`, `query:done.sent`, `models:list.providers`, `settings:get.{providers, failover, keyHints, keyStorage}`, `provider:status`, the `settings:*` provider channels, `query:submit`/`query:retry` `{provider, model}`; mocked in `tools/ui-shots/lanes/llm.mjs` | llm | sheet, settings, bar | B-038, B-119, B-121, B-122 |
| K-20 | `agent:confirm-request.{value, truncated}`; `agent:step.planIndex` | agent | sheet | B-006, B-144 |
| K-21 | `agent:consent-request.{provider, model, modelName, recipients}` | agent | sheet | B-051 |
| K-22 | listen `ui:text-scale {scale}`, invoke `settings:set-text-scale {scale}`, `settings:get.{textScale, osTextScale}` | windows | bar, settings | B-022 |
| K-23 | Window channels: `output:blur`, `output:cycle-target`, `output:close`, `output:close-all`, `output:last-answer`, `output:undo-reset`, `output:glass`, `window:nudge`, `bar:dirty`, `bar:clear`, `bar:ready`, `bar:prefill-open`, `bar:note`, `a11y:announce`, `a11y:say` | windows | bar, sheet, settings | B-018, B-023, B-027, B-100..B-105, B-178 |
| K-24 | `history:list -> {rows, enabled}`; `settings:get.overlayPos` | windows | settings | B-179 |
| K-26 | `llm::fake::Fake::push_fixture(Value)` plus fixture fields `status`, `retryAfter`, `stallAfterHeadersMs`, `provider`, so other lanes' probes can script the fake LLM | llm | windows, agent, platform | llm C1 |
| K-30 | Selftest API: `Report::{must, log}`, `Cx::{eval, input, wait_for}`, `probes::<lane>::run(&mut Cx)`, `tools/smoke-labels/<lane>.txt` enforcement | harness | all | B-009 |
| K-31 | ui-shots lane API: `tools/ui-shots/lanes/<lane>.mjs` exporting `{ scenes, mocks, checks, cssLintEnforce }`, run at DPR 1 and 1.5 | harness | all renderer lanes, llm (mocks) | B-053 |
| K-40 | `glass.css` tokens (design-visual §3), `.sheet-bar`, `.g-menu`, `renderer/icons.svg`, `AevalGlass.scrimFor(rect, level)`, `--text-scale`, `--accent-text` | bar | sheet, settings, capture | B-054 |
| K-41 | `html[data-maximized]` set by `frame.js` | bar | settings, windows | B-054 |
| K-42 | `AevalA11y.say(text, priority)` in `glass.js` | bar | sheet, settings, capture | B-058 |
| K-43 | `frame.js` window menu (Alt+Space) on every framed page | bar | sheet, settings | B-158 |

K-25 is intentionally unused.

## Shared documents (DESIGN, PARITY, ARCHITECTURE)

Lanes never edit these files. Each lane appends to `.claude/rust-rewrite/entries/<lane>.md`:

```
## <DESIGN|PARITY|ARCHITECTURE> · <section or row id> · <D-xx or B-xxx>
Add | Replace "<exact current text>" with:
<exact new text>
```

For a PARITY row whose status or evidence changes: `## PARITY · <row id> · B-xxx` then `status: <value>; evidence: <UNIT/SMOKE label>`. The merge agent folds each lane's entries into the documents in the merge commit, one lane at a time, and deletes the entries file. A lane writes the entry in the same commit as the change it records.

## Smoke labels and ui-shots

- From harness C1, the rust workflow fails when any label listed in `tools/smoke-labels/*.txt` is missing from the smoke log or prints `=FAIL`. A lane adds a label to its file in the same commit that makes it print `=ok`; never list a label you have not implemented, never remove one to get green.
- Each brief lists the PARITY labels the lane ports and the new `SMOKE+` labels its items name. The harness final pass (B-148) adds `tools/smoke-labels/required.txt` (all PARITY labels plus every `SMOKE+` label here) and enforces the union.
- ui-shots: each renderer lane owns its scenes, mocks and checks in `tools/ui-shots/lanes/<lane>.mjs`. design-visual §4 states: bar 1-13 and 54-57, sheet 14-40, settings 41-50, capture 51-53. A file listed in `cssLintEnforce` must pass the CSS gate (B-145) from then on.

## Merge order and expected conflicts

1. C1 merges as they turn green: platform, windows, harness, capture, llm, agent, bar.
2. Checkpoint merges of green branches, then the final merges in this order: platform, windows, capture, llm, agent, bar, sheet, settings, harness (final pass: B-148 strict gates, then B-149 review loops).
3. Expected conflicts: none inside owned files. The only shared-text spots are the allowlist blocks (separated by marker lines), the `overlay.css` section boundaries (header lines are separators), the Cargo.toml lane blocks, and the entries folding, which the merge agent does sequentially. `Cargo.lock` is untracked. If a merge conflicts anywhere else, a lane edited a file it does not own: the merge agent keeps the owner's side and files the other side back to that lane as a Blocker.

## Items: P0

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

### B-005 · P0 · capture · One canonical DIP layout that cannot overlap; every mapped-back rect carries its display
- **Sources:** review-desktop P0-1, P1-6; review-core P1-1 (screen half).
- **Files:** `src-tauri/src/capture/screen.rs`, `src-tauri/src/windows/screen.rs`, `src-tauri/src/capture.rs`, `src-tauri/src/region.rs`, `src-tauri/src/lens.rs`, `src-tauri/src/buddy.rs`.
- **Fix:**
  1. Build Chromium-style adjacent DIP bounds once, in one function used by both screen modules: the primary keeps origin (0,0) and its DIP size; every other display is placed edge to edge with the neighbour it touches physically (sorted by distance to primary, then device name), at `neighbour_dip_edge + (phys_offset_along_edge / own_scale)`. No display's DIP rect may overlap another's. `windows/screen.rs` keeps its public API but delegates to this layout.
  2. `matching` breaks ties toward the primary, then the display the rect's centre is in, using a first-max scan instead of `Iterator::max_by` (which returns the last).
  3. `region` stores the display device (or the physical rect) beside `ACTIVE` and `PENDING_SHOT`; `capture::shot_image` takes that display instead of calling `matching`. Provide `region::set_active_on(rect_dip, device)` for host/find.rs "Ask about this" (consumed by llm, see contract K-12).
  4. `lens::set_hits`, the loupe (`capture::crop_dip`) and `buddy::agent_point` carry the display id from the physical side (`matching_phys` is unambiguous). Provide `buddy::agent_point_phys(pt_px, label)` (consumed by agent, K-13).
- **Acceptance:** UNIT `capture::screen::tests::overlapping_mixed_dpi_layout_is_adjacent` (primary 1920x1080@100%, secondary 2560x1440@150% at x=1920: DIP rects touch and do not overlap); UNIT `capture::screen::tests::rect_in_old_overlap_maps_to_primary`; UNIT `capture::tests::area_on_primary_crops_primary`; UNIT in `windows::screen` mirroring the first two; SMOKE+ `dpiRoundTrip`.

### B-006 · P0 · agent · The confirm request carries the full text that will be typed
- **Sources:** review-llm-agent P0-1 (Rust half).
- **Files:** `src-tauri/src/agent/run.rs`, `src-tauri/src/agent/describe.rs`.
- **Fix:** Add `"value": <full text>` to the `agent:confirm-request` payload for `type`, `form_input` and `set_value` (passwords stay dotted, as `describe` does today). Keep the described `text` label. Add `"truncated": true` when the label's quote is shorter than the value. Contract K-20.
- **Acceptance:** UNIT `agent::run::tests::confirm_payload_carries_full_value` (a 200-character type step: payload `value` equals the input, `truncated == true`); SMOKE+ `confirmFullText` (agent probe asserts the emitted payload).

### B-007 · P0 · sheet · The confirm card shows the full value and enables Allow only once it is visible
- **Sources:** review-llm-agent P0-1 (renderer half); design-ux U45 (sweep shares the gate).
- **Files:** `renderer/trail.js`, `renderer/overlay.css` (agent trail region).
- **Fix:** Render `value` (K-20) in a scrollable `pre`-wrapped block under the card title, max height 8 lines, `--font-mono` 13/20, with `aria-label="Text that will be typed"`. If `truncated` and the block overflows, Allow stays `aria-disabled="true"` until the block has been scrolled to its end or expanded with a `Show all` text button; then the 700 ms settle (U45) applies. Older payloads without `value` render as today.
- **Acceptance:** ui-shots scene `sheet-confirm-long-text` and check `confirmAllowWaitsForFullText` in `tools/ui-shots/lanes/sheet.mjs`.

### B-008 · P0 · agent · Keyboard activation of a focused control goes through the gate
- **Sources:** review-llm-agent P0-2.
- **Files:** `src-tauri/src/agent/gate.rs`, `src-tauri/src/agent/run.rs` (`inspect`).
- **Fix:** When a `key` chord contains `space` or `enter`, or `type` text contains a space or newline, `inspect` reads the focused node from `lazy.tree()` and the gate runs the click checks on it: `name_verbs` on name, automation id and help text; an unnamed button is unreadable (ask); a mail or chat app with a clickable role asks; a risky page asks. `space` and `tab` leave `FREE_KEYS`: `space` is free only when the focused role is an editable text or document role. `tab` stays free (it only moves focus) but the next activation is checked as above.
- **Acceptance:** UNIT `gate::tests::space_on_focused_send_button_asks`; UNIT `gate::tests::typed_space_on_button_asks`; UNIT `gate::tests::space_in_text_field_is_free`; SMOKE+ `gateKeyActivation`.

### B-009 · P0 · harness · A real smoke runner with a probe API every lane can use
- **Sources:** parity-ipc B1, SMK-01..08, §3; parity-core and parity-features "no runtime proof"; review-desktop P2-5 (DXGI must be gated, consumed by B-029).
- **Files:** `src-tauri/src/selftest.rs` → `src-tauri/src/selftest/{mod.rs, report.rs, eval.rs, input.rs, probes/mod.rs}`; creates `src-tauri/src/selftest/probes/<lane>.rs` (one per lane, then owned by that lane); `tools/smoke-labels/<lane>.txt`; `.github/workflows/rust.yml` (smoke step).
- **Fix:**
  1. Keep the names lib.rs calls (`smoke_enabled`, `scaffold`, `PING`, `quiet_run` semantics) so lib.rs needs no edit; `scaffold` becomes the full runner when `AEVAL_SMOKE` is set and the shots runner (B-010) when `AEVAL_SHOT` is set (`smoke_enabled()` returns true for either).
  2. `Report`: `must(label, ok, why)` prints `[SMOKE] <label>=ok` or `[SMOKE] <label>=FAIL:<why>`; `log(label, value)` prints `[SMOKE] <label>=<value>`. The last line is `[SMOKE] done` (exit 0) or `[SMOKE] failed: a, b` (exit 1); an uncaught panic prints `[SMOKE] error=<msg>` and exits 1. Flush stdout before `app.exit`.
  3. `Cx::eval(window_label, js) -> Result<Value>` through `with_webview` and `ICoreWebView2::ExecuteScript` with its completion handler (webview2-com, pre-enumerated in platform C1), JSON-decoding the result, 5 s timeout.
  4. `Cx::input`: `SendInput` click, drag, wheel, key chord and text at physical points, plus `Cx::wait_for(timeout, cond)`.
  5. Blocks: run each lane's `probes::<lane>::run(&mut cx)` inside its own `catch_unwind`; a panic prints `must("probe.<lane>", false, msg)`. Keep the Electron block labels `v2basics`, `v21basics`, `glassGates`, `v2desktop` (each FAIL only when its block throws) mapped onto groups of lane probes.
  6. Run the desktop selfcheck inside the smoke with `Report::new("SMOKE")` (desktop/checks.rs already takes a prefix), `ownPoint` = pill centre via `windows::screen::dip_to_px`, `ownHandles` = every webview HWND, `notepad = GITHUB_ACTIONS set || (AEVAL_DESKTOP_NOTEPAD set && != "0")`. When neither is set print `[SMOKE] v2desktop=skipped (not on CI)`. Open Notepad for the v2desktop block and close it after (labels `v2NotepadOpen`, `v2NotepadClosed`, harness-owned) using the desktop client helpers.
  7. rust.yml "Smoke" step: fail on any `=FAIL` line, on a missing `[SMOKE] done`, and on any label listed in `tools/smoke-labels/*.txt` that does not appear; timeout 360 s (installer-check's budget).
  8. Ship this in harness C1 (K-30): the runner, Report, eval, input, the empty probe files and empty label files, with `version` and `done` already in `tools/smoke-labels/harness.txt`.
- **Acceptance:** rust.yml smoke log shows `[SMOKE] version=`, `[SMOKE] done`, and `[SMOKE] probe.<lane>=ok` for all nine lanes; a deliberately failing must in a throwaway commit turns the step red (state the sha in the harness Progress section, then revert).

### B-010 · P0 · harness · `AEVAL_SHOT`: photograph the real exe, run the 100/125/150 matrix through the contrast gate
- **Sources:** parity-ipc B2, SHOT-01..05; parity-features #14 (GLASS-04, GLASS-05, GLASS-08); parity-core LIFE-03 (consumer).
- **Files:** `src-tauri/src/selftest/shots.rs`, `src-tauri/src/backdrop/contrast.rs`, `src-tauri/src/backdrop/contrast-probe.js`, `src-tauri/src/backdrop/contrast-hide.css`, `.github/workflows/rust.yml`.
- **Fix:** `selftest::shots(app, dir)` when `AEVAL_SHOT=<dir>`: drive each surface into the named state through the same hooks IPC uses; photograph each window with `ICoreWebView2::CapturePreview` (PNG) via `with_webview`; use a real screen grab for `12-live-overlay` with `backdrop::pause()` then content protection lifted for one frame then `backdrop::resume()`. Write `<dir>/<name>.png` and print `[SHOT] <name>=<w>x<h>`; frames 01-17 with the `[SHOT] fit.<name>=window<h>:surface<h>` probes (Electron lib/shots.js, main.js:3080) and 18-30 (lib/shots-v2.js). With `AEVAL_SHOT_MATRIX_ONLY=1` render the 13 matrix surfaces over the white, black and wallpaper fixtures at the scale in `AEVAL_SHOT_SCALE` (WebView2 is forced by B-015), name them `m-<surface>-<backdrop>-<scale>.png`, run every frame through `backdrop::contrast`, print `[SHOT] matrix scale=<s> contrastMin=<x> failedFrames=<n>`, exit 1 when `failedFrames>0`. Always exit within 600 s. Add a rust.yml step that runs the matrix at 1.0 on the built exe and fails on `failedFrames>0`.
- **Acceptance:** rust.yml shots step log shows `[SHOT] 01-...` through `30-...` and `[SHOT] matrix scale=1 ... failedFrames=0`; installer job (B-052) `shots.written`, `shots125.*`, `shots150.*` pass.

## Items: S1 and P1

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

### B-028 · P1 · capture · The area selector returns the area, never hangs, and claims Escape in time
- **Sources:** parity-features #6 (CAP-11, polish e796cd2), #16 (CAP-10); review-desktop P2-12; parity-ipc B8 (IPC-12).
- **Files:** `src-tauri/src/region.rs`, `src-tauri/src/desktop/channels.rs`.
- **Fix:** `finish`: `settle(finish_rect(origin, r))` first, then `close(app)`. Tag `Pending` with a generation; `Destroyed` and `Focused(false)` handlers settle only their own generation. Add a 5 s load timeout that destroys the window and settles `None`. Call `changed()` from inside `select()` after `open_window` succeeds. The `region:pick` IPC arm returns early in HUD mode and routes through the `start_region` hook like the hotkey path, instead of calling `region::pick` itself.
- **Acceptance:** capture check `[CAPTURE] regionDragReturnsTheBox=ok`; UNIT `region::tests::stale_close_does_not_settle_new_pick`; SMOKE `regionEscape`, `regionSelectorCancels`.

### B-029 · P1 · capture · DXGI never serves a pointer-only first frame, and CI gates the DXGI path
- **Sources:** review-desktop P1-5, P2-5.
- **Files:** `src-tauri/src/capture/dxgi.rs`, `src-tauri/src/desktop/checks.rs`.
- **Fix:** For a duplication with no `last`, loop inside the 900 ms budget until a frame has `LastPresentTime != 0` (or `AccumulatedFrames > 0`); reject an all-zero first frame; if none arrives return Err so the call falls back to GDI rather than caching. In checks.rs, when `last_backend()` is `dxgi` require the probe result as a `must`, assert the whole-screen shot shows the fixture (mean brightness band, not black), and print `[CAPTURE] dxgi=unavailable` loudly when the runner cannot run DXGI.
- **Acceptance:** capture check `[CAPTURE] dxgiFirstFrame=ok`, `[CAPTURE] wholeScreenShowsFixture=ok`; UNIT `dxgi::tests::pointer_only_first_frame_is_skipped` (fake frame source).

### B-030 · P1 · capture · The backdrop idles when nothing is read, and each tick costs a fraction of today's
- **Sources:** review-desktop P1-2, P1-3, P2-14.
- **Files:** `src-tauri/src/lens.rs`, `src-tauri/src/backdrop.rs`, `src-tauri/src/capture/encode.rs`, `src-tauri/src/capture/dxgi.rs`, `src-tauri/src/capture.rs`.
- **Fix:** `lens::windows()` counts only visible lens windows (`flags::is_visible(hwnd)`, Win32, no main-thread hop), and `lens::clear()` destroys lens windows when no hits or frame remain. `grab_all` captures only displays hosting a visible glass window (`display_for`). Keep the last frame `Arc` and JPEG per display and skip conversion and encoding when `Arc::ptr_eq` says DXGI returned the same frame. Resize BGRA directly (`fast_image_resize` `U8x4`, `use_alpha(false)`) from a borrowed `ImageRef`, then convert the small image; crop BGRA rows before converting for area captures. Reuse the readback buffer. The release profile override `[profile.release.package.fast_image_resize] opt-level = 3` is pre-enumerated in platform C1.
- **Acceptance:** SMOKE `snapCost` (blockP95 < 25 ms) and SMOKE+ `backdropIdle` (after Find closes and only the pill shows, pace returns to 5 s within 6 s); capture check `[CAPTURE] tickMs=<n>` logged with a must of < 40 ms per 1080p display on the runner.

### B-031 · P1 · capture · The buddy window is reused, never destroyed and re-created
- **Sources:** review-desktop P1-4.
- **Files:** `src-tauri/src/buddy.rs`, `renderer/buddy.js`.
- **Fix:** Do not destroy the buddy window between sessions: hide it, reset page state with a `buddy:reset` event, and reuse it; `create_window` builds a new window only when none exists or the existing one is closing (track a `destroying` generation). `ready` stays true across reuse.
- **Acceptance:** SMOKE+ `buddyReuse` (points shown, new answer with points before dismiss: pointer visible within 500 ms; first `agent_point` after a tour waits < 300 ms); SMOKE `buddyPoints`, `buddyPointsDone`.

### B-032 · S1 · capture · Capture fidelity: physical pixels up to 1600, 4:4:4 chroma, the foreground window's display first
- **Sources:** review-desktop P2-18; parity-features #9 (active display definition); parity-features CAP-01 (`contains` vs `nearest`).
- **Files:** `src-tauri/src/capture/encode.rs`, `src-tauri/src/capture/screen.rs`, `src-tauri/src/capture.rs`, `src-tauri/src/windows/screen.rs`.
- **Fix:** Size the whole-screen image from physical pixels within the 1600 px long-edge budget (1920x1080@150% goes out at 1600x900, not 1280x720). Encode JPEG with 4:4:4 chroma at q88 (stays under the 1.6 MB cap). Define the active display once (K-10) as: the monitor of the foreground window when it is not an AevalBuddy window, else the display under the pointer, else the nearest display to the pointer; `screen::active()`, `screen::latch_active()` and `screen::latched()` are shared by capture and window placement (B-025). Point and region DIP mapping updates follow the new image size (`crop_scales_by_what_arrived` still holds).
- **Reconciled:** Electron sized at DIP with 4:2:0 and pointer-first display (CAP-01/02). The product constraint "capture fidelity bounds answer quality" wins. Decision D-09.
- **Acceptance:** UNIT `encode::tests::whole_screen_uses_physical_pixels_within_budget` (replaces `whole_screen_matches_electron_thumbnail_sizes`, recorded in PARITY); UNIT `encode::tests::chroma_is_444`; UNIT `screen::tests::active_prefers_foreground_monitor`; SMOKE `capture` (`ok:<bytes>bytes:<w>x<h>` shows physical size).

### B-033 · S1 · capture · The first-run tour leads with what works and never prints a blank shortcut
- **Sources:** design-ux U10, U1 (step order); design-visual F26 (tour copy); parity-ipc B7 (context); copy deck C-40..C-44.
- **Files:** `src-tauri/src/buddy.rs`.
- **Fix:** New `walkthrough` steps, all quoting only `captureQuery`: (1) overlay, pointer on the orb: `Press <captureQuery>, then type a word you can see on screen.` advances on `find-hit`; (2) card: `To get answers, connect an AI provider. OpenRouter has free models.` buttons `Connect` (opens Settings at section `providers`) and `Later`, advances on `key-saved`, skipped when any key exists; (3) overlay: `Now ask anything about your screen.` advances on `first-answer`, skipped without a key; (4) overlay, pointer on the left token (`#intent-token`): `This shows what Enter will do. AevalBuddy guesses from what you type, and a task always asks before anything risky; Esc stops it.` button `Got it`. Never format an empty chord: fall back to the main chord, and a test asserts no step text contains two spaces in a row. The old `try` card is gone (B-129 keeps the `try` handler's prefill correct for compatibility).
- **Reconciled:** design-visual F26 rewrote the old steps' copy; design-ux U10 reorders and rewrites them; U10's deck wins and F26's "fall back to the main chord" rule is kept. Decision D-06.
- **Acceptance:** UNIT `buddy::tests::walkthrough_order_and_no_double_spaces` (replaces the TOUR-01 order assertions, recorded in PARITY); SMOKE `onboardingSteps` (redefined `0>1>2>3`), SMOKE `walkthrough`.

### B-034 · P1 · capture · Lens and pointer pages say they are ready before the first draw
- **Sources:** parity-features #15 (LENS-01, TOUR-04, ASK-11).
- **Files:** `src-tauri/src/lens.rs`, `src-tauri/src/buddy.rs`, `src-tauri/src/desktop/channels.rs`, `renderer/lens.js`, `renderer/buddy.js`, `src-tauri/src/ipc.rs` and `preload.js` (capture block only).
- **Fix:** `lens.js` and `buddy.js` invoke `lens:ready` / `buddy:ready` after registering their `on()` listeners (K-14). Rust marks the window ready and flushes the queued `lens:draw` / `buddy:show` on that invoke, not on `PageLoadEvent::Finished`.
- **Acceptance:** SMOKE `lensFirstDraw` (`3:true:true`), SMOKE+ `lensReady`, SMOKE `pointResolved`.

### B-035 · P1 · capture · A profile migrated as solid follows the glass slider
- **Sources:** parity-features #7 (NEW-12, REQ-5).
- **Files:** `src-tauri/src/backdrop.rs`.
- **Fix:** `reasons_for` drops the `surface_solid` reason: `glassIntensity` 0 is the solid path (DESIGN "Liquid glass you set"); `surface` stays in the file for rollback only.
- **Acceptance:** UNIT `backdrop::tests::solid_profile_at_07_is_live`; capture check `[CAPTURE] glassLive=ok` (RUNNER glassLive).

### B-036 · P1 · llm · Find and Ask send native model ids to native providers; native picks are saved and never healed against OpenRouter
- **Sources:** review-llm-agent P1-1 (Find and picker half); parity-features #1, #2 (save-model half).
- **Files:** `src-tauri/src/host/find.rs`, `src-tauri/src/host/settings.rs`, `src-tauri/src/llm/settings.rs`, `src-tauri/src/host/ask.rs`.
- **Fix:** `FindHost::model` branches on the active candidate: native → `llm::settings::model_for(settings, provider, Kind::Find)`; OpenRouter → `resolve_live`. The four model-save channels (`settings:save-model`, `save-brain`, `save-find-model`, `save-agent-model`) are routed in `host::settings::route` (K-15) and take an optional `provider`; for a native provider they write `nativeModels.<provider>.<kind>` (via `Store::set_raw`, K-08) and never run `heal`. `heal` and `heal_agent_model` skip ids that are not OpenRouter ids. Ask uses the native pick when one is stored.
- **Acceptance:** UNIT `host::find::tests::native_only_find_body_uses_native_default`; UNIT `host::settings::tests::native_pick_is_saved_per_provider_and_not_healed`; SMOKE+ `nativeModelIds` (fake LLM records the request URL and model for a native-only profile).

### B-037 · S1 · llm · Providers are tried in the person's order; a key is only sent to the provider it belongs to
- **Sources:** design-ux U2, §3.3, §3.5; review-llm-agent P2-12; parity-features #2; parity-core STORE-41 (DESIGN paragraph).
- **Files:** `src-tauri/src/llm/settings.rs`, `src-tauri/src/llm/provider.rs`, `src-tauri/src/llm/mod.rs`.
- **Fix:** `active()` becomes `candidates(&settings) -> Vec<Candidate{provider, model_for(kind)}>` (K-16) in `providerOrder` order (stored via `Store::set_raw("providerOrder", [...])`), skipping providers with no key, a key whose `Provider::detect` names a different provider, and providers in a cooldown (B-046). Default order: OpenRouter, Anthropic, OpenAI, Gemini. Migration on read: when `providerOrder` is absent and the legacy `provider` override is set, that provider goes first. `active()` stays as `candidates().first()` for callers not yet migrated.
- **Reconciled:** providerOrder replaces the hidden `provider` override (design-ux §3.3). Decision D-13.
- **Acceptance:** UNIT `llm::settings::tests::order_and_skips`; UNIT `llm::settings::tests::legacy_provider_migrates_to_top`; UNIT `llm::settings::tests::mismatched_key_is_skipped`; SMOKE+ `providerOrder`.

### B-038 · S1 · llm · Settings channels for every provider: save, test, clear, order, failover, status, hints
- **Sources:** design-ux §3.2, §3.5, U21 (Rust half), U2; parity-features #2; parity-ipc B14 (IPC-78).
- **Files:** `src-tauri/src/host/settings.rs`, `src-tauri/src/host/mod.rs` (`settings_extra`, `key_saved`), `src-tauri/src/host/ask.rs` (`settings:test-key`), `src-tauri/src/llm/client.rs` (`test_key`), `src-tauri/src/ipc.rs` and `preload.js` (llm block only).
- **Fix:** In `host::settings::route` (K-15): `settings:save-key {provider, key}` tests before storing and returns `{ok, status, message, keyHint}` or `{ok:false, error, detected}` (`detected` = provider the prefix names); `settings:clear-key {provider}`; `settings:test-key {provider}` returns `{ok, status, message, credit}`; `settings:set-provider-order {order}`; `settings:set-failover {acrossProviders, askBeforePaid}` (default `{true, true}`); `settings:clip-key` returns `{found, masked, provider}` for all four prefixes; `settings:use-clip-key` keeps testing before it advances the tour (Decision D-10) and its error reads `The clipboard no longer holds a key.` (C-33). Status values: `not-added`, `checking`, `ready`, `rejected`, `no-credit`, `busy`, `blocked` (free models off), `unchecked`. Listen `provider:status {id, status, statusText, busyUntil}` to Settings and the overlay. `settings_extra` adds `providers: [{id, name, order, hasKey, keyHint, status, statusText, checkedAt, busyUntil, credit}]`, `failover`, `keyHints`, `keyStorage` (from `Store::key_storage()`, K-09). List every channel this lane now routes in `entries/llm.md` so windows can delete the superseded core rows (B-107).
- **Acceptance:** UNIT `host::settings::tests::save_key_tests_before_storing`; UNIT `host::settings::tests::wrong_prefix_reports_detected`; UNIT `host::settings::tests::settings_extra_shape`; ui-shots mock in `tools/ui-shots/lanes/llm.mjs` exporting the payload shapes for settings (consumed by settings scenes).

### B-039 · S1 · llm · Connect OpenRouter in one click, and a key test that catches the free-model privacy block
- **Sources:** design-ux U1; copy deck C-57, C-65.
- **Files:** `src-tauri/src/llm/auth.rs` (new, declared in `llm/mod.rs`, so no crate-root edit), `src-tauri/src/llm/client.rs`, `src-tauri/src/host/settings.rs`, `src-tauri/src/ipc.rs` and `preload.js` (llm block only).
- **Fix:** `auth:openrouter-connect`: bind `127.0.0.1:<free port>`, build a PKCE S256 verifier and challenge, open `https://openrouter.ai/auth?callback_url=http://localhost:<port>/callback&code_challenge=<S256>&code_challenge_method=S256` in the default browser, read `code` from the callback, POST `https://openrouter.ai/api/v1/auth/keys` `{code, code_verifier, code_challenge_method:"S256"}`, store `key` with `set_provider_key("openrouter", ..)`, answer the tab with a plain page `AevalBuddy is connected. You can close this tab.`, emit `key-saved`. Timeout 5 min → `Didn't hear back from OpenRouter. Try again, or paste a key instead.` Verify the current OpenRouter PKCE parameters against openrouter.ai docs before building and note the check in Progress. `test_key(OpenRouter)`: after the credit check send one `max_tokens: 1` request to the default `:free` vision model; a data-policy refusal returns status `blocked` with C-65 text and action `Open OpenRouter privacy settings`; success text `Works. Checked just now.` (C-57).
- **Acceptance:** UNIT `llm::auth::tests::pkce_challenge_is_s256_of_verifier`; UNIT `llm::auth::tests::callback_parses_code_and_rejects_bad_requests` (local loopback round trip with a fake token endpoint); UNIT `llm::client::tests::data_policy_probe_reports_blocked` (fixture in `tools/fixtures/llm/errors.json`).

### B-040 · S1 · llm · Each window asks on its own; at most two answers stream at once
- **Sources:** design-ux U4; parity-features #4 (host half); review-core P2-7; parity-features "Low" (submit_to early return leaves busy stuck).
- **Files:** `src-tauri/src/host/ask.rs`, `src-tauri/src/host/mod.rs` (hook registration).
- **Fix:** `current()` becomes `HashMap<String /*label*/, Current>`; `abort(app, label)` cancels only that window's token, installed atomically (`map.insert(label, new)` returns the old one, which is cancelled). Register `Hooks.abort_query_for` (K-03) and use `AppState::query_started/ended` so `busy` means "any running". `query:cancel {label?}` cancels the caller's own window (or the focused output when no label). A global cap of 2 concurrent answers: a third waits with `query:stage` `Waiting for your other answer to finish` and starts when a slot frees. `submit_to` returning early (no output window) clears its own busy entry. The stamped `gen` stays per window.
- **Reconciled:** U4 allows two concurrent and parity-features asks for per-label state; review-core P2-7's atomic replace is the per-label insert. Decision D-15.
- **Acceptance:** UNIT `host::ask::tests::second_window_does_not_abort_first`; UNIT `host::ask::tests::third_query_waits_for_a_slot`; SMOKE+ `perWindowAsk`, SMOKE+ `concurrentCap`.

### B-041 · P1 · llm · A stalled connection fails over instead of spinning forever
- **Sources:** review-llm-agent P1-3.
- **Files:** `src-tauri/src/llm/client.rs`, `src-tauri/src/llm/sse.rs`, `src-tauri/src/llm/wire.rs`, `src-tauri/src/find.rs`.
- **Fix:** Client `connect_timeout(10 s)` and `tcp_keepalive(30 s)`. An idle timeout inside `next_chunk`: 45 s with no bytes at all (keep-alive comments count) returns `ApiError::http(408, ..)` so `with_failover` moves on. Non-streamed calls get an overall cap of 90 s.
- **Acceptance:** UNIT `llm::client::tests::idle_stream_times_out_as_408` (local server that sends headers then stalls; tokio time paused); SMOKE+ `timeoutFailsOver` (fake LLM with a stalled first model).

### B-042 · P1 · llm · Failover never hands the retry a retired or blind model, and its copy only claims what happened
- **Sources:** review-llm-agent P1-4, P2-11; parity-features #3 (FAIL-05, AGENT-09, polish 2559dc7).
- **Files:** `src-tauri/src/llm/catalogue.rs`, `src-tauri/src/llm/failover.rs`, `src-tauri/src/llm/errors.rs`, `src-tauri/src/host/ask.rs`, `tools/fixtures/llm/errors.json`, `tools/fixtures/llm/catalogue.json`.
- **Fix:** `next_free_model`: among untried free curated ids prefer one present in the live catalogue and `usable_for(kind)` (Vision: accepts images; Agent: supports tools); a cold cache means curated order. Keep a per-session bad-model set (ids that returned 404) and skip them as first attempt and as stand-ins. Stamp `attempts` on the returned `ApiError` (K-17). Copy claims stand-ins only when `attempts > 1` (the polish-loop `{alsoTried}` rule), with the final wording from B-119. "Answering directly" after a reader stand-in succeeded reuses the model that last answered.
- **Reconciled:** review-llm (filter by `usable_for`) and parity-features (prefer live, else first free) are combined; the bad-model set is review-llm P2-11.
- **Acceptance:** UNIT `catalogue::tests::stand_in_is_live_and_sees_images`; UNIT `failover::tests::bad_model_set_is_skipped`; UNIT `errors::tests::single_attempt_copy_claims_no_stand_in`; SMOKE `failoverRateLimit`, `failoverCapped`, SMOKE+ `failoverStandInLive`.

### B-043 · P1 · llm · The daily free cap and empty quotas say what fixes them, and never burn more requests in the same account
- **Sources:** review-llm-agent P1-5.
- **Files:** `src-tauri/src/llm/errors.rs`, `src-tauri/src/llm/failover.rs`.
- **Fix:** In `message_for`, match the provider detail for `per-day`, `free-models-per-day`, `insufficient_quota`, `exceeded your current quota`, `RESOURCE_EXHAUSTED` and `quota`, and return code `daily-cap` / `no-credit` with text saying what fixes it (add credit, or wait until tomorrow). `should_failover` returns false for the account-wide daily cap inside the same provider. Cross-provider hops (B-046) still apply.
- **Reconciled:** P1-5 says "do not fail over" on the daily cap; design-ux §3.4 hops providers on 429. Both hold: no hop between free models of the same account, a hop to another provider is allowed under the paid-switch rule.
- **Acceptance:** UNIT `errors::tests::daily_cap_is_not_transient`; UNIT `failover::tests::daily_cap_stays_in_account`.

### B-044 · P1 · llm · Find's related and concept stages fail over like Ask
- **Sources:** review-llm-agent P1-7.
- **Files:** `src-tauri/src/find.rs`, `src-tauri/src/host/find.rs`.
- **Fix:** Run each stage through `with_failover(model, |t| llm.next_free_model(Kind::Find, t), || nothing_arrived, ..)` for `:free` ids, and `resolve_live` the Find model when Find opens. Native providers use their own model (B-036) with no failover.
- **Acceptance:** UNIT `find::tests::related_stage_fails_over_on_429`; SMOKE+ `findFailover`; SMOKE `findRelated`, `findConcept`.

### B-045 · S1 · llm · Every disclosure names who actually receives the screen
- **Sources:** design-ux U6 (Rust half); copy deck C-30, C-32, C-33.
- **Files:** `src-tauri/src/llm/mod.rs` (or `llm/settings.rs`), `src-tauri/src/host/mod.rs`.
- **Fix:** `llm::recipients(&settings) -> Vec<Recipient{id, name, model, modelName, free}>` (K-18): the ready providers in try order with the model each would use for Vision. Register `Hooks.overlay_extra` (K-07) returning `{recipients}` and add `recipients` to `settings_extra`. agent includes it in `agent:consent-request` (B-051).
- **Acceptance:** UNIT `llm::tests::recipients_follow_order_and_skip_unready`; SMOKE+ `recipients` (Anthropic-only profile: `overlay:state.recipients[0].id == "anthropic"`).

### B-046 · S1 · llm · Failover across providers, never to a paid account by surprise
- **Sources:** design-ux §3.4, U15 (cross-provider half); review-llm-agent P1-5 (interaction).
- **Files:** `src-tauri/src/host/ask.rs`, `src-tauri/src/llm/failover.rs`, `src-tauri/src/llm/settings.rs`.
- **Fix:** Candidates in order (B-037), skipping Not added, Key rejected, No credit (native), Busy until cooldown, and Free models off (OpenRouter unless a paid OpenRouter model is picked). Inside OpenRouter's free models the existing up-to-three failover. Hop to the next provider only before any answer text was shown, on 429, 5xx, 408, 404, 401, 402 or a timeout while online; never on 400, a refusal, Stop, after text started, or with a pinned model (B-122). Free to paid with `askBeforePaid` on (default): emit `query:error` with `code: "paid-hop"`, tone `note`, text C-63 (`OpenRouter's free models are busy. Ask <Provider> (<model name>) instead? <Provider> bills your account for it.`) and actions `[{id:"hop", label:"Ask <Provider>", provider, model}, {id:"wait", label:"Keep waiting"}, {id:"never-ask", label:"Don't ask again"}]`. While switching, stage `OpenRouter is busy. Asking <Provider>…`; after, `query:done.usage.via` = `<Provider>` and `usage.hopReason`. Cooldowns: 429 marks Busy for Retry-After or 60 s; 401 marks Key rejected until the key changes or a test passes; 402 marks No credit and re-tests silently after 10 min; each change emits `provider:status`. A newly rejected key raises one bar notice `Anthropic rejected your key, so answers come from OpenRouter for now.` with action `Fix key`.
- **Reconciled:** review-llm-agent's "with_failover never crosses providers" was a correctness check of today's code; design-ux §3.4 makes crossing the product behaviour with the paid guard. Decision D-14.
- **Acceptance:** UNIT `host::ask::tests::hop_only_before_text`; UNIT `host::ask::tests::free_to_paid_asks_first`; UNIT `llm::settings::tests::cooldowns`; SMOKE+ `crossProviderHop` (fake LLM: OpenRouter 429 x3, Anthropic ready with `askBeforePaid=false` → answer via Anthropic).

### B-047 · P1 · agent · The agent uses native model ids, picks its provider at start, and honours a pinned model
- **Sources:** review-llm-agent P1-1 (agent half); parity-features #1 (AGENT-15); design-ux U20 (agent half), §3.4.
- **Files:** `src-tauri/src/host/agent.rs`, `src-tauri/src/agent/run.rs`.
- **Fix:** `start_run` picks the first candidate (K-16) once: native → `model_for(settings, provider, Kind::Agent)`; OpenRouter → `resolve_live`; never `heal_agent_model` a native id. The run keeps that provider for its whole life (no mid-run switch). `agent:start` and `agent:do-it` accept optional `{provider, model}`; a pinned model never fails over. The trail opens in `outputs::fresh` (K-04).
- **Acceptance:** UNIT `host::agent::tests::native_only_run_uses_native_model`; UNIT `host::agent::tests::pinned_model_never_fails_over`; SMOKE+ `agentNativeModel`.

### B-048 · P1 · agent · Taking over the mouse stops the step that was about to fire
- **Sources:** review-llm-agent P1-2.
- **Files:** `src-tauri/src/agent/run.rs`.
- **Fix:** Immediately before `acting = true`, and between calls in the per-turn loop, if `paused` then `wait_while_paused().await`; on resume re-run `inspect` and the gate (the screen may have changed), or return `Outcome::text("Not executed: the user took over.")` and drop the turn's remaining calls.
- **Acceptance:** UNIT `run::tests::pause_during_dodge_drops_the_action`; SMOKE `agentTakeover`, SMOKE+ `takeoverStopsStep` (fake LLM returns two calls; pause after the gate; dispatched count stays 0).

### B-049 · P1 · agent · The safety gate is a real allowlist
- **Sources:** review-llm-agent P1-6.
- **Files:** `src-tauri/src/agent/gate.rs`, `src-tauri/src/agent/run.rs`.
- **Fix:** No card only for known-harmless roles (menubar and menu items without risky verbs, tabs, tree items, scroll bars, text fields, documents) or a small harmless-name list; any other clickable without a harmless reason asks. `double_click` on list or tree items in `explorer` or the desktop is treated as "opens or runs the file" and asks. Read modal text for any window or tree node with role `dialog`, not only class `#32770`. Add the missing verbs (shut down, restart, trash, clear all, clear history, unsubscribe, cancel subscription, deactivate, close account, end task) with Unicode word boundaries, plus localized equivalents for the shipped UI languages (de, fr, es, it, pt, ja, zh: send, delete, buy, pay, post, remove). Fix the module doc so it describes what the code does. Update the smoke-v21 gate tables (`gateBypasses`, `gatePassesPlainInput`, `gateFailsClosed`, `gatePlanFirst`, `gateChords`) to the new verdicts.
- **Reconciled:** AGENT-05's verdict tables change; PARITY records the stricter gate. Decision D-27.
- **Acceptance:** UNIT `gate::tests::unknown_named_button_asks`; UNIT `gate::tests::explorer_double_click_asks`; UNIT `gate::tests::web_dialog_text_is_read`; UNIT `gate::tests::localized_send_asks`; SMOKE `gateBypasses`, `gateFailsClosed`.

### B-050 · P1 · agent · A timed-out helper request can never act later, and Stop cancels typing in progress
- **Sources:** review-desktop P1-1; review-llm-agent P2-1.
- **Files:** `lib/desktop/host.cs`, `src-tauri/src/desktop/client.rs`, `src-tauri/src/desktop/protocol.rs`, `src-tauri/src/agent/run.rs`.
- **Fix:** Give each helper request a cancellation flag; `Handle` sets it on timeout, and every input primitive (`SendInputs`, `SetCursorPos`, `MoveTo`, `SetFocus`, pattern `Invoke`/`SetValue`/`Toggle`, `FocusWindow`) checks it before acting; `TypeInto` and `KeyCmd` check it per chunk. Add a `cancel` op that sets the flag of every in-flight request. Rust: treat TIMEOUT on any input op as an unknown outcome (re-read state before any retry), size `key`'s timeout from repeat x chords as `type` does, and check `ended()` immediately before every desktop op that acts (after `window_set().await`, after a STALE_REF re-read, between the two ops of `focus_window` by title). Esc sends `cancel`.
- **Acceptance:** UNIT `client::tests::key_timeout_scales_with_repeat`; SMOKE+ `cancelStopsTyping` in `selftest/probes/agent.rs` (type 2000 characters into Notepad through `desktop::client`, send `cancel` after 200 ms; text length < 2000 and stable 500 ms later; CI has Notepad); SMOKE+ `helperTimeoutNoLateInput`, SMOKE `agentStopMs`.

### B-051 · S1 · agent · The consent request names the real recipient
- **Sources:** design-ux U6 (agent half), C-30.
- **Files:** `src-tauri/src/host/agent.rs`.
- **Fix:** `agent:consent-request` gains `recipients` (K-18, from `llm::recipients`) and `provider`/`model`/`modelName` of the candidate the run will use (K-21). The renderer builds C-30 from it (B-171).
- **Acceptance:** UNIT `host::agent::tests::consent_payload_names_provider`.

### B-052 · P1 · harness · Install, smoke and photograph the Tauri installer on a runner
- **Sources:** parity-ipc B5, SHOT-06, PKG-03..07, UPD-15 (verify), §3.1; memory "test in the cloud".
- **Files:** `.github/workflows/rust.yml` (new job `installer`, needs `build`), `tools/installer-check.ps1`.
- **Fix:** A job that downloads the `aevalbuddy-tauri-nsis` artifact, reads the version from package.json, and runs `tools/installer-check.ps1 -Installer <setup.exe> -Version <v> -Out out` (all Checks: install.*, smoke.*, shots.*, shots125.*, shots150.*, uninstall.*). Add an "Electron upgrade" phase for B-016: install Electron v1.1.1 from the GitHub release, then the Tauri installer `/S`, then print `[INSTALL] electronRemoved=ok|FAIL` and `[INSTALL] uninstallEntries=<n>` (`reg query` of HKCU Uninstall for `AevalBuddy*`). Print `[INSTALL] launchAtLogin=<Run value>` and `[INSTALL] startupApproved=<hex>` after toggling via the smoke (B-081). Publish the installed exe's shots to the `shots` branch under `rust-exe/<ref-with-dashes>/latest/` (newest 15) and the logs through ci-publish. The job's results land under `ci-logs:<ref>/<sha>/rust/` like the others; until every lane's probes exist the job fails only on install, uninstall and shots exit codes (it never ignores a Check it runs).
- **Acceptance:** rust ci-logs `installer.log` shows every installer-check Check `=ok`, `[INSTALL] electronRemoved=ok`, `[INSTALL] uninstallEntries=1`; PNGs present on `origin/shots:rust-exe/<ref>/latest/`.

### B-053 · S1 · harness · ui-shots: per-lane scene files, both scales, real backdrops, a virtual clock, the shipped default width
- **Sources:** design-visual §4 (general changes), F27 (harness half); design-ux U8 (200% set); parity-features #14.
- **Files:** `tools/ui-shots/shots.mjs`, `tools/ui-shots/mock-bridge.js`, `tools/ui-shots/bridge.test.mjs`, `lib/backdrop-fixture.html`, `.github/workflows/ui-shots.yml`; creates `tools/ui-shots/lanes/<lane>.mjs` (one per lane, then owned by that lane).
- **Fix:** In harness C1 (K-31): each lane file exports `{ scenes, mocks, checks, cssLintEnforce }` (document the exact shape in a header comment of `shots.mjs`); `shots.mjs`, `mock-bridge.js` and `bridge.test.mjs` import all nine. Run every scene at `deviceScaleFactor` 1 and 1.5 (suffix `-150`), plus a `--text-scale: 2` variant for scenes flagged `textScale: true`. Add backdrops `dark-editor` (VS Code-like syntax-coloured page) and `photo-busy` (high-frequency edges, text, grids) to `lib/backdrop-fixture.html`. A virtual clock: stub `performance.now` and `requestAnimationFrame` and step them, so glob frames show real timing. Shoot the shipped default bar width (600 after B-087/B-106) as well as 640. Publish to `shots:rust-d-ui/<ref-with-dashes>/latest/` so parallel lanes do not overwrite each other, retrying the push on a race.
- **Acceptance:** ui-shots ci-logs `shots.log` lists scenes from every lane file at both scales; `origin/shots:rust-d-ui/<ref>/latest/` holds `*-150.png` and `dark-editor`/`photo-busy` variants.

### B-054 · S1 · bar · One token system and the shared components every page uses
- **Sources:** design-visual §3 (type, spacing, radius, shadows, motion), F7 (component), F20, F21 (component), F30 (tokens), F34, F35, F37 (tokens), F40, F5 (frame.js); design-ux U9, U26, U8 (CSS half); parity-features #19 (token pass).
- **Files:** `renderer/glass.css`, `renderer/glass.js`, `renderer/frame.js`, `renderer/icons.svg` (new), `renderer/index.html`, `renderer/overlay.css` (bar regions).
- **Fix:** In bar C1 (K-40, K-41): add design-visual §3's tokens to `glass.css :root` verbatim (type `--t-*` with `--font-small`, `--w-body: 450`, `--w-strong: 600`; spacing `--sp-*`, `--stage`, `--inset-*`; radius `--r-sheet 28 / --r-card 16 / --r-plate 12 / --r-row 8 / --r-key 6 / --r-cap 999`; `--shadow-control`, `--shadow-sheet`, `--shadow-menu`; motion `--ease-*`, `--d-*`), every type size as `calc(Npx * var(--text-scale))` with `--text-scale: 1`, and `--ctl`, `--ctl-s`, `--bar-h`, `--sheet-bar-h` scaled too. `--accent-text: #EBC0E9` (dark tone; `#823A80` in the light tone, B-159) for every accent-coloured text and icon on glass; `--accent-bright` stays for fills, rims and the lens ring. Components: `.sheet-bar` (44 px, padding 0 8 0 16, title `600 13px/18px`, tools `.g-icon.small` 28 px gap 4, last button 8 px from the glass), `.g-menu` (`glass thin`, r16, padding 4, `--shadow-menu`, rows min-height 36 padding 0 12 r12, hover `--fill-1`, selected `--fill-2` plus the shared check, entrance `drop-in 220ms var(--ease-morph)`, exit 120 ms fade), `renderer/icons.svg` sprite (one X, chevron, check, copy, gear, history, plus, stop, clock, bubble, viewfinder, pointer; 20 px grid; `stroke-width: 1.5` with `vector-effect: non-scaling-stroke`, or Fluent System Icons Regular filled paths; sizes 18 in 32 px buttons, 16 in 28, 14 inline). Focus ring: `outline: 2px solid rgb(255 255 255 / .92); outline-offset: 2px` plus `box-shadow: var(--own-shadow, none), 0 0 0 5px rgb(0 0 0 / .38)`; the bar's `#bar:focus-within > .gl` gets the U26 double ring. `frame.js` sets `html[data-maximized]` from `getCurrentWindow().onResized` + `isMaximized()` and hides its handles when maximized. The intent menu adopts `.g-menu`. Record the radius rule "28/16/12/8/6/capsule, inner = outer − inset" and the palette entry for `--accent-text` in `entries/bar.md`.
- **Acceptance:** check `tokensPresent` (every §3 token resolves in a page) and `accentTextContrast` (≥ 4.5:1 over the matrix worst case) in `tools/ui-shots/lanes/bar.mjs`; scenes `bar-focus-ring`, `bar-menu`; SMOKE `styleSamples`.

### B-055 · S1 · bar · "Very very" reads liquid while text stays legible
- **Sources:** design-visual F6, F11; design-ux U51 (measure half).
- **Files:** `renderer/glass.css`, `renderer/glass.js`.
- **Fix:** Implement design-visual F6 steps 1-6 and F11 exactly: edge-clear scrim (`--scrim-edge: calc(var(--scrim) - 0.26 * var(--spec))`, interior difference drawn in `.gl::before` inset by half the band); blur held for reading (thin `18px - 8px*k`, thick `32px - 8px*k`, saturation per F6.2 with F11's thin `1.1 + 0.9k`); lensing `band = round(4 + 20k + 8*spec)`, `scale = round(base*2k + 12*spec)`, `spec = max(0, 2k-1)`; thickness shade and rim per F6.4; `morph(200)` on window move; scrim slope doubled over dark backdrops; thin scrim `0.58 + 0.24y`. `measure()` uses the 98th percentile luma when k > 0.5. Record the level table (design-visual §3 "Glass levels") in `entries/bar.md` as the DESIGN Material update. `refract` on Settings/History `#app` is the settings lane's markup (B-073).
- **Acceptance:** scenes `glass-level-{0,25,50,75,100}` on wallpaper, white, dark-editor and photo-busy (design-visual §4 state 45, bar half); check `glassStopsDiffer` (pixel difference between 0.5 and 1.0 frames over photo-busy above a threshold); SMOKE `styleStable`; harness matrix stays `failedFrames=0`.

### B-056 · S1 · bar · A question can span several lines
- **Sources:** parity-features #10 (polish e51ca29).
- **Files:** `renderer/index.html`, `renderer/bar.js`, `renderer/overlay.js`, `renderer/overlay.css` (bar regions).
- **Fix:** `#query-input` becomes a `<textarea rows="1">`; Enter sends, Shift+Enter inserts a newline, Alt+Enter and Ctrl+Enter keep their meanings; auto-grow to 7 rows (INPUT_MAX_HEIGHT 280 already in layout.rs) and report height through the existing overlay resize path; Up/Down recall only when the caret is on the first/last line.
- **Acceptance:** SMOKE+ `barMultiline` (bar probe pastes three lines via `Cx::input`, `query:submit` text contains two `\n`); scene `bar-multiline`; SMOKE `barKeys`.

### B-057 · S1 · bar · The bar shows where the next question goes
- **Sources:** design-ux U5 (bar half), C-14.
- **Files:** `renderer/index.html`, `renderer/bar.js`, `renderer/overlay.css` (bar regions).
- **Fix:** When `overlay:state.visibleOutputs >= 2` (K-05), show `#target-chip` between the field and the area button: `Continues: <title>` (28 characters, ellipsis, full title in `title`) and a 24 px x button `aria-label="Start a new conversation instead"` that sends `chat:new`. One window: no chip. The input's `aria-describedby` gains `Continues the conversation "<title>".` Ctrl+Tab / Ctrl+Shift+Tab invoke `output:cycle-target`.
- **Acceptance:** scene `bar-target-chip`; check `targetChipOnlyWithTwoWindows`.

### B-058 · S1 · bar · Announcements are heard wherever focus is
- **Sources:** design-ux U7 (renderer half).
- **Files:** `renderer/glass.js`, `renderer/index.html`, `renderer/overlay.js`.
- **Fix:** `glass.js` exports `AevalA11y.say(text, priority)` (K-42): speaks into the page's own live region when `document.hasFocus()`, else invokes `a11y:announce`; every page listens to `a11y:say` and speaks it locally. The bar's `#announce` gets a second, assertive region (`role="alert"`, `aria-live="assertive"`) for errors and task questions.
- **Acceptance:** check `sayRoutesByFocus` (mock: unfocused page invokes `a11y:announce`); SMOKE+ `a11yRelay` (shared with windows).

### B-059 · S1 · bar · Every page applies the text scale and handles Ctrl+=, Ctrl+- and Ctrl+0
- **Sources:** design-ux U8 (renderer half).
- **Files:** `renderer/glass.js`.
- **Fix:** On `ui:text-scale {scale}` set `--text-scale` on `:root`. Handle Ctrl+= / Ctrl+- / Ctrl+0 in a `keydown` listener (pages see keys before the bridge blocks browser zoom): step the stored factor by 0.1 within 1.0..2.0 and invoke `settings:set-text-scale`. glass.js is loaded by every page, so outputs, Settings, History, lens and buddy follow.
- **Acceptance:** scenes flagged `textScale: true` render at 200% (harness variant); check `zoomKeysInvokeTextScale`.

### B-060 · S1 · bar · Esc in the bar clears text, then closes the bar, and leaves outputs alone
- **Sources:** design-ux U3 (bar half), U19 (Esc row).
- **Files:** `renderer/bar.js`, `renderer/overlay.js`.
- **Fix:** Esc with text clears it; Esc on an empty bar sends `overlay:collapse` (which no longer aborts or hides outputs, B-018). Tooltip and name of the close button: `Close the bar` / `Close (Esc)` (C-13).
- **Acceptance:** check `escClearsThenCloses`; SMOKE+ `escKeepsAnswer` (shared with windows).

### B-061 · S1 · sheet · The glob never hides its own words, and they arrive inside the glass
- **Sources:** design-visual F1.
- **Files:** `renderer/glob.js`, `renderer/output.html`, `renderer/overlay.css` (answer sheet region).
- **Fix:** Implement F1 steps 1-5 exactly: `#glob { z-index: 0 }` with `#opanel.globbing > :not(.gl) { position: relative; z-index: 1 }`; clip the words to the shape while globbing (`sheet.style.clipPath = path(<d translated by -sheetRect.left,-top>)`, `pt()` gains an offset; cleared in `finish()`); reveal at T_REVEAL 0.70 with the F1.3 CSS; close adds `closing` 80 ms before the shape reverses (F1.4); reduced motion keeps the 150 ms fade.
- **Acceptance:** virtual-clock scenes `glob-open-{0,4,10,18,30,45,62,72,82,90,100}` (§4 state 36); check `globWordsNeverOutsideShape` (pixels outside the shape mask are wallpaper in every frame); SMOKE+ `globMorphMs`.

### B-062 · S1 · sheet · The glob reads as one liquid body, and falls back cleanly when the sheet is not beside the bar
- **Sources:** design-visual F2, F25; parity-features #17 (NEW-05, renderer half).
- **Files:** `renderer/glob.js`, `renderer/output.html`, `renderer/overlay.css` (answer sheet region).
- **Fix:** Rewrite `shapeAt()` to F2's timeline table exactly (D_OPEN 480, D_CLOSE 260, `easeSpring = bezier(0.2, 0.9, 0.25, 1)`, phases A drop / B stretch-and-pinch with horizontal tangents and OVERLAP 6 / C recoil / Land / Words at 0.70). Add `#glob-thin` (`glass thin`, clipped by the same path, opacity 1→0 over t 0.10→0.55, `--scrim` from `AevalGlass.scrimFor(barRect, 'thin')`, K-40); the `#glob-mask` black rect over the bar plus 1 px; rim `stroke-width: calc(2 * var(--px))`; `#glob { filter: drop-shadow(0 4px 12px rgb(0 0 0 / .28)) }`. Fallback (F25): when the bar-to-sheet gap exceeds 14 px or they do not overlap on the cross axis, skip the shape and scale the sheet 0.94→1 toward the bar over 280 ms `--ease-morph` with the F1 reveal; close mirrors in 160 ms. Keep `globSmooth < 34 ms`.
- **Acceptance:** check `globBudArea` (at t = 0.04 the shape area is < 8% of the sheet's); scenes `glob-close-{0,25,50,75,100}`, `glob-above-bar`, `glob-side`, `glob-cascaded`, `glob-seam-150` (§4 states 37-40); check `globSmooth`.

### B-063 · S1 · sheet · Answers render as a typeset, accessible document
- **Sources:** design-visual F3; design-ux U11.
- **Files:** `renderer/markdown.js`, `renderer/output.js`, `renderer/overlay.css` (answers region).
- **Fix:** Lists stay open across blank lines when the next non-blank line is an item of the same type; `<ol start="N">`; nesting by `floor(leadingSpaces/2)` inside the current `<li>`; consecutive text lines join into one paragraph; tables (header `^\|.*\|$` plus separator `^\|?\s*:?-{3,}`) with alignment and `inline()` per cell inside `<div class="md-table"><table>`, with F3's table CSS; nested list CSS. Semantics: `#` → `<h2>`, `##` → `<h3>`, `###`+ → `<h4>` (classes `md-h md-hN` kept), paragraphs `<p class="md-p">`, fences `<pre><code>`, links `<a href="URL" data-href="URL" title="URL">label</a>` with ` <span class="md-host">(host)</span>` in `--label-2` 13 px when the label is not the host (click stays intercepted).
- **Acceptance:** scene `sheet-markdown-fixture` (§4 state 20: h1-h3, loose and tight ordered lists, nested lists, table, long code, inline code by punctuation, blockquote, hr, links) at 1 and 1.5; checks `looseListIsOneOl`, `tableRenders`, `headingsAreHeadings`, `linksShowHost`; SMOKE `markdown`, SMOKE+ `markdownStructure`.

### B-064 · S1 · sheet · A wide or maximized answer keeps a 640 px reading measure
- **Sources:** design-visual F4.
- **Files:** `renderer/overlay.css` (answer sheet and find regions).
- **Fix:** Delete the `padding: 2px 2px 2px 0` shorthand that cancels the measure; `--measure: 640px`; `#chat { padding-block: 2px; padding-inline-start: max(0px, calc((100% - var(--measure)) / 2)); padding-inline-end: max(8px, calc((100% - var(--measure)) / 2)); }`; `pre` and `.md-table` may use the full measure; same measure for `#find-view`.
- **Reconciled:** DESIGN says 760; 640 is about 68 characters at 15/24 Segoe UI Variable Text. Decision D-05.
- **Acceptance:** scene `sheet-maximized-1600` (§4 state 21); check `readingMeasure640` (chat text column width ≤ 640 at 1600 px); SMOKE+ `readingMeasure`.

### B-065 · S1 · sheet · One title bar: the shared sheet bar, a glyph for the kind, tools that appear when wanted
- **Sources:** design-visual F7 (outputs), F18, F20 (adoption), F42 (sheet bar light); design-ux U9 (`#copy-all[data-done]`).
- **Files:** `renderer/output.html`, `renderer/output.js`, `renderer/overlay.css` (answer sheet region).
- **Fix:** Use `.sheet-bar` (K-40) in output.html. Replace `.sheet-kind` text with the bar token's glyph (bubble, viewfinder, pointer from `icons.svg`) at 16 px in `--label-2`, keeping the word in `aria-label`. At rest show only Close; Copy, History and Start over fade in on `#sheet-bar:hover, :focus-within, body[data-scrolled="1"]` (opacity 0→1, 150 ms) keeping their slots. Hide the title-bar Copy while the conversation has one answer. All glyphs from the sprite. `class="interactive"` on `#sheet-bar` (0.06 strength). `#copy-all[data-done]` uses `--accent-text`.
- **Acceptance:** scenes `sheet-title-rest`, `sheet-title-hover` (§4 state 22 half); check `oneTitleBarSystem` (output, settings and history title bars share height and font).

### B-066 · S1 · sheet · Nothing in an output says the same thing twice
- **Sources:** design-visual F8 (renderer half); design-ux U9 (`.trow-note.waiting`).
- **Files:** `renderer/output.js`, `renderer/trail.js`, `renderer/overlay.css` (answer sheet and agent trail regions).
- **Fix:** First user message of a conversation gets `is-title` and is `.sr-only` (follow-ups show the chip; copy-all includes it). Drop the trail `Task` title span. Trail head status while asking: `Step N`. Row note shortened to `Answer in the bar`, colour `--accent-text`. When a failed row already shows the reason, the banner shows only the remedy (Rust side B-143).
- **Acceptance:** scenes `sheet-first-answer`, `sheet-task-asking`, `sheet-task-failed`; check `questionNotShownTwice`.

### B-067 · S1 · sheet · Esc in an output moves focus, never closes or stops anything
- **Sources:** design-ux U3 (output half).
- **Files:** `renderer/output.js`.
- **Fix:** Esc in an output: if the bar is open invoke `ui:focus-bar`, else `output:blur` (B-018). Remove the `overlay:collapse` call at output.js:574-576.
- **Acceptance:** check `outputEscNeverCollapses`; SMOKE+ `escKeepsAnswer`.

### B-068 · S1 · sheet · Outputs announce through the relay, briefly
- **Sources:** design-ux U7 (output half), C-37.
- **Files:** `renderer/output.js`, `renderer/trail.js`.
- **Fix:** All announcements go through `AevalA11y.say` (K-42). Answer done: `Answer ready. <first sentence, at most 160 characters>. Press F6 to read it.` (polite). Errors: the error text (assertive). Task asks: `The task has a question: <question>. Type your answer in the bar.` (assertive).
- **Acceptance:** check `answerAnnounceIsShort` (mock: `a11y:announce` text ≤ 220 characters, starts `Answer ready.`).

### B-069 · S1 · settings · A Providers page that holds, shows and orders every key
- **Sources:** design-ux U2, §3.1-3.6, U1 (renderer half), U21 (renderer half); copy deck C-50, C-51, C-57, C-58, C-65; parity-features #2.
- **Files:** `settings/settings.html`, `settings/settings.js`, `settings/settings.css`.
- **Fix:** Rail `Account` → `Providers`, page `#pg-providers`, `h1 Providers`. Build design-ux §3.1's layout and §3.2's state table verbatim (every state's status word, body text and actions; copy from `.claude/rust-rewrite/audit/design-ux.md` §3.2), §3.3's ordering (drag, `Move up`/`Move down`/`Use first`, Alt+Up/Down; announce `<Provider> is now first. It answers by default.`), the two failover switches (`When one is busy or fails, try the next`; `Ask me before switching from free models to a paid provider`) and the storage line (C-58, from `keyStorage`). OpenRouter's Not-added state leads with `Connect OpenRouter` (`auth:openrouter-connect`) and a `Paste a key instead` link. Channels and payloads: K-15/K-19 (`settings:save-key {provider,key}`, `clear-key {provider}`, `test-key {provider}`, `set-provider-order`, `set-failover`, `clip-key`, `provider:status`, `settings:get.providers/failover/keyHints/keyStorage`). §3.6 accessibility: `<ol id="providers" aria-label="Providers, in the order AevalBuddy tries them">`, row button names `OpenRouter, 1 of 4, Ready, default`, `aria-expanded`/`aria-controls`, visible field labels, focus returns to the row after Remove, Replace or a move; one `role="status"` line speaks status changes.
- **Acceptance:** scenes `settings-providers-{none,ready-two,rejected,no-credit,busy,blocked,testing,removing,wrong-provider}` (§4 state 42 expanded); checks `providerRowsHaveExpandedState`, `removeSendsProvider`, `reorderInvokesOrder`.

### B-070 · S1 · settings · Settings rows are real buttons with state, and actions look like actions
- **Sources:** design-ux U13; design-visual F23.
- **Files:** `settings/settings.html`, `settings/settings.js`, `settings/settings.css`.
- **Fix:** Each `.body` gets an id; the whole `.row-head` is the disclosure button (role and handler on the row, `.edit::before { content:""; position:absolute; inset:0 }` on a `position:relative` row), `aria-expanded`, `aria-controls`, `aria-labelledby="<h2 id> <summary id>"` (reads "API key, Connected, collapsed"); hover `--fill-1` with radii by position. Action rows get a trailing text button instead of a chevron (`Move…` for bar position, `Replay` for the tour) with value summaries (`Top right`, `4 steps`). Disclosure animates with `grid-template-rows: 0fr → 1fr` over `--d-med var(--ease-morph)`.
- **Acceptance:** checks `noButtonNamedEdit`, `everyDisclosureHasExpanded`; scene `settings-row-hover-disclosing` (§4 state 48).

### B-071 · S1 · settings · History rows are two separate buttons, the delete target is 28 px, and deleting can be undone
- **Sources:** design-ux U12, C-81.
- **Files:** `history/history.html`, `history/history.js`, `history/history.css`.
- **Fix:** Row markup `<div class="item" role="listitem"><button class="item-open">…</button><button class="del" aria-label='Delete "<title>"'>…</button></div>` inside `role="list"`; `.del` 28 x 28, visible at rest in `--label-3`, `--label` on hover and focus. Delete hides the row and shows a footer note `Deleted "<title>". Undo` for 6 s; `history:remove` is sent when the note expires or on `pagehide`. The Delete key on a row deletes with the same undo.
- **Acceptance:** checks `noNestedButtons`, `deleteIsUndoable`; scene `history-row-hover`, `history-deleted-undo`.

### B-072 · S1 · settings · The glass slider's labels sit on their stops, with ticks and a magnetic snap
- **Sources:** design-visual F9.
- **Files:** `settings/settings.css`, `settings/settings.js`.
- **Fix:** F9 exactly: absolute-positioned stop labels at `left: calc(11px + (100% - 22px) * n/4)` with `translateX(-50%)` (ends flush), track ticks via the radial-gradient background, snap to the nearest 0.25 within 0.04 on `change`, current stop label `--label`, others `--label-2`.
- **Acceptance:** check `sliderLabelsCentredOnStops` (each label's centre within 1 px of its stop); scenes `settings-glass-{0,25,50,75,100}`.

### B-073 · S1 · settings · Settings and History share the sheet bar, sit on the stage margin, refract, and go full bleed when maximized
- **Sources:** design-visual F7 (Settings/History), F24 (CSS half), F5 (renderer half), F6.5 (`refract`), F20 (adoption).
- **Files:** `settings/settings.html`, `settings/settings.css`, `history/history.html`, `history/history.css`.
- **Fix:** Use `.sheet-bar` (left-aligned title, 28 px close from the sprite; drop `justify-content: center` and the absolutely positioned close). `body { padding: var(--stage) }` on both pages (the native band moves to the glass with B-024). `class="glass thick refract"` on `#app`. `html[data-maximized] body { padding: 0 } html[data-maximized] #app { --r: 0px; border-radius: 0; }` (K-41).
- **Acceptance:** scenes `settings-maximized`, `history-maximized` (§4 states 47, 50); check `oneTitleBarSystem` (shared with sheet).

### B-074 · S1 · settings · A Text size row
- **Sources:** design-ux U8 (Settings half), C-59.
- **Files:** `settings/settings.html`, `settings/settings.js`.
- **Fix:** Settings, General: `Text size` slider 100% to 200% step 10 with the note `Starts from your Windows text size.`; reads `textScale` and `osTextScale` from `settings:get`, writes `settings:set-text-scale` (K-22).
- **Acceptance:** scene `settings-text-size`; check `textSizeRowInvokes`.

### B-075 · S1 · settings · The privacy and task copy says who receives the screen and what the mouse does
- **Sources:** design-ux U6 (Settings half), U31 (Settings half); copy deck C-32, C-52.
- **Files:** `settings/settings.html`, `settings/settings.js`.
- **Fix:** The Privacy "Who sees your screen" paragraph is generated from `settings:get.recipients` (K-18) with C-32's text. The Tasks note becomes C-52 (`Esc, the Stop button or the tray stops it; moving the mouse pauses it. Screenshots are never saved.`).
- **Acceptance:** check `privacyNamesRecipients` (mock Anthropic-first: paragraph names Anthropic before OpenRouter).

## Items: S2 and P2

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

### B-108 · P2 · capture · Failed DXGI opens back off, and outputs share a device per adapter
- **Sources:** review-desktop P2-1.
- **Files:** `src-tauri/src/capture/dxgi.rs`.
- **Fix:** Cache open failures per device name with a 10 s backoff (cleared on WM_DISPLAYCHANGE) and go straight to GDI during it; share one D3D11 device per adapter LUID.
- **Acceptance:** UNIT `dxgi::tests::open_failure_backs_off`; capture check `[CAPTURE] dxgiOpensPerTick=<n>` must ≤ 1 per 10 s when forced to fail.

### B-109 · P2 · capture · HDR desktops are captured with correct SDR tone
- **Sources:** review-desktop P2-2.
- **Files:** `src-tauri/src/capture/dxgi.rs`.
- **Fix:** Detect HDR outputs (`IDXGIOutput6::GetDesc1` colour space `G2084`); for them use `IDXGIOutput5::DuplicateOutput1` with `R16G16B16A16_FLOAT`, divide by the SDR white level (`DISPLAYCONFIG_SDR_WHITE_LEVEL`), clamp and sRGB-encode. SDR outputs keep the current path. A runner cannot show HDR: log `[CAPTURE] hdr=off|on` and unit-test the conversion; record "verified on hardware: no" in the lane's Blockers if no HDR check is possible.
- **Acceptance:** UNIT `dxgi::tests::scrgb_to_srgb_at_sdr_white`; capture check prints `[CAPTURE] hdr=off`.

### B-110 · P2 · capture · A hung capture thread cannot hang every later capture
- **Sources:** review-desktop P2-3.
- **Files:** `src-tauri/src/capture.rs`.
- **Fix:** `recv_timeout(3 s)`; on timeout swap in a fresh worker (replace the sender in `worker()`) and return Err so Ask's text-only fallback (CAP-06) runs.
- **Acceptance:** UNIT `capture::tests::hung_worker_is_replaced`; SMOKE+ `captureFailNull`.

### B-111 · P2 · capture · A panic in a backdrop tick never freezes the glass; recovery rebroadcasts
- **Sources:** review-desktop P2-4; parity-features "Low" (GLASS-02 recovery).
- **Files:** `src-tauri/src/backdrop.rs`.
- **Fix:** Reset `busy` with a drop guard; `run_loop` calls `schedule(None)` after a caught panic. After a capture-failed solid tier, retry every 30 s (not at fast pace) and broadcast `glass:tier live` when a capture succeeds again.
- **Acceptance:** UNIT `backdrop::tests::busy_resets_after_panic`; UNIT `backdrop::tests::recovery_rebroadcasts_live`; SMOKE `glassTier`.

### B-112 · P2 · capture · Screen frames are readable only by the app's own origin
- **Sources:** review-desktop P2-13.
- **Files:** `src-tauri/src/backdrop.rs`.
- **Fix:** Send `Access-Control-Allow-Origin: http://tauri.localhost` and refuse requests whose `Origin` is another origin.
- **Acceptance:** UNIT `backdrop::tests::frame_protocol_cors_is_app_only`; SMOKE `glassLoaded`.

### B-113 · P2 · capture · A window never receives another display's frame
- **Sources:** review-desktop P2-15.
- **Files:** `src-tauri/src/backdrop.rs`.
- **Fix:** When a window's display has no frame this tick, skip that window instead of sending `frames.keys().next()` with the wrong bounds.
- **Acceptance:** UNIT `backdrop::tests::missing_display_frame_skips_window`.

### B-114 · P2 · capture · Pages capture builds are crash-guarded, and a dead buddy page stops eating clicks
- **Sources:** review-desktop P2-16; parity-core LIFE-12 (split-c builders).
- **Files:** `src-tauri/src/buddy.rs`, `src-tauri/src/region.rs`, `src-tauri/src/lens.rs`.
- **Fix:** Call `crash::guard_webview` (K-02) from the region, lens and buddy builders. On buddy page load (`on_page_load` Finished) and on the ProcessFailed path, `set_click_through(true)`. Watchdog: if no `buddy:set-ignore` arrives within 3 s while `ignoring == false` and the cursor is outside the callout's last rect, restore click-through.
- **Acceptance:** UNIT `buddy::tests::watchdog_restores_click_through`; SMOKE+ `buddyReload` (crash the buddy renderer with DevTools `Page.crash`; the page reloads and the buddy window is click-through again within 5 s).

### B-115 · P2 · capture · The desktop selfcheck types into Notepad only on CI or when asked
- **Sources:** parity-core LIFE-16.
- **Files:** `src-tauri/src/desktop/checks.rs`.
- **Fix:** `notepad = GITHUB_ACTIONS is set || (AEVAL_DESKTOP_NOTEPAD is set && != "0")`.
- **Acceptance:** UNIT `checks::tests::notepad_gating`; desktop selfcheck log on CI still shows `desktopNotepadIncluded=ok`.

### B-116 · P2 · capture · Capture fidelity is measured against ground truth
- **Sources:** parity-features #21 (CAP-13).
- **Files:** `src-tauri/src/desktop/checks.rs`, `src-tauri/src/desktop/fixtures/` (new, if a fixture file is needed).
- **Fix:** Render the 12 px text fixture in a webview, take its lossless `CapturePreview` as ground truth at the same scale, capture the same region through the production path, and compute SSIM and edge energy against the ground truth; must SSIM ≥ 0.90 and edge energy ≥ 0.85x ground truth at 100% (thresholds tuned once on the runner and recorded).
- **Reconciled:** parity-features suggested comparing with the Electron capture; an Electron capture is itself lossy, so lossless ground truth is the better reference and also proves B-032's gain. Decision D-25.
- **Acceptance:** capture check `[CAPTURE] captureFidelity=ok ssim=<x> edges=<y>` (RUNNER `captureFidelity`).

### B-117 · S2 · capture · The lens looks like magnified text, and its frame follows DESIGN
- **Sources:** design-visual F29, F28 (lens.css half).
- **Files:** `renderer/lens.css`, `renderer/lens.html`, `renderer/lens.js`.
- **Fix:** Loupe `box-shadow: inset 0 0 0 var(--px) rgb(255 255 255 / .35), 0 0 0 1.5px var(--accent-bright), 0 0 0 2.5px var(--hit-rim), 0 6px 16px rgb(0 0 0 / .35);`, no 0.7 top highlight, radius `--r-row` (8). Control frame `border: 2px solid var(--accent)` with a `0 0 0 1px rgb(0 0 0 / .5)` outer line. `#why` capsule when single-line, else r16, padding 8/12. Delete `--skin` and the file's own `--accent` values; use glass.css tokens only.
- **Reconciled:** design-visual offered "2 px per DESIGN or record the 3 px bright frame"; DESIGN's 2 px plus a dark outline keeps it visible on dark editors without a departure.
- **Acceptance:** scenes `lens-hits-white`, `lens-hits-dark-editor` (§4 state 52); lens.css listed in `cssLintEnforce` in `tools/ui-shots/lanes/capture.mjs`.

### B-118 · S2 · capture · The area selector says what to do, briefly, and talks to screen readers
- **Sources:** design-visual F26 (region hint); design-ux U42.
- **Files:** `renderer/region.html`, `renderer/region.js`, `renderer/region.css`.
- **Fix:** Hint `Drag to select an area` plus an `Esc` keycap, and `Alt+arrows resize` added to the keyboard hint. Body `role="application" aria-label="Choose an area of the screen"`; a polite live region announces `Area 960 by 540. Arrows move it, Alt+arrows resize, Enter asks, Esc cancels.` once on the first key and the size on change (throttled 500 ms).
- **Acceptance:** scenes `region-idle-white`, `region-idle-dark`, `region-dragging`, `region-too-small` (§4 state 51); check `regionAnnouncesSize`.

### B-119 · S2 · llm · Errors carry a code, the provider, the model's name and the actions that fix them
- **Sources:** design-ux U14, U34, U46 (C-61); review-llm-agent P2-10; parity-features ERR-12; copy deck C-21, C-22, C-23, C-61, C-70..C-74.
- **Files:** `src-tauri/src/llm/errors.rs`, `src-tauri/src/host/ask.rs`, `src-tauri/src/llm/catalogue.rs`, `tools/fixtures/llm/errors.json`.
- **Fix:** `query:error` payload becomes `{gen, message, question, code, provider, model, modelName, retryInMs?, tone: 'error'|'note', actions: [{id, label, ...}]}` (K-19). Codes and actions per design-ux U14's table (`bad-key`: Fix key, Ask <next>; `no-credit`: Add credit, Use a free model; `free-busy`/`rate-limited`: B-120; `model-gone`/`no-vision`: Choose model; `data-policy`: Open OpenRouter privacy settings, Test again; `offline`: none, waits; `server`/`unknown`: Retry, Copy details; plus `daily-cap` from B-043 and `paid-hop` from B-046). Use catalogue display names everywhere people read (stage labels C-21..C-23, usage line, errors), ids only in `title` and Copy details. Fix the misleading texts: 404 on a paid model claims no stand-in; 5xx says "on this model" unless a fallback was tried; a mid-stream body error is not "No connection" when online; a Gemini 400 "API key not valid" uses the bad-key wording; a Gemini safety block or Anthropic `refusal` says the model declined; an empty detail prints the error kind, not "Error". New wording from the copy deck rows above.
- **Acceptance:** UNIT `errors::tests::error_table_is_verbatim` (errors.json regenerated to the new deck); UNIT `errors::tests::codes_and_actions`; UNIT `errors::tests::gemini_invalid_key_is_bad_key`.

### B-120 · S2 · llm · Busy free models retry on their own, honouring Retry-After; 529 counts as busy
- **Sources:** design-ux U15, C-71, C-72; review-llm-agent P2-4 (classification half).
- **Files:** `src-tauri/src/host/ask.rs`, `src-tauri/src/llm/errors.rs`, `src-tauri/src/llm/client.rs`.
- **Fix:** Parse `Retry-After` (seconds or HTTP date) into `ApiError.retry_after_ms` (K-17); add 529 (Anthropic `overloaded_error`) to the retry and failover classes. For `free-busy` and `rate-limited` (after free-model failover and any provider hop), emit `query:error` with `tone: 'note'`, `retryInMs`, text `Free models are busy right now. Trying again in <n> s.` and actions `Try now` (+ `Ask <Provider> instead` when another provider is ready); wait Retry-After (default 20 s, then 40 s) and retry automatically at most twice; the final text is `Free models are still busy. Try again in a minute, or add another provider for busy times.` with `Retry` and `Add a provider`.
- **Acceptance:** UNIT `errors::tests::retry_after_parsed`; UNIT `host::ask::tests::busy_retries_twice_then_stops`; SMOKE+ `busyAutoRetry`.

### B-121 · S2 · llm · Every answer can say exactly what was sent
- **Sources:** design-ux U16 (Rust half).
- **Files:** `src-tauri/src/host/ask.rs`, `src-tauri/src/ipc.rs` and `preload.js` (llm block only).
- **Fix:** `query:done` gains `sent: {provider, providerName, model, modelName, free, image: "screen"|"region"|null, imageSize: [w,h], windowText: {app, chars}|null, history: n, reader: {modelName}|null, msgId}` (K-19). Keep the JPEGs of the last three answers per window in memory only (never on disk); new invoke `output:sent-image {msgId}` returns a `data:image/jpeg;base64,…` URL (the page CSP already allows `img-src data:`) or `null` when evicted. No frame-protocol change, so no capture dependency.
- **Reconciled:** Decision D-38.
- **Acceptance:** UNIT `host::ask::tests::done_payload_has_sent`; UNIT `host::ask::tests::sent_images_keep_last_three`; SMOKE+ `sentPanel` (fake LLM answer: `sent.image == "screen"` and `output:sent-image` returns a data URL).

### B-122 · S2 · llm · One question can be pinned to a model, and a retry can pick another
- **Sources:** design-ux U20 (Rust half).
- **Files:** `src-tauri/src/host/ask.rs`, `src-tauri/src/llm/catalogue.rs`.
- **Fix:** `query:submit` and `query:retry` accept `{provider, model}`; a pinned model never fails over and never hops. `models:list` gains `{providers: [{id, name, groups}]}` with every ready provider's models (existing fields unchanged for current callers).
- **Acceptance:** UNIT `host::ask::tests::pinned_model_is_used_and_never_fails_over`; SMOKE+ `modelPin`.

### B-123 · P2 · llm · Native model lists come from the providers, with the curated table as fallback
- **Sources:** parity-features #2 (provider detail: hardcoded lists, `sees_images`/`supports_tools` true for every native id).
- **Files:** `src-tauri/src/llm/client.rs`, `src-tauri/src/llm/provider.rs`, `src-tauri/src/llm/catalogue.rs`.
- **Fix:** `list_models` for Anthropic (`GET /v1/models`), OpenAI (`GET /v1/models`) and Gemini (`GET /v1beta/models`), cached 5 min like OpenRouter's, filtered to chat models; image and tool support from metadata where given (Gemini `supportedGenerationMethods`, Anthropic/OpenAI by family table), curated table as fallback and for defaults. A stored native id missing from the live list heals to the provider default in memory (never on disk).
- **Acceptance:** UNIT `llm::client::tests::native_list_parsing` (fixtures per provider in `tools/fixtures/llm/`); UNIT `catalogue::tests::native_retired_id_heals_in_memory`.

### B-124 · P2 · llm · The first question after picking an area captures fresh
- **Sources:** parity-features #13 (CAP-09).
- **Files:** `src-tauri/src/host/ask.rs`.
- **Fix:** Use the pick-time shot only when it is under 1 s old; otherwise capture fresh.
- **Reconciled:** Decision D-08.
- **Acceptance:** UNIT `host::ask::tests::stale_pick_shot_is_not_reused`.

### B-125 · P2 · llm · Wire robustness: errors in 200 bodies, bounded SSE, valid turn order, Gemini sampling, skill size
- **Sources:** review-llm-agent P2-9, P2-13, P2-16, P2-17, P2-19.
- **Files:** `src-tauri/src/llm/client.rs`, `src-tauri/src/llm/sse.rs`, `src-tauri/src/llm/wire.rs`, `src-tauri/src/host/ask.rs`, `src-tauri/src/skills.rs`, `src-tauri/src/llm/prompt.rs`.
- **Fix:** A 200 body with an `error` object returns `stream_error(&data["error"])`. Cap the SSE line buffer at 1 MB and error out. Drop leading assistant turns in `query_messages`. Omit `temperature` for Gemini (as for OpenAI and Anthropic). Cap a skill body at 8 KB, mark it `truncated` in `skills.list()` so Settings can say so.
- **Reconciled:** Decision D-18 (Gemini temperature omitted).
- **Acceptance:** UNIT `client::tests::error_in_200_body_is_an_error`; UNIT `sse::tests::buffer_is_capped`; UNIT `ask::tests::history_never_starts_with_assistant`; UNIT `wire::tests::gemini_body_has_no_temperature`; UNIT `skills::tests::large_skill_is_truncated`.

### B-126 · P2 · llm · Corporate networks work: system certificates and the system proxy
- **Sources:** review-llm-agent P2-18.
- **Files:** `src-tauri/src/llm/client.rs` (reqwest features are pre-enumerated in platform C1).
- **Fix:** Use rustls with native roots (`rustls-tls-native-roots`) so a TLS-inspection proxy's root in the Windows store is trusted, and honour the Windows system proxy (reqwest's system-proxy support in the locked version; if it does not read the registry/WinHTTP settings, read them with `WinHttpGetIEProxyConfigForCurrentUser` and pass `Proxy::all`). Apply the same client config to the updater only through a shared builder if platform agrees (else leave updater as is and note it).
- **Reconciled:** Decision D-19.
- **Acceptance:** UNIT `client::tests::builder_uses_native_roots_and_proxy` (inspect config); Progress records a manual reasoning check since a proxy cannot be simulated on the runner.

### B-127 · P2 · llm · Pointing by name works for non-Latin labels
- **Sources:** review-llm-agent P2-15.
- **Files:** `src-tauri/src/llm/elements.rs`.
- **Fix:** `norm_label` keeps Unicode letters and digits (`char::is_alphanumeric`), lowercased, like the UIA path in host/ask.rs.
- **Acceptance:** UNIT `elements::tests::cyrillic_and_cjk_names_match`; SMOKE `pointByName`.

### B-128 · P2 · llm · Find stays fast on one-letter queries and keeps offsets right on any text
- **Sources:** review-llm-agent P2-14.
- **Files:** `src-tauri/src/find.rs`, `src-tauri/src/host/find.rs`.
- **Fix:** Cap hits at 500; build the collapsed text once per span; serialize hits once per change; fold per character with an offset map so offsets, `match` and rects never drift (`İ`, `ﬁ`, `™`).
- **Acceptance:** UNIT `find::tests::one_letter_query_is_capped`; UNIT `find::tests::folding_keeps_offsets`; SMOKE `findLiteralMs` (< 50).

### B-129 · P2 · llm · Pin the Ask gating rules in a test, and keep the tour's try card correct
- **Sources:** parity-core LIFE-14; parity-ipc B7 (IPC-69, EVT-31).
- **Files:** `src-tauri/src/host/ask.rs`, `src-tauri/src/host/mod.rs`.
- **Fix:** UNIT for the exact-text and bar-context gating (main.js:1810, 2027). `on_try` opens the bar with prefill `open Notepad and write a haiku` (kept for any card that still sends `try`, though B-033 retires that card).
- **Acceptance:** UNIT `host::ask::tests::exact_text_gating_matches_main_js`; UNIT `host::tests::try_card_prefills_haiku`.

### B-130 · P2 · agent · Two quick starts cannot leave an unstoppable run
- **Sources:** review-llm-agent P2-2.
- **Files:** `src-tauri/src/agent/run.rs`, `src-tauri/src/host/agent.rs`.
- **Fix:** Check-and-set under one `inner.run` lock before any await; a second start returns `busy`.
- **Acceptance:** UNIT `run::tests::concurrent_starts_yield_one_run`.

### B-131 · P2 · agent · Failover never switches tool dialect under an old system prompt
- **Sources:** review-llm-agent P2-3.
- **Files:** `src-tauri/src/agent/run.rs`.
- **Fix:** Rewrite `messages[0]` when `use_tools` changes on failover (or fail over only within the same dialect).
- **Acceptance:** UNIT `run::tests::dialect_switch_rewrites_system_prompt`.

### B-132 · P2 · agent · Overloads and Retry-After are retried properly; native thinking turns get time
- **Sources:** review-llm-agent P2-4 (agent half).
- **Files:** `src-tauri/src/agent/run.rs`.
- **Fix:** Honour `ApiError.retry_after_ms` (K-17) up to 20 s instead of the fixed 2 s / 6 s; treat 529 as retryable after the first action; native agent turns get a 180 s cap (or stream), reported as a timeout only after it.
- **Acceptance:** UNIT `run::tests::retry_after_is_honoured`; UNIT `run::tests::overload_529_retries`.

### B-133 · P2 · agent · Coordinates are validated against the screenshot they came from
- **Sources:** review-llm-agent P2-5.
- **Files:** `src-tauri/src/agent/run.rs`, `src-tauri/src/agent/tools.rs`.
- **Fix:** Require a finite positive scale; reject non-numeric components and points outside the shot or the person's region; `zoom` updates nothing but its tool text says coordinates stay in screenshot space.
- **Acceptance:** UNIT `run::tests::bad_coordinates_are_rejected`.

### B-134 · P2 · agent · A process name can never panic the run
- **Sources:** review-llm-agent P2-6.
- **Files:** `src-tauri/src/agent/run.rs`.
- **Fix:** Strip a case-insensitive ASCII `.exe` suffix by byte length (`p.len() - 4`) instead of slicing at the lowercased stem's length.
- **Acceptance:** UNIT `run::tests::app_label_kelvin_and_dotted_i`.

### B-135 · P2 · agent · Finding the last JSON object is linear
- **Sources:** review-llm-agent P2-7.
- **Files:** `src-tauri/src/agent/tools.rs`.
- **Fix:** One forward pass with a brace stack (string-aware), or scan only the last 8 KB.
- **Acceptance:** UNIT `tools::tests::last_object_is_linear` (1 MB of `}` returns in < 50 ms).

### B-136 · P2 · agent · `get_page_text` is redacted like every other read
- **Sources:** review-llm-agent P2-8.
- **Files:** `src-tauri/src/agent/run.rs`.
- **Fix:** Pass the text through `redact()`.
- **Acceptance:** UNIT `run::tests::page_text_is_redacted`; SMOKE `passwordRedacted`.

### B-137 · P2 · agent · The helper client never talks to a dying helper and survives odd output
- **Sources:** review-desktop P2-6, P2-17.
- **Files:** `src-tauri/src/desktop/client.rs`.
- **Fix:** At a reply timeout set `ready = false` and take `proc` immediately. Count a start timeout as a death (three-deaths rule). Read stdout with `read_until(b'\n')` and `String::from_utf8_lossy`.
- **Acceptance:** UNIT `client::tests::start_timeout_counts_as_death`; UNIT `client::tests::non_utf8_line_does_not_stop_reader`.

### B-138 · P2 · agent · The helper dies with the app, and system tools are run by absolute path
- **Sources:** review-desktop P2-7.
- **Files:** `src-tauri/src/desktop/client.rs`, `src-tauri/src/host/agent.rs`.
- **Fix:** Put the PowerShell child in a Job object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (feature `Win32_System_JobObjects`, pre-enumerated). Use `%SystemRoot%\System32\taskkill.exe` and `%SystemRoot%\System32\cmd.exe` absolute paths.
- **Acceptance:** UNIT `client::tests::system_tools_use_absolute_paths`; SMOKE `helperNotAtBoot` still passes.

### B-139 · P2 · agent · Install paths with accents, CJK or smart quotes keep the prebuilt helper
- **Sources:** review-desktop P2-8.
- **Files:** `src-tauri/src/desktop/protocol.rs`, `src-tauri/src/desktop/client.rs`.
- **Fix:** Pass `$AevalCache` and `$AevalPrebuilt` base64-encoded with the same `[Text.Encoding]::UTF8.GetString(...)` wrapper the sources use.
- **Acceptance:** UNIT `protocol::tests::bootstrap_paths_are_base64` (path `C:\Users\José’s\…` round-trips).

### B-140 · P2 · agent · Desktop ops look up window handles without main-thread hops
- **Sources:** review-desktop P2-9.
- **Files:** `src-tauri/src/desktop/client.rs`.
- **Fix:** Use `windows::flags::hwnd_for(label)` (cached), falling back to `hwnd()` only for unregistered windows.
- **Acceptance:** UNIT where testable; SMOKE `desktopOwnExcluded`.

### B-141 · P2 · agent · Clicks land on the corrected pixel
- **Sources:** review-desktop P2-10.
- **Files:** `lib/desktop/host.cs`.
- **Fix:** After `MoveTo`, send button down/up as plain `MOUSEINPUT` with only the button flag (no MOVE/ABSOLUTE); same for drag.
- **Acceptance:** SMOKE+ `clickExact` in `selftest/probes/agent.rs` (click a known small target in a webview fixture through `desktop::client`; `elementAt` at the click point names the target). The desktop selfcheck files belong to capture, so the check lives in the agent probe.

### B-142 · P2 · agent · One slow app cannot stall every tree read
- **Sources:** review-desktop P2-11.
- **Files:** `lib/desktop/host.cs`.
- **Fix:** Insert a placeholder generation under `genLock`, register `AddStructureChangedEventHandler` outside it, then publish the handler.
- **Acceptance:** desktop selfcheck `uiaTreeMs` unchanged or better; code note in Progress.

### B-143 · S2 · agent · Task copy says "task" and gives the remedy once
- **Sources:** design-visual F8 (Rust half); design-ux U46 (C-60), C-62.
- **Files:** `src-tauri/src/agent/run.rs`, `src-tauri/src/agent/describe.rs`.
- **Fix:** "The agent stopped" → "The task stopped" everywhere (run.rs:68, :739, :1971); internal error → `The task stopped because of a problem inside AevalBuddy. Nothing else was done.` (C-60); already running → `A task is already running.` (C-62). When a failed step row already shows the reason, the final message carries only the remedy (`Start Outlook normally, then Retry.`).
- **Acceptance:** UNIT `run::tests::final_text_is_remedy_only_after_failed_step`; existing trail text tests updated.

### B-144 · S2 · agent · Each act says which plan item it serves
- **Sources:** design-visual F22 (Rust half).
- **Files:** `src-tauri/src/agent/run.rs`.
- **Fix:** `agent:step` gains `planIndex` (K-20): the index of the plan item the act serves (from the model's tool call when given, else the count of completed acts clamped to the plan length).
- **Acceptance:** UNIT `run::tests::step_payload_has_plan_index`.

### B-145 · S2 · harness · A CSS gate for off-system values
- **Sources:** design-visual F28 (gate half), F36 (rule); parity-features NEW-09.
- **Files:** `tools/ui-shots/bridge.test.mjs`.
- **Fix:** Grep CSS for `border-radius: (4|5|8|10|12|14|18|20)px` not via tokens, paddings and gaps off the 4-grid (2 px allowed with a `/* hairline */` or `/* optical */` comment), raw `rgb()`/hex colours outside `:root` token blocks. Enforce per file for every file listed in a lane's `cssLintEnforce` from day one; the final pass (B-148) enforces all CSS.
- **Acceptance:** ui-shots `bridge.log` shows `css-lint: <n> files enforced, 0 violations`.

### B-146 · S2 · harness · MIGRATE: an Electron profile round-trips through Rust and back
- **Sources:** parity-core §9 ("Every STORE row also needs MIGRATE").
- **Files:** `src-tauri/src/selftest/migrate.rs`, `tools/fixtures/profile-v1.1.1/` (new: `aevalbuddy.json`, `history.json`, `Local State` from an Electron v1.1.1 run with a test key).
- **Fix:** Under `AEVAL_SMOKE`, copy the fixture into a scratch userData, boot, read every STORE row's value through the store API, write a change, and assert the file still parses for Electron's rules (unknown keys kept, v1 key fields mirrored, values in Electron's types). Print `[SMOKE] migrate.<STORE-id>=ok` per row.
- **Acceptance:** SMOKE `migrate.STORE-01` … `migrate.STORE-44` present and ok.

### B-147 · S3 · harness · The contrast matrix covers the glass extremes and the light tone
- **Sources:** design-ux U51 (matrix half); design-visual F10 (gate half).
- **Files:** `src-tauri/src/selftest/shots.rs`, `src-tauri/src/backdrop/contrast-probe.js`.
- **Fix:** Add k = 0 and k = 1 to the matrix (wallpaper and white, 100% and 150%) and light-tone runs (white and photo-busy) once B-159 lands; the probe measures with the 98th percentile when k > 0.5.
- **Acceptance:** `[SHOT] matrix ... failedFrames=0` including `m-*-k100-*` and `m-*-light-*` frames.

### B-148 · S2 · harness · Final strict gates: every PARITY label present, every CSS file linted, installer Checks all required
- **Sources:** parity-ipc §3 (206 labels), §3.1; design-visual F28.
- **Files:** `tools/smoke-labels/required.txt` (new), `.github/workflows/rust.yml`, `tools/ui-shots/bridge.test.mjs`.
- **Fix:** After all lanes have merged (merge step 10), add `required.txt` (the 206 PARITY labels from parity-ipc §3 minus the 3 superseded, plus every SMOKE+ label named in this backlog) and fail when any is missing from the union of lane label files or from the log; enforce CSS lint on all CSS; make every installer-check Check required in the installer job.
- **Acceptance:** rust and ui-shots green on rust-rewrite with the strict gates on.

### B-149 · S2 · harness · The five review loops on the real exe
- **Sources:** parity-features #19 (REQUESTS 7, 12); memory "multi-perspective reviews".
- **Files:** `.claude/rust-rewrite/reviews/{designer,ease-of-use,accessibility,power-user,trust}.md` (new).
- **Fix:** After B-148 is green, run each lens over the installed-exe shot set (`rust-exe/<ref>/latest/`) and write one file per lens: findings with severity and the shot that shows them. File every S1/P1 finding as a new item in `BACKLOG.md` under "Post-merge findings" (append-only section) for the coordinator to route.
- **Acceptance:** the five files exist with at least the shot list reviewed; any S1 finding is listed in BACKLOG "Post-merge findings".

### B-150 · S2 · bar · The intent token is a glyph at rest and tells the truth while a task waits
- **Sources:** design-visual F12; design-ux U18, C-12.
- **Files:** `renderer/index.html`, `renderer/bar.js`, `renderer/intent.js`, `renderer/overlay.css` (bar regions).
- **Fix:** Empty and unlocked: a 32 x 32 glyph button (`padding: 0; width: var(--ctl); justify-content: center`, no fill, no rim, `--label-2`, hover `--fill-1`), `.intent-name` visually hidden. Typed text inferred Ask: glyph only. Inferred Find or Do, or any locked intent: the word appears with the existing FLIP width morph (240 ms `--ease-morph`). Locked: accent glass. While a task waits, `renderToken` uses `agentWaiting()`: `Reply` (asking), `Start` (plan), `Continue` (paused), Do glyph, locked style, menu disabled, `aria-label="<name>. Enter sends this to the running task."`. An explicit lock (prefix, Ctrl+1/2, menu) is honoured: `onEnter` routes to `agent:continue` only when `!bar.lock`. Tooltip and menu text unchanged (REQUESTS #6).
- **Acceptance:** scenes `bar-token-rest`, `bar-token-find`, `bar-token-reply`, `bar-token-start`; check `tokenSaysReplyWhileAsking`; SMOKE `intentTable`, SMOKE+ `tokenReply`.

### B-151 · S2 · bar · A keyboard layer for the bar, with hints that keyboard users can see
- **Sources:** design-ux U19 (bar column), U43, U44, C-13.
- **Files:** `renderer/bar.js`, `renderer/overlay.js`, `renderer/index.html`, `renderer/overlay.css` (bar regions).
- **Fix:** Bar keys per U19's table: Ctrl+N new conversation (`chat:new`), Ctrl+Tab / Ctrl+Shift+Tab cycle target, Ctrl+W close the target (`output:close`), Ctrl+Shift+W close all, Ctrl+Shift+C copy the target's last answer (`output:last-answer`), Ctrl+. stop (`query:cancel` / `agent:stop`), Ctrl+Shift+R retry the target (`query:retry {label}`), Ctrl+R ask about an area (`region:pick`), Ctrl+, Settings, Ctrl+H History, F6 to the target answer. Tooltips gain the key and buttons `aria-keyshortcuts`. U43 semantics: `#examples` `role="group" aria-label="Suggestions"`; guess card `role="group" aria-labelledby="guess-text"`, buttons with `aria-keyshortcuts="Enter"`/`"Alt+Enter"` and their `<kbd>` hidden from the name; `#region-clear` `aria-label="Stop using the area and ask about the whole screen"`; `#close-btn` `Close the bar`. U44: the token's tooltip text also shows as a small glass tooltip under the token on `:focus-visible` after 600 ms and on hover after 400 ms (only on the token).
- **Acceptance:** check `barShortcutsInvoke` (each chord invokes the right channel on the mock); SMOKE `barKeys`, `keySwallow`.

### B-152 · S2 · bar · `@` picks a model for one question
- **Sources:** design-ux U20 (bar half).
- **Files:** `renderer/bar.js`, `renderer/index.html`, `renderer/overlay.css` (bar regions).
- **Fix:** `@` as the first character opens `#model-menu` (`.g-menu`, `role="listbox"`) listing every ready provider's models grouped (`role="group" aria-label="<Provider>"`) from `models:list.providers` (K-19), filtered by the text after `@`; Enter or Tab picks; Esc closes and leaves the text; no match: `No model called "<text>". Esc to keep typing.` The pick becomes `#model-chip` after the token (`Sonnet 5.5` plus x, `aria-label="Model for this question: <name> via <Provider>. Press to remove."`); Backspace at position 0 removes the chip before the lock. Ask and Do send `{provider, model}`; with Find the chip shows `title="Find uses the Find model"` and is ignored.
- **Acceptance:** scenes `bar-model-menu`, `bar-model-chip`; check `modelPinSentWithSubmit`.

### B-153 · S2 · bar · Esc clears a half-typed task answer before it stops the task
- **Sources:** design-ux U17 (renderer half).
- **Files:** `renderer/bar.js`.
- **Fix:** Send `bar:dirty {dirty}` on input, debounced 100 ms; on `bar:clear` clear the field and keep focus.
- **Acceptance:** check `dirtyFlagSentAndClearHonoured`; SMOKE+ `escClearsBeforeStop` (shared with windows).

### B-154 · S2 · bar · The first keystrokes after the hotkey are never lost
- **Sources:** design-ux U25 (renderer half).
- **Files:** `renderer/overlay.js`, `renderer/bar.js`.
- **Fix:** A capture-phase `keydown` on `document`: if `state.mode === 'panel'`, focus is not in the field and the key is printable without Ctrl/Alt, focus the field and let the key through. Invoke `bar:ready` once the field is focused after an open.
- **Acceptance:** SMOKE+ `earlyKeys` (bar probe sends keys 5 ms after the hotkey; the field holds them); SMOKE+ `barOpenMs` (shared with windows).

### B-155 · S2 · bar · Enter without a provider visibly points at the fix
- **Sources:** design-ux U23, C-15.
- **Files:** `renderer/bar.js`, `renderer/overlay.css` (bar regions).
- **Fix:** When `query:submit` returns `needs-key`, replay a 240 ms 2 px horizontal `nudge` on `#notice` (fade under reduced motion), announce `Answers need an AI provider. Press Tab to connect one.`, and make the notice button the next tab stop after the field.
- **Acceptance:** check `needsKeyNudgesAndAnnounces`; SMOKE `queryWithoutKey` (shared with llm).

### B-156 · S2 · bar · Bar copy names what is needed, not one vendor, and shows short notes
- **Sources:** design-ux C-1..C-11, U6 (bar half), U38 (status note); design-visual F26 (bar copy).
- **Files:** `renderer/overlay.js`, `renderer/bar.js`, `renderer/index.html`.
- **Fix:** Apply copy deck rows C-1 to C-11 verbatim (from `.claude/rust-rewrite/audit/design-ux.md` §4 "Bar"). Notices that mention a provider read from `overlay:state.recipients` (K-18). Listen `bar:note {text}` (K-23) and show it in the status row for 4 s (used by the window cap, B-178).
- **Acceptance:** check `noOpenRouterInBarCopy` (with an Anthropic-only mock no bar string names OpenRouter); scene `bar-notice-nokey`, `bar-notice-hotkey-taken`.

### B-157 · S2 · bar · In a contrast theme the working indicator stays visible
- **Sources:** design-ux U29.
- **Files:** `renderer/glass.css`.
- **Fix:** `@media (forced-colors: active) { .glass.orbit { outline: 2px solid Highlight; outline-offset: -2px } #pill.orbit { outline-style: dashed } .pill-dot { forced-color-adjust: none; background: Mark } }`.
- **Acceptance:** scenes `forced-colors-bar-working`, `forced-colors-answer-streaming` (§4 state 55).

### B-158 · S2 · bar · A window menu moves, sizes, maximizes and closes from the keyboard
- **Sources:** design-ux U30 (renderer half).
- **Files:** `renderer/frame.js`, `renderer/glass.css`.
- **Fix:** Alt+Space or a right-click on a title bar opens a `.g-menu` `role="menu"`: `Move`, `Size`, `Maximize`/`Restore`, `Close`. In Move or Size, arrows move or resize 16 px (Shift 64) through `window:nudge` (K-23); Enter keeps, Esc reverts. frame.js is shared, so outputs, Settings and History all get it (K-43).
- **Acceptance:** scene `window-menu-open`; check `windowMenuNudges`; SMOKE+ `windowNudge` (shared with windows).

### B-159 · S2 · bar · Glass turns light over light content
- **Sources:** design-visual F10.
- **Files:** `renderer/glass.js`, `renderer/glass.css`.
- **Fix:** `glass.js` computes the median luma under each window's largest glass layer (y50) beside y90; `html[data-tone="light"]` when y50 > 0.72 for 2 consecutive frames, back to dark when y50 < 0.55; 300 ms crossfade including label colours. Light tokens, rim, scrim (thin `0.42 + 0.30(1-y)`, thick `0.62 + 0.20(1-y)`), accent (`--accent-text: #823A80`) and text-shadow exactly as F10 lists. Record the DESIGN Material departure ("dark tint always" → adaptive tone) in `entries/bar.md`.
- **Reconciled:** Decision D-20. The contrast gate gains light-tone runs (B-147).
- **Acceptance:** scenes `light-tone-{bar,answer,settings,history}` over white and photo-busy at 1 and 1.5 (§4 state 56); check `toneFlipsWithHysteresis`.

### B-160 · S2 · sheet · Streaming appends instead of re-rendering the whole answer
- **Sources:** design-visual F14.
- **Files:** `renderer/output.js`, `renderer/markdown.js`, `renderer/overlay.css` (answers region).
- **Fix:** Split the raw text at the last blank line outside a code fence; render earlier blocks once and append; re-render only the tail per frame. New blocks get `animation: md-in 160ms var(--ease-out)` (`from { opacity: 0; filter: blur(2px) }`). While busy the tail element gets a static 6 px trailing dot in `--label-2`, removed on done.
- **Acceptance:** scene `sheet-mid-stream` (§4 state 15); check `streamingKeepsEarlierNodes` (node identity of block 1 unchanged across 10 deltas); check `selectionSurvivesStream`.

### B-161 · S2 · sheet · Code reads light and copies in one click; headings and rhythm follow the scale
- **Sources:** design-visual F15, F16; design-ux U11 (Copy code button).
- **Files:** `renderer/markdown.js`, `renderer/output.js`, `renderer/overlay.css` (answers region).
- **Fix:** F15 CSS for inline code and `pre`; each fence wrapped in `<div class="md-code">` with the info string (`fs-meta`, `--label-2`) and a 28 px `.g-icon.small` Copy button (`aria-label="Copy code"`, top 6 right 6) shown on hover/focus-within and always on the last block; copies `codeBuf.join('\n')` and shows a check for 1.5 s; horizontal edge fade only when scrollable. F16: `.md-h1, .md-h2 { font: 600 17px/22px var(--font-text) }`, `.md-h3 15/22`, `.md-h { margin: 20px 0 6px }`, `.md-p { margin: 0 0 12px }`, `.md-gap { display: none }`, `.md-body { line-height: 24px }`, list spacing per F16.
- **Acceptance:** scene `sheet-markdown-fixture` (shared with B-063); check `codeCopyButtonCopies`.

### B-162 · S2 · sheet · A quiet usage line and a "What was sent" panel
- **Sources:** design-visual F17; design-ux U16 (renderer half), C-29; design-ux §3.4 (hop suffix).
- **Files:** `renderer/output.js`, `renderer/overlay.css` (answers region).
- **Fix:** Default usage line: `<model name> via <Provider> · <total s>` plus `· <Provider> was busy` after a hop; `free` only when paid models are on; full telemetry in `title` and the context menu (`Copy details`). `.msg-meta { flex-wrap: nowrap }`, `.usage` ellipsis in `--label-3`, hidden below 360 px (container query). A quiet `What was sent` disclosure (`aria-expanded`) opens U16's panel built from `query:done.sent` (K-19): `Sent to <Provider> (<model name>):` / `Your question` / `A screenshot of the whole screen, 1920 x 1080 [View]` (View loads `output:sent-image`) or `The text of the <app> window, 2,340 characters` / `<n> earlier messages from this conversation` / (free only) `Free models' providers may keep and train on what they receive.`
- **Reconciled:** F17 wants "Gemma 3 27B · 3.2 s"; U16/C-29 want "… via OpenRouter". Combined as above.
- **Acceptance:** scenes `sheet-done`, `sheet-sent-panel`; check `usageLineIsShort`.

### B-163 · S2 · sheet · Front and back outputs look different, and the target window wears a rim
- **Sources:** design-visual F19 (renderer half); design-ux U5 (target rim).
- **Files:** `renderer/output.js`, `renderer/overlay.css` (answer sheet region).
- **Fix:** From `output:meta` (K-06) set `body[data-front]` and `body[data-target]`. Back: title `--label-2`, the lighter shadow and `.gl::after` at .7 per F19, 150 ms transition. Target while the bar is open: `body[data-target="1"] #opanel > .gl::after` accent-text rim per U5.
- **Acceptance:** scenes `sheet-front-back`, `sheet-target-rim` (§4 state 22).

### B-164 · S2 · sheet · The sheet grows in the page and reports its rect
- **Sources:** design-visual F13 (renderer half).
- **Files:** `renderer/output.js`, `renderer/overlay.css` (answer sheet region).
- **Fix:** `:root { interpolate-size: allow-keywords; } #opanel { height: auto; transition: height var(--d-med) var(--ease-morph); overflow: clip; }`; `body[data-grow="up"] #opanel { top: auto; bottom: var(--stage); }` from `output:meta.grow`; `requestResize()` sends `output:glass {top, height}` once per frame instead of a window resize (K-23). Keep sending `output:resize` too until windows' B-027 lands on rust-rewrite, then stop.
- **Acceptance:** check `sheetReportsGlassRect`; SMOKE `answerFollowsTranscript` (shared with windows).

### B-165 · S2 · sheet · The plan is the checklist
- **Sources:** design-visual F22 (renderer half).
- **Files:** `renderer/trail.js`, `renderer/overlay.css` (agent trail region).
- **Fix:** Each act row nests under the plan item it serves (`agent:step.planIndex`, K-20) as a 13 px `--label-2` detail line, collapsed when done. Without `planIndex`, mark plan item i done when i acts completed and keep acts behind the "Steps" disclosure. Pending items `--label-2`, running `--label` with the arc, done `--label-2` with a check.
- **Acceptance:** scenes `sheet-task-running`, `sheet-task-done-expanded` (§4 states 29, 33); check `planChecksOff`.

### B-166 · S2 · sheet · Error rows offer the fix; busy is a calm note that counts down; a paid hop asks first
- **Sources:** design-ux U14, U15 (renderer half), §3.4 (paid-hop card); design-visual F31.
- **Files:** `renderer/output.js`, `renderer/overlay.css` (answers region).
- **Fix:** `addErrorRow(message, question, actions, tone)` renders `query:error.actions` (K-19) as `.retry-btn` capsules in order, each mapped to its effect (`Fix key` → `overlay:open-settings {section: 'prov-<id>'}`, `Choose model` → Settings Models, `Add credit` / privacy → `open-external`, `Retry`/`Try now` → `query:retry`, `hop` → `query:retry {label, provider, model}`, `never-ask` → `settings:set-failover {askBeforePaid:false}` then hop, `Copy details` → clipboard). `tone: 'note'` uses the neutral note row with a clock icon and a countdown from `retryInMs` (text updates each second, not announced; announced at start and at retry). F31: `.error-row { background: var(--fill-1); box-shadow: var(--chip-rim); border-radius: var(--r-plate) }`, danger glyph only in `--danger`, Retry wraps under long text.
- **Acceptance:** scenes `sheet-error-short`, `sheet-error-long`, `sheet-busy-countdown`, `sheet-paid-hop` (§4 state 18); check `errorActionsInvoke`.

### B-167 · S2 · sheet · Keyboard shortcuts in output windows
- **Sources:** design-ux U19 (output column), C-25, C-26, C-27.
- **Files:** `renderer/output.js`, `renderer/output.html`.
- **Fix:** Ctrl+N (new conversation, focus to the bar), Ctrl+Tab / Ctrl+Shift+Tab (focus next/previous output via `output:cycle-target`), Ctrl+W (`output:close {label}`), Ctrl+Shift+W (`output:close-all`), Ctrl+Shift+C (copy this conversation), Ctrl+. (stop), Ctrl+Shift+R (retry here), Ctrl+, (Settings), Ctrl+H (History), F6 to the bar. Tooltips `Close (Ctrl+W)`, `Copy conversation (Ctrl+Shift+C)`, `History (Ctrl+H)`, `Stop (Ctrl+.)`, with `aria-keyshortcuts`.
- **Acceptance:** check `outputShortcutsInvoke`.

### B-168 · S2 · sheet · "Start over" keeps the window and offers Undo
- **Sources:** design-ux U22 (renderer half), C-24, C-34, C-35.
- **Files:** `renderer/output.js`, `renderer/output.html`.
- **Fix:** Rename `+` to `Start over` (`aria-label="Start over in this window"`, tooltip `Start over`). On `chat:reset` keep the sheet visible with one neutral row `Started over. The last conversation is in History.` and `Undo` (`output:undo-reset`, K-23); the empty sheet reads `Your next question goes here.`
- **Acceptance:** scene `sheet-started-over`; check `startOverOffersUndo`.

### B-169 · S2 · sheet · Find's empty result offers next steps, and the results list works from the keyboard
- **Sources:** design-ux U24, U28.
- **Files:** `renderer/trail.js`, `renderer/output.html`, `renderer/overlay.css` (find region).
- **Fix:** Under the empty text, buttons `Ask about "<query>"` (`find:to-ask`) and, when scope is the window, `Look on the whole screen` (`find:scope`); no key hints. Results keep `role="listbox"` with roving focus: rows `tabindex="-1"` except the current (`0`), Up/Down/Home/End move focus and send `find:step {index}`, Enter goes to the hit.
- **Acceptance:** scenes `find-empty`, `find-results` (§4 states 24, 25); check `findListboxArrows`.

### B-170 · S2 · sheet · Task progress is heard
- **Sources:** design-ux U27, C-39.
- **Files:** `renderer/trail.js`.
- **Fix:** Each row gets an `.sr-only` prefix (`Planned:`, `Now:`, `Done:`, `Skipped:`, `Failed:`). Announce each new act when it starts, throttled to one per 2 s (`Step 5: Type "notes.txt" in Notepad`), failures immediately, via `AevalA11y.say`; confirm cards stay assertive with `aria-describedby` on `.confirm-reason`.
- **Acceptance:** check `trailRowsHaveSrPrefixes`, `stepAnnouncesThrottled`.

### B-171 · S2 · sheet · The consent sheet says who receives the screen and that the mouse pauses
- **Sources:** design-ux U31, U6 (consent half), C-30, C-31.
- **Files:** `renderer/trail.js`.
- **Fix:** Build C-30 from `agent:consent-request.{provider, modelName, recipients}` (K-21): `Each step sends a screenshot and the window's text to <Provider> (<model name>).` plus the free-model line only for free models. C-31: `Move the mouse to pause it. Press Esc to stop it.`
- **Acceptance:** scene `sheet-consent` (§4 state 27); check `consentNamesProvider`.

### B-172 · S2 · sheet · Retry can pick another model
- **Sources:** design-ux U20 (renderer half).
- **Files:** `renderer/output.js`, `renderer/overlay.css` (answers region).
- **Fix:** Retry on answers and error rows becomes a split button: `Retry` and a 28 px chevron `aria-label="Retry with another model"` opening the same grouped list (`models:list.providers`, `.g-menu`); sends `query:retry {label, provider, model}`.
- **Acceptance:** scene `sheet-retry-menu`; check `retryWithModelInvokes`.

### B-173 · S2 · settings · Models per provider, with an automatic default
- **Sources:** design-ux U35, §3.3 (models half).
- **Files:** `settings/settings.html`, `settings/settings.js`, `settings/settings.css`.
- **Fix:** Top row `Choose models: Automatic (recommended)` / `Choose for each job`; the four job pickers show only in the second. A segmented `Models for:` with one segment per provider that has a key (default: the Default provider); each job row edits that provider's pick (`settings:save-model* {provider}`, K-19); `Show paid models` only on the OpenRouter segment, others show `Every model here is billed per question.`
- **Acceptance:** scenes `settings-models-automatic`, `settings-models-anthropic`, `settings-models-combo-open` (§4 state 43); check `modelSaveSendsProvider`.

### B-174 · S2 · settings · History reads calmly, tells the truth when off, and works from the keyboard
- **Sources:** design-visual F39; design-ux U36, U32 (renderer half), U37, C-80, C-82.
- **Files:** `history/history.html`, `history/history.js`, `history/history.css`.
- **Fix:** F39: focus the search on open but no halo until the first key or Tab; `Delete all` a quiet `--label-2` text button with danger on hover; sentence case time labels; group heads aligned with row text (`padding-left: 16px`). U36: rows show `20 min ago` / `3 h ago` today, else the time under its day group, and `12 Oct` under Earlier. U32: when `history:list.enabled` is false (K-24) the empty text is C-80 with a `Turn on` link (`settings:set-save-history`). U37: Up/Down move between rows, Enter opens, Delete deletes (with B-071's undo), a printable key focuses search with that character, Esc clears then closes; row context menu `Ask again` (`bar:prefill-open {text}`, K-23) and `Copy question`. Delete-all confirm text C-82.
- **Acceptance:** scenes `history-list`, `history-search-results`, `history-empty`, `history-off`, `history-delete-all` (§4 states 49, 50); checks `historyArrowsMove`, `historyOffSaysOff`.

### B-175 · S2 · settings · One layer of CSS in Settings and History, on the token scale
- **Sources:** design-visual F28 (settings half), F34, F35, F37 (adoption); parity-features NEW-09; design-ux U9 (Settings accent text).
- **Files:** `settings/settings.css`, `history/history.css`.
- **Fix:** Delete the superseded v0.x rules (settings.css:1-906 where overridden) and keep only the override values mapped to tokens (radii per K-40's table: groups r12, rail rows r8, combo options r8; keycaps from glass.css; spacing on the 4-grid; weights 450/600; `--font-small` for 12 px). Any accent text uses `--accent-text`. List both files in `cssLintEnforce` in `tools/ui-shots/lanes/settings.mjs`.
- **Acceptance:** ui-shots `css-lint` 0 violations for both files; scenes for every Settings page at default size (§4 state 41).

### B-176 · S2 · settings · Pickers and choices follow the shared menu and ARIA patterns; Delete all confirms inline
- **Sources:** design-visual F21 (Settings combo); design-ux U40, U41, U52, C-54.
- **Files:** `settings/settings.html`, `settings/settings.js`, `settings/settings.css`.
- **Fix:** The model combo popover becomes `.g-menu` (solid tier inside the page: `--solid` body, same rim and radius). Choice groups: roving tabindex, arrows move and select. Model lists: `role="group" aria-label="Free"` wrappers instead of `aria-hidden` headers; the combo button gets `aria-controls`. `showPage` uses `behavior: 'auto'` under reduced motion. Delete all: inline `Delete all <n> conversations?` with `Delete` / `Keep`, focus on Keep, result in the `role="status"` line.
- **Acceptance:** checks `radiosRoveWithArrows`, `deleteAllConfirmsInline`; scene `settings-combo-open`.

## Items: S3

### B-177 · S3 · platform · A taken shortcut comes with free alternatives
- **Sources:** design-ux U48 (Rust half), C-56.
- **Files:** `src-tauri/src/hotkeys.rs`.
- **Fix:** When a chord is taken, probe `Ctrl+Alt+A`, `Win+Shift+Space`, `Ctrl+Shift+Space` and return the free ones in the `shortcuts:set` error payload (`{ok:false, error, free: [..]}`), error text C-56 `Another app already uses that shortcut. Try another.`
- **Acceptance:** UNIT `hotkeys::tests::taken_returns_free_alternatives` (with an injected registrar); UNIT `validation_matches_main_js` updated.

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

### B-180 · S3 · capture · The tour card is calm and concentric
- **Sources:** design-visual F38, F28 (cursor-buddy.css half), F20 (callout X).
- **Files:** `renderer/buddy.js`, `renderer/buddy.html`, `renderer/cursor-buddy.css`.
- **Fix:** Set the message at once with `animation: md-in 160ms var(--ease-out)` and announce it once (no typewriter, no caret). Close button in the text row (`float: right; margin: -4px -4px 0 8px`), drop `.bubble-head` when there are no pips. Radius 28, padding 16 (12 under the actions row). Shared X from `icons.svg`. Delete the v0.3 callout rules above the override; tokens only. List `cursor-buddy.css` in `cssLintEnforce`.
- **Acceptance:** scenes `buddy-callout`, `tour-step-{1,2,3,4}` (§4 state 53).

### B-181 · S3 · llm · A door to custom answer styles
- **Sources:** design-ux U49 (Rust half).
- **Files:** `src-tauri/src/host/settings.rs`, `src-tauri/src/skills.rs`.
- **Fix:** Invoke `skills:open-folder` creates `<exe>\skills` if missing and opens it in Explorer (absolute `explorer.exe` path).
- **Acceptance:** UNIT `skills::tests::folder_path_is_exe_relative`.

### B-182 · S3 · bar · Motion and light details in the bar
- **Sources:** design-visual F32, F42, F36 (bar rows).
- **Files:** `renderer/glass.css`, `renderer/glass.js`, `renderer/overlay.css` (bar regions and the reduced-motion tail), `renderer/index.html`.
- **Fix:** `--d-reduced: 150ms` used by every reduced-motion fade (overlay.css tail; glob.js reads the token, which sheet consumes). Drop the bar's word blur on fold under reduced motion (`filter: none`). `interactive` on `#bar` (and support for `.interactive` on sheet bars at 0.06); optional: buttons brighten to `--fill-3` on `:active` instead of shrinking. Snap `.intent-token` padding and gap to the 4-grid.
- **Acceptance:** scenes `reduced-motion-bar-morph`, `reduced-motion-token-lock` (§4 state 54); css-lint clean for `glass.css` and `overlay.css` bar regions.

### B-183 · S3 · sheet · Sheet refinements: Do it only for steps, a visible confirm settle, quieter chrome, one inset, one scale
- **Sources:** design-ux U39, U45, C-20, C-28; design-visual F26 (wait message, confirm hint), F30 (plates), F33, F34, F35, F36 (sheet rows), F37 (sheet plates), F41.
- **Files:** `renderer/output.js`, `renderer/trail.js`, `renderer/overlay.css` (answer sheet, answers, find and agent trail regions).
- **Fix:** Show "Do it" only when at least two list lines match `^\s*(\d+[.)]|[-*])\s+(click|double-click|right-click|press|open|type|select|choose|go to|drag|enter)\b`; its tooltip C-28. Confirm buttons `aria-disabled="true"` with a 700 ms fill sweep (static under reduced motion), then enabled (shares B-007's gate). Wait text C-20 (`Waiting for <model name> · N s. Free models queue when they're busy.`, explanation in `title`). Remove the confirm card's visible "Tab to a button, then Enter" hint (still announced). Plates (`.confirm-card`, `.msg.consent`) lose their drop shadow. Delete `#opanel::after`; scrollbar thumb `rgb(255 255 255 / .18)` at rest, `.40` while `body[data-scrolling]` (800 ms after a scroll). Display face only at 18 px and up; `strong` 600; off-grid values in F36's sheet rows snapped; plates r12 (`--r-plate`); text starts 16 px from the sheet edge in every kind with marks hung in the gutter (F41: `.find-row { padding: 8px 12px 8px 4px }`, `.find-view { margin: 0 -10px 0 -12px }`, trail matched).
- **Acceptance:** checks `doItOnlyForSteps`, `confirmSweepThenEnable`, `textInsetIs16InEveryKind`; css-lint clean for overlay.css sheet regions.

### B-184 · S3 · settings · Small Settings doors and notes
- **Sources:** design-ux U33, U47 (renderer half), U48 (renderer half), U49 (renderer half), U50, C-53, C-55, C-64; review-llm-agent P2-19 (skill note).
- **Files:** `settings/settings.html`, `settings/settings.js`, `settings/settings.css`.
- **Fix:** Shortcuts page: a read-only group `In AevalBuddy windows` listing U19's keys plus F6, Up/Down recall and Backspace (not the Ask/Find/Do keys); F1 in any Settings page opens it. `Bar position` row with the summary from `overlayPos` (`Top right`) and a `Move…` button (C-53). A taken shortcut shows the free alternatives as one-click buttons (`shortcuts:set` `free`). Answers page: `Open styles folder` (`skills:open-folder`) and `Add a .md file to make your own style.`; a truncated skill shows `Only the first 8 KB is used.` When `glass:tier` reasons include `transparency-off`, the Liquid glass section adds C-64. Recorder hint C-55.
- **Acceptance:** scenes `settings-shortcuts-capturing`, `settings-shortcuts-conflict` (§4 state 44), `settings-general`; check `shortcutConflictOffersFree`.

## Post-merge findings

(Append-only. B-149's review loops and the merge agent add S1/P1 findings here for the coordinator to route.)
