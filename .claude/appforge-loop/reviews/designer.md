# AevalBuddy v2: design review (Human Interface pass)

Reviewer brief: one unified input (Ask / Find / Agent resolved from what is typed), the whole product in
iOS 26 / macOS Tahoe Liquid Glass, built on Windows. Read: DESIGN.md, SPEC-v2.md, renderer/*, settings/*,
history/*, lib/lens.js, main.js window options, `git show 00993b3` and `1688147` (the acrylic history).
Looked at: all 36 frames in `shots/latest`.

Confidence marks: [V] verified in this repo or in a frame, [M] from platform knowledge, check on the
runner before building on it, [L] my inference.

One caveat about the evidence. Every frame is a page render over a flat `#3E404A` (or a fixed terminal
fixture in 12). None of them can show glass, so nothing here can be judged "legible over arbitrary
content" from the shots. Finding 10 fixes the shot pipeline so the next review can.

---

## Ranked findings

| # | Impact | Finding | Where |
| --- | --- | --- | --- |
| 1 | Highest | Real glass without touching window styles: render the backdrop inside the window from a captured, blurred, refracted snapshot | new `renderer/glass.css`, `glass.js`, `main.js` |
| 2 | Highest | One field, intent inferred, a token chip instead of a mode label, ghost alternate, sigils | `renderer/index.html`, `overlay.js`, `overlay.css` |
| 3 | High | One static stage window per display area, morph in CSS, retire `setBounds` animation | `lib/motion.js`, `main.js` |
| 4 | High | Legibility system: two materials, adaptive scrim from measured backdrop luminance, brighter label | `glass.css` |
| 5 | High | The shadow is clipped by the window edge today, so there is no depth at all | `overlay.css` `--lift`, `main.js` bounds |
| 6 | High | Type, spacing, radius re-scaled for a 56px capsule bar and 28px sheets | tokens |
| 7 | Medium | Find: list and lens share one mark; the current hit becomes a magnifying glass lens | `overlay.css .find-row`, `lens.css`, `lens.js` |
| 8 | Medium | Agent: outcome first, plan becomes the checklist, confirm becomes a glass sheet | `renderer/trail.js`, `overlay.css .trail*`, `.confirm-card` |
| 9 | Medium | Settings and history: grouped glass lists, not a dark IDE panel; buddy callout loses the mascot | `settings/*`, `history/*`, `cursor-buddy.css` |
| 10 | Medium | Prove it: shots over three backdrops, a contrast gate, a window-style invariant gate | `lib/shots-v2.js`, `smoke-v2.js` |

---

## 1. The material system on Windows

### 1.1 Why acrylic was reverted (from the commit, not from memory)

`00993b3` says acrylic "looked better in an isolated test" and in use: the overlay fell behind other
windows, the collapse left nothing visible to click, the desktop showed repaint artifacts. The cause it
names is real and structural: `backgroundMaterial` requires `transparent: false`, so the window is no
longer a layered window, and everything this app does over time (`setOpacity`, `setIgnoreMouseEvents`,
topmost re-assertion, show/hide) either toggles `WS_EX_LAYERED` underneath the backdrop or fights it.

Three more reasons, none in the commit, that mean acrylic would be wrong even if it were stable [M]:

| Problem | Consequence for Liquid Glass |
| --- | --- |
| DWM rounds a top-level window to a fixed 8px (or 4px with `DWMWCP_ROUNDSMALL`) and nothing else | The bar must be a 28px capsule and sheets 28px. DWM cannot do it. `SetWindowRgn` can, but with aliased edges and it also clips the shadow |
| System backdrop (Mica certainly, desktop acrylic in WinUI also) swaps to a flat fallback colour when the window is not the active window | This app shows the answer window with `showInactive` and the lens is `focusable: false`. The glass would turn grey exactly when the agent runs |
| The backdrop is one fixed recipe (blur radius, tint, noise) | No refraction, no luminance-adaptive tint, no specular rim. It is frosted glass, not Liquid Glass |

Decision: do not use `backgroundMaterial` on any window. Not on the bar, not on Settings.
The `acrylic` code path stays deleted.

### 1.2 The recipe that satisfies the constraint that caused the revert

Constraint: windows keep exactly the flags they have today (`transparent: true`, layered, topmost
`screen-saver`+1, content-protected). Nothing about window style changes over time. The glass lives
inside the page.

The enabling fact is already in SPEC-v2 section 1.2: every window is excluded from capture
(`WDA_EXCLUDEFROMCAPTURE`, build >= 19041), so `desktopCapturer` returns the desktop as it is behind us,
without us in it. That is the backdrop `backdrop-filter` cannot get, because a transparent Electron
window has nothing behind it inside Chromium. [V] (DESIGN.md v0.4 item 8 states the limitation.)

**Pipeline**

1. Main takes a small frame of the display the window is on: `desktopCapturer.getSources({ types:
   ['screen'], thumbnailSize: { width: Math.round(d.size.width / 6), height: Math.round(d.size.height / 6) },
   fetchWindowIcons: false })`. On 2560x1440 that is 427x240. Budget: under 25ms of main-thread time
   [L, measure on the runner].
2. Send it as a `Blob`/`ImageBitmap` to each glass window (never a file; never written to disk; it
   lives only in renderer memory, in a content-protected window).
3. Each glass element paints it as a `background-image`, offset to match its screen position, scaled up.
   A 1/6 thumbnail upscaled bilinearly is already a free ~6px box blur; a CSS `blur(14px)` on top finishes it
   cheaply because the source has no detail left to blur.
4. Refresh: on open, then 2 Hz while a glass surface is visible and the pointer is not mid-drag, 1 Hz
   while the agent runs, paused while hidden. Cross-fade old to new over 240ms so refreshes never pop.
   While the window is moving (HUD), only the offset changes.
5. Fallback ladder, chosen once per window and exposed as `body[data-glass]`:
   `live` (build >= 19041, capture OK) / `solid` (current skin, with the new rim and tokens) when
   capture is unavailable, when Windows "Transparency effects" is off (read `HKCU\Software\Microsoft\
   Windows\CurrentVersion\Themes\Personalize\EnableTransparency` once at boot and on `nativeTheme
   updated`), or in forced-colors / high contrast.

**Staleness is the honest cost.** A video playing behind the bar updates at 2 Hz rather than 60. That is
acceptable because the bar is a 56px strip over a blur radius larger than the motion detail, and it is a
trade DESIGN.md already makes elsewhere (the lens re-snapshots, it does not track).

**Layer stack** (new file `renderer/glass.css`, linked by overlay, output, settings, history, lens,
cursor-buddy, so there is one place a material is defined; today `--skin` is copy-pasted in
`overlay.css:31` and `lens.css:21`, and settings and history carry their own `rgba(32,33,39,.98)`
gradient at `settings.css #app`).

```html
<div class="glass thin" id="bar">
  <div class="glass-bg"><div class="glass-snap"></div></div>   <!-- clipped to the shape -->
  <i class="glass-rim"></i>                                    <!-- specular edge -->
  ...content...
</div>
```

```css
.glass {
  --r: var(--r-sheet);
  position: relative;
  isolation: isolate;
  border-radius: var(--r);
  /* outer: a dark hairline so the edge reads on white documents, a contact shadow, a soft lift.
     Sized to fit the 20px stage margin in finding 5. */
  box-shadow:
    0 0 0 var(--px) rgba(0, 0, 0, .30),
    0 1px 2px rgba(0, 0, 0, .24),
    0 8px 22px -4px rgba(0, 0, 0, .34);
}
.glass-bg { position: absolute; inset: 0; border-radius: inherit; overflow: hidden; z-index: -2; }
.glass-snap {
  position: absolute; inset: -24px;                 /* blur and refraction need bleed past the edge */
  background-image: var(--snap);                    /* blob: URL set by glass.js */
  background-size: calc(var(--disp-w) * 1px) calc(var(--disp-h) * 1px);
  background-position: calc(var(--bx) * 1px + 24px) calc(var(--by) * 1px + 24px);  /* glass.js: minus element screen offset */
  filter: blur(14px) saturate(1.7) brightness(.94);
}
.glass.refract .glass-snap { filter: url(#refract) blur(14px) saturate(1.7) brightness(.94); }
.glass-bg::after {                                  /* the scrim: finding 4 */
  content: ""; position: absolute; inset: 0;
  background: rgb(var(--tint) / var(--scrim));
}
```

**Refraction approximation.** Real Liquid Glass bends the backdrop in a band along the edge and leaves the
middle flat. Chromium has no `backdrop-filter` here, but it does have `feDisplacementMap` on ordinary
elements, and our "backdrop" is now an ordinary element:

```html
<svg width="0" height="0" aria-hidden="true">
  <filter id="refract" x="0" y="0" width="100%" height="100%" color-interpolation-filters="sRGB">
    <feImage id="refract-map" href="data:image/png;base64,..." preserveAspectRatio="none"/>
    <feDisplacementMap in="SourceGraphic" in2="refract-map" scale="22"
                       xChannelSelector="R" yChannelSelector="G"/>
  </filter>
</svg>
```

`glass.js` generates the map on a canvas when the element's size changes (debounced 120ms): neutral
`rgb(128,128,128)` everywhere; inside a 16px edge band, derive the inward normal from the rounded-rect
signed distance `d` (0 at the edge, 16 deep), set R and G to `128 + 127 * n * (1 - d/16)^2`. Sign and
scale 22 are tuned by eye (flip the sign if the rim looks like it magnifies instead of bending). Rules:
apply it only on `.glass.refract` (the bar and the answer sheet; never on rows, chips, or buttons);
remove it during any morph (`data-morph="1"` on body) and restore it 120ms after settle, because a
displacement filter on an animating layer re-rasterises every frame [M]; fall back to the plain blur
when `data-glass="solid"` or when the filter is not supported. Plain blur is a legitimate, good-looking
default; refraction is the last 10%.

**Specular rim.** One lit edge, light from the top left, a weaker bounce at the bottom right, nearly
nothing on the sides. Done as a masked ring so it is one crisp `--px` line, not a blurred shadow:

```css
.glass-rim {
  position: absolute; inset: 0; border-radius: inherit; padding: var(--px); pointer-events: none; z-index: 1;
  background: conic-gradient(from 0deg at 50% 50%,
    rgba(255,255,255,.16) 0deg,   rgba(255,255,255,.05) 70deg,
    rgba(255,255,255,.34) 130deg, rgba(255,255,255,.10) 200deg,
    rgba(255,255,255,.05) 270deg, rgba(255,255,255,.66) 318deg,
    rgba(255,255,255,.16) 360deg);
  -webkit-mask: linear-gradient(#000 0 0) content-box, linear-gradient(#000 0 0);
  -webkit-mask-composite: xor; mask-composite: exclude;
}
.glass::after {   /* inner top glow: the glass body catching the same light */
  content: ""; position: absolute; inset: 0; border-radius: inherit; pointer-events: none;
  box-shadow: inset 0 1px 0 rgba(255,255,255,.22), inset 0 -1px 1px rgba(0,0,0,.22),
              inset 0 0 18px rgba(255,255,255,.04);
}
```

Replaces `--bevel`/`--edge` (`overlay.css:26`, `:32`), which light all four sides equally: a uniform 1px
`rgba(201,207,221,.17)` border reads as a bordered rectangle, not as a lit object.

**Pointer response (interactive glass).** On `pointermove` over a `.glass`, glass.js sets `--mx/--my`; the
rim and buttons gain `radial-gradient(90px circle at var(--mx) var(--my), rgba(255,255,255,.14), transparent 70%)`.
This is Fluent's old Reveal highlight, which makes it feel native to Windows as well as to iOS 26. Press on
a control: `transform: scale(.96)` 120ms `--ease-out`, release settles through `--ease-settle`.

**Honest limits.** [M] A snapshot cannot show a window that is also content-protected (other apps doing
the same). Multi-monitor: capture the display `screen.getDisplayMatching(win.getBounds())`, re-snapshot
on `display-metrics-changed`. HDR content tone-maps to SDR in the thumbnail, which is fine for a blur.
DPI: `glass.js` sets `--px` to `Math.round(dpr) / dpr` px so 1px edges land on whole device pixels at
125% and 150% (otherwise every hairline in the app is a 1.25px smear).

### 1.3 Gates that stop this being a second acrylic

Written first, because the last attempt died for lacking them. All run on the runner (installer.yml),
per the standing rule.

| Gate | Asserts |
| --- | --- |
| `styleStable` | `GetWindowLongPtr(GWL_EXSTYLE)` and `GWL_STYLE` of every window are identical at boot, after 20 show/hide cycles, after collapse, and after 60s idle. Style never changes at runtime |
| `topmostHolds` | after 60s with a second topmost window present, our window is still above it in `EnumWindows` order |
| `collapseClickable` | after collapse the pill has a hit-testable rect (`WindowFromPoint` at its centre returns our HWND) |
| `noSetOpacity` | grep gate: `setOpacity` does not appear in `main.js` or `lib/` |
| `snapCost` | 50 snapshots at 1/6 scale: p95 under 25ms main-thread |

---

## 2. The unified input

### 2.1 What the user sees

There is no mode label and no mode switch. One 56px capsule, one field. A small glass token sits at the
left of the text and says what the bar will do with what you typed.

```
 idle      (o)  Ask, find, or do                                  /  find    >  do     (gear)
 typing    [ chat  Ask ]  what's this error on my screen|                 Tab  Find        (gear)
 typing    [ find  Find ]  invoice|                                       Tab  Ask         (gear)
 typing    [ arrow Agent ]  rename these screenshots by date|             Tab  Ask         (gear)
 locked    [ find  Find x ]  invoice|                  (user typed "/" : the token is solid)
```

Anatomy (all in `renderer/index.html`, replacing `#mode-label` and `#bar-status`'s mode logic):

| Part | Spec |
| --- | --- |
| Token (`#intent`) | 32px high capsule, 12px left inset (concentric with the 28px bar radius), 8px horizontal padding, 16px glyph, 13px label weight 600 at `--label`. Fill `rgba(255,255,255,.10)`, 1px rim `rgba(255,255,255,.14)`. Inferred: as stated. Locked by a sigil: fill `rgb(197 111 195 / .28)`, rim `rgb(197 111 195 / .5)`, and a 12px `x` glyph appears on hover |
| Glyphs | Ask: message bubble (already the pill's). Find: viewfinder (four corner ticks, same shape the lens draws). Agent: the product's own pointer arrow from `renderer/buddy.html`. Stroke 1.6, not the current 2 (Feather's weight reads as web, SF Symbols as 1.5 medium) |
| Field | 18px / 24px, weight 400, `--label`, caret `--accent-bright`. Placeholder `Ask, find, or do` at `--label-3` (`.42`). Not "anything about your screen": that sentence only works after the word Ask and it is wrong for two of the three verbs |
| Ghost alternate (`#intent-alt`) | right-aligned inside the field's flex row, 13px `--label-3`, `Tab` keycap + the second-best intent name. Only shown when the top two scores are within .30. Never takes layout width from the field (position absolute against the field's right padding) |
| Trailing controls | one 32px circular glass button (settings). Area becomes a chip in the status row, not an icon in the bar. Close `x` is removed: Esc and click-outside already collapse |
| Status row | glass-thin, 28px, appears only when it has something to say (Find count, area chip, offline, hint). Same content as today's `#bar-status`/`#region-chip`/`#notice` merged into one row, 12px `--label-2` |
| Empty-state legend | while the field is empty and focused, a quiet `/ Find   > Do` in two 6px-radius keycaps at `--label-3` at the right. Teaches the sigils once, disappears on first character. No tooltip tour |

### 2.2 Prefixes (explicit, instant, always win)

| Typed at position 0 | Result | Notes |
| --- | --- | --- |
| `/` | Find, locked | Slash is search in every browser, Slack and Vim; also `Ctrl+Shift+G` opens the bar with this token already set |
| `>` | Agent, locked | Command-palette convention, and it reads as an arrow; `Ctrl+Shift+U` opens with it set |
| `?` | Ask, locked | For the rare sentence that would otherwise infer Agent or Find; `Ctrl+Shift+Space` opens unlocked |

Rules: the sigil is consumed into the token on keydown (the character never lands in the field). First
`Backspace` at caret position 0 with a locked token turns it back into the literal character, so `/usr/bin`
is still askable; second `Backspace` does nothing. Word prefixes (`find`, `do`) are deliberately not
supported: "find the bug in this function" is a question. `Esc` clears the text first, collapses on the
second press (Spotlight behaviour). `Ctrl+1/2/3` stay for power users and lock the token.

### 2.3 Inference (soft, local, under 5ms, no model call)

Runs 90ms after the last keystroke. Pure function in a new `lib/intent.js` (so smoke can assert it on a
table). Scores 0..1 for each of ask / find / agent:

| Signal | Pushes toward |
| --- | --- |
| ends with `?`; starts `what, why, how, who, when, can, could, does, is, are, should, explain, summarize, tell me, help`; contains `this`, `error`, `mean`; 5+ words | Ask |
| 1 to 3 words and no verb; contains digits, quotes, `.`, camelCase, snake_case; starts `where is, where's, find, show me, highlight, locate` | Find |
| starts with an imperative from the action lexicon (`open, click, type, rename, move, save, send, create, delete, close, scroll, go to, launch, install, turn, fill, copy, paste, drag`) and no `?` | Agent |
| `how do I` / `how to` before any verb | cancels Agent (so "how do I save this as pdf" is Ask) |

| Input | Top | Alt |
| --- | --- | --- |
| `what's this error?` | Ask .95 | |
| `invoice` | Find .85 | Ask |
| `invoice 2291` | Find .90 | |
| `where is the submit button` | Find .78 | Ask .70 (ghost shown) |
| `rename these screenshots by date` | Agent .90 | |
| `open settings` | Agent .62, below the .85 bar, so the token stays Ask | ghost reads `Tab  Do it` (see asymmetry below) |
| `send the report to Sam` | Agent .90 | the confirm gate still guards the send |

**Asymmetry, because the costs are not equal.** A wrong Ask or Find is a wasted second. A wrong Agent can
click things. So: Ask and Find may be chosen on score >= .55. Agent is chosen by inference only at >= .85;
between .55 and .85 the token stays Ask and the ghost reads `Tab  Do it`. A locked `>` always wins. The
token is always showing the truth before Enter, so nothing happens that the chip did not announce.

**Hysteresis.** Switch the token only if the new top beats the current by .15, or the current is under
.50. Never switch during IME composition. Cross-fade (below) is the only motion; no flicker on a
half-typed word.

**Keys.** `Enter` runs the token. `Tab` swaps to the alternate (cycles ask, find, agent when there is no
alternate; Shift+Tab reverses). This frees `Tab` from today's Find-scope meaning (`overlay.js:286`), which moves to
the scope chip in the status row (click, or `Alt+S`). `Ctrl+Enter`: in Find with a current hit, ask about
it (as SPEC 4.3); otherwise run the alternate without changing the token. `Down` moves focus into Find
results or the agent trail.

Find is live: when the token is Find (locked, or inferred >= .80 with 1 to 3 words) the lens updates per
keystroke exactly as today and Enter steps to the next hit. When the token is Ask, typing does nothing to
the screen. This is the one place inference changes behaviour before Enter, which is why its threshold is
the highest of the non-agent ones and why the token must already be visible.

### 2.4 Inline disambiguation

Never a modal, never a menu. The ghost alternate (above) is the whole mechanism: Enter does the token,
Tab does the other. When the user overrides (Tab) twice for a similar shape in one session, lower the
inferred weight of that signal for the session (a small in-memory table; do not persist).

### 2.5 Morphing and motion (see also finding 3)

| Moment | Spec |
| --- | --- |
| Token changes intent | width animates to new content (FLIP, 240ms `--ease-morph`); old glyph scales 1 to .8 and blurs 0 to 3px over 120ms while the new one scales .8 to 1 and unblurs over 160ms starting at 60ms; label slides 6px vertically with the same stagger. The field and bar never move |
| Locked by sigil | fill cross-fades 160ms, plus one 4% scale settle (`--ease-settle`) on the token only |
| Ghost appears/disappears | opacity only, 150ms |
| Orb to bar | 320ms `--ease-morph`: `width` 44px to bar width, `border-radius` 22px to 28px; content pinned and fading in over 140ms, delayed 100ms |
| Bar to bar + sheet | the sheet grows from the bar's bottom edge on `grid-template-rows: 0fr` to `1fr` (animatable in Chromium) over 360ms with `scale .985 to 1` from top centre and opacity over 200ms. Rim and refraction are re-enabled 120ms after |
| Busy (answer coming) | the specular rim's conic start angle rotates one turn per 2.4s (`@property --a`), replacing the three thinking dots in `.think-dots`. The light travelling the edge is the "working" state of the glass itself. It is the only continuous animation and it stops with the answer |

---

## 3. Stage window: let CSS do the morphing

Today each surface is its own OS window and `lib/motion.js` animates `setBounds` at 60Hz from the main
process. That is why DESIGN.md has a page of traps (resize limit unlock, stamps, mode-aware limits) and
why opening a panel can never look like Apple: an OS window cannot vsync with the compositor, so
anything morphing is stepping at a timer.

Proposal: one transparent **stage** window per work area, large enough for the bar plus the tallest
answer plus a 20px margin (about 560 x 760 at the saved width), never resized while a conversation is
live. Everything that today animates as a window animates as CSS inside the stage on the compositor:
orb to bar, bar to bar + sheet, sheet height, confirm sheet rising, Settings is the exception (below).
The bar and the answer are the same glass object, so there is no seam, no second backdrop offset, and
no gap between two windows that looks like a mistake (`12-live-overlay.png`: two panels with a hard
~12px gap).

Click-through for the transparent remainder is the mechanism the pill already uses
(`overlay.js:340-354`: `setIgnoreMouseEvents(true, { forward: true })` toggled by `elementFromPoint`). It
generalises: ignore unless the pointer is over `.glass`.

Wins: no `setBounds` animation; the DESIGN.md "Resizing" section shrinks to the stage width (and CSS drag
handles on the glass edge, `resize: horizontal` or pointer-captured, are only an enhancement); the
topmost timer churns less because windows stop moving; the acrylic failure mode (window style flapping)
is structurally impossible. Cost: one larger layered surface (cheap) and re-implementing the user's
saved width as a CSS variable. Do this after 1 and 2, but design 1 and 2 as if it exists.

Settings and History remain their own windows (a window you park and read), but get the same material.

---

## 4. Legibility over arbitrary content

Glass over a white document with slate text fails contrast. Computed: slate `#C9CFDD` has relative
luminance .62; over a 112/255 grey (what a 62% dark scrim leaves over pure white) the ratio is 3.2:1, below
4.5. To hit 4.5 with palette slate the scrim over white would have to be .72, which kills the glass. Two
changes, both required:

1. **A brighter label.** New token `--label: #F4F6FB` (L about .92) for primary text on glass. Secondary
   `--label-2: rgba(244,246,251,.72)`, placeholder and idle chrome `--label-3: .50`. This is a deliberate
   exception to "six values", to be added to DESIGN.md's table next to `--skin` with this reason: the
   palette slate was chosen for a known dark grey; glass needs the label to be brighter than the worst
   surface it can sit on. Text also gets `text-shadow: 0 1px 1px rgba(0,0,0,.28)` on `.glass.thin` only.
2. **Adaptive scrim, driven by what is behind.** `glass.js` already holds the 1/6 snapshot; for each
   glass element it reads the 90th-percentile luminance Y (0..1) of the pixels under it (draw the crop
   into a 16x9 canvas, `getImageData`, 2ms) and sets `--scrim`:

| Material | Scrim `--scrim` | Over black | Over mid grey | Over white | Contrast of `--label` over white |
| --- | --- | --- | --- | --- | --- |
| `thin` (bar, chips, status, token, buttons) | `.42 + .28 * Y` | .42 | .56 | .70 | 6.5:1 |
| `thick` (answer sheet, settings, history, cards with paragraphs) | `.70 + .12 * Y` | .70 | .76 | .82 | 10:1 |

`--tint: 18 20 28` (a cool near-black, drawn from shadow grey and indigo, not pure black, so the glass
keeps a hint of the brand). Blur and saturation: thin `blur(14px) saturate(1.7) brightness(.94)`, thick
`blur(28px) saturate(1.5) brightness(.9)`. Smooth Y over 300ms so a scrim never snaps.

The rule that makes glass trustworthy: **controls on glass, reading on thick glass.** The bar is
thin. Anything long (answer prose, settings paragraphs, history previews) sits on `thick`. This is
Apple's own split; `clear` glass is for media only and this product has no media.

Rim legibility on pale content: the `0 0 0 var(--px) rgba(0,0,0,.30)` outer line in 1.2 is what keeps
the white specular edge visible against a white page. Same job `--hit-rim` and `--danger-edge` do today,
now generic.

`prefers-reduced-transparency`, forced-colors and `data-glass="solid"`: `.glass-snap` hidden, scrim fixed
at `.94`, rim kept. Reduced motion: no morph, no orbit; fades only (unchanged from DESIGN.md).

---

## 5. Depth today is clipped away

`#panel` and `#opanel` are `position: fixed` flush against the window edge (`overlay.css:225-257`) and
the window bounds are the panel rect (`main.js` input rect, `lib/motion.js`). `--lift` is `0 1px 2px
rgba(0,0,0,.45), 0 18px 44px rgba(0,0,0,.55)` (`overlay.css:27`). Everything past the window edge is
discarded, so the 44px soft shadow is invisible; only the 2px contact shadow survives inside the 1px
border. Frame `03-panel.png` confirms it: no shadow, just a bordered rectangle. DESIGN.md claims a
"deep soft one"; it is not rendered.

With the stage window (finding 3) there is a 20px transparent margin and the shadow in 1.2
(`0 1px 2px .24, 0 8px 22px -4px .34`, tuned to fit 20px) draws. If the stage is not built first, minimum
fix: add 20px of transparent padding to every window's bounds and pin the panel inside it, with
click-through on the padding. Glass without a shadow looks like a sticker; this is why the current
surfaces read as flat even where the gradient is good.

---

## 6. Type, spacing, radius

**Font.** `font-family: -apple-system, "Segoe UI", sans-serif` (`overlay.css:67`, repeated in five
sheets) resolves to plain Segoe UI on Windows. Windows 11 ships Segoe UI Variable, which is the platform's
own face with optical sizing: `"Segoe UI Variable Text", "Segoe UI Variable Display", "Segoe UI", system-ui,
sans-serif`. Use Display for 18px and up. Transparent layered windows get grayscale antialiasing, not
ClearType, so light-on-dark text renders thinner; compensate with `font-weight: 450` on the variable
face (falls back to 400) for body, 500 for the bar input, and `text-rendering: optimizeLegibility`.
`-apple-system` stays out; it does nothing here.

| Role | Today | Proposed |
| --- | --- | --- |
| Bar input | 15/32 (`#query-input`) | **18/24**, weight 450. A hero field has to be the largest thing on screen |
| Token label | 13 slate .7 | 13 / 600 `--label` |
| Answer body | 14/1.55 | **15/1.5**, weight 450 |
| Headings in answers | 16/15/14 | 17 / 15 semibold / 15 semibold. 14 for h3 is the same size as the body: not a heading |
| Secondary, rows, status | 13 | 13 |
| Meta floor | 12 | 12 (unchanged; never lower) |
| Counts, steps, timings | `tabular-nums` on some | `font-variant-numeric: tabular-nums` on all of them |

**Radius** (the 4/6/8/12 scale is a card-UI scale; glass is concentric and large):

| Token | Value | Used by |
| --- | --- | --- |
| `--r-sheet` | 28px | bar (56px high), answer sheet, settings, history, tour callout |
| `--r-card` | 16px | confirm sheet, code blocks, grouped settings lists (28 minus the 12px gutter: concentric) |
| `--r-row` | 10px | find rows, history rows, rail items |
| `--r-key` | 6px | keycaps |
| capsule | 999px | tokens, chips, buttons, orb |

**Spacing.** Keep 4/8/12/16/24/32. Bar: 56px high, 12px padding on all sides, 12px between token and
field, controls 32px. Sheet: 16px padding (was 12), 12px between blocks, 8px gap below the bar when
docked into one object. The uneven rhythm in `04-answer.png`, with about 40px of air above the last
question chip against 3px between paragraphs, comes from `.md-gap` (8px) plus `.msg.assistant + .msg.user`
(4px) plus the `.msg` stacking; make turn spacing a single `gap: 20px` between turns and 12px inside.

**Hairlines.** Separation by whitespace and a `--hairline` of `rgba(255,255,255,.08)` (white, not slate,
because on glass a slate line goes grey); never full-width inside a group, inset 16px.

**Focus.** `:focus-visible` (`overlay.css:81`) draws a 2px accent outline plus a 1px slate halo: a web-form
ring, and grape on grape-tinted glass is nearly invisible. Replace with `outline: 2px solid rgba(255,255,255,.85);
outline-offset: 2px; box-shadow: 0 0 0 4px rgba(0,0,0,.28)` for any control on glass.

**Motion tokens, amended.** Keep `--ease-glide` (`cubic-bezier(.32,.72,0,1)`) and `--ease-out`. Add:

| Token | Value | Used for |
| --- | --- | --- |
| `--ease-morph` | `cubic-bezier(.22, 1, .36, 1)` | size and radius morphs (stronger than glide, no overshoot) |
| `--ease-settle` | `cubic-bezier(.34, 1.3, .64, 1)` (about 5% overshoot) | token lock, button release, current-hit pop. Tune by eye to under 6% |

This amends one rule in DESIGN.md ("nothing bounces"): Apple's glass morphs settle, and a single 5%
settle on small shape changes is what makes the surface feel liquid rather than CSS. Not looping, not on
windows or sheets. This is a decision for the owner; everything else in this review works without it
(set `--ease-settle` to `--ease-morph`).

---

## 7. Find

| Now | Change |
| --- | --- |
| `.kind-mark` is an 8px filled or ringed circle (`overlay.css:1112`); the lens draws filled or outlined rectangles. Two vocabularies for one idea | `.kind-mark` becomes a 14x8 rounded-rect swatch (`radius 3`) filled `rgb(197 111 195 / .5)` for literal and outlined 1.5px for related and concept: the list shows the same glyph the screen shows |
| `.find-row.current` fill `rgba(130,58,128,.22)` (`overlay.css:1105`) over grey is a muddy mauve (`19-find-results.png`) | current row = a glass-thin pill: `rgba(255,255,255,.12)` fill, `--px` white `.16` rim, `--r-row`, and a 2px `--accent-bright` bar at its leading edge. Accent as a mark, not a wash |
| Hits are flat 22% mauve boxes with a black 1px rim (`lens.css:58`, `20-lens.png`) | literal: `rgb(197 111 195 / .26)` fill, `--r` 5px, inner top highlight `inset 0 1px 0 rgba(255,255,255,.35)`, the outer black rim stays (it is the legibility rim on white). Related/concept: 1.5px ring, same |
| The current-hit ring is the same 2px box, slightly larger (`#ring`) | **The current hit becomes a lens.** In the lens window, `#ring` gets a `canvas` background of the 1/6 snapshot *re-cropped at 1:1 from a fresh capture of that rect* scaled 1.18 about its centre, `border-radius: 8px`, with the rim from 1.2 and `--ease-settle` pop (scale .94 to 1) on arrival. The text under the current hit is magnified and pushed apart from its neighbours, which is exactly Liquid Glass's lensing gesture, and it is far easier to read where "you are" than a 2px ring. Capture crop only on `find:current` (one 400x200 grab, about 8ms), fall back to the ring when capture is unavailable or the page scrolled since |
| The why chip uses the old `--skin` and an 8px radius (`lens.css:90`) | glass-thin capsule/16px card, 13px `--label`, the same material as everything else; arrow-less, 10px offset from the ring |
| `This window` is plain 12px text at the status row's right; nothing says it is a control | scope chip: capsule, 12px, a chevron-down glyph, rim on hover |
| Empty state (`26-find-empty.png`): a small bordered box with two lines of 13px grey, the same weight, left-aligned, no way forward | one 15px `--label` line (`Nothing on screen matches "zebra"`), one 13px `--label-2` line, and the alternate as a real chip: `Tab  Ask about it instead`. The code already has `findState.action === 'ask'` |
| Count `2 exact · 2 related` at 12px low contrast | 13px, the two numbers carrying their swatches: `[swatch] 2   [ring-swatch] 2`; the words go to the tooltip |

Control frame (`#frame`, `#chip`): keep it solid (DESIGN rule), but as a bezel not a line. 2px
`--accent-bright` at .9, 1px inner `rgba(255,255,255,.22)`, 1px outer `--hit-rim`; chip becomes a glass-thin
capsule with the arrow glyph: `Agent is working  ·  Esc to stop` (the user's own words in 20 frames: say
"working", not "using your computer", which is the Claude-in-Chrome phrasing and a little alarming).

---

## 8. Agent trail

`22-24` read as a developer log: `Plan`, `Read Notepad 240ms`, `Press Ctrl+Shift+S in Notepad 180ms`.

| Now | Change |
| --- | --- |
| Plan printed as a numbered list, then the executed steps printed again as rows (`22-agent-running.png`: "Open Save As / Choose the desktop / Name it / Save" then "Press Ctrl+Shift+S / Click Desktop / Type..."). The same story twice | The plan items **are** the rows. Each starts as a quiet pending row; when a step runs it takes the running mark and its verb text appears under the plan line (`Press Ctrl+S` as 12px `--label-2`). One list, ticking |
| 6px dots as status marks, running is a violet dot | 16px glyphs: pending = 6px ring, running = a 14px arc spinner (rim-orbit language, 900ms linear), done = check in a 14px circle at `--label-2`, failed = x in a circle at `--danger`, skipped = dash. Glyph-shape changes carry state; colour is secondary (also fixes colour-only state in `.trow-mark`) |
| Millisecond timings on every row (`240ms`, `180ms`) | Show only for steps over 1.5s. Reads never get a row; they collapse into the previous action's caption |
| `Read` rows at 12px between 14px rows: ragged baseline | one row size, 14px; 4px row gap, 12px between plan lines |
| Done state (`24-agent-done.png`): the outcome `Saved notes.txt to your desktop.` is last, 14px, same weight as the log | Outcome **first**: 17px result line at top of the block, `5 steps  v` disclosure below it, collapsed |
| Error (`25-agent-error.png`): the refusal appears in the row *and* in the closing sentence ("window runs as administrator" twice), with a Retry that looks like a disabled chip (`.retry-btn`, twilight at .45) | one message: the row shows the glyph and the action; the closing line gives the reason and the fix once; Retry is a glass-thin button with a rim, not a blue-grey chip |
| Stop is the same twilight chip | glass-thin button, 32px, rim; becomes the only control in the busy state |
| **Confirm** (`23-agent-confirm.png`): a flat opaque `#303D6A` blue card with `Skip  [Ctrl+Backspace]` and a purple `Allow [Ctrl+Enter]` button. The bluest, flattest, most "Bootstrap alert" object in the product, and a 14-character keycap that crowds the buttons | a **glass sheet** that rises from the bottom of the answer sheet (translate 14px, `--ease-settle`, 260ms), `thick` material, `--r-card` 16px, 16px padding. Title 15/600 `Click "Save" in Notepad`; one line 13px `--label-2` combining the reasons (`Saves the file. Asked because the button is named "Save".`). Buttons: `Skip` (glass-thin, 32px) and `Allow` (prominent: `rgb(197 111 195 / .55)` fill, `inset 0 1px 0 rgba(255,255,255,.35)`, 32px, 600). Shortcuts live in the buttons' `title` and a single 12px line `Enter to allow, Backspace to skip` only while the sheet has focus. The buddy arrow points at the target at the same moment (already specified) |

---

## 9. Settings, history, onboarding, buddy

**Settings** (`29`, `30`): it is a dark IDE settings panel. Opaque `rgba(32,33,39,.98)`, a bold label per
row, "Edit" as a bare text link, no row affordance. Rebuild as macOS System Settings: the sidebar is a
floating `glass thin` column (160px, `--r-card`) with the current item a `--r-row` pill at
`rgba(255,255,255,.12)` (the 2px accent bar goes; accent is for the primary action); content is
**grouped lists**: each group a `thick` glass card (`--r-card` 16px) holding rows 40px high with 16px
side padding and an inset hairline between rows. Row label 14 regular `--label` (not 600: bold is for
the 13px/600 group heading above each card, in `--label-2`), value 14 `--label-2`, a chevron-right glyph
instead of `Edit`. The one-line `Permissions` row opens a popover list or a sub-page; the radio list in
`29` marks the selected row only by a mauve tint, so add a trailing check glyph (non-colour cue).
Row heights are 24px for switches and 49px for others in `29` (ragged); one 40px row everywhere.
Switches stay iOS-style but use the glass token for the off state and `--accent` tinted glass for on.
Title bar: `Settings` 13/600 centred, close a 32px circle.

**History** (`16`): same grouped list. Date section headers (`Today`, `Yesterday`, `This week`) at
13/600 `--label-2`; rows 56px, title 14 weight 500, preview 13 `--label-2`, meta 12 `--label-3`; the
search field is a glass-thin capsule with the 14px magnifier, not a bordered 8px box (it is the one
bordered input left, against DESIGN's own rule 1). Hover pill `--r-row`, delete as a trailing circle.
Consider showing History as a view *in the stage* (clock button in the sheet header) rather than a window.

**Onboarding and buddy** (`28-onboard-*`, `11-buddy.png`, `cursor-buddy.css`): the callout is the only
surface in the product that is a solid saturated slab: `#10254F` fill with a `#303D6A` border, a
purple smiley face and the name `Buddy`. A named mascot face is the most generic AI-product element in
the app, and it contradicts DESIGN rule 5 (no product name on its own surface). Changes:
glass-thin card at `--r-sheet` 20px; drop `Buddy` and the face; the tail (a rotated bordered square,
`.bubble-tail`) is a visible diamond seam in glass: remove it, place the card 14px down-right of the arrow
tip instead, the arrow is the connector; replace `1 of 4` with four 6px pips (current one is a 16px wide
capsule, morphing 240ms); body 14px/1.45 `--label`; `Skip` and `Later` as quiet text buttons, `Try it`
as the prominent glass button. The arrow itself stays the one purple solid object, but give it the glass
treatment: fill `linear-gradient(#D888D6, #823A80)` with a 1px `rgba(255,255,255,.7)` stroke and a
`drop-shadow(0 2px 4px rgba(0,0,0,.45))` (as now). Do not add a landing ring: v0.3 removed it on purpose.
Tour copy must change with finding 2: it currently teaches three hotkeys and three modes; it should teach
one (`Ctrl+Shift+Space`), then `/` and `>`.

**Pill** (`01-pill.png`): a 72x44 rounded rectangle containing a Feather chat-bubble icon, the word
`Ask`, and a 6px dot. Becomes the **orb**: 44px glass circle (thin), the arrow glyph at 20px, no label.
State by shape not text: waiting-conversation = 8px accent badge at the top right with a 2px glass ring;
busy = the rim orbit; agent = the step number inside the orb (`5`, tabular); failed = `--danger` badge. One
glyph, three states, nothing to read in the idle state.

---

## 10. Per-screenshot: what is generic or off-brand

| Frame | Observation | Fix (finding) |
| --- | --- | --- |
| `01-pill` | Feather chat icon plus the word "Ask" on a grey gradient with a 1px border: reads as a chat-widget launcher, and "Ask" will be wrong once there are no modes. The dot is a 6px mystery | orb, arrow glyph, state by badge (9) |
| `02-panel-nokey` | Bar at 15px; the only saturated object is a flat violet 6px-radius rectangle `Open settings`; the notice icon is stock Feather alert-circle; copy `No OpenRouter API key saved yet.` is a status, not a next step | prominent glass button; one status row; `Add a key to get started` (2, 6) |
| `03-panel` | 12px radius on a 62px bar: a rectangle, not a capsule. Uniform 1px border. No shadow rendered. Three same-weight icons (crop, gear, X) read as a toolbar. Label `Ask` and a dangling `anything about your screen` | capsule, token, one control, shadow (1, 2, 5) |
| `04-answer` | A good typographic start, undermined by: 14px body; the blue-tinted inline-code chip with a border (reads as GitHub); a black code well with a visible blue border; the rhythm gap above the last chip; the question chip's 4px "tail" corner (iMessage nod, wrong here) | 15px; code on `rgba(0,0,0,.28)` no border, `--r-card`; turn gap 20px; plain capsule for the question (6) |
| `05-answer-thinking` | Three 5px loop dots (the generic AI typing indicator) and a blue-grey `Stop` chip | rim orbit; glass-thin Stop (2.5, 8) |
| `12-live-overlay` | Best frame for the argument: two opaque slabs over live text, a hard 12px gap between the bar and the sheet, the selected prefill in saturated grape. Neither slab sees what is behind it | one object, live backdrop (1, 3) |
| `18-panel-find` | `This window` looks like static text; count line is 12px, low contrast; no hint that Enter steps | scope chip, swatch count, `Enter next` in the status row (7) |
| `19-find-results` | Hollow and filled circles are a radio-button idiom; current row a muddy mauve wash; whole list is one flat grey card | rect swatches; glass pill for the current row; thick sheet (7) |
| `20-lens` | Hits are dull mauve boxes; ring has a double outline (pink then black); chip is a dark box in the old skin; top frame is a 2px line | filled hits with highlight; lens magnifier; glass chip; bezel (7) |
| `21-panel-agent` | Same bar with a different word; the mode switch is a text label you have to know to click | token (2) |
| `22-agent-running` | Developer log; plan duplicated; ragged baselines; ms timings | checklist, glyphs (8) |
| `23-agent-confirm` | Flat `#303D6A` slab, `Ctrl+Backspace` keycap, button hierarchy unclear | glass sheet (8) |
| `24-agent-done` | Result is the quietest thing in the frame | outcome first (8) |
| `25-agent-error` | The refusal appears twice; Retry looks disabled | one message (8) |
| `26-find-empty` | A tooltip-sized box floating in a window of its own, no next action | headline + alternate chip (7) |
| `27-find-offline` | Acceptable content, but the offline fact is plain grey text; should read as a state | glyph + 12px `--label-2` in the status row (2.1) |
| `28-onboard-1..4` | Saturated indigo slab, named smiley mascot, diamond tail seam, `1 of 4` | glass card, pips, no tail (9) |
| `29`, `30` | Settings as an IDE panel: bold labels everywhere, `Edit` links, mauve-only selection, ragged row heights | grouped glass lists (9) |
| `16-history` | Opaque near-black, a bordered search box, no date grouping | grouped glass list (9) |
| `11-buddy` | A recoloured Windows cursor with a smiley bubble | glass callout, no face (9) |

---

## 11. Make the next review possible

`lib/shots-v2.js` photographs every surface over one flat colour. Add, per surface: three backdrops
(a white document with 11px body text, a black code editor, a saturated wallpaper photo), 3 captures
each, and a contrast gate: sample the text pixels' luminance against the pixels 3px outside each glyph
box and fail below 4.5:1 for 14px+ text, 3:1 for 12px. Also add the 1.25 and 1.5 DPI frames for 1px
edges. `styleStable`, `topmostHolds`, `collapseClickable`, `noSetOpacity`, `snapCost` from 1.3 join the
smoke `must()` list. The photograph that DESIGN.md v1 says nobody opens is the reason this matters:
make the gate read the numbers, not wait for someone to look.

---

## 12. DESIGN.md changes this requires (so the file stays the authority)

- Replace the "`--skin` ... the material that replaced acrylic" row with `--scrim`, `--tint`, `--label*`,
  `--r-*`, `--px`, `--ease-morph`, `--ease-settle`, each with its reason (1.2, 4, 6).
- Replace the v0.5 "Resizing" section with the stage window rule (3), keeping the min/max-size warning
  for Settings and History, which stay as windows.
- Add a "Material" section stating: windows never change style at runtime; glass is rendered in the page;
  the backdrop is a captured snapshot, never written to disk; the fallback ladder; the five gates in 1.3.
- Amend "nothing bounces" to the settle rule, if the owner agrees (6).
- Replace the "Mode label" and "Bar status" rows with the intent token, ghost alternate, and status row
  (2.1), and the Tab / Ctrl+1..3 rows in SPEC 4.3 with 2.2 and 2.3's keys.
- Mode names `Ask` / `Find` / `Agent` stay as nouns on the token and on the tray, per "One name per action".

## What I did not decide

Whether the stage window (3) lands in the same release as the material (1). They are independent: glass
works in today's windows with the 20px margin hack from finding 5. The order that gives the biggest visible
gain for the least risk is 1, then 2, then 3, with 4 shipped alongside 1 because glass without the scrim
fails on white documents.
