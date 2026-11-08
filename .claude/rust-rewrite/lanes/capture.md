# Lane brief: capture

Branch `rust/capture`, cut from `rust-rewrite`. 21 items. Cross-lane tables (ownership, hotspots, contracts, decisions, merge order) are in `.claude/rust-rewrite/BACKLOG.md`; everything you need to do your items is in this file.

## Goal

What the model sees is the screen the person is looking at, as sharp as the budget allows: one display layout that cannot overlap, capture from the foreground window's monitor at physical resolution with 4:4:4 chroma, a DXGI path that never serves a pointer-only frame, HDR handled, a backdrop that idles and cannot freeze, a region selector that always returns, lens and pointer pages that are ready before they draw, and a calm tour that leads with what works.

## Files you own

`src-tauri/src/capture.rs`, `src-tauri/src/capture/**`, `src-tauri/src/windows/screen.rs`, `src-tauri/src/backdrop.rs` (not `backdrop/contrast*`), `src-tauri/src/region.rs`, `src-tauri/src/lens.rs`, `src-tauri/src/buddy.rs`, `src-tauri/src/desktop/{mod,channels,checks,selfcheck,win,scoring}.rs`, `renderer/{lens.html,lens.js,lens.css,buddy.html,buddy.js,cursor-buddy.css,region.html,region.js,region.css}`, your block in the IPC allowlists (after windows C1), and (after harness C1) `src-tauri/src/selftest/probes/capture.rs`, `tools/smoke-labels/capture.txt`, `tools/ui-shots/lanes/capture.mjs`.

## Contracts you provide (C1 first)

- **K-10** `screen::active()`, `latch_active()`, `latched()`, `work_area_of(display)` in `windows/screen.rs` (the B-032 definition; a C1 stub may return the display under the pointer).
- **K-11** `pub screen::forced_scale()`.
- **K-12** `region::set_active_on(rect_dip, device)`; **K-13** `buddy::agent_point_phys(pt_px, label)`.

## Contracts you consume

- **K-01**, **K-02** crash guards (platform) for B-114.
- Windows C1's allowlist blocks before adding `lens:ready` / `buddy:ready` (B-034).
- **K-40** tokens and `icons.svg`, **K-42** `AevalA11y.say` (bar) for B-117, B-118, B-180.
- **K-30**, **K-31** (harness).

## Smoke labels you port

`capture`, `regionCapture`, `regionSharper`, `regionPointTarget`, `regionSelectorCancels`, `regionEscape`, `captureActiveDisplay`, `captureFailNull`, `dpiRoundTrip`, `pointResolved`, `buddyPoints`, `buddyPointsDone`, `walkthrough`, `onboardingSteps`, `glassTier`, `glassLoaded`, `snapCost`, `glassCost`, `lensFirstDraw`, `lensWhyClear`, `lensHits`, `helperNotAtBoot`, the 33 desktop selfcheck labels, and the capture-check lines `regionDragReturnsTheBox`, `dxgiFirstFrame`, `wholeScreenShowsFixture`, `glassLive`, `tickMs`. Exact names: `.claude/rust-rewrite/audit/parity-ipc.md` §3 and `desktop/checks.rs`.

## ui-shots scenes you own

design-visual §4 states 51-53 (region selector, lens, tour and buddy), in `tools/ui-shots/lanes/capture.mjs`.

## Shared-document entries you write

D-06, D-09, D-25, D-33 (D-26 needs no entry); PARITY evidence for CAP, GLASS, LENS, TOUR, REGION, DESK rows your items close.

## Standing item: port your smoke labels

Every PARITY label under "Smoke labels you port" and every new label below is printed by `src-tauri/src/selftest/probes/capture.rs` through the K-30 API, with the Electron semantics from `.claude/rust-rewrite/audit/parity-ipc.md` §3 and `main.js` on `main`, and listed in `tools/smoke-labels/capture.txt` in the same commit that makes it print `=ok`. Acceptance: the rust workflow's smoke log shows each label `=ok` and the labels gate passes.

New labels your items add, beyond the PARITY list above (you print these): `backdropIdle`, `buddyReload`, `buddyReuse`, `lensReady`.

## Items

