# Entries from the capture lane

Folded into DESIGN.md, PARITY.md and ARCHITECTURE.md by the merge agent.

## DESIGN · The screen the user is actually looking at · D-09
Replace "Both follow the mouse pointer now, and the shot carries its display with it, so the ratio mapping
is already right once the correct one is picked." with:
Both follow one definition of the active display now: the monitor of the foreground window when that
window is not ours, else the display under the pointer, else the display nearest the pointer. The bar
latches it when it opens, so a question typed into the bar (which has focus by then) is about the screen
the person was working on, and capture and window placement read the same latch. The shot carries its
display with it, so the ratio mapping is already right once the correct one is picked. Reason: the
pointer is often parked on another monitor while the person types into an app on this one; the
foreground window is the better witness of where they are looking (D-09).

## DESIGN · v0.6: the pipeline was starving its own brain · D-09
Add (after the table):
The Rust build spends the same budget on real pixels. The whole screen is sized from the display's
physical pixels, not its DIP size: a 1920x1080 display at 150% goes out at 1600x900, where Electron sent
1280x720 because it asked `desktopCapturer` for a thumbnail at the DIP size. JPEG chroma is kept at full
resolution (4:4:4), where Skia's encoder halved it both ways (4:2:0) and smeared coloured text and thin
coloured lines. Both stay inside the 1.6 MB budget at quality 88. Reason: answer quality is bounded by
capture fidelity (D-09).

## ARCHITECTURE · D1 Capture · D-09
Replace "downscale, and a JPEG encoder with the same quality ladder (88, -8 steps, floor 62, 1.6 MB budget)." with:
downscale from the display's physical pixels to a 1600 px long edge, and a JPEG encoder at 4:4:4 chroma
with the same quality ladder (88, -8 steps, floor 62, 1.6 MB budget). The whole screen is the active
display (D-09): the foreground window's monitor when that window is not ours, else the display under the
pointer, else the nearest one. `windows::screen::latch_active()` records it when the bar opens;
`windows::screen::latched()` and `capture::screen::latched_display()` read the same latch.

## ARCHITECTURE · D11 `screen.*` row · B-005
Replace "`windows/screen.rs` over `EnumDisplayMonitors` + `GetDpiForMonitor`, same rounding as Electron" with:
`capture/screen.rs` enumerates (`EnumDisplayMonitors` + `GetDpiForMonitor`) and builds Chromium's DIP
layout once (`dip_origins`): the primary keeps (0, 0); every other display is placed edge to edge with the
display it touches physically, walking out from the primary (nearest first, then device name), so no two
DIP rects overlap and a DIP rect names one display. The offset along the shared edge is Chromium's
ScaleOffset: measured on the parent's scale when the display starts after the parent's start, on its own
scale when it starts before. `windows/screen.rs` keeps its API over the same layout. Reason: WebView2
pages report `window.screenX` in Chromium's layout, so any other layout puts page and Rust coordinates on
different displays; the old origin/scale rule overlapped at mixed DPI (a 150% display at x=1920 started
at DIP 1280, inside the 100% primary).

## PARITY · POS-24 · B-005
status: done; evidence: UNIT `windows::screen::tests::overlapping_mixed_dpi_layout_is_adjacent`, `windows::screen::tests::rect_in_old_overlap_maps_to_primary`, `capture::screen::tests::layouts_never_overlap_whatever_the_scales`; SMOKE+ `dpiRoundTrip` follows when the harness probe API lands

## PARITY · CAP-07 · B-005
status: done; evidence: UNIT `capture::screen::tests::matching_picks_most_overlap_by_id_not_order`, `capture::screen::tests::ties_go_to_the_primary_then_the_centre`, `capture::tests::area_on_primary_crops_primary`

## PARITY · CAP-01 · B-032
Replace "Whole screen: the display under the pointer (fallback primary), not the primary." with:
Whole screen: the display the bar was opened on (`latched`), defined once (D-09): the foreground window's
monitor when that window is not ours, else the display under the pointer, else the nearest display to
the pointer, else the primary.
status: changed (D-09); evidence: UNIT `capture::screen::tests::active_prefers_foreground_monitor`, `capture::screen::tests::latch_finds_its_display_only_while_it_exists`

