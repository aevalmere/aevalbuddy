# Design review: visual and interaction craft

Reviewer lens: principal product designer. The bar is Apple first-party work (macOS Tahoe and iOS 26 Liquid Glass, Spotlight, Siri and Apple Intelligence surfaces, Control Center), plus Raycast and Arc.
Branch `rust-rewrite`. Read-only review. I did not run the app.

Evidence:
- `origin/shots:rust-d-ui/latest/*`: the 16 Rust renderer shots.
- `origin/shots:latest/*`: the Electron v2.1 set and the `m-*` matrix.
- The renderer source, cited as `file:line`.
- Three extra headless-Edge renders, made with the same mock bridge and backdrop fixture as `tools/ui-shots/shots.mjs`. The script wrote only to the scratchpad and used none of the app:
  - `…/scratchpad/design-visual/extra/A-output-maximised.png`: an output window at 1600x1000, with `#chat` padding measured in the page.
  - `…/scratchpad/design-visual/extra/B-output-markdown-150.png`: a markdown torture answer at 150% DPR.
  - `…/scratchpad/design-visual/extra/C-glob-strip.png`: glob frames with the content fade allowed to finish.

(Full scratchpad path: `C:\Users\LIGHT_~1\AppData\Local\Temp\claude\1fc9f557101c46d6\cdb268a1-f823-45a8-ab51-d9febb9899c7\scratchpad\design-visual\`.)

---

## 1. Verdict

1. **Already at Apple level.** The material architecture: a measured scrim, a one-device-pixel specular rim, and the 56 px capsule bar. The intent token's design is concentric. There is one family of motion curves. Settings and History are calm grouped lists at default size. The restraint holds everywhere: no decorative gradients, glows, uppercase labels or emoji, and text colours are solid. The bar's idle state and the closing fold are first-party quality.
2. **Biggest gap: the glob, the signature motion, does not read as liquid.**
   - It reads as a 240 px pill spawned below the bar, with a hard-cornered trapezoid "tab" and a visible seam and colour change at the bar's rim.
   - The separation happens inside a 12 px gap after the sheet is already 93% grown.
   - The content is hidden under the glob and pops in at 440 ms. Some of it leaks outside the glass meanwhile.
3. **The slider does not deliver.** "Very very" is indistinguishable from "Liquid": the material is about 83% opaque at every stop, and Settings and History never refract. Nothing adapts to light content, so over a white document every surface becomes a flat grey slab.
4. **Answers are not yet a typeset document, and "full screen" cannot happen.**
   - Loose lists number "1. 1. 1.", tables print raw pipes, nested lists flatten, `##` headings are body size, and code blocks have no Copy.
   - A maximised output runs 1500 px lines, because a CSS shorthand cancels the reading measure.
   - Rust caps Settings, History and outputs at 900 px, so request #4's full-screen layout cannot be reached.
5. **Consistency is short of one system.** There are two title-bar systems, two menu materials, and five icon stroke weights. Text is duplicated: the question shows twice, and a task's wait is announced three times. Small copy and elevation details read as a web template, not as first-party work.

Counts: **S1 = 9, S2 = 20, S3 = 13** (42 findings).

---

## 2. Findings (most impactful first)

Key: **R** means renderer-only. **R+Rust** means it needs Rust-side work too.

### S1: visibly broken or inconsistent

#### F1. S1. The glob hides its own content, then the content pops in; text leaks outside the glass
- **Surface and shots:** output open. `rust-d-ui/latest/04d-glob-70.png` and `04e-glob-90.png` show no content at 70% and 90%. `extra/C-glob-strip.png`, frame 0.58, shows the usage line visible *below* the glob edge on the bare wallpaper.
- **Cause:**
  - `#glob` is `position: fixed; inset: 0; z-index: 3` (`renderer/overlay.css:661-667`). It stacks above all of `#opanel`.
  - Its backdrop layer `.gs` is an opaque picture of the desktop. So when `reveal` turns the words on at t > 0.55 (`renderer/glob.js:304-306`, `overlay.css:647-650`), they are covered everywhere inside the glob.
  - The words are visible only where they overhang the still-smaller shape. At t = 1 `finish()` hides the glob (`glob.js:276-280`) and the full text appears in one frame.
  - Close is abrupt too: `close()` adds `globbing` with no transition, so the words cut to 0 in one frame (`glob.js:326`, `overlay.css:642-645`).
- **Fix (R):**
  1. Z-order: set `#glob { z-index: 0; }`. While globbing, add `#opanel.globbing > :not(.gl) { position: relative; z-index: 1; }`.
  2. Clip the words to the shape while globbing. In `draw(d)` (`glob.js:216`), also set `sheet.style.clipPath = path('<d translated by -sheetRect.left, -sheetRect.top>')`. Give `pt()` an offset argument so that `poly()` and `roundRect()` can emit the translated string. Clear `clip-path` in `finish()`.
  3. Reveal at grow ≥ 0.95, not at t > 0.55. Use the new timeline in F2 (T_REVEAL = 0.70). CSS:
     `#opanel.globbing > :not(.gl) { opacity: 0; filter: blur(4px); transform: translateY(4px) scale(0.99); }`
     `#opanel.globbing.reveal > :not(.gl) { opacity: 1; filter: none; transform: none; transition: opacity 140ms var(--ease-out), filter 140ms var(--ease-out), transform 200ms var(--ease-morph); }`
  4. Close: `#opanel.closing > :not(.gl) { opacity: 0; filter: blur(3px); transition: opacity 80ms ease, filter 80ms ease; }`. Add `closing` 80 ms before the shape starts reversing.
  5. Under reduced motion, keep the existing 150 ms fade.

#### F2. S1. The glob shape is a detached pill with a hard-cornered tab, a seam at the bar, a material change, and an invisible separation
- **Surface and shots:** `04a-glob-08` through `04e-glob-90`, and the zoomed crops `crops/glob08.png` and `glob70.png`.
- **What is wrong:**
  - **Spawns fully formed.** At t = 0 the bud is already `budW = max(96, 40% of bar)` wide and 26 px tall, 4 px below the bar (`glob.js:186-188`). The 8% frame is a finished 240 x 34 capsule, not something emerging.
  - **Hard corners.** The neck's cubics leave the bar diagonally: `c1 = [u0 - w, v0 + d*0.55]` (`glob.js:159-164`). Every join, with the bar and with the bud, is a sharp corner. It reads as a trapezoid or bow-tie, not a meniscus.
  - **Seam.** The neck reaches only 2 px into the bar (`B.v1 - 2`, `glob.js:203`). The bar's own bottom rim line runs straight across the neck.
  - **Material change.** The glob is thick glass, much darker plum, while the bar is thin glass, lighter magenta. "One body" visibly changes material at the join.
  - **Late pinch.** The pinch is at t = 0.6, when `easeSwell` has the sheet at 93% (computed: grow(0.6) = 0.929). The neck only ever spans the 12 px `OUTPUT_GAP` (`src-tauri/src/windows/layout.rs:22`), so no one sees a separation.
  - **Flat bead.** The bar-side bead is a half-ellipse with a flat top cut at the bar edge, which reads as a cut-off bowl (70% frame).
  - **Pops at the end.** The glob has no shadow (`#glob > .gl { box-shadow: none }`, `overlay.css:669-672`). At t = 1 the sheet's shadow appears in one frame, and refraction pops in 120 ms later (`glass.js:400-405`).
- **Fix (R).** Rewrite `shapeAt()` (`glob.js:181-210`) to this timeline. `D_OPEN = 480`, `D_CLOSE = 260`, t is linear time 0..1. New easing `easeSpring = bezier(0.2, 0.9, 0.25, 1)`, which approximates a spring with response 0.42 s and damping 0.88, with no overshoot.

  | Phase | t | What is drawn |
  | --- | --- | --- |
  | A, drop forms | 0 → 0.18 (0-86 ms) | One meniscus hanging from the bar edge at `u0`. Width `lerp(56, 140, easeOut(t/0.18))`, depth below `B.v1` `lerp(0, 22, …)`. Its top sits **6 px inside the bar** (`OVERLAP = 6`), so it covers the bar's rim where it attaches. The joins have horizontal tangents (fillet radius ≈ 10). |
  | B, stretch and pinch | 0.18 → 0.62 (86-298 ms) | Body rect `q` goes from the drop's rect toward `S` with `grow = easeSpring(clamp((t-0.08)/0.78, 0, 1))`. Neck from `B.v1 - OVERLAP` to `q.v0 + 8`. Attach half-width `a = lerp(70, 10, s)`, sheet half-width `b = lerp(70, 18, s)`, waist `w = lerp(60, 1, s**1.4)`, where `s = smooth(0.18, 0.62, t)`. **Tangents horizontal at both ends:** `c1 = [u0 - w, v0]`, `c2 = [u1 - w, v1]` (mirror for the right side). Pinch at 0.62, with the sheet at about 85%. |
  | C, recoil | 0.62 → 0.82 (298-394 ms) | Bar-side bead: half-width 10 → 0, height 6 → 0, retracting toward the bar. Sheet-side bump: half-width 18 → 0, height 5 → 0, on `--ease-out`. No overshoot (DESIGN: windows never bounce). |
  | Land | 0.86 → 1.0 (413-480 ms) | `grow` reaches 1 at t = 0.86. The sheet's own `.gl`, with shadow and refraction, fades 0 → 1 over 70 ms while `#glob` fades 1 → 0. Then `finish()`. |
  | Words | from 0.70 (336 ms) | F1 reveal, 140 ms. |

  - **Material.** Give `#glob` a second, inner layer `<div id="glob-thin" class="glass thin">` at `inset: 0`, clipped by the same path. Fade its opacity 1 → 0 over t 0.10 → 0.55. Set its `--scrim` from `AevalGlass.scrimFor(barRect, 'thin')`, a new export that runs `measure()` for a rect. The 6 px overlap is then the same picture, scrim and blur as the bar, and the join disappears.
  - **Rim.** In `#glob-mask` (`renderer/output.html:23-26`), add a black `<rect>` covering the bar rect plus 1 px, so the glob's rim stroke is never drawn inside the bar. Use `stroke-width: calc(2 * var(--px))` so the rim stays one device pixel.
  - **Shadow during the morph.** `#glob { filter: drop-shadow(0 4px 12px rgb(0 0 0 / 0.28)); }`. The clip is on `.gl`, the child, so the parent's drop-shadow follows the shape.
  - **Close** stays the reverse on `--ease-glide` over 260 ms. The words leave first (F1.4) and the sheet `.gl` hands over to the glob in 60 ms.
  - **Gate.** Keep `globSmooth < 34 ms`. Add a harness check that at t = 0.04 the shape's area is under 8% of the sheet's.

#### F3. S1. The markdown renderer breaks common answer structures
- **Surface and shots:** every answer. `extra/B-output-markdown-150.png` and `extra/A-output-maximised.png`.
- **What is wrong (`renderer/markdown.js`):**
  1. **Loose lists.** A blank line calls `closeList()` (`:93-98`), so items separated by blank lines become separate `<ol>` and render **"1. 1. 1."**. Models emit loose lists constantly.
  2. **Tables.** No table support: the rows print as raw `| a | b |` lines.
  3. **Nesting.** Indentation is ignored, so nested lists flatten.
  4. **Paragraphs.** Every source line becomes its own `.md-p` with 8 px margin, so soft-wrapped paragraphs split into blocks.
  5. **Numbering.** `start` numbers are lost (`3.` after a code block restarts at 1).
  6. **Headings.** `h4`-`h6` collapse to `h3` with no style difference from `h2` (see F16).
- **Fix (R):**
  - **Lists:** keep a list open across blank lines when the next non-blank line is an item of the same type. Emit `<ol start="N">` from the first item's number. Track indent per line, `Math.floor(leadingSpaces / 2)`, to open nested `<ul>`/`<ol>` inside the current `<li>`.
  - **Paragraphs:** join consecutive non-blank text lines into one `.md-p` with a space.
  - **Tables:** a header row matching `^\|.*\|$` followed by a separator matching `^\|?\s*:?-{3,}` starts a table. Parse cells, alignment from the colons, and `inline()` per cell. Emit `<div class="md-table"><table>…</table></div>`.
  - **Table CSS** (`overlay.css`, after `:1097`):
    `.md-body .md-table { overflow-x: auto; margin: 8px 0 12px; border-radius: 12px; box-shadow: inset 0 0 0 var(--px) var(--hairline); }`
    `.md-body table { border-collapse: collapse; width: 100%; font-size: 14px; line-height: 20px; font-variant-numeric: tabular-nums; }`
    `.md-body th { font-weight: 600; text-align: left; color: var(--label); background: var(--fill-1); }`
    `.md-body th, .md-body td { padding: 8px 12px; }`
    `.md-body tr + tr td { box-shadow: inset 0 var(--px) 0 var(--hairline); }`
    `.md-body td[data-align="right"] { text-align: right; }`
  - **Nested lists:** `.md-body li > ul, .md-body li > ol { margin: 4px 0 0; }`. Use `list-style: disc` at level 1, `circle` at level 2, and a 20 px indent per level.
  - **Gate:** add a markdown fixture to the harness (section 4).

#### F4. S1. A maximised or wide output has no reading measure
- **Surface and shots:** `extra/A-output-maximised.png`. Measured in the page at 1600 px: `#chat` paddingLeft `0px`, width 1528. Lines run about 200 characters.
- **Cause:** `#chat` sets `padding-inline: max(0px, calc((100% - 760px) / 2))` at `overlay.css:712`. The shorthand `padding: 2px 2px 2px 0` at `:718` then overrides it. DESIGN's "past 760 px the transcript centres" never happens. 760 px at 15 px is also about 105 characters, too long for prose.
- **Fix (R):** delete `:718`. Set `padding-block: 2px; padding-inline-start: max(0px, calc((100% - var(--measure)) / 2)); padding-inline-end: max(8px, calc((100% - var(--measure)) / 2));` with `--measure: 640px`. That is about 68 characters at 15/24 Segoe UI Variable Text.
  - Keep `.md-body pre` and `.md-table` free to use the full measure.
  - Apply the same measure to `#find-view`.
  - Update DESIGN's "760" to 640, and record why: line length.

#### F5. S1. Settings, History and outputs cannot go full screen; a maximised sheet keeps its corners and margins
- **Surface and shots:** `rust-d-ui/latest/07-settings-fullscreen.png` shows a 1600 px Settings. The app cannot produce it:
  - `frame_meta()` sets `max: (900.0, 0.0)` for Settings and History (`src-tauri/src/windows/mod.rs:895-902`).
  - Outputs use `max: (store::MAX_W, 0)` with `MAX_W = 900` (`src-tauri/src/windows/outputs.rs:31-38`, `src-tauri/src/store/mod.rs:42`).
  - `WM_GETMINMAXINFO` writes that into `ptMaxTrackSize` (`flags.rs:395-412`), and Windows clamps a maximised window to it.
  - So request #4 fails in the real app, and the shot misleads.
  - A maximised page also keeps its 28 px rounded sheet inside transparent 14/18/22 px margins (`settings/settings.css:923-926`, `history/history.css:362-365`). A floating card on top of a maximised window reads as broken.
- **Fix (R+Rust):**
  - **Rust:** `frame_meta().max = (0.0, 0.0)` (unbounded). Output `max.0 = 0.0`, or the work-area width. Keep the minimums.
  - **Renderer:** listen to Tauri `getCurrentWindow().onResized` and `isMaximized()`, and set `html[data-maximized]`. Then `html[data-maximized] body { padding: 0 } html[data-maximized] #app, html[data-maximized] #opanel { --r: 0px; border-radius: 0; }`, and hide the frame handles (`frame.js`: `display: none` when maximised).
  - **Harness:** keep shot 07, but also assert the Rust limits through `bridge.test.mjs`, or a Rust unit test that max is 0.

#### F6. S1. "Very very" looks the same as "Liquid"; large sheets never refract
- **Surface and shots:** `06-settings-glass-050.png` vs `06-settings-glass-100.png`. Both the Settings sheet and the answer window are visually identical.
- **Cause:**
  - The thick scrim only moves `(0.5 - k) * 0.16 * (1 - y)` (`renderer/glass.js:46-49`). Over the 90th-percentile luma of a normal wallpaper (y ≈ 0.75-0.85) that is −0.01 to −0.03, so the sheet stays about 83% opaque.
  - Blur *falls* as k rises (`glass.css:149-157`). The interior gets busier, which is the opposite of what keeps a reading surface legible.
  - Settings and History are `class="glass thick"` with no `refract` (`settings/settings.html:12`, `history/history.html:12`), so lensing never appears there.
  - The fixture wallpaper is a smooth blur, so even where refraction exists the harness cannot show it (section 4).
- **Fix (R).** Make liquidity live at the **edge**, where text never sits, and keep the reading interior frosted. This is how Liquid Glass reads clear and stays legible.
  1. **Edge-clear scrim.** In `.glass > .gl` (`glass.css:123-133`), `background: rgb(var(--tint) / var(--scrim-edge))` with `--scrim-edge: calc(var(--scrim) - 0.26 * var(--spec))`. In `.glass > .gl::before` (`:160-175`), draw the interior difference: `inset: calc(var(--band) * 0.5); border-radius: calc(var(--r) - var(--band) * 0.5); filter: blur(calc(var(--band) * 0.5));` and set its scrim colour `rgb(var(--tint) / calc((var(--scrim) - var(--scrim-edge)) / (1 - var(--scrim-edge))))`. At k ≤ 0.5 `--spec = 0`, so nothing changes. At k = 1 the outer band shows the desktop at about 26 points less scrim, while text (inset ≥ 16 px) keeps the measured scrim. `glass.js` sets `--band` to the refraction band per layer.
  2. **Blur stays high for reading.** `.glass.thin { --gf: blur(calc(18px - 8px * var(--glass-k))) saturate(calc(1.1 + 1.2 * var(--glass-k))) brightness(0.94); }` gives 18 → 10. `.glass.thick { --gf: blur(calc(32px - 8px * var(--glass-k))) saturate(calc(1.1 + 0.8 * var(--glass-k))) brightness(0.9); }` gives 32 → 24. Both match today exactly at k = 0.5.
  3. **Stronger lensing at Very and Very very.** In `glass.js:362-363`: `band = Math.round(4 + 20 * k + 8 * spec)` (14 at 0.5, 32 at 1) and `scale = Math.round(base * 2 * k + 12 * spec)` (thin 56, thick 68 at 1), where `spec = Math.max(0, 2 * k - 1)`.
  4. **Thickness.** Append to `::before` `box-shadow`: `inset 0 -12px 24px -16px rgb(0 0 0 / calc(0.35 * var(--spec))), inset 0 0 0 var(--px) rgb(255 255 255 / calc(0.08 * var(--spec)))`. Make the rim's 318° stop `rgba(255,255,255, calc(0.62 + 0.23 * var(--spec)))`.
  5. **Refract everywhere.** Add `refract` to `#app` in Settings and History. In `glass.js:243-249`, when the window moves, call `morph(200)` so the displacement is dropped during a drag, as it already is for morphs.
  6. **Scrim slope.** Double it over dark backdrops (still 0 at y = 1, so the white-page floor holds): thin `(0.5 - k) * 0.32 * (1 - y)`, thick `(0.5 - k) * 0.30 * (1 - y)`.
  7. **Record the new level table in DESIGN** (section 3).

#### F7. S1. Two title-bar systems
- **Surface and shots:** `03-output-answer` and `05-outputs-multi` against `06-settings-*`, `08-settings-account` and `09-history`.

  | | Outputs | Settings and History |
  | --- | --- | --- |
  | Title | left-aligned, 13/600, after a kind chip | centred, 14/600 |
  | Height | 44 px | 52 px |
  | Close | 28 px button, 8 px from the edge | 32 px button at 12/12 |
  | X stroke | 1.7 | 2.0 |

  Sources: `overlay.css:575-617`, `settings/settings.css:948-975`, `history/history.css:382-409`. Request #7 asks for one formatting system.
- **Fix (R).** Make one `.sheet-bar` component in `glass.css` and use it in all three pages. Left-aligned is the Windows 11 convention, and outputs need the room for tools.
  - `height: var(--sheet-bar-h) /* 44 */; padding: 0 8px 0 16px; display: flex; align-items: center; gap: 8px;`
  - Title: `font: 600 13px/18px var(--font-text); color: var(--label); flex: 1; min-width: 0; text-overflow: ellipsis`.
  - Tools: `.g-icon.small` at 28 px with `gap: 4px`. The last button's right edge sits 8 px from the glass, vertically centred at 22 px.
  - The hairline under the bar appears only when content scrolls under it, as now.
  - Settings and History drop `justify-content: center` and the absolutely positioned close, and use the same 28 px close.
  - All X glyphs come from one shared SVG (F20).

#### F8. S1. Duplicated content and instructions (the spirit of request #6)
- **Surface and shots:**
  - `03-output-answer`, `05-outputs-multi` and `10-white-bar-output`: the first question is the window title *and*, directly below it, the right-aligned question chip.
  - `05-outputs-multi`, task window: "Task" appears in the title chip *and* the trail head (`renderer/trail.js:131`). "Waiting for your answer" appears in the head status (`trail.js:253`) *and* "Waiting for your answer in the bar" on the row (`trail.js:200`) *and* the bar placeholder (`bar.js:424-428`).
  - `latest/25-agent-error.png`: the failed row says "Refused: that window runs as administrator…", and the banner repeats it ("The agent stopped: that window runs as administrator…").
- **Fix (R, plus Rust copy):**
  - `output.js:151-162`: for the first user message of a conversation, add class `is-title` and style `.msg.user.is-title` as `.sr-only` (`overlay.css:68-76`). Follow-ups show the chip, and copy-all still includes it.
  - `trail.js:131`: drop the `trail-title` "Task" span.
  - `trail.js:253`: `asking: 'Step N'`, no "Waiting…".
  - `trail.js:200`: shorten to "Answer in the bar", in accent as now.
  - **Rust** `src-tauri/src/agent/run.rs`: the final error message carries only the remedy when a failed step already shows the reason ("Start Outlook normally, then Retry."). Rename "The agent stopped" to "The task stopped" (also `:68`, `:739`, `:1971`). The surfaces call it a task.

#### F9. S1. Liquid glass slider: stop labels sit off their stops; no ticks
- **Surface and shots:** `06-settings-glass-*`. "Subtle" is centred at 30% and "Very" at 70%, but the stops are at 25% and 75%. The ends are fine.
- **Cause:** `.glass-stops { grid-template-columns: repeat(5, 1fr) }` (`settings/settings.css:1414-1429`).
- **Fix (R):**
  - `.glass-stops { position: relative; height: 16px; }`
  - `.glass-stops span { position: absolute; top: 0; transform: translateX(-50%); white-space: nowrap; }`
  - Place span n at `left: calc(11px + (100% - 22px) * n/4)`. The thumb is 22 px, so its centre travels from 11 px to 100% − 11 px. First and last: `transform: none` with `left: 0` / `right: 0; left: auto`.
  - Ticks on the track (`:1383-1388`): prepend to the background `radial-gradient(circle at center, rgb(255 255 255 / .5) 0 1.5px, transparent 2px) 11px 50% / calc((100% - 22px) / 4) 6px repeat-x`.
  - Snap magnetically: in settings.js, on `change`, round to the nearest 0.25 when within 0.04.
  - The current stop's label is `--label`, the others `--label-2`.

### S2: below the bar

#### F10. S2. No adaptive tone: over light content everything is a grey slab
- **Surface and shots:** `10-white-bar-output`, `latest/m-*-white-*` (tour, answer, settings).
- **Cause:** one dark tint everywhere. iOS 26 and macOS Tahoe glass flips to a light rendering over light content. That is why it can be clear *and* legible. This is the largest perceived gap with Apple after the glob.
- **Fix (R).** Per-window tone.
  - **When to flip.** `glass.js` computes the median luma under the window's largest glass layer (y50, beside the y90 it already computes). Set `html[data-tone="light"]` when y50 > 0.72 for 2 consecutive frames. Back to dark when y50 < 0.55, the hysteresis.
  - **Crossfade.** Over 300 ms. `--scrim` already transitions, so add a transition on the label colours.
  - **Light tokens:** `--tint: 246 247 250; --label: #1C1C1F; --label-2: #3C3D44; --label-3: #4F515A; --fill-1: rgb(0 0 0 / .05); --fill-2: rgb(0 0 0 / .08); --fill-3: rgb(0 0 0 / .12); --hairline: rgb(0 0 0 / .10); --chip-rim: inset 0 0 0 var(--px) rgb(0 0 0 / .10);`
  - **Light rim:** keep the white specular, add `0 0 0 var(--px) rgb(0 0 0 / .12)` outside.
  - **Light scrim:** thin `a = 0.42 + 0.30 * (1 - y)`, thick `a = 0.62 + 0.20 * (1 - y)`.
  - **Light accent:** `--accent` (#823A80) on light. `.glass.thin` text-shadow becomes `0 1px 0 rgb(255 255 255 / .4)`.
  - The contrast gate (`lib/contrast.js`) gains light-tone runs.
  - Record it in DESIGN under Material, as a departure from "dark tint always".

#### F11. S2. The bar and the sheet look like two materials side by side
- **Surface and shots:** `03-output-answer`. The thin bar is a saturated magenta and the thick sheet a dark plum. Request #7 asks for the closing bar and the outputs to be consistent. The glob also exposes the difference (F2).
- **Cause:** thin `0.46 + 0.32y` against thick `0.72 + 0.16y` (`glass.js:46-49`), plus blur 14 against 28 and saturation 1.7 against 1.5.
- **Fix (R):** narrow the gap so thin reads as the same glass, a touch lighter.
  - Thin `a = 0.58 + 0.24 y`, thick unchanged.
  - Thin saturation `calc(1.1 + 0.9k)`, which is 1.55 at 0.5.
  - At y = 0.75 that puts the bar at 0.76 against the sheet at 0.84. The contrast floor over white improves.
  - Update DESIGN's thin formula.

#### F12. S2. At rest the intent token reads as a mode dropdown and repeats the placeholder; it goes stale in task-reply mode
- **Surface and shots:** `01-bar-first-run` and `02-bar-after-first` show "◯ Ask │ Ask, find, or do": "Ask" is printed twice side by side, and the token is a bordered capsule, which is a classic mode switch. `05-outputs-multi` shows the token still saying "Ask" while the bar asks for the task's answer.
- **Fix (R)** (`renderer/index.html:33-50`, `overlay.css:335-409`, `bar.js` token sync):
  - **Empty and not locked:** the token collapses to a 32 x 32 glyph button with `padding: 0; width: var(--ctl); justify-content: center;`, no fill, no rim, `color: var(--label-2)`, and hover `--fill-1`. `.intent-name` is visually hidden.
  - **Text typed, inferred Ask:** glyph only. Ask is the default, so naming it adds nothing.
  - **Inferred Find or Do, or any locked intent:** the word appears with the existing FLIP width morph (240 ms `--ease-morph`).
  - **Locked:** accent glass, as now.
  - **Task waiting** (`body[data-waiting="asking"]`): `data-intent="reply"`, the pointer glyph and the word "Reply", locked style, menu disabled.
  - Tooltip and menu unchanged (request #6).

#### F13. S2. Output height grows by OS window resizes while streaming
- **Surface:** answer window during streaming (not photographed).
- **Cause:** `outputs::resize` runs a 200 ms, 60 Hz `motion::animate` setBounds glide for each content step (`src-tauri/src/windows/outputs.rs:205-239`, `:200`). WebView2 relayouts and repaints on every OS resize. The bottom edge steps and the text jitters, which is a different motion language from the in-page glass (DESIGN, v0.5: "Bounds animate"). Apple's sheets grow in-surface.
- **Fix (R+Rust):**
  - **Rust:** when an output is shown, set its outer height once to `output_max_height(wa) + 2*MARGIN`, or the user height, and stop animating bounds for content growth. Keep `T_OUT_GROW` only for user-initiated moves.
  - **Rust:** take a new `output:glass` message `{top, height}` (debounced to one per frame) and store it as the glass rect for `hit_test` (`flags.rs:324`), so the resize bands follow the visible sheet.
  - **Renderer:** `:root { interpolate-size: allow-keywords; }` `#opanel { height: auto; transition: height var(--d-med) var(--ease-morph); overflow: clip; }`. WebView2 is evergreen Chromium, which supports this.
  - **Renderer:** `body[data-grow="up"] #opanel { top: auto; bottom: var(--stage); }` when the sheet sits above the bar. The empty area below the sheet is already click-through (`data-passthrough="on"`).
  - **Renderer:** `requestResize()` (`output.js:327-339`) sends `output:glass` instead of asking for a window resize.

#### F14. S2. Streaming re-renders the whole answer every frame; there is no streaming affordance
- **Cause:** `renderLive()` replaces `innerHTML` of the whole body on every animation frame (`output.js:310-320`, `markdown.js:108-110`). Selection is lost mid-stream, and earlier blocks re-layout. New text appears in jumps with no softening. Apple Intelligence and Raycast AI fade new text in.
- **Fix (R):**
  - Render incrementally. Split the raw text at the last blank line outside a code fence. Blocks before it are rendered once and appended. Only the tail block is re-rendered per frame.
  - Newly appended block elements get `animation: md-in 160ms var(--ease-out)` with `@keyframes md-in { from { opacity: 0; filter: blur(2px); } }`.
  - While `busy`, the tail element gets `::after { content: ""; display: inline-block; width: 6px; height: 6px; margin-left: 4px; border-radius: 50%; background: var(--label-2); vertical-align: middle; }`. That is a static trailing dot, not a loop, and it is removed on done. The rim orbit stays the activity signal.

#### F15. S2. Code is set too heavy and has no Copy
- **Surface and shots:** `03-output-answer` and `extra/B`.
- **What is wrong:**
  - Inline code inherits weight 450 and renders as bold mono.
  - Its `padding: 1px 6px` (`overlay.css:1036-1043`) leaves an apparent space before punctuation ("LNK1104 .", "/MT .").
  - It breaks across lines with a chopped background.
  - Fenced blocks have no Copy, no language label, and no edge fade for horizontal scroll (`:1045-1057`).
- **Fix (R):**
  - `.md-body code { font-weight: 400; font-size: 0.875em; padding: 0.1em 0.35em; border-radius: var(--r-key); background: rgb(0 0 0 / .28); -webkit-box-decoration-break: clone; box-decoration-break: clone; }`
  - `.md-body pre { font: 400 13px/20px var(--font-mono); padding: 12px 16px; border-radius: 12px; position: relative; }`
  - markdown.js wraps each fence in `<div class="md-code">`. Fill a header slot with the fence's info string (`fs-meta`, `--label-2`) and a 28 px `.g-icon.small` Copy button at the top right (`top: 6px; right: 6px`). It shows on `.md-code:hover, :focus-within` and is always visible when the block is the last child. Copy writes `codeBuf.join('\n')`, and the icon turns into a check for 1.5 s.
  - Horizontal fade: `mask-image: linear-gradient(to right, #000 calc(100% - 24px), transparent)` only when `scrollWidth > clientWidth`, toggled by a class.

#### F16. S2. Heading scale and paragraph rhythm
- **Surface and shots:** `03-output-answer`, where "Fix it" is a `##` heading.
- **Cause:** only `.md-h1` gets 17 px; `##`, the level models actually use, renders 15/600, the same size as body (`overlay.css:1006-1019`). DESIGN says the first heading level is 17 semibold. The space above a heading is md-gap 4 + md-p 8 + 16 = 28 px, but blank lines inside lists add gaps (see F3).
- **Fix (R):**
  - `.md-h1, .md-h2 { font: 600 17px/22px var(--font-text); }` (Text, not Display; see F34).
  - `.md-h3 { font: 600 15px/22px var(--font-text); }`
  - `.md-h { margin: 20px 0 6px; }`
  - `.md-p { margin: 0 0 12px; }`
  - `.md-gap { display: none; }`, since paragraphs come from F3's joining now.
  - `.md-body { line-height: 24px; }`
  - `li { margin: 4px 0; }`, `ul, ol { margin: 4px 0 12px; padding-left: 24px; }`

#### F17. S2. The usage line is developer telemetry on its own wrapped line
- **Surface and shots:** `03`, `05`, `06`, `extra/B`. "0.9 s to first word · 3.2 s · 1,340 tokens · free · google/gemma-3-27b-it:free" wraps under Copy / Do it and right-aligns on a line of its own, as noise under every answer.
- **Cause:** `output.js:118-131` and `:425-436`, `.usage` at `overlay.css:1667-1673`.
- **Fix (R):**
  - The default line is the model's display name and the total time only: "Gemma 3 27B · 3.2 s". Show "free" only when the person has paid models enabled.
  - The full string moves to `title` and to the context menu ("Copy details").
  - `.msg-meta { flex-wrap: nowrap; }` `.usage { min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--label-3); }`
  - Hide `.usage` when the meta row is narrower than 360 px (container query on `.msg-meta`).
  - Update DESIGN's line about the usage line.

#### F18. S2. Title-bar tools crowd the title; the kind chip is a badge; Copy appears twice
- **Surface and shots:** `05-outputs-multi` ("Rename the screenshots in D…") and `06-*` ("Summarise the quarterly numbers o…").
- **Cause:** four always-on icons plus a text chip leave about 55% of the bar for the title. The answer's own Copy duplicates the title bar's Copy when there is one answer.
- **Fix (R)** (`output.html:36-70`, `overlay.css:586-617`):
  - Replace `.sheet-kind` text with the bar token's glyph (bubble, viewfinder, pointer) at 16 px in `--label-2`. That ties each window to the verb that made it and removes a badge. Keep the word in `aria-label`.
  - At rest show only Close. Copy, History and New fade in on `#sheet-bar:hover, #sheet-bar:focus-within, body[data-scrolled="1"]` (opacity 0 → 1, 150 ms). They keep their layout slot, so nothing shifts.
  - Hide the title-bar Copy while the conversation has one answer.

#### F19. S2. Several open outputs: no front or back distinction
- **Surface and shots:** `05-outputs-multi`. All three sheets carry the same shadow, rim and title weight. Request #11 makes several windows normal.
- **Fix (R+Rust):**
  - **Rust:** already stamps `o.touched` (`outputs.rs:190`). Emit `output:front {label}` to all outputs when it changes.
  - **Renderer:** `body[data-front="0"] .sheet-title { color: var(--label-2); }` `body[data-front="0"] #opanel > .gl { box-shadow: 0 0 0 var(--px) rgb(0 0 0 / .3), 0 4px 12px -6px rgb(0 0 0 / .35); }` `body[data-front="0"] #opanel > .gl::after { opacity: .7; }`
  - Transition 150 ms.

#### F20. S2. Icons: one family, one optical weight
- **Cause.** Stroke widths are 1.6 (bar and sheet tools), 1.7 (both X glyphs), 2.0 (Settings and History X, the error and search glyphs, the Do pointer) and 2.5 (callout X). Glyph sizes run 12, 14, 15, 16 and 17 px. A 24-unit 1.6 stroke at 15 px renders 1.0 px, faint at 100%. The gear is Feather's, with thin teeth, and the copy glyph is custom. Sources: `index.html:36-84`, `output.html:40-68`, `settings.html:15-17`, `history.html:15-28`, `buddy.html` (callout close).
- **Fix (R).** Use Fluent System Icons Regular (MIT, Windows 11's own family) as inline filled paths on a 20 px grid. Or keep the current paths but give every `<svg>` `vector-effect="non-scaling-stroke"` on its paths and `stroke-width: 1.5` (CSS px).
  - Sizes: 18 px in 32 px buttons, 16 px in 28 px buttons, 14 px inline.
  - One shared X, one chevron and one check, defined once in a `<template>` or sprite, used by all three pages and the callout.

#### F21. S2. Menus: two materials and two checkmarks
- **Surface and shots:** the bar's `#intent-menu` is thin glass (`overlay.css:413-452`). Settings' model popover is a solid `#23252D` slab (`settings/settings.css:1287-1293`).
- **Checks:** the intent menu uses a text "✓" glyph (`overlay.css:449-452`). Settings draws a CSS check (`settings.css:1315-1327`).
- **Rows:** intent-menu rows are r10 inside an r16 menu with 4 px padding, which is not concentric.
- **Fix (R):**
  - One `.g-menu` in glass.css: `class="glass thin"`, `--r: 16px; padding: 4px; box-shadow: var(--shadow-menu)` (section 3).
  - Rows `min-height: 36px; padding: 0 12px; border-radius: 12px;`, hover `--fill-1`, selected `--fill-2` plus the shared check glyph (F20) at the right.
  - Entrance `drop-in 220ms var(--ease-morph)`, exit 120 ms fade.
  - Settings' combo popover becomes `.g-menu`. It lives inside the Settings page, so it is glass on glass: use the solid tier there (`--solid` body, same rim, same radius).

#### F22. S2. Task trail: the plan and the steps are two lists, and the plan never checks off
- **Surface and shots:** `latest/22-agent-running`, `23-agent-confirm`, `25-agent-error`. Four plan rows ("Open Save As…") sit above three action rows ("Press Ctrl+Shift+S…"). The plan stays as open circles while actions complete, and plan rows in `--label` read *brighter* than finished actions in `--step-done`, which inverts the hierarchy. Sources: `overlay.css:1485-1546`, `trail.js`.
- **Fix (R, plus agent payload):**
  - The plan is the checklist. Each action row nests under the plan item it serves, as a 13 px `--label-2` detail line, collapsed after it finishes. This needs `planIndex` on `agent:step` from Rust's agent loop.
  - Until that exists, the renderer marks plan item *i* done when the run has completed *i* acts, and moves the act rows behind the existing "Steps" disclosure.
  - Pending plan items use `--label-2`, the running one `--label` with the arc, done ones `--label-2` with a check.

#### F23. S2. Settings rows: only the chevron is clickable, chevrons do actions, and an instruction sits in a value slot
- **Surface and shots:** `07-settings-fullscreen` and `08-settings-account`.
- **What is wrong:**
  - Only the 28 px `.edit` chevron toggles a row (`settings/settings.js:46-51`). The 44 px row has no hover and no hit target.
  - "Window layout | Drag edges to resize | >" opens a layout editor, and "Tour | Four steps, about a minute | >" replays the tour (`settings/settings.html:419-433`). Chevrons promise navigation or disclosure, not an action.
  - "Drag edges to resize" is an instruction, not a value.
  - Disclosure pops open with no height animation.
- **Fix (R):**
  - The whole `.row-head` is the button: role and handler on the row, `cursor: default`, hover `background: var(--fill-1)` across the group's width with `border-radius` matching its position (first, last or middle row).
  - Action rows get a trailing text button instead of a chevron: "Move bar…", "Replay". The summary becomes a value: "Top centre" (the bar's anchor) and "4 steps".
  - Animate disclosure: `.body { display: grid; grid-template-rows: 0fr; transition: grid-template-rows var(--d-med) var(--ease-morph); } section[data-open="1"] .body { grid-template-rows: 1fr; } .body > * { min-height: 0; overflow: hidden; }`

#### F24. S2. Settings and History margins disagree with the native resize band
- **Cause:** the visible sheet is inset 14 / 18 / 22 px (top, sides, bottom) by body padding (`settings.css:923-926`, `history.css:362-365`). Rust gives these windows `margin: 0.0` (`mod.rs:895-902`). The native band (`flags.rs:324-364`) therefore sits at the invisible outer edge, 14-22 px away from the glass, while `frame.js` puts a second band at the glass. There are two resize zones, and one is invisible.
- **Fix (R+Rust):**
  - CSS: `body { padding: var(--stage); }` on both pages. The shadow fits in 20 px (section 3, `--shadow-sheet`).
  - Rust: `frame_meta().margin = stage::MARGIN` for Settings and History, so the native hit-test, the click-through and the glass agree. Window bounds must add the margin the way outputs do.

#### F25. S2. The glob breaks when an output is not next to the bar
- **Cause:** `place_output` cascades windows by 24 px (`layout.rs:125-160`), and outputs can be dragged anywhere. `glob.js` still draws a neck to the bar (`sideOf`, `:100-106`). Any part beyond this window's 20 px margin is clipped, which leaves a stub.
- **Fix (R):** in `open()`, if the gap between the bar and the sheet is more than 14 px (20 margin − 6 overlap), or the sheets do not overlap on the cross axis, skip the shape. Instead, scale the sheet from 0.94 to 1 with `transform-origin` toward the bar, over 280 ms `--ease-morph`, plus a fade with the F1 content reveal. Close mirrors it in 160 ms.

#### F26. S2. Copy that teaches or says the same thing differently
- **Tour step 4:** "Press {key} and say what to do. Agent asks before anything risky" (`src-tauri/src/buddy.rs:91-93`). With the agent chord unbound (the default), this renders as "Press and say", which is broken (`latest/28-onboard-4`). It also says "Agent", while the bar says "Do" and Settings says "Tasks". Fix: "Type what to do in the bar. AevalBuddy asks before anything risky; Esc stops it." Fall back to the main chord when the agent chord is empty.
- **Tour step 1:** "Paste an OpenRouter API key…" (`buddy.rs:66`). Request #2 adds Claude, OpenAI and Gemini. Fix: "Paste an API key from Claude, OpenAI, Gemini or OpenRouter, then Save."
- **Wait message:** "Waiting for the model · 12 s. Free models queue at busy times; Stop is beside this." (`output.js:108`). Fix: "Waiting for the model · 12 s", with the explanation in `title`.
- **Confirm card hint:** "Tab to a button, then Enter" (`trail.js:321`). Remove it visually; it is already announced (`trail.js:341`).
- **Region hint:** "Drag the part of the screen you want to ask about, or use the arrow keys and Enter" (`renderer/region.html`, `#hint`). Fix: "Drag to select an area" plus `Esc`.

These are Rust strings and renderer strings.

#### F27. S2. The default bar is 420 px: cramped, the close button hidden, and the harness shoots 640
- **Cause:** the store default is `inputWidth: 420` (`src-tauri/src/store/mod.rs:91, 646-652`), with `PANEL_WIDTH 420` (`layout.rs:10`). At 420 the close button is hidden (`overlay.css:315-323`, `min-width: 520px`). Text gets about 200 px after the token and three icons. Every shot uses 640, so the shipped default is never photographed. Spotlight is about 680 wide and Raycast 750.
- **Fix (R+Rust):** default to 600 in both the store and `PANEL_WIDTH`. Keep the stored widths of existing users. The harness shoots the default (section 4).

#### F28. S2. Legacy stylesheets still leak off-system values
- **Cause:** `settings/settings.css:1-906` is the v0.x sheet. It is overridden by blocks at `:908` and `:1359`, but its values still apply wherever the overrides miss:
  - `.combo > button` radius 12 (`:1269`)
  - `.combo-option` padding 8/10
  - settings `kbd` radius 4 with padding 3/8 (`:649-658`; glass.css's keycap is r6, padding 5)
  - section body `gap: 10px` (`:1132`)
  - `.sc-row` padding 5
  - footer padding 10

  `renderer/lens.css:7-23` still defines `--skin`, the superseded token, and its own `--accent` values. `cursor-buddy.css` keeps the v0.3 callout rules above its override (`:150-320`).
- **Fix (R):** collapse each file to one layer. Delete the superseded rules and keep only the override values, mapped to tokens. Add a lint check to `tools/ui-shots/bridge.test.mjs`: grep the CSS for `border-radius: (4|5|8|12|14|18|20)px` and for paddings that are not on the scale. Fails the run.

#### F29. S2. Lens: a candy loupe, and a control frame that departs from DESIGN
- **Surface and shots:** `latest/20-lens.png`. The current hit renders as a glossy pink pill: a 1.18 loupe plus a 0.7 white inset highlight plus a 2 px accent ring plus a 22 px shadow (`lens.css:206-212`). It reads as a button, not as magnified text.
- **Control frame:** `border: 3px solid var(--accent-bright)`, radius 14 (`lens.css:242-247`). DESIGN says 2 px `#823A80` and records no departure.
- **Chips:** `#why` radius 14 with padding 7/14 (`:229-232`) is off the scale.
- **Fix (R):**
  - Loupe `box-shadow: inset 0 0 0 var(--px) rgb(255 255 255 / .35), 0 0 0 1.5px var(--accent-bright), 0 0 0 2.5px var(--hit-rim), 0 6px 16px rgb(0 0 0 / .35);`. Drop the 0.7 top highlight. Radius 8.
  - Frame: `border: 2px solid var(--accent)` with a `0 0 0 1px rgb(0 0 0 / .5)` outer line, or record the 3 px bright frame as a DESIGN departure with its reason (visibility on dark editors).
  - `#why`: capsule if single-line, else radius 16. Padding 8/12.

### S3: refinements

#### F30. S3. Elevation: one shallow shadow for everything
- **Cause:** `--shadow` (`glass.css:98-101`) is shared by the bar, the sheets, the chips and the tour card. In-sheet plates add their own drop shadow (the confirm card's `0 8px 24px -8px`, `overlay.css:1593`), which is a shadow inside glass. Apple keeps in-surface plates flat.
- **Fix (R):**
  - `--shadow-control: 0 0 0 var(--px) rgb(0 0 0 / .30), 0 1px 2px rgb(0 0 0 / .20), 0 4px 12px -4px rgb(0 0 0 / .35)` for the bar, chips and orb.
  - `--shadow-sheet: 0 0 0 var(--px) rgb(0 0 0 / .34), 0 2px 6px rgb(0 0 0 / .18), 0 10px 22px -10px rgb(0 0 0 / .50)` for outputs, Settings and History. It fits the 20 px margin.
  - `--shadow-menu: 0 0 0 var(--px) rgb(0 0 0 / .3), 0 8px 18px -6px rgb(0 0 0 / .45)`.
  - Remove the drop shadow from `.confirm-card` and `.msg.consent`. They are plates, and the rim plus the fill are enough.

#### F31. S3. The error row is a third surface style
- **Surface and shots:** `latest/14-answer-error.png`. A pink-tinted box with a pink rim (`overlay.css:950-973`). Plates elsewhere are neutral `--fill-1` plus a rim.
- **Fix (R):** `.error-row { background: var(--fill-1); box-shadow: var(--chip-rim); border-radius: 12px; }`. Keep the danger glyph in `--danger`. Place Retry under the text, left-aligned, when the message is longer than two lines (`flex-wrap: wrap`, with Retry as `flex-basis: 100%` aligned start).

#### F32. S3. Reduced-motion details
- Every animation becomes a 100 ms fade (`overlay.css:1696-1705`), but DESIGN says 150 ms. Fix: `--d-reduced: 150ms` and use it there and in `glob.js` (fading, `:286-290`).
- The bar's word blur on fold (`overlay.css:257-261`) should be dropped under reduced motion (`filter: none`).

#### F33. S3. Scrollbar and bottom fade
- `#opanel::after` is a tint gradient (`overlay.css:756-773`) that darkens the glass bottom, a "grey band". It duplicates the `.edge-fade` mask the transcript already has (`output.html:74`). Fix: delete `#opanel::after`; the mask does the job.
- The scrollbar thumb is always visible at 0.28 (`overlay.css:730-749`). Keep it visible (DESIGN), but quieter at rest: `--scroll-thumb: rgb(255 255 255 / .18)`, and 0.40 while `body[data-scrolling]`, set for 800 ms after each scroll event.

#### F34. S3. Optical sizes: Display used small, Small never used
- **Cause:** Segoe UI Variable Display is set at 15 and 17 px: `.md-h` (`overlay.css:1007`), `.trail-outcome` (`:1326`), `.confirm-text` (`:1610`), `.find-empty::first-line` (`:1252`), `.hud-title` (`:1136`) and `.pill-label` (`:122`). Display's tight spacing reads cramped below 18 px. The Small optical size, made for 12 px, is never used.
- **Fix (R):** `--font-small: "Segoe UI Variable Small", "Segoe UI Variable", "Segoe UI", system-ui, sans-serif` for every 12 px style (meta, keycaps, kind glyph labels, usage). Display only at 18 px and up (the bar, page titles). Text everywhere else.

#### F35. S3. Strong weight drift
- `strong` is 650 (`overlay.css:1095-1097`), find matches 650, titles 600, and the trail's failed glyph 700. Fix: one strong weight, `--w-strong: 600`.

#### F36. S3. Off-grid spacing in overlay.css
These values are not on the 4-grid:

| Selector | Off-grid value | Line |
| --- | --- | --- |
| `.find-row` | padding 10/12 | `:1183` |
| `.trow` | gap 10, padding 4/0 | `:1347-1348` |
| `.trail-banner`, `.note-row` | gap 10 | `:1562` |
| `.intent-token` | padding-left 10, gap 6 | `:340-341` |
| `.md-body ul` | margin 2, padding-left 22 | `:1023-1024` |
| `li` | margin 3 | `:1028` |
| `.sheet-kind` | padding 2/8 | `:591` |
| `.find-view` | margin -10/-4 | `:1171` |

**Fix (R):** snap to 4, 8, 12, 16, 20 or 24. Allow 2 px only for hairline and optical nudges, and comment it where used.

#### F37. S3. Radii are not concentric everywhere
The rule should be inner = outer − inset:

| Element | Now | Should be | Why |
| --- | --- | --- | --- |
| Plates in the transcript (`.error-row`, `.confirm-card`, `.trail-banner`, `pre`) | 16 | 12 | they sit 16 px inside a 28 sheet |
| Intent-menu rows | 10 | 12 | 4 px inside an r16 menu |
| Settings rail rows | 10 | 8 | 8 px inside the r16 rail |
| Settings and History groups | 16 | 12 | about 16 px inside a 28 sheet |
| Callout card | 20 | 28 | see F38 |

**Fix (R):** adopt section 3's radius table. Update DESIGN's "28/16/10/6" to "28/16/12/8/6/capsule, inner = outer − inset".

#### F38. S3. Tour and callout card
- **Surface and shots:** `latest/11-buddy`, `28-onboard-*`.
- **What is wrong:**
  - The message is typed out character by character, with a blinking caret (`renderer/buddy.js:150-182`, `cursor-buddy.css:198-207`). It is a gimmick, it delays reading, and on `role="status"` it changes text every 16 ms.
  - The close X has its own head row, which leaves an empty band above a one-line message.
  - The radius is 20, and the padding 12/14/14/16 is asymmetric (`cursor-buddy.css:330-341`).
- **Fix (R):**
  - Set the text at once, with `animation: md-in 160ms var(--ease-out)`, and announce it once.
  - Put the close button in the text row: `float: right; margin: -4px -4px 0 8px;`. Drop `.bubble-head` when there are no pips.
  - Radius 28, padding 16 (12 under the actions row), so the 32 px capsule buttons are concentric (16 + 12 = 28).

#### F39. S3. History details
- The search field opens focused, so its 3 px accent halo is the loudest thing on screen at rest (`09-history`, `history.css:424-431`). Fix: focus on open but suppress the halo until the first key or Tab, using `:focus-visible` semantics. The rim brightening alone is enough.
- "Delete all" is a filled capsule in danger colour, always visible in the footer. Fix: make it a quiet text button in `--label-2` with danger colour on hover. The confirm step is unchanged.
- Time-label casing is inconsistent ("yesterday" vs "Wednesday" in `history.js`). Fix: sentence case for all.
- Group heads ("Today") sit 4 px in (`history.css:466-475`). Fix: align with row text, `padding-left: 16px`. Settings' page title `h1` has the same 4 px offset (`settings.css:1016-1022`); align it with the group edge, `padding-left: 0`.

#### F40. S3. The focus ring removes control rims
- `:focus-visible { box-shadow: 0 0 0 5px rgba(0,0,0,.38) }` (`glass.css:310-314`) replaces each control's own `box-shadow` (the chip rim, the prominent bevel).
- **Fix (R):** move the dark halo to `outline` plus `outline-offset`. Use `outline: 2px solid rgb(255 255 255 / .92); outline-offset: 2px;` and add the dark halo as a second ring via `box-shadow: var(--own-shadow, none), 0 0 0 5px rgb(0 0 0 / .38)`, with each control setting `--own-shadow`.

#### F41. S3. Content insets differ between output kinds
- Answer text starts 16 px from the sheet edge. Find row text starts at 34 (`find-view` −4 + row 12 + swatch 14 + gap 12). Trail rows start at 44.
- **Fix (R):** keep the leading marks, but hang them in the gutter. Text starts at 16 in every kind and the marks sit at 0-14 inside the row's own padding. Find becomes `.find-row { padding: 8px 12px 8px 4px; }` with `.find-view { margin: 0 -10px 0 -12px; }`, or the equivalent. Match the trail.

#### F42. S3. Only the orb and the suggestion chips catch the pointer light
- The `.interactive` pointer-follow highlight (`glass.js:166-176`, `glass.css:167`) is on the orb and the example chips only.
- **Fix (R):** add `interactive` to `#bar` and to each `#sheet-bar` region (a title-bar light), at 0.06 rather than 0.09 on sheets. It is the desktop analogue of Liquid Glass's touch illumination. Optional: on `:active`, buttons brighten (`--fill-3`) instead of shrinking to 0.96, which is closer to iOS 26 glass controls.

---

## 3. Proposed tokens (glass.css `:root`; DESIGN.md "One system" updated to match)

### Type
| Token | Size/line | Weight | Face | Use |
| --- | --- | --- | --- | --- |
| `--t-meta` | 12/16 | 400 | Small | usage, keycaps, timestamps, slider stops |
| `--t-caption` | 13/18 | 400 / 600 | Text | secondary text, sheet titles (600), buttons (600) |
| `--t-row` | 14/20 | 400 / 600 | Text | rows, trail steps, find snippets, settings rows |
| `--t-body` | 15/24 | 450 | Text | answers (measure 640 px, about 68 ch) |
| `--t-title` | 17/22 | 600 | Text | answer h1/h2, trail outcome, confirm title |
| `--t-bar` | 18/24 | 450 | Display | the bar input |
| `--t-page` | 22/28 | 600 | Display | Settings page titles |
| `--t-code` | 13/20 (inline 0.875em) | 400 | Cascadia Mono | code |

Weights: `--w-body: 450; --w-strong: 600`. No 650 or 700. Numerals: `tabular-nums` on counts, times, step numbers and tables.

### Spacing (4-grid)
`--sp-1: 4px; --sp-2: 8px; --sp-3: 12px; --sp-4: 16px; --stage: 20px; --sp-5: 24px; --sp-6: 32px`.

Use 2 px only for hairline or optical nudges.

Insets:
- `--inset-text: 16px`: text from any sheet edge.
- `--inset-row: 12px`: selectable rows from a sheet edge.
- `--inset-menu: 4px`.

### Radius (inner = outer − inset)
| Token | Value | Where |
| --- | --- | --- |
| `--r-sheet` | 28 | bar (56 px capsule), outputs, Settings, History, tour card |
| `--r-card` | 16 | things 12 px inside a sheet (find rows, rail container), menus and popovers, guess card |
| `--r-plate` | 12 | things 16 px inside a sheet (transcript plates, code, tables, settings and history groups, question chip), menu rows (4 px inside 16) |
| `--r-row` | 8 | rows 8 px inside a 16 container (rail rows, combo options), lens ring and loupe |
| `--r-key` | 6 | keycaps, inline code |
| `--r-cap` | 999 | buttons, chips, fields, token |

### Glass levels (Settings slider, k)
| | 0 None | 0.25 Subtle | 0.5 Liquid (default) | 0.75 Very | 1 Very very |
| --- | --- | --- | --- | --- | --- |
| Body | `--solid` | backdrop | backdrop | backdrop | backdrop |
| Blur thin / thick | – | 16 / 30 | 14 / 28 | 12 / 26 | 10 / 24 |
| Saturation thin / thick | – | 1.33 / 1.30 | 1.55 / 1.50 | 1.78 / 1.70 | 2.0 / 1.9 |
| Scrim thin (y = p90 luma) | – | `0.58+0.24y` +0.08(1−y) | `0.58+0.24y` | −0.08(1−y) | −0.16(1−y) |
| Scrim thick | – | `0.72+0.16y` +0.075(1−y) | `0.72+0.16y` | −0.075(1−y) | −0.15(1−y) |
| Edge band scrim (outer `--band`) | – | = interior | = interior | interior − 0.13 | interior − 0.26 |
| Refraction band / scale thin / thick | none | 9 / 11 / 14 | 14 / 22 / 28 | 23 / 39 / 48 | 32 / 56 / 68 |
| Refracting surfaces | – | bar, outputs, Settings, History | same | same | same |
| Rim width / peak | 1 px / 0.55 | 1 px / 0.62 | 1 px / 0.62 | 1.5 px / 0.73 | 2 px / 0.85 |
| Sheen (162°, top-left) | 0 | 0 | 0 | 0.08 | 0.16 |
| Thickness shade (bottom inset) | 0 | 0 | 0 | 0.18 | 0.35 |
| Tone | dark | auto light/dark (F10) | auto | auto | auto |

The contrast floor holds at every stop: the scrim slope is 0 at y = 1, the edge band never carries text (sheet text inset 16 ≥ band/2), and the light tone takes over on bright content.

### Shadows
`--shadow-control`, `--shadow-sheet` and `--shadow-menu`, as in F30. No shadows on plates inside glass.

### Motion
| Token | Value | Use |
| --- | --- | --- |
| `--ease-glide` | `cubic-bezier(0.32, 0.72, 0, 1)` | window-level, exits, close of the glob |
| `--ease-out` | `cubic-bezier(0.16, 1, 0.3, 1)` | micro motion, content reveals |
| `--ease-morph` | `cubic-bezier(0.22, 1, 0.36, 1)` | shape changes, sheet height, disclosure |
| `--ease-spring` | `cubic-bezier(0.2, 0.9, 0.25, 1)` | glob body growth (spring with response about 0.42 s and damping 0.88, no overshoot) |
| `--ease-swell` | `cubic-bezier(0.5, 0, 0.18, 1)` | (retire; replaced by phase A + spring) |
| `--ease-settle` | `cubic-bezier(0.34, 1.3, 0.64, 1)` | small arrivals only (token lock, loupe, switch) |
| `--d-press` | 100ms | press feedback |
| `--d-fast` | 150ms | hover, opacity, reduced-motion fade (`--d-reduced`) |
| `--d-reveal` | 140ms | content appearing in a landed surface |
| `--d-med` | 240ms | messages, sheet height, disclosure |
| `--d-morph` | 320ms | orb to bar, token width |
| `--d-glob` | 480ms | glob open (timeline F2) |
| `--d-glob-close` | 260ms | glob close |

---

## 4. States the Rust shot harness must add (`tools/ui-shots/shots.mjs`)

General changes to the harness:
- **DPR.** Run every scene at `deviceScaleFactor` 1 *and* 1.5. Windows desks are 125-150%; every Rust shot today is at 100%.
- **Backdrops.** Add two to `lib/backdrop-fixture.html`: `dark-editor`, a VS Code-like dark page with syntax-coloured text, and `photo-busy`, a high-frequency image with edges, text and grids. Refraction is invisible on the smooth `wallpaper` blob, so lensing cannot be judged today.
- **Default sizes.** Shoot the shipped default bar width (420 now, 600 after F27) as well as 640.
- **Glob timing.** Take glob frames with a virtual clock. Stub `performance.now` and `requestAnimationFrame` and step them, instead of `preview()`, so the content-reveal timing is photographed as it really plays. `preview()` toggles `reveal` with its 180 ms transition, so content never shows in the current frames.

### Bar (`renderer/index.html`)
1. Orb idle, orb working (rim orbit frame), orb with chat badge, orb with error dot, orb with task step number.
2. Orb to bar morph at 25/50/75%, and the closing fold at 25/50/75%.
3. Empty bar at the default width, first run (have) and after (have).
4. Typing with the inferred intent at low and high confidence: Ask, Find, Do.
5. Locked by sigil (`/`, `>`), and the intent menu open (Alt+Down).
6. Guess card (a guessed task: "Just answer / Run as a task / Find it").
7. Region chip active ("Asking about a 640 x 320 area · Whole screen").
8. Find status row with count and scope chip, and the stage lines while related matching runs.
9. Notice: no key with "Open settings", and capture unavailable.
10. Task waiting: answer, plan approval ("Enter starts the task"), paused.
11. Bar anchored bottom, with examples above it (`column-reverse`).
12. Bar at 320 px minimum width, and with the close button hidden.
13. Layout editor (HUD), resting and snapped.

### Answer output (`renderer/output.html`)
14. Thinking (stage label plus orbit), and the long wait ("Waiting for the model · 12 s").
15. Mid-stream, with a partial markdown block and an open fence.
16. Done (have). Copy pressed ("Copied"). Do it shown. "Find this on screen instead" shown.
17. Two-turn conversation, scrolled, with the title hairline and bottom edge fade.
18. Error row with Retry: short message, and a four-line message.
19. Stopped mid-answer.
20. Markdown fixture: h1-h3, loose and tight ordered lists, nested lists, table, long code block with horizontal scroll, inline code next to punctuation, blockquote, hr, links.
21. Maximised (1600 x 1000), and dragged to fixed height (`outputFixedHeight`).
22. Front and back output (F19), and hover on the title bar (tools revealed, F18).

### Find output
23. Reading ("Looking…").
24. Results with the current row (have, in multi), offscreen rows, related-stage running.
25. Empty ("Nothing on screen matches…").
26. Offline.

### Task output
27. Consent sheet (first run).
28. Planning, then plan approval with Start.
29. Running (step arc).
30. Confirm card, and open-link confirm with the URL.
31. Asking (have, in multi).
32. Paused with Continue.
33. Done, collapsed and expanded.
34. Failed with banner (latest/25 equivalent).
35. Stopped.

### Glob (virtual clock)
36. Open at 0, 4, 10, 18, 30, 45, 62, 72, 82, 90 and 100%.
37. Close at 0, 25, 50, 75 and 100%.
38. Reduced-motion fade.
39. Output above the bar (bar anchored bottom), output to the side, and an output cascaded 24 px (the F25 fallback).
40. At 1.5 DPR, the join at 400% crop, which is the seam check.

### Settings (`settings/settings.html`)
41. Every page (Account, Models, Answers, Find, Tasks, Privacy, Shortcuts, General) at default size.
42. Account: no key, key saved, and each provider key row (Claude, OpenAI, Gemini, OpenRouter; request #2).
43. Models: combo open with the list, and "Show paid models" on.
44. Shortcuts: capturing a chord, and a conflict warning.
45. Liquid glass at all five stops (0, 0.25, 0.5, 0.75, 1) beside an answer *and* the bar, on wallpaper, white, dark-editor and photo-busy backdrops.
46. Updates: checking, downloading with progress, ready ("Restart now"), failed or offline sentence.
47. Narrow (below 560 px, rail hidden), and maximised with the Rust limits honoured (F5).
48. Row hover and disclosure mid-animation (F23).

### History (`history/history.html`)
49. List (have), search with results and `mark`, no results, empty (no history yet), history saving off.
50. Delete-all confirm, row hover with delete, and maximised.

### Region, lens, buddy
51. Region: hint idle, dragging with the size chip, too-small state, over white and over dark.
52. Lens: literal hits, related rings, current loupe, why chip, control frame plus "using your computer" chip; over white and dark-editor.
53. Buddy: pointer plus callout (final text), tour steps 1-4 including the final card with Try it / Later, agent pointer mid-glide.

### Accessibility and appearance matrix
54. `prefers-reduced-motion: reduce`, for the bar morph, glob, thinking and the token lock.
55. `prefers-contrast: more`, and `forced-colors: active` (a contrast theme), for the bar, answer, Settings and History.
56. Light tone (F10) for every glass surface over white and photo-busy, at 1 and 1.5 DPR.
57. Keyboard focus-visible on every control type: icon, button, chip, row, field, slider thumb, menu row.