| ID | Severity | Title |
|---|---|---|
| B-005 | P0 | One canonical DIP layout that cannot overlap; every mapped-back rect carries its display |
| B-028 | P1 | The area selector returns the area, never hangs, and claims Escape in time |
| B-029 | P1 | DXGI never serves a pointer-only first frame, and CI gates the DXGI path |
| B-030 | P1 | The backdrop idles when nothing is read, and each tick costs a fraction of today's |
| B-031 | P1 | The buddy window is reused, never destroyed and re-created |
| B-032 | S1 | Capture fidelity: physical pixels up to 1600, 4:4:4 chroma, the foreground window's display first |
| B-033 | S1 | The first-run tour leads with what works and never prints a blank shortcut |
| B-034 | P1 | Lens and pointer pages say they are ready before the first draw |
| B-035 | P1 | A profile migrated as solid follows the glass slider |
| B-108 | P2 | Failed DXGI opens back off, and outputs share a device per adapter |
| B-109 | P2 | HDR desktops are captured with correct SDR tone |
| B-110 | P2 | A hung capture thread cannot hang every later capture |
| B-111 | P2 | A panic in a backdrop tick never freezes the glass; recovery rebroadcasts |
| B-112 | P2 | Screen frames are readable only by the app's own origin |
| B-113 | P2 | A window never receives another display's frame |
| B-114 | P2 | Pages capture builds are crash-guarded, and a dead buddy page stops eating clicks |
| B-115 | P2 | The desktop selfcheck types into Notepad only on CI or when asked |
| B-116 | P2 | Capture fidelity is measured against ground truth |
| B-117 | S2 | The lens looks like magnified text, and its frame follows DESIGN |
| B-118 | S2 | The area selector says what to do, briefly, and talks to screen readers |
| B-180 | S3 | The tour card is calm and concentric |

Work in severity order (P0, then S1 and P1, then S2 and P2, then S3), after your C1 if you have one.

### B-005 · P0 · capture · One canonical DIP layout that cannot overlap; every mapped-back rect carries its display
- **Sources:** review-desktop P0-1, P1-6; review-core P1-1 (screen half).
- **Files:** `src-tauri/src/capture/screen.rs`, `src-tauri/src/windows/screen.rs`, `src-tauri/src/capture.rs`, `src-tauri/src/region.rs`, `src-tauri/src/lens.rs`, `src-tauri/src/buddy.rs`.
- **Fix:**
  1. Build Chromium-style adjacent DIP bounds once, in one function used by both screen modules: the primary keeps origin (0,0) and its DIP size; every other display is placed edge to edge with the neighbour it touches physically (sorted by distance to primary, then device name), at `neighbour_dip_edge + (phys_offset_along_edge / own_scale)`. No display's DIP rect may overlap another's. `windows/screen.rs` keeps its public API but delegates to this layout.
  2. `matching` breaks ties toward the primary, then the display the rect's centre is in, using a first-max scan instead of `Iterator::max_by` (which returns the last).
  3. `region` stores the display device (or the physical rect) beside `ACTIVE` and `PENDING_SHOT`; `capture::shot_image` takes that display instead of calling `matching`. Provide `region::set_active_on(rect_dip, device)` for host/find.rs "Ask about this" (consumed by llm, see contract K-12).
  4. `lens::set_hits`, the loupe (`capture::crop_dip`) and `buddy::agent_point` carry the display id from the physical side (`matching_phys` is unambiguous). Provide `buddy::agent_point_phys(pt_px, label)` (consumed by agent, K-13).
- **Acceptance:** UNIT `capture::screen::tests::overlapping_mixed_dpi_layout_is_adjacent` (primary 1920x1080@100%, secondary 2560x1440@150% at x=1920: DIP rects touch and do not overlap); UNIT `capture::screen::tests::rect_in_old_overlap_maps_to_primary`; UNIT `capture::tests::area_on_primary_crops_primary`; UNIT in `windows::screen` mirroring the first two; SMOKE+ `dpiRoundTrip`.

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

### B-180 · S3 · capture · The tour card is calm and concentric
- **Sources:** design-visual F38, F28 (cursor-buddy.css half), F20 (callout X).
- **Files:** `renderer/buddy.js`, `renderer/buddy.html`, `renderer/cursor-buddy.css`.
- **Fix:** Set the message at once with `animation: md-in 160ms var(--ease-out)` and announce it once (no typewriter, no caret). Close button in the text row (`float: right; margin: -4px -4px 0 8px`), drop `.bubble-head` when there are no pips. Radius 28, padding 16 (12 under the actions row). Shared X from `icons.svg`. Delete the v0.3 callout rules above the override; tokens only. List `cursor-buddy.css` in `cssLintEnforce`.
- **Acceptance:** scenes `buddy-callout`, `tour-step-{1,2,3,4}` (§4 state 53).

