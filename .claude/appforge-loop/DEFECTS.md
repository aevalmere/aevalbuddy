# Defects

| ID | LOCATION | DEFECT | SEVERITY | CLASS | STATUS |
|---|---|---|---|---|---|
| D1 | lib/shots-v2.js: 18/19/20 | Find shots show the real (empty) Find, not the fixture: no count under the bar, results window says "Nothing on screen matches", lens frame blank. Prefill starts a real Find that lands after the fixture | MAJOR | harness | FIXED (iter 1, verified iter 2) |
| D2 | tray / settings / history | Same action, different names: agent = "Do a task…" / "Agent" / "Doing tasks"; toggle = "Hide the overlay" / "Hide or show" / "Show or hide overlay"; "Clear all" vs "Delete all"; "Check for updates" vs "Check now" | MAJOR | naming | FIXED (iter 1) |
| D3 | 12-live-overlay | Selected text in the bar paints Windows blue (no ::selection rule) | MINOR | palette | FIXED (iter 1) |
| D4 | overlay/settings/region css | Off-scale: 11px type (keycap, combo-group, capture-warning, region kbd); radius 5 and 10 | MINOR | scale | FIXED (iter 2) |
| D5 | 24-agent-done | Last trail row's time ("90ms") not right-aligned with the rows above | MINOR | alignment | NOT A DEFECT (iter 2: pixel measure, every time label ends at x=378) |
| D6 | 05/15/22 vs 14/25 | Secondary actions disagree: Stop is an outlined box, Retry a filled twilight chip | MINOR | consistency | FIXED (iter 2) |
| D7 | 14-answer-error | Rate-limit copy is long and says "every free model this app knows about" | MINOR | copy | FIXED (iter 3) |
| D8 | 28-onboard-4 | Tour step 4 leaves "it." alone on the last line and is the only step without its shortcut | MINOR | copy | FIXED (iter 3) |
| D9 | 11-buddy | Bubble says "click to close" under an × that already closes it | MINOR | copy | FIXED (iter 3) |
| D10 | 08/30 settings | "Show paid models" switch floats between the Models title and its rows, outside the row grid | MINOR | layout | FIXED (iter 2) |
| D11 | 03 panel | Mode label + placeholder read "Ask  Ask about your screen", "Find  Find on screen" | MINOR | copy | FIXED (iter 3) |
| D12 | 09 settings | Shortcuts page title and its only row both read "Shortcuts" | MINOR | naming | FIXED (iter 1) |
| D13 | installer smoke | updateFeed failed: GitHub answered the updater with HTTP 529 (run 42052205865). External; assert kept. Iter 5: bounded retry on 5xx/timeout in smoke and update-check.ps1 | MINOR | flake/external | FIXED (iter 5) |
| D14 | settings/history/overlay/buddy css | 20 secondary-text colours below DESIGN's 0.70 alpha floor (history meta at 0.36) | MAJOR | contrast | FIXED (iter 2) |
| D15 | renderer/trail.js setView | Switching the answer window to Find shows stale content: hits that arrived while chat was showing were never drawn (19-find-results blank after D1) | MAJOR | state | FIXED (iter 2) |
| D16 | 20-lens | Lens frame is an empty backdrop: no hits, no control frame. Root cause (iter 4): lib/lens.js sent only when !isLoading(), and isLoading() is still true inside did-finish-load, so the on-load redraw was skipped too; a fresh lens whose updates all came while loading never drew. Real product bug for any Find with no follow-up update | MAJOR | product/lens | FIXED (iter 4) |
| D17 | renderer/region.js:46 | Em dash in UI string "W × H — too small" | MINOR | copy | FIXED (iter 3) |
| D18 | lib/shots.js SAMPLE_CHAT (04/05/12) | Sample answer's list item "1. Install the package:" ends in a colon with nothing under it; the command sits after item 2, so the most-seen frame reads as a broken list | MINOR | copy/fixture | FIXED (iter 6) |
| D19 | settings/settings.html:223 (29) | Agent page row still titled "Doing tasks": the third agent name D2 set out to remove. Its value is the approval rule, not the action | MAJOR | naming | FIXED (iter 6) |
| D20 | renderer/output.html:38 (07) | Dock badge says "Docked to input"; the same card calls that window "Input bar" | MINOR | naming | FIXED (iter 6) |
| D21 | renderer/lens.js placeRing (20) | The why chip goes below the ring whatever is there, and covers the next hit (concept hit at y=264 hidden in 20-lens) | MINOR | layout | FIXED (iter 7) |
| D22 | settings/settings.css .note (29) | Agent page note ends on "saved." alone; settings notes had no orphan control (the tour card got text-wrap: pretty in iter 3, settings did not) | MINOR | typography | FIXED (iter 8) |
| P1 | lib/smoke-glass.js styleStable | NOACTIVATE compared to the pill sample, fails every pill->bar | BLOCKER | gate | FIXED (iter 1-4) |
| P2 | renderer/overlay.js ignoreState | Orb not clickable after a collapse until the pointer leaves and returns (collapseClickable) | BLOCKER | product/input | FIXED (iter 1-4) |
| P3 | main.js collapsePanel | Bar-to-orb fold plays after the window has shrunk; invisible | MAJOR | motion | FIXED (iter 1-4) |
| P4 | lib/contrast.js | sr-only live regions, ::selection text and link underlines measured as visible text | BLOCKER | gate | FIXED (iter 1-4) |
| P5 | history.css / settings.css :root | --danger and --hairline overridden with pre-glass values; Delete all 4.09 on white | MAJOR | contrast | FIXED (iter 1-4) |
| P6 | lib/backdrop.js useFixture | Fixture covers 2/3 of the screen at 125/150 (forced scale) | MAJOR | harness | FIXED (iter 1-4) |
| P7 | history search field | Inner input box drawn inside the capsule: two edges | MAJOR | visual | FIXED (iter 1-4) |
| P8 | output answer actions | Copy / Do it are bare text, no control shape | MINOR | visual | FIXED (iter 1-4) |
| P9 | lib/contrast.js clipTo (iter 1) | Clipped by the zero-height body; overlay/output frames measured runs=0 and passed | BLOCKER | gate | FIXED (iter 1-4) |
| P10 | renderer/cursor-buddy.css #callout-actions .act | Tour Skip is a bare word next to a capsule primary | MINOR | visual | FIXED (iter 5) |
| P11 | renderer/overlay.css #chat-tools | Toolbar overlaps the transcript by 10 px; scrolled bubble slides under the icons at 100% | MINOR | layout | FIXED (iter 6) |