## PARITY · CAP-02 · B-032
Replace "Whole screen is downscaled so the long edge is at most 1600 px (never upscaled);" with:
Whole screen is taken at the display's physical pixels and downscaled so the long edge is at most 1600 px
(never upscaled; D-09, Electron used the DIP size);
status: changed (D-09); evidence: UNIT `encode::tests::whole_screen_uses_physical_pixels_within_budget` (replaces `encode::tests::whole_screen_matches_electron_thumbnail_sizes`), SMOKE `capture`, capture check `captureSize`

## PARITY · CAP-05 · B-032
Replace "JPEG quality 88, stepping down by 8 while over 1.6 MB, floor 62." with:
JPEG quality 88 with 4:4:4 chroma (D-09; Electron's Skia encoder used 4:2:0), stepping down by 8 while
over 1.6 MB, floor 62.
status: changed (D-09); evidence: UNIT `encode::tests::chroma_is_444`, `encode::tests::ladder_starts_at_88_and_stops_at_62`

## PARITY · CAP-06 · B-110
status: done; evidence: UNIT `capture::tests::hung_worker_is_replaced` (a capture that takes over 3 s fails its call and a fresh capture thread serves the next one); SMOKE+ `captureFailNull` follows when the harness probe API lands

## PARITY · GLASS-02 · B-035
Replace "Tier solid when: setting solid, cannot hide from capture," with:
Tier solid when: cannot hide from capture (the stored `surface: 'solid'` is no longer a reason since
B-035: `glassIntensity` 0 is the solid path, so a profile migrated as solid follows the slider; the key
stays in the file for rollback, NEW-12, REQ-5),
status: changed (NEW-12); evidence: UNIT `backdrop::tests::solid_profile_at_07_is_live`, `backdrop::tests::transparency_setting_is_not_a_reason`; capture check `glassLive`

## PARITY · GLASS-02 · B-111
status: done; evidence: UNIT `backdrop::tests::busy_resets_after_panic`, `backdrop::tests::recovery_rebroadcasts_live` (after three capture failures the glass is retried every 30 s, not at the fast pace, and the pages hear `glass:tier live` when a capture succeeds); SMOKE `glassTier`

## PARITY · GLASS-01 · B-112
Add to the row: frames are readable only by our own pages: the `aeval-frame` protocol sends
`Access-Control-Allow-Origin: http://tauri.localhost` (never `*`) and refuses another `Origin` with 403.
status: done; evidence: UNIT `backdrop::tests::frame_protocol_cors_is_app_only` (the 403 path is live once lib.rs passes the whole request, see the lane's Blockers); SMOKE `glassLoaded`

## PARITY · GLASS-01 · B-113
Add to the row: a window whose display has no frame this tick keeps its last frame; it is never sent
another display's frame at the wrong bounds.
status: done; evidence: UNIT `backdrop::tests::missing_display_frame_skips_window`

## PARITY · LIFE-16 · B-115
status: done; evidence: UNIT `desktop::checks::tests::notepad_gating` (Notepad only when `GITHUB_ACTIONS` is set or `AEVAL_DESKTOP_NOTEPAD` is set and not "0"; the Rust port had defaulted to on); desktop selfcheck log `desktopNotepadIncluded=ok`

## DESIGN · Components table, "Tour card" row · D-06
Replace "| Tour card | pointer window | the last tour step has no arrow: a card with `Try it` (primary) and `Later` |" with:
| Tour card | pointer window | the step that asks for a key has no arrow: a card with `Connect` (primary, opens Settings at Providers) and `Later`; the last step points at the bar's intent token with `Got it` |
Reason (D-06): the tour leads with what works without a key (Find), asks for a key only when there is none,
asks the first question only once there is one, and ends on the token that says what Enter will do. Every
step quotes only the `captureQuery` chord; an empty chord falls back to "Click the orb", never a blank.

## PARITY · TOUR-01 · B-033
Replace "Four steps: key in Settings (`#api-key-input`, scope `#sec-key`, advance `key-saved`), Ask (`first-answer`), Find (`find-hit`), Agent card with Try it / Later. Messages carry the live chords." with:
Four steps (D-06): Find on the orb (`find-hit`), a card asking for a provider with Connect / Later (`key-saved`,
skipped when a key exists), Ask (`first-answer`, skipped without a key), the bar's intent token with Got it.
Only the main chord is quoted, never an empty one. The old Try card is gone (`try` still works, B-129).
status: changed (D-06); evidence: UNIT `buddy::tests::walkthrough_order_and_no_double_spaces` (replaces `buddy::tests::tour_has_four_steps_with_live_chords`), `buddy::tests::steps_apply_by_whether_a_key_exists`; SMOKE `onboardingSteps` (`0>1>2>3`), `walkthrough`

## PARITY · TOUR-04 · B-031
Add to the row: the pointer window is kept between sessions (hidden, page reset with `buddy:reset`), so a
new answer's points show at once and the agent's first cursor after a tour does not wait for a page.
status: done; evidence: SMOKE+ `buddyReuse`; SMOKE `buddyPoints`, `buddyPointsDone`

## PARITY · LENS-01 · B-034
Add to the row: the lens and pointer pages invoke `lens:ready` / `buddy:ready` after their listeners are
registered; Rust flushes the queued draw or show on that invoke, not on the page-load event.
status: done; evidence: SMOKE `lensFirstDraw` (`3:true:true`), SMOKE+ `lensReady`, SMOKE `pointResolved`

## PARITY · GLASS-01 · B-030
Add to the row: only displays hosting a visible glass window are captured; a hidden lens window does not
hold the fast pace, and the lens windows go when Find's hits and the agent frame are both gone. A frame
DXGI reports unchanged is not converted or encoded again; frames are shrunk while still BGRA.
status: done; evidence: SMOKE `snapCost`, SMOKE+ `backdropIdle`; capture check `tickMs` (< 40 ms per 1080p frame), `tickReusesFrame`

## PARITY · LIFE-12 · B-114
Add to the row: the lens, region and buddy builders call `crash::guard_webview`; a reloaded or crashed
buddy page lets the mouse through again, and a page that holds the mouse with no word for 3 s while the
pointer is off its bubble has click-through restored.
status: done; evidence: UNIT `buddy::tests::watchdog_restores_click_through`; SMOKE+ `buddyReload`

## ARCHITECTURE · D1 Capture · B-108
Add: a failed DXGI open keeps that output on GDI for 10 s (cleared when the display layout changes), and
outputs on one adapter share one D3D11 device, keyed by adapter LUID.
evidence: UNIT `dxgi::tests::open_failure_backs_off`; capture check `dxgiOpensPerTick` (≤ 1 over a forced-failure burst)

## ARCHITECTURE · D1 Capture · D-33
Add: an HDR output (`IDXGIOutput6` colour space G2084/P2020) is duplicated with `DuplicateOutput1` in
`R16G16B16A16_FLOAT`; each linear scRGB channel is divided by the output's SDR white level
(`DISPLAYCONFIG_SDR_WHITE_LEVEL`, 200 nits when unreadable), clamped and sRGB-encoded through a 64K-entry
lookup table. SDR outputs keep the BGRA path. Reason: an HDR desktop read as 8-bit comes out washed out or
clipped, and the model reads tone as content. Not verified on HDR hardware (lane Blockers).
evidence: UNIT `dxgi::tests::scrgb_to_srgb_at_sdr_white`; capture check `hdr=off` on the runner

## PARITY · LENS-01 · B-117
Add to the row: the current hit's loupe is a thin glass rim (inner 1 px white line, 1.5 px bright accent,
2.5 px dark rim, soft 6/16 drop) at radius 8 with no top highlight; the control frame is DESIGN's 2 px
`--accent` with a 1 px dark outer line; the why chip is a capsule on one line and r16 once it wraps,
padding 8/12. lens.css uses glass.css tokens only (`--skin` and its own accent values are gone).
status: done; evidence: SHOT `lens-hits-white`, `lens-hits-dark-editor`; lens.css in `cssLintEnforce`

## PARITY · CAP-11 · B-118
Add to the row: the hint reads "Drag to select an area" with an `Esc` keycap and "Arrows move,
Alt+arrows resize"; the page is `role="application"` named "Choose an area of the screen", and a polite
live region says "Area W by H. Arrows move it, Alt+arrows resize, Enter asks, Esc cancels." on the first
key, then the size as it changes (at most every 500 ms).
status: done; evidence: SHOT `region-idle-white`, `region-idle-dark`, `region-dragging`, `region-too-small`; check `regionAnnouncesSize`