## Rules

The coordinator's rules, verbatim: "LF endings; never run the app locally; never weaken or skip a test or gate to get green; commit messages in sentence case describing the effect like the existing log, each ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; push only its own branch".

What each one means here:

- **LF endings** in every file you write. Before each commit run `git diff --cached --check` and `git ls-files --eol <changed files>` (expect `i/lf w/lf`). The repo has no `.gitattributes`, so your editor's endings are what gets committed.
- **Never run the app locally.** No `cargo tauri dev`, no launching a built exe, no `npm start` of the Electron app. This machine has no Rust toolchain and nine lanes share it; CI is the build and the test bed. Local checks are limited to `node --check <file>` for JS and reading code.
- **Never weaken or skip a test or gate to get green.** No `#[ignore]`, no deleted or loosened assertions, no raised thresholds, no `continue-on-error`, no label removed from a label file, no scene removed from ui-shots. An expected value changes only where one of your items specifies the new value (for example B-049's gate tables or B-087's defaults test).
- **Commits** in sentence case describing the effect, like `git log --oneline -15` on `rust-rewrite` (for example "Keep the first-run tour out of the capture and desktop checks"). Item IDs may go in the body. Every message ends with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Push only your own branch**, `rust/capture`: `git push origin HEAD:rust/capture`. Never push `rust-rewrite`, `main`, `ci-logs`, `shots` or another lane's branch; never force-push.
- **Never poll the GitHub API (60 req/hr shared by the whole machine).** No `gh run`, `gh api`, `gh pr`, or `curl` to api.github.com. CI results come only from `tools/ci-wait.mjs`, which reads the `ci-logs` branch with git.
- Edit only the files listed under "Files you own". If you need a change elsewhere, write it under Blockers (file, the change, which item needs it) and carry on with other items. Never create the per-lane files that harness C1 creates (`selftest/probes/<lane>.rs`, `tools/smoke-labels/<lane>.txt`, `tools/ui-shots/lanes/<lane>.mjs`); merge `origin/rust-rewrite` once they exist.
- Do not edit `DESIGN.md`, `PARITY.md`, `ARCHITECTURE.md`, `REQUESTS.md`, `BACKLOG.md` (only B-149 appends to its "Post-merge findings" section), the audits, other lanes' briefs, the Electron reference (`main.js`, `lib/**` except files you own, `package*.json`, `electron-builder.yml`) or `README.md`.
- Never commit a real API key. Tests and probes use fake keys and the fake LLM.

## Verification loop

Every cycle:

1. Make a batch of changes: one or more items, each with its unit tests, probe labels, ui-shots scenes and checks, and its entries.
2. `git diff --cached --check`, `node --check` on changed `.js` and `.mjs` files, re-read the diff against the item specs.
3. Commit, then push once: `git push origin HEAD:rust/capture` (first push: `git push -u origin HEAD:rust/capture`).
4. Wait for CI with the script below. If the push touched `renderer/`, `settings/`, `history/`, `src-tauri/bridge/` or `tools/ui-shots/`, wait on `ui-shots` as well as `rust`.
5. Exit 0: record the items as green in Progress with the sha. Exit 1: read what it printed, fix, and go to step 1. Exit 2: follow the printed sha, or wait again.

The coordinator's CI recipe, verbatim:

- Usage: `node tools/ci-wait.mjs <sha> [workflow=rust|ui-shots] [--timeout-min 30]` (sha can be HEAD/short). It `git fetch`es the `ci-logs` branch every 45 s; no GitHub API.
- Exit 0 = green; 1 = failure (prints summary.md, errors.txt, last 80 lines of the failing gate's log, and the `git show origin/ci-logs:<dir>/fmt.patch | git apply` command when rustfmt fails); 2 = timeout or cancelled (prints the newer sha to wait on if a later push superseded it).
- Full logs: `origin/ci-logs:<ref with / as ->/<sha>/<workflow>/` holds summary.md, meta.json, errors.txt, and prebuild/fmt/clippy/test/build/smoke/capture/desktop .log (ui-shots: bridge/playwright/shots .log). `<ref>/latest/<workflow>/` = newest run.
- ui-shots only runs when renderer/settings/history/bridge/tools/ui-shots paths change, so `ci-wait <sha> ui-shots` exits 2 on other pushes; that is expected.
- Pushing a new commit to the same branch cancels the running CI for the older sha (concurrency cancel-in-progress), so batch changes and push once per cycle.

For this lane the log directory is `origin/ci-logs:rust-capture/<sha>/<workflow>/` and the newest run is `origin/ci-logs:rust-capture/latest/<workflow>/`. Read a file with `git show "origin/ci-logs:rust-capture/latest/rust/smoke.log"` (use PowerShell or set `MSYS_NO_PATHCONV=1` in Git Bash, which otherwise mangles the `ref:path` argument). The rust workflow takes up to an hour on a cold cache; use `--timeout-min 70` when the build is cold. When the same sha is the tip of two branches (right after merging `origin/rust-rewrite` with nothing new on top), add `--ref rust/capture`.

## Starting and staying current

1. Your worktree starts at `rust-rewrite` HEAD. Create the branch with `git switch -c rust/capture` if it does not exist.
2. Read this brief, the cross-lane tables in `.claude/rust-rewrite/BACKLOG.md` (lanes, ownership, hotspots, contracts), `DESIGN.md`, and for each item the audit sections and PARITY rows it cites. The code is the ground truth for current behaviour; `main.js` on `main` is the ground truth for Electron behaviour.
3. Push your C1 (if "Contracts you provide" lists any) before anything else, with a commit message starting `C1 contracts:`. When it is green, write `C1 green at <sha>` in Progress.
4. Each cycle, `git fetch origin`. When a contract you need has landed (`git grep -n "<symbol>" origin/rust-rewrite -- <file>`) or `rust-rewrite` has moved, `git merge origin/rust-rewrite` (a merge commit; never rebase or force-push). A conflict in a file you own: resolve it. A conflict in a file you do not own means you edited it by mistake: take `origin/rust-rewrite`'s side and note it in Progress.
5. If CI fails for a reason entirely outside your files (a broken merge on `rust-rewrite`, a runner outage), record it under Blockers with the log line and keep working on what you can verify.

## Shared documents

Never edit `DESIGN.md`, `PARITY.md` or `ARCHITECTURE.md`. In the same commit as the change, append to `.claude/rust-rewrite/entries/capture.md`:

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

- C1 green at f343f64 (rust run 42453705672). K-10 `windows::screen::{latch_active, latched, work_area_of}` (`active` is still pointer-first until B-032), K-11 `windows::screen::forced_scale` (pub), K-12 `region::set_active_on(rect: Rect, device: &str)`, K-13 `buddy::agent_point_phys(pt_px: (f64, f64), label: &str)` (async).
- Batch A (9f3adb9) passed fmt, the capture check and the desktop selfcheck, and failed clippy and tests on one closure in `hung_worker_is_replaced` that borrowed its limit; fixed in f0dd61f.
- Batch B (511ed81) passed unit tests (289) and failed fmt (one line) and clippy's MSRV check (`Option::take_if` is 1.80, MSRV 1.77); both fixed in b0ae62b.
- In CI (batch C): B-005, B-032, B-035, B-110, B-111, B-112, B-113, B-115, B-028, B-029, and the first 19 smoke labels (probes/capture.rs). Harness, windows, llm C1 and the NSIS seeding merged. The 33 desktop selfcheck labels go into capture.txt once harness's B-009 step 6 (the selfcheck inside the smoke) is on rust-rewrite; listing them before would fail the gate, since nothing prints them yet. Notes for the integrator: the DIP layout follows Chromium's ScaleOffset (the offset along a shared edge is measured on the parent's scale when a display starts after the parent's start), not the brief's simpler `phys_offset / own_scale`, so Rust's DIP bounds match `window.screenX` in WebView2 pages; both agree for displays aligned at an edge. `windows::screen::Display` gained `key: u32` (the capture-side `Display::id`), and K-10's `active()` is now foreground-first (D-09). `capture::crop_dip` and `backdrop::{crop, crop_url}` take the display id as a second argument.

## Blockers

(Format: `- B-xxx: <what blocks it>; <why>; <what would unblock it>`.)

- B-112 (partial): `src-tauri/src/lib.rs:119-121` (platform) should call `backdrop::serve_request(&request)` instead of `backdrop::serve(request.uri().path())`; the handler only gets the path today, so it cannot see `Origin` and the 403 for foreign origins never fires (the CORS header is already app-only). One-line change in lib.rs; `serve_request` exists and is unit-tested through `serve_from`.
