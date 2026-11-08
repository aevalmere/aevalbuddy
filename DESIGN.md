# AevalBuddy design constraints

Written before the v0.2 surfaces were built. Every visual change builds to this file; anything outside it
is a defect.

## What this app is remembered for

The assistant feels like part of Windows, not a website floating over it: you talk to it like a messaging
app, and it can physically point at your screen when an answer needs a "click here".

## The one pushed dimension

Material and motion, together (v2.1 Phase B). The surfaces are liquid glass: they take their light from
whatever is behind them, and they change shape instead of appearing. Colour, type and layout stay quiet
and systematic. Nothing loops for decoration. Everything decelerates, and one curve, the settle, may
overshoot by 5% and come back (the owner approved this; it amends "nothing bounces", see below).

## Material: liquid glass, drawn in the page

Every surface is a sheet of glass cut from a picture of the desktop behind it. The glass is drawn by the
page, not by Windows: `renderer/glass.css` and `renderer/glass.js`, shared by every window.

- **Backdrop.** `lib/backdrop.js` captures a 1/6-scale JPEG of each display with `desktopCapturer`, at
  2 Hz while a glass surface other than the pill is visible, every 5 s while only the pill is, and not
  at all while nothing is. Our windows are excluded from capture (`WDA_EXCLUDEFROMCAPTURE`), so the
  picture never contains us and the glass never shows itself. Only pages that load `glass.js` get
  frames.
- **Body.** The frame is placed under each glass layer at its screen position, then blurred and
  saturated: thin glass `blur(14px) saturate(1.7) brightness(0.94)`, thick glass `blur(28px) saturate(1.5) brightness(0.9)`.
- **Edge.** The outer 14 px of a refracting layer bends the backdrop outward with an `feDisplacementMap`
  built per layer (scale 22 thin, 28 thick). Only the bar and the answer sheet refract. Small controls
  do not, because at their size it reads as a smear.
- **Rim.** A specular line is drawn with a masked conic gradient: brightest at the top left, faint at
  the bottom right, `--px` wide so it stays one physical pixel at every scale.
- **Scrim.** Between the backdrop and the text there is a tint, `rgb(16 18 28 / a)`. Its alpha is
  measured, not chosen: `glass.js` reads the 90th-percentile luma of the frame under each layer and sets
  thin `a = 0.46 + 0.32 Y`, thick `a = 0.72 + 0.16 Y`. A white document gets a darker scrim than a
  black editor. That is what holds text contrast over white, black and busy content.
- **Thin and thick.** Thin glass is for controls: the pill, the bar, chips, menus, the tour card.
  Thick glass is for reading: the answer sheet, Settings, History, the HUD ghosts.
- **Solid tier.** The same surfaces, with the backdrop replaced by `--solid`, an opaque top-lit
  gradient. Used when the person asks for it (Settings, General, Glass), and automatically under
  `prefers-reduced-transparency`, forced colours, Windows transparency effects off (both superseded:
  see "Liquid glass you set" below), when the windows
  cannot be hidden from capture (so a frame would contain us), or after three capture failures in a row.
  The tier and its reason are on `html[data-glass]` and `data-glass-why`, and Settings says which.
- **Shadow and margin.** The overlay and the answer window are 20 px larger than their glass
  (`lib/stage.js`). The margin is transparent and click-through, so the shadow and the morphs are never
  clipped by the window edge. Bounds code speaks in glass rectangles, and the wrapper adds the margin.

### Cost

Measured on the GitHub runner by the `snapCost` and `glassCost` smoke gates and logged with every run
(PROGRESS.md has the numbers). A capture may not block the main thread for more than 25 ms at p95.

### Edges, controls and the surfaces without a backdrop

- **Scroll edge.** A list that scrolls under a sheet's edge fades into the glass over 24 px, only on an
  edge with more content behind it (`.edge-fade` in `glass.css`, driven by `animation-timeline: scroll()`).
  A list at rest at its top has a crisp top; a list that does not scroll has no fade.
- **Every control has a shape.** The answer's Copy and Do it are capsules with `--fill-1` and the chip rim,
  like Stop and Retry. The History and New conversation icons sit in the sheet's top band at rest in
  `--label-3`; hidden until hover they left an empty band that read as a layout fault.
- **Selection is a raised fill, not a stripe.** The current Find row is `--fill-2` with its own rim. The
  swatch already carries the hit's colour.
- **The area selector** draws no backdrop, so its size chip, hint and keycaps are the solid tier of thin
  glass: `rgb(16 18 28 / .86)`, the rim, the sheets' shadow, capsule radius, Segoe UI Variable. It repeats
  the tokens it uses because it does not load `glass.css`.
- **Focus on a field** brightens its rim and adds a 3 px accent halo outside it, instead of a white ring.

## Palette

The six values stay, and each has a new job on glass:

| Token | Value | Job |
| --- | --- | --- |
| `--label` | `#F4F6FB` | primary text on glass. Solid, never alpha |
| `--label-2` | `#C9CFDD` (pale slate) | secondary text. Solid |
| `--label-3` | `#B8BECA` | tertiary text and placeholders. Solid |
| `--accent` | `#823A80` (grape soda) | the accent on the solid tier, the pointer body |
| `--accent-bright` | `#C56FC3` | the accent on live glass: locked token, current row bar, lens ring |
| `--accent-glass` | `rgb(197 111 195 / .42)` | the prominent glass button's fill |
| `--danger` | `#F2A0A8` | errors and Delete all (lightened from `#E4818C` to hold 4.5:1 on the scrim) |
| `--tint` | `16 18 28` | the scrim, drawn from shadow grey and space indigo |
| `--fill-1/2/3` | white at .08 / .13 / .20 | hover, pressed and selected fills inside glass |
| `--hairline` | white at .10, `--px` high | inset separators between rows |

Text is never alpha. On glass the background changes under every frame, so an alpha label's contrast is
whatever the desktop is that second. Secondary and tertiary text are solid colours with a measured
floor instead. The contrast gate (`lib/contrast.js`) reads every run of text against the pixels actually
under it in each shot: 4.5:1 for body text, 3:1 for large text and UI.

## Type

Segoe UI Variable: Display for the bar and titles, Text for everything else (fallback Segoe UI), Cascadia
Mono or Consolas for code. The bar's text is 18/24. Answers and body text are 15/1.5. Then 14 for rows,
13 secondary, 12 metadata (floor). The first Markdown heading level inside answers is 17 semibold, the rest step down from there.

## Spacing and radius

Spacing scale: 4, 8, 12, 16, 24, 32.

Radius scale: 28 for sheets and the bar, 16 for cards and grouped lists, 10 for rows and fields, 6 for
keycaps, capsule (999) for chips, buttons and the search field. The bar is a 56 px capsule.

Borders: none. A glass edge is its rim and its shadow. Separators inside a group are hairlines inset
from the text's left edge, `--px` high, so they stay one physical pixel at 100%, 125% and 150%.

New profiles start with a 600 px bar (the `inputWidth` default; windows' `PANEL_WIDTH` matches). At 420 px the close button was hidden and a question had about 200 px after the token and the icons; Spotlight is about 680 px and Raycast 750. A width a person already stored is kept.

## Motion

| Token | Value | Used for |
| --- | --- | --- |
| `--ease-glide` | `cubic-bezier(0.32, 0.72, 0, 1)` | window-level entrances (unchanged) |
| `--ease-out` | `cubic-bezier(0.16, 1, 0.3, 1)` | in-surface micro motion (unchanged) |
| `--ease-morph` | `cubic-bezier(0.22, 1, 0.36, 1)` | one shape becoming another: pill to bar, token width, sheet growth |
| `--ease-settle` | `cubic-bezier(0.34, 1.3, 0.64, 1)` | the last 5%: a token locking, the loupe landing, a switch |
| `--d-fast` | 150ms | hover, press, opacity |
| `--d-med` | 240ms | message entrances |
| `--d-morph` | 320ms | shape changes |
| `--d-sheet` | 360ms | the answer sheet growing out of the bar |

- The pill becomes the bar: the glass layer's insets animate from the orb's rectangle to the bar's, and
  the content fades in once the shape is most of the way there. Collapsing runs it backwards, and main
  waits for it before shrinking the window to the pill.
- The bar grows the answer sheet: a clip-path reveal from the bar's edge on `--ease-morph`.
- The intent token morphs its width and label with FLIP. Locking it plays the settle.
- Thinking is a rim light orbiting the sheet's edge, not three dots. The words "Thinking" or the
  current stage sit in the transcript for a screen reader and for anyone who does not see motion.
- Find's current hit is a magnifier: the pixels under the hit, cut at full resolution, scaled 1.18 inside
  a glass rim, landing with the settle.
- Reduced motion is read live (`matchMedia` change, not just at load). Under it every morph is a 150 ms
  cross-fade and the orbit stops.

Rules kept: animate transform, opacity, clip-path and insets inside the page. Never animate window
opacity (see v0.5). Exits run at roughly half the entrance duration.

### Superseded, with reasons

| Old rule | Now | Why |
| --- | --- | --- |
| Nothing bounces | One curve may overshoot 5% and settle | approved by the owner. A 5% settle reads as a physical object coming to rest, which is what glass that changes shape needs. It is only for arrivals of small things (token lock, loupe, switches), never for windows |
| The surface is opaque (v0.5) | The surface is see-through glass, with a measured scrim | v0.5 was right that 97% acrylic let a bright terminal show through text. The scrim is now measured per frame from the backdrop's luma, and the contrast gate proves the result on white, black and colourful backdrops at three scales |
| Depth is a material, not a compositor feature (v0.5) | Kept, and extended | the glass is still drawn in the page. Nothing asks Windows for acrylic or mica, so none of the window-style churn that broke v0.4 comes back. `noSetOpacity` and `styleStable` gate it |
| `--skin` gradient on every window | `--solid`, only on the solid tier | the gradient survives as the fallback material |
| Secondary text is slate at 0.70 alpha minimum | Secondary text is a solid colour | alpha text on glass has no fixed contrast |
| Segoe UI, 15 input, 14 body | Segoe UI Variable, 18/24 bar, 15/1.5 body | the bar is the product's face; at 15 px in a 56 px capsule it read as a form field |
| Radius 4/6/8/12 | 28/16/10/6/capsule | concentric radii: a 28 sheet holds 16 cards holds 10 rows. 12 px windows read as web cards next to Windows 11's own 8 px windows under glass |
| 1 px twilight border on overlay windows | rim light plus shadow, no border | the rim does the boundary job (3:1) on dark and light backdrops, and a border under refraction draws a second edge |
| One window per surface, each a separate rectangle | Each window has a 20 px transparent margin | a single stage window was judged too risky: the layout editor places input and output independently, and about 54 call sites own those bounds. The margin gives the shadow and the morphs room without touching that logic |
| The tour's mascot face, name and tail | A plain glass card with pips | a face on a utility reads as a toy. The arrow is the connector; pips carry progress |

## References, by mechanism

- Raycast: the window appears scaling ~0.95 to 1 from its anchor with a fast decelerating curve, no bounce.
- macOS sheets (and Emil Kowalski's vaul): one signature curve reused everywhere, `cubic-bezier(0.32, 0.72, 0, 1)`.
- iMessage: new messages rise a few pixels and fade in; the transcript scrolls, the window does not jitter.
- Game HUD editors: layout editing is a dedicated mode with drag, snap, and an explicit non-drag control
  for every drag action (dock and detach are also buttons).

## v0.3 plan: what changes and why

Written after photographing every surface with `npm run shots` (see below). Each item names the defect
seen in the shot, the cause, and the change.

| # | Defect in the shot | Cause | Change |
| --- | --- | --- | --- |
| 1 | Answers take many seconds to start | the picker only offered `:free` models, which are queued and rate limited | curated tiers: one recommended fast model as the default, a fast tier, and the free tier kept for zero-credit accounts. Labels carry live price and context, so they never go stale |
| 2 | The pointer is a pulsing violet reticle | it was drawn as a ring plus dot, centred on the target | an arrow cursor drawn to the Windows geometry, tip exactly on the target pixel, no ring, no pulse, one soft drop shadow |
| 3 | Weak models ramble and miss the point | the system prompt described the app but never how to think | skills: markdown procedure files loaded at boot and appended to the system prompt, so the answer shape is enforced outside the model |
| 4 | The callout is a floating box with a close X jammed into the text | it was built as a tooltip, not a character | speech bubble with a tail aimed at the arrow, text typed out, a small buddy face, and drag to move it |
| 5 | Settings reads as four unrelated boxes; shortcut rows wrap and misalign | rows were flex with a full-text warning inline; every card carried an always-reserved status line | numbered steps (connect, choose a model, choose how it answers), a three-column grid for shortcuts so every row aligns, status lines that take no space until they say something |
| 7 | Opening flashes: the window appears at full size, then the contents scale | Electron paints the window at its final bounds before the renderer has the new mode | the window bounds are animated from the pill outward on the glide curve, opacity ramps from 0, and content staggers in behind it |
| 8 | Dead grey band under a short answer | the answer window only ever grew, in 24px steps | exact content height with an 8px deadband; it still refuses to shrink mid-stream |

### Window motion (new)

Window bounds are animated in the main process, because CSS cannot move an OS window. One helper drives
every case on `--ease-glide`, sampled at 120Hz:

| Transition | From | To | Duration |
| --- | --- | --- | --- |
| Pill to panel | pill rect, opacity 1 | input rect | 260ms |
| Panel to pill | input rect | pill rect | 170ms |
| Answer appears | 92% height at the dock edge, opacity 0 | full rect, opacity 1 | 300ms |
| Answer leaves | full rect | 96%, opacity 0 | 150ms |
| Answer regrows | old rect | new rect | 200ms |

Exits are roughly half their entrance, per the rule above. Under `prefers-reduced-motion` (read once from
the renderer at boot) every window animation collapses to a single setBounds.

## Every surface ships four states

| Surface | Loading | Empty | Error | Offline |
| --- | --- | --- | --- | --- |
| Answer window | thinking dots inside the assistant bubble | window stays hidden until a first message exists | inline row naming the failure and the fix | network errors name the connection and the retry key |
| Model picker | "Loading models..." label | never empty: live list, else known-models fallback with a labelled note | inline note when the live fetch failed | fallback list keeps the picker usable |
| Settings key | saved state shown as a chip | "No key yet" chip plus the walkthrough | inline message next to the field | not applicable (local) |

## v0.4: why the input and answer surfaces read as amateur

"It does not feel like Cluely" is a real observation, and it is not about the animation. Reading the
captured surfaces against what premium overlays actually do, there are eight specific tells. Each is a
rule violated, not a matter of taste.

| # | Tell in the current build | Why it reads cheap | Rule |
| --- | --- | --- | --- |
| 1 | A bordered, differently-coloured text field sits inside a bordered panel | Boxes inside boxes. Every serious command bar makes the bar itself the field | One surface per window. Nothing inside a window gets its own border |
| 2 | Focus draws a 2px accent ring plus a slate halo around the inner field | A glowing outline is a 2010 web form | Focus lifts the whole surface: border brightens, nothing glows |
| 3 | Paddings run 14/16/12 on the panel, 8/10 in the field, 30px buttons | Three rhythms in a 66px bar | One rhythm: 12px gutter, 32px control height, everywhere |
| 4 | The gear and X sit at full contrast, same weight as the text | Chrome competes with content | Idle chrome at 55%, full only on hover or focus |
| 5 | The answer window carries a title bar reading "AevalBuddy" | Branding inside your own overlay is noise the user already knows | No product name on a surface the product owns |
| 6 | Both the question and the answer sit in coloured bubbles | Bubbles read as a toy chat app; an answer is a document | The question is a quiet right-aligned chip; the answer is prose directly on the surface |
| 7 | The notice is a filled pill inside a filled panel | Third level of nesting | Separate with a hairline at 8% and whitespace, never another fill |
| 8 | Surfaces are 97% opaque flat grey with a hard shadow | It sits on top of the desktop rather than belonging to it | Translucent surface, real background blur, a 1px inner highlight along the top edge, one large soft shadow |

Number 8 needs an OS feature, not CSS: `backdrop-filter` cannot see past a window boundary, so a
transparent Electron window has nothing to blur. Windows 11 exposes real acrylic through
`backgroundMaterial`, which is incompatible with `transparent: true`. That trade is tested in the app
and kept only if it survives a real screen capture, not assumed.

### Surface tokens (new)

| Token | Value | Note |
| --- | --- | --- |
| gutter | 12px | one padding value for panel, bar, and answer |
| control | 32px | every icon button and field row |
| hairline | `rgba(201, 207, 221, 0.10)` | the only divider |
| top highlight | `inset 0 1px 0 rgba(201, 207, 221, 0.09)` | the glass bevel |
| shadow | `0 16px 40px rgba(0, 0, 0, 0.45)` | one soft shadow, no hard offset |
| chrome idle | 0.55 opacity | icon buttons at rest |

### What the v0.4 rebuild actually changed

Verified against a live composited capture, not a page render: the query bar is one surface with the text
sitting on it, focus is the caret plus a border lift, chrome idles at 55%, the answer is prose with only
the question keeping a container, and the window supplies the corner, the edge, and the shadow wherever
Windows 11 acrylic is available.

One defect only the live capture could have caught: the blanket `:focus-visible` rule was drawing a 1px
halo around the query field, which redrew exactly the nested box the rebuild removes. A page render shows
it as intended styling; over a real desktop it reads as the old design.

## v0.5: acrylic reverted, and what that cost

(v2.1 Phase B keeps this section's lessons and replaces its material: see Material, above.)

Acrylic shipped in v0.4 on the strength of a two-window comparison over the desktop. It looked better in
that test and it broke the app in use: the overlay started falling behind other windows, the collapse
left nothing visible to click, and the desktop picked up repaint artifacts. The test was the problem. It
proved one frame of appearance and exercised none of show, hide, opacity, click-through toggling, or
topmost over time, which is where an opaque window differs from a layered one.

Three things came out of it, and all three stay:

- Depth is a material, not a compositor feature. A top-lit gradient, one crisp edge, a contact shadow and
  a deep soft one read as dimensional without any OS help. The surface is opaque, because at 97% a bright
  terminal behind it was still legible through the text on top.
- Window opacity is not an animation channel on Windows. `setOpacity` toggles the layered window style,
  and driving it at frame rate is what put artifacts on the desktop. Bounds animate; fades happen in CSS
  inside the window. The frame rate came down from 120Hz to 60.
- Topmost is re-asserted on a slow timer as well as whenever a window settles. An overlay quietly falling
  behind is the one failure it cannot have.

### Resizing

Every window is draggable at the edges, and the sizes persist. Two rules keep it honest:

| Surface | Width | Height |
| --- | --- | --- |
| Input bar | 320 to 900, user's | pinned to content |
| Answer | 320 to 900, user's | follows the transcript until dragged, then held until New chat |
| Settings | 380 to 900, user's | user's |
| Pill, layout ghosts | pinned | pinned |

Resizability is expressed through minimum and maximum sizes, never `setResizable`. On a frameless window
clearing that flag strips the thick-frame style and setting it back does not restore it, so a window
switched off once would stay unresizable for the session. Equal limits pin a window just as firmly and
are reversible.

The other trap: `setBounds` is clamped by the same limits a drag is. Every programmatic move unlocks the
limits first and re-applies them after, and each one is stamped so the resize handler can tell it from a
drag. Without the stamp, the collapse animation ran the window down to pill width and stored that as a
chosen width; without mode-aware limits, the panel minimum was applied to the pill and Windows widened
it to satisfy them.

## v0.6: the pipeline was starving its own brain

Answers were imprecise, and the user's guess at why was right: the reading was bad before the thinking
started. Four separate losses stacked up, each one invisible on its own.

| Loss | What it did | Fixed by |
| --- | --- | --- |
| Capture at 1280 wide, JPEG 70 falling to 35 | On a 2560 display that is a halving, and quality 35 turns 12px UI text to grey mush. Every misread identifier started here | 1600 long edge, quality 88, floor 62 |
| The fastest free model as the default reader | 4B active parameters, chosen for speed, doing the one job where accuracy is everything | A model built for reading images is the default; the fast one is still there, labelled as the speed trade |
| Pixel coordinates from a model that cannot see | The brain was copying digits out of a report and, when they were not there, inventing them. Numbers always looked plausible, so nothing caught it | The brain names a target, the app resolves the name against the reader's table, and an unmatched name points at nothing |
| A reading capped at 900 tokens with one TEXT line | The brain only ever knows what the reader wrote down; a truncated error message is an unanswerable question | 1600 tokens, an explicit instruction to copy the whole error, and a FOCUS line |

The coordinate grid moved from pixels to 0 to 1000 across the view. A vision model asked for "the centre
in pixels, the view is 1600x1000" has to hold two numbers it was never given; the normalised grid is the
convention these models are trained on and needs no context to be correct.

### The half of that fix that never landed

The grid replaced pixels in the reader's prompt and nowhere else. The model answering straight from the
screen — the one running whenever brain mode is off, or whenever the reader loses the format and the
request falls back to a single stage — kept being told "you can see a 1600x900 pixel view, give me pixel
coordinates". It answered on the grid anyway, because that is what it was trained on and it has no way to
measure a view the provider resized on the way in. The app then divided those numbers by the image size.

| The model means | It writes | The old mapping landed on | Off by |
| --- | --- | --- | --- |
| the centre of the screen | `500,500` | a third across, half way down | 487px |
| a control top right | `750,250` | the middle of the screen | 721px |
| a button bottom right | `938,972` | nothing: y over 900 failed the range check | dropped |

Every point drifted up and to the left, and everything down the right or bottom edge silently vanished —
the answer said "click here" and no pointer appeared. One `GRID_RULE` constant now feeds both prompts, so
there is one convention to be right about. A pair over 1000 can only be view pixels, so it is still read
as such rather than thrown away; below 1000 the two readings are genuinely indistinguishable, which is
what the prompt is for.

### Everything downstream of a name that never matched

Pointing in the default configuration runs entirely through the reader's table, so a row that fails to
parse is a pointer that never appears. Six of the shapes a small model actually writes were being dropped
on the floor by one regex that expected `label | x,y`:

| Dropped | Now |
| --- | --- |
| `- Save button \| x=120, y=340` | the line comes apart in steps — strip the bullet, find the pair at the end, take the rest as the label |
| `1. Save button \| 120, 340` | numbered lists are bullets |
| `- Save button \| 0.12, 0.34` | fractions of one and percentages are scaled onto the grid |
| `- **Save button**: 120, 340` | markdown and trailing punctuation come off the label |

The name lookup had the mirror problem. It took the first containment hit in list order, so a table
holding "Terminal tab" and "Problems tab" resolved the name "tab" to Terminal every time — by position,
not by meaning. Every row is scored now, and a name that fits two of them equally well fails the margin
and points at nothing.

### Three pixels of arrow

Three comments across three files said the arrow's tip sat at its element origin. The apex of the path is
at (3.38, 0.29), which is why `transform-origin` was `3px 2px`. The stylesheet now pulls the svg back by
exactly that, and the comments say where the number comes from so the next person who moves the path
moves the offset with it.

### The screen the user is actually looking at

Capture and the pointer window both called `getPrimaryDisplay()`. On a two-monitor desk that is a coin
flip: the app read one screen, answered about it, and pointed at it, while the user was working on the
other. Both follow the mouse pointer now, and the shot carries its display with it, so the ratio mapping
is already right once the correct one is picked.

### Pointing, as a contract

| Who is answering | Directive | Resolved by |
| --- | --- | --- |
| A model looking at the screen | `[[point x=.. y=.. \| label]]` | The 0-1000 grid, mapped to the display by ratio, which is DPI-proof |
| A text-only brain | `[[point Save button]]` | Every row of the reader's ELEMENTS table scored — exact, then containment weighted by length, then word overlap balanced against both. The best one wins, and only if it beats the runner-up |

Dropping is the important half. A pointer in the wrong place is worse than no pointer, because it is
confidently wrong in a way the user cannot check. That now covers ambiguity as well as absence: a name
that two rows answer equally well is a name with no answer.

A third outcome used to exist and should not have. A directive the parser could not read was printed to
the user as raw `[[point ...]]` brackets, which loses the pointer and shows the machinery. Both shapes
are read loosely now, and anything that opens like a directive and still cannot be read is swallowed.

## Useful, not just finished

The craft was ahead of the usefulness. Everything below is the difference
between an app that demos well and one that stays open.

### An application you could lose

Every window sets `skipTaskbar` and the Windows `toolbar` type. No taskbar
button, nothing in Alt-Tab: correct for an overlay, and wrong for the only copy
of a program. Hide it with its shortcut, forget the shortcut, and there was no
surface left to click. Not a window, not a menu, not the quit handler, which
was reachable only from a settings panel reachable only from the overlay. Task
Manager was the exit.

A tray icon closes it: ask, ask about an area, show or hide, new chat, history,
settings, quit. Its icon is the pointer's own arrow rasterised from the same
polygon and carried as base64, so the repository stays text-only and the app
keeps its no-build-step promise.

### Three controls that were missing or lying

| What the app said | What actually happened |
| --- | --- |
| `query:cancel` allowlisted in the preload and handled in main | No renderer ever called it. There was no Stop, on an app whose default models are free, queued, and routinely half a minute from a first token |
| Five error messages ending "Press Enter to retry" | The input clears on send and an empty box sends the default question, so pressing Enter asked "what's on my screen" instead of the thing that failed |
| A 429 shown as an error to resolve by hand | Free tiers rate limit by design. Every one of those was a dead end where the app knew a working alternative and did not use it |

Stop now sits in the message's own meta row rather than in the thinking dots,
so it survives the first token and can still stop a stream. Retry re-sends the
question that failed, and the error strings stopped promising a keystroke that
did something else. A 429, a 404 on a retired model, or a 5xx moves to the next
free model of the same kind and names it in the status line; a 401, a 402, or a
malformed request does not, because no other model would answer those
differently. Never for a paid model, never after text has reached the screen,
never more than three attempts.

### Spending the pixel budget where the question is

Capture fidelity is already recorded above as the root of bad answers, and the
fix so far was 1280 to 1600 on the long edge. That could not finish the job,
because the budget is spent on the whole desktop when the question is about one
dialog:

| What is sent | Pixels per display point | 12px UI text arrives as |
| --- | --- | --- |
| A 2560x1440 desktop, long edge 1600 | 0.62x | about 7px, guessed at |
| A 600x300 area of it | 1.0x, untouched | 12px, read |

So: `Ctrl+Shift+A`, drag a box. The whole screen is still asked of
`desktopCapturer` at the budget directly, because decoding a 4K desktop at full
size in order to shrink it is wasted work; an area is grabbed at native
resolution and cropped, and only shrunk if the selection itself is larger than
the budget. It is also a fraction of the tokens, so it is faster and cheaper.

Three things follow from a selection being a conversation-level state rather
than a per-message one. It persists across follow-ups, so "and the line above
it?" looks at the same box. The panel carries a chip for as long as it is in
force, because an answer built from a corner of the screen with nothing saying
so is indistinguishable from a bad answer. And both prompts are told they are
looking at an area, so the reader stops writing `SCREEN: Visual Studio Code`
for a crop of one error message.

The pointer follows it too. A grid coordinate is a fraction of the view, and
the view is now the crop, so `shot.region` replaces `shot.display` in the
mapping. Getting that wrong would have reintroduced the drift the grid fix
removed, scaled by however small the selection was.

### Amnesia was a defensible default and still a cost

Nothing survived quitting. That is a reasonable stance for something that sees
whatever happens to be in front of you, and it meant the answer that fixed your
build yesterday was gone and you asked for it again.

Text only, never a screenshot and never a reading of one, on by default with a
switch and a delete-all beside it. What is written is exactly what was already
readable in the transcript. One JSON file rewritten whole through a temp file
and a rename, debounced so a ten-turn conversation does not write ten times,
and saved after every answer rather than at the end so a crash loses nothing.
Search reaches the message bodies, not only the titles, which is the difference
between a list and something you can find an answer in.

### A scrollbar has to be visible to count as one

The transcript scrolled the whole time. The thumb was `#303D6A` against a panel that runs `#3E404A` to
`#262930`, which is roughly the same value, so it read as a surface with no way down. That is the same
thing as not scrolling. The thumb is now pale slate at 26%, the bottom edge fades while there is more
below, and the transcript's own ceiling is sent from the main process so it agrees with the window's
rather than clipping under it.

### Windows: off the taskbar, above everything

`skipTaskbar` asks the shell to drop the button. `WS_EX_TOOLWINDOW`, which `type: 'toolbar'` sets, takes
the window out of the window list altogether, so it leaves Alt-Tab too. Settings was the one window that
had neither, plus no always-on-top, which is why the app still had a labelled taskbar entry and why that
window hid behind things. Every window now carries the same three: tool window, skip taskbar, and
always-on-top at `screen-saver` with a relative level of 1, which puts it above other topmost windows
rather than level with them.

Content protection is guarded on the Windows build. Below 19041 the call does nothing, and settings says
so rather than letting the user believe the overlay is hidden when it is not.

## v1: three things that were true, and unverified

Each of these survived every previous round for the same reason: nothing in the
project was watching for it. Two of the three were even being measured.

### The default model had been retired out from under the app

`npm run shots` photographed the model picker every time it ran, and the
picker labelled the app's own shipped default "Custom model id". The smoke run
printed `modelDefaultOffered=false` on every launch. Both were correct, and
both went past unread, because a photograph nobody opens and a value in a
column of ninety are the same as no check at all.

`nvidia/nemotron-nano-12b-v2-vl:free` is not in OpenRouter's catalogue any
more — not retired to paid, not renamed, absent from all 418 entries. So every
fresh install asked its first question of a model that does not exist, took a
404, and failed over. The app recovered, which is exactly why nobody noticed:
the failover built to hide this class of problem hid it.

The replacement is chosen for the same reason the original was.
`nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free` describes itself as a
perception sub-agent for a larger agent system, which is the job this app
gives it: look at the screen, report what is there, and let the brain act on
the report. 3B of its 30B parameters are active per token, so it does not pay
for that in latency.

The fix that matters is not the id. A suite that only reports is a suite that
watches things break, so the run now separates observations from invariants:
`must()` fails the run and takes the exit code with it. Four to begin with —
the windows exist, the tray came up, and both defaults are offered by the live
catalogue. The last one is the check that was already there, wired to a
consequence.

### It happened again, so installs now heal themselves

In Nov 2026 `nvidia/nemotron-3-super-120b-a12b:free`, the default brain, left
the free tier (the paid id is still listed), and `thinkingmachines/inkling:free`
went with it. The smoke assert caught the default this time, which is the
assert doing its job and stays. What it cannot do is reach a copy that is
already installed: a user who saved a retired id kept 404ing into failover on
every question, and only a release could change that.

So the ids in use are now resolved against the live catalogue (cached five
minutes, fetched alongside the screen capture) before each question, and once
at startup. An id that is gone is replaced by the shipped default of its kind,
or the first curated free model still live. A saved choice is rewritten so
settings shows what is answering; the shipping default is swapped in memory
only, so the next release's default still reaches that install. The
replacement is always free, the same rule as failover: a retired paid choice
is not moved onto a different bill. Offline, nothing is known to be gone, so
the id is kept and failover handles it. The smoke run checks the healing
against the real catalogue with two ids known to be retired (`healRetired`).

The new brain is `nvidia/nemotron-3.5-super-vl:free`, the direct successor at
the same 120B total and 12B active, so the wait stays where it was. It lists
tool calling, which the Desktop Agent mode needs. It also reads screenshots,
so it joins the vision list as the careful, slower option.

### An application wearing someone else's logo

The `.exe`, the installer, the Start Menu shortcut and the Add/Remove Programs
entry all showed Electron's default icon, and `electron-builder` said so on
every single build: `default Electron icon is used`. It is invisible to the
visual self-check because it is not a surface the app draws.

It is generated rather than committed, for the same reason the tray icon is
carried as base64: the repository stays text-only and there is no build step
to install. `tools/make-icon.js` reads the pointer's path out of
`renderer/buddy.html` and its colours out of `renderer/cursor-buddy.css`,
flattens the quadratics, rasterises with a supersampled distance test and
writes seven sizes into an ICO. No dependencies, because the app has none.

So the icon *is* the pointer, not a picture of it. Move the path and the icon
moves too; there is no second drawing to keep in sync, which is the failure
mode every hand-exported icon eventually has.

### A process that could end without leaving a trace

Every window sets `skipTaskbar` and the Windows tool-window style. That is
right for an overlay, and it means an unhandled error had no way to announce
itself: no taskbar button to go dark, no window left on screen holding the
message. The app was simply not there any more, and the shortcut did nothing.
That is the same failure the tray icon exists to fix, arriving by another
route.

Unhandled exceptions and rejections are now written to `errors.log` beside the
settings, announced once as a notification, and survived rather than fatal.
Node is right that a process is in an undefined state after an uncaught
exception; for this app the alternative is not a clean restart, it is
vanishing. A renderer that dies is reloaded, because an always-on-top blank
rectangle is worse than no window.

And it is tested by throwing one: the smoke run emits a deliberate unhandled
rejection and asserts it reached the log. A safety net nobody has fallen into
is not known to be a net.

## Updates: an installed copy that keeps itself current

The updater existed and had never been watched doing its job. Nothing
installed an old version and saw it become the new one, its log went to a
console a packaged app does not have, and a download that dropped half way
rejected a promise nobody held, which crash.js correctly reported as
"AevalBuddy hit an error". For a dropped Wi-Fi connection.

### When it looks

20 seconds after launch, every four hours, 15 seconds after the machine wakes
(if the last good check is over half an hour old), and the moment the network
returns after a check found it gone. Never while an answer is streaming: a due
check waits a minute and asks again.

### What failure looks like

Nothing alarming, because none of it is the user's problem. Offline, a 429
from GitHub, a 5xx, a refused connection, a download whose sha512 did not
match: each is retried on 2, 5, 15, 30, 60, then 120 minutes (with 10% jitter)
and then the usual four hours. Settings says one plain sentence ("You're
offline. AevalBuddy checks again once you're connected." / "The update server
did not answer. Trying again at 3:05 PM."). The tray says nothing. The raw
error goes to `updater.log` beside `errors.log`. This follows the rule the
free models taught the app: a 429 is the everyday state, not an exception.

### What success looks like

| Surface | Downloading | Ready |
| --- | --- | --- |
| Tray menu, top | disabled `Downloading 1.2.0… 42%` | `Restart to update to 1.2.0` |
| Tray tooltip | unchanged | `AevalBuddy — 1.2.0 is ready, restart to update` |
| Settings > About | summary, a sentence with MB and %, a 4px progress bar, "New in 1.2.0:" and the first lines of the release notes | the same plus a primary `Restart now` |
| Settings footer | nothing | `Restart to update` link |
| Notification | none | one, silent, held back until no answer is streaming |

Restart while an answer streams does not cut it off: the tray reads
`Restarting after this answer…` and the restart happens when it finishes. If
the user never restarts, the installer runs when the app quits. The first
launch after an update says `Updated to 1.2.0` once, as a silent notification
that opens the release page, and in About for that session.

About follows the settings pattern: one line (name, `1.1.1 · up to date`,
Details), the body only while open. The progress bar is slate on a hairline
track, not the accent: the accent is for the action, and a bar filling is not
one. It moves by `transform: scaleX`, never width.

### Release notes are UI now

The first list items of a release's notes are what an installed copy shows
under "New in 1.2.0", so `release.yml` writes "What's new" from the commit
subjects since the previous tag first and the install instructions second.

### Departures, with reasons

| Departure | Reason |
| --- | --- |
| A second notification type besides the crash notice | the tray is often behind the overflow chevron; a downloaded update nobody hears of is applied only at a quit that may be weeks away. One per version, silent, never mid-answer |
| `app.setAppUserModelId('com.aevalbuddy.app')` at startup | Windows attributes a toast to the Start Menu shortcut's AUMID; without the same id, the notification can show under a generic name or its click never reaches the app |

### How it is proven

`.github/workflows/update-check.yml` installs the release the README links to,
rewrites only its `app-update.yml` to point at a local feed laid out like
GitHub's, and waits for that untouched old version to download build A of
the commit and leave it, sha512-matched, in the updater's `pending` folder.
Build A then updates to build B through `AEVAL_UPDATE_FEED`, quits by itself
(`AEVAL_UPDATE_QUIT=1`), and B must be the registered version afterwards. The
smoke run asserts the shipped feed points at the public repository and that a
check against it settles; against a refused port it must read as a retry.
Both variables do nothing unless set.

## v2: Find, Agent, and the command bar

The pill opens one bar with three verbs over one conversation: Ask about the
screen, Find on it, or have the Agent do something. All three share one
perception engine (`lib/desktop`) that reads an app's own UI Automation text
before it ever looks at pixels. SPEC-v2.md is the plan; this section is what
was built and the values it uses.

### New tokens

| Token | Value | Where | Why |
| --- | --- | --- | --- |
| `--hit-fill` | `rgba(197, 111, 195, 0.22)` | literal Find hits | the lens sits over arbitrary content like the area selector, so it uses `--accent-bright`; at 22% black text under it stays readable on white and it still shows on a dark editor |
| `--hit-ring` | `--accent-bright`, 1.5px | related and concept hits | outline versus fill is the whole literal/related distinction, so no second hue |
| `--hit-current` | `--accent-bright`, 2px, plus `--hit-rim` | the current hit's ring | the one mark that says "you are here" |
| `--hit-rim` | `rgba(0, 0, 0, 0.55)` | outside every lens mark | legibility on pale content, the job `--danger-edge` does for the warning |
| `--control-frame` | `#823A80`, 2px solid | display edge while the agent runs | the pointer's colour already means "the app is acting here"; solid, never glowing or pulsing |
| `--step-done` | slate at 0.70 | finished trail rows, reads | done steps recede; the running row is full slate |

`--danger` and `--danger-edge` are reused for the pill's failed-run dot and
the failed trail row's mark. No new hues.

### New surfaces

| Surface | Window | Rules |
| --- | --- | --- |
| Mode label | overlay | a word where the text starts (`Ask`, `Find`, `Agent`), slate 0.70, 13px, no box; clicking it cycles |
| Bar status | overlay | Find's count (`3 exact · 5 related`) and stage lines, or Agent's hint; hairline above, no fill, takes no space when empty |
| Find results | answer window, Find view | snippet 14px with the match in semibold, why 12px slate 0.70, a filled dot for literal and a ring for related, hairlines between rows, the current row tinted grape soda at 22% |
| Lens | `lib/lens.js`, one per display with something to draw | click-through, never focusable, content-protected; hits, the ring, the why chip in the buddy bubble's material, the control frame and its chip |
| Agent trail | answer window, inside the transcript | one block per run: plan, one row per step (verb + target), state on the right, rows rise like messages; the running row carries the thinking dots |
| Confirm card | inside the trail | radius 8, twilight fill, the action as a sentence, the model's reason and the app's reason, Allow primary with a `Ctrl+Enter` keycap, Skip as a link with `Ctrl+Backspace` |
| Consent sheet | inside the transcript, first run only | same card; what the agent can do, what it asks about, how to stop |
| Settings rail | settings at 560px and wider | 160px rail, current page full slate with a 2px grape-soda bar; below 560 the rail goes and the column scrolls |
| Tour card | pointer window | the last tour step has no arrow: a card with `Try it` (primary) and `Later` |

### Motion additions

| Transition | Value |
| --- | --- |
| Hit appears | opacity, `--d-fast`, staggered 12ms per hit, capped at 300ms |
| Current-hit ring moves | translate and size on `--ease-glide`, `--d-med` |
| Agent cursor before an action | the arrow glides on the glide curve for 240ms; the action fires on arrival |
| Mode switch | label and placeholder cross-fade over `--d-fast`; the bar never resizes for it |
| Control frame | opacity in over `--d-fast`; never animates while shown |

Reduced motion: hits appear at once, the ring and the cursor jump, the frame
still appears because it is information.

### Rules

- Perception order is UIA, then OCR, then vision. Sending pixels when the app
  offered the string is a capture-fidelity regression.
- During an agent run no AevalBuddy window takes focus except a confirm card,
  a question, or the consent sheet, and focus goes back to the target before
  the next action. The smoke run counts violations (`agentFocusKept`).
- The safety gate is code in the executor, never a prompt: a control or
  dialog named with a risky verb, Enter in a compose field, a password field,
  any `open`, or the model's own flag asks first. Own windows and elevated
  windows are refused.
- The lens and the pointer are ours but never block a click: the desktop
  engine is told they are `block: false`.
- Every window, including ones a module creates later, gets content
  protection when it is created (`browser-window-created`), not when someone
  remembers to.

### Departures from SPEC-v2, with reasons

| Departure | Reason |
| --- | --- |
| Settings names the vision model "Screen model" and the optional second one "Answer model", not "Answer model" and "Screen reader" | the spec's names had the roles backwards: the second model is the one that answers from the reading, the first is the one that reads |
| Tab with text in the bar, in Find, switches This window / Whole screen; Tab on an empty bar cycles modes | the spec gives Tab both jobs; text in the bar is the only signal that tells them apart without a second key |
| The ring animates width and height as well as transform | a scaled 2px border stops being 2px; it is one element on a static layer, so the layout cost is nothing |
| UIA line hits are sliced from the line's rectangle in proportion to characters | UIA gives one rectangle per line, not per character; exact for monospace, close for proportional text, and OCR word hits are exact |
| The tray's download progress moved from a disabled top line into the `Check for updates` item | SPEC 4.4 puts every update state on that item; a ready update still gets the top line. A failed check reads `Update failed, try again` only after the person clicked the item; background failures stay silent, as the Updates section says |
| `tools/fixtures/llm` ships inside the app | the smoke run tests the installed exe, and the fake model needs its fixtures there; a few KB, inert unless `AEVAL_FAKE_LLM` is set |
| Boot-time check is a ceiling, not "within 10% of v1.1.1" | no v1.1.1 boot number was ever recorded; the run now logs `bootMs` and asserts the helper is not started at boot, which is the only v2 change that could move it |

### One name per action

The first look at the installed exe's photographs found the same action
named three ways across surfaces: the agent was `Do a task…` in the tray,
`Agent` in the shortcut list, `Doing tasks` in settings; the visibility
toggle was `Hide the overlay`, `Hide or show`, and `Show or hide overlay`;
history's wipe was `Delete all` in settings and `Clear all` in its own
window. The tray's labels are SPEC 4.4's and the smoke run asserts them, so
everything else follows the tray.

| Action | Name everywhere | Tray adds |
| --- | --- | --- |
| captureQuery | Ask about my screen | |
| captureRegion | Ask about an area | `…` (opens a selector) |
| findOnScreen | Find on screen | `…` |
| agentTask | Do a task | `…` |
| toggleVisibility | Hide or show the overlay | the current verb only: `Hide the overlay` / `Show the overlay` |
| history wipe | Delete all | |
| update check | Check for updates | |

Mode names in the bar stay nouns (`Ask`, `Find`, `Agent`), because they name
where you are, not what a click does.

Settings rows are nouns too. The Agent page's row is `Permissions` (when the
agent asks, and how many steps it may take); it was still `Doing tasks` after
the first pass. The answer window's dock control is `Dock` / `Detach` and
its badge `Docked`: the layout card already says what it docks to, and
`Docked to input` named the input bar a third way.

Selected text uses grape soda at 60% with pale slate text. Without a
`::selection` rule Chromium paints the Windows highlight blue, which was the
brightest colour in the bar whenever a prefilled task was selected.

### Held to the scales, after the first look

- Secondary text was at 0.36 to 0.62 alpha in twenty places (history's
  meta line, settings summaries, Edit links, switch labels, the callout
  hint) against the 0.70 floor above. All of it is at 0.70 now. Icon chrome
  (0.55 idle) and input placeholders are not text you have to read and stay
  where they were.
- 11px type in four places (keycaps, the model list's group label, the
  capture warning, the area selector's key hint) moved to the 12px floor.
  The group label also lost its uppercase tracking and its 0.45 alpha.
- Radius 5 and 10 became 6 and 8. Padding 7 and 10 became 8 and 12.
- Stop and Retry are the same control in two moments, so they share one
  look: the filled twilight chip, 12px semibold. Stop used to be an
  outlined box, the only bordered button inside a window.
- The capture warning was drawn in the accent; it is a warning, so it uses
  `--danger`.
- `Show paid models` filters every picker on the Models page, so it sits on
  that page's title line instead of between the title and the first row.

## v2.1: one input that works out what is meant

The owner asked for one text input that figures out what the person wants
instead of three modes and a toggle. The plan behind this section is
`.claude/appforge-loop/PLAN-v2.1.md`; this records what it changed here.

### The token

The word at the start of the bar is now what Enter will do with the text
(`Ask`, `Find`, `Do`), guessed live by `renderer/intent.js` from the words
and from whether they occur in the window's own text. Guessed, it is slate
at 0.72 with no box; chosen, it gains a 10% slate fill. A low-confidence
guess dims its word slightly. Phase B dresses it in glass and keeps its
hooks: `.intent-token`, `data-intent`, `data-locked`, `data-confidence`.

The guess is never trusted with the computer. A task that was only guessed
shows a card under the bar; Enter answers it as a question, and running it
takes a click or `Alt+Enter`. Ties go to Ask.

### Departures from DESIGN v2, with reasons

| Departure | Reason |
| --- | --- |
| Mode names in the bar are verbs (`Ask`, `Find`, `Do`), not nouns, and `Agent` is `Do` in the bar and `Tasks` in Settings | the word now says what Enter does, not where you are; "Agent" named the mechanism, not the action |
| Tab never switches modes or Find's scope | Tab belongs to focus order; taking it trapped keyboard and screen-reader users in the input. Ctrl+1/2/3, the prefixes and Alt+Down replace it |
| One global chord, `Ctrl+Alt+Space`; the other four start unbound | the five defaults collided with VS Code (`Ctrl+Shift+A`, `G`, `U`) and Chrome (`Ctrl+Shift+A`, `H`). Stored chords equal to an old default migrate; hand-set ones stay |
| `--ease-settle: cubic-bezier(.34, 1.3, .64, 1)`, a 5% overshoot, for the token and the guess card | approved by the owner in v2.1. "Nothing bounces" still holds for windows and drags |
| Escape is global only while a task runs, the tour or area selector covers the screen, or the layout editor is open; Enter is never global | a global key is taken from every app. The bar, the answer and Settings handle Escape in their own windows; a paused or planned task continues from the focused Start button |
| The bar never reopens in the last mode | a remembered Find or Do turned the next question into a search or a task |
| The tray's first item is `Ask, find, or do…`, and `Help and feedback…` sits under Settings | the first item opens the one input; the smoke run's `trayOrder` follows |
| Ask sends text alone when the window gave 800+ characters and the question is not about how something looks | perception order is UIA first; the screenshot was being sent even when the text said everything |

### Safety rules added

- The gate reads every chord in a key sequence; every Enter (under any
  name), Delete, F-key, Win or Alt chord asks, as does any newline or tab in
  typed text, typing in a terminal, the Run box or an address bar, any drag,
  a click in a mail or chat app, and a dialog whose text names a risky verb.
  An exception inside the gate asks; it never allows.
- Text handed to a task (Do it, Retry from here, Find hits) is wrapped as
  untrusted data, and the task waits for its plan to be approved.
- A confirm card takes focus itself, offers Skip first, has no key chord for
  Allow, and ignores input for 700 ms. An open-link card shows the full URL
  with its host in bold.

## Tauri rewrite: the requests that changed the surfaces

The owner's list for the rewrite is `.claude/rust-rewrite/REQUESTS.md`. Items 3 to 11 change what the
surfaces do and look like; this section is what was built for them and why, and the departures from the
sections above. The Rust half of each is `.claude/rust-rewrite/ARCHITECTURE.md` section 10.

### One system

Request 7 asked for the whole app to look consistent and intentional. The tokens already existed in this
file; the surfaces had drifted from them (30 px and 28 px buttons side by side, 18 and 20 px radii, 7/14 and
10/14 paddings). They are now named in `glass.css` and every touched rule uses them:

| Kind | Tokens |
| --- | --- |
| Spacing | `--sp-1..6` = 4, 8, 12, 16, 24, 32 |
| Type | `--fs-meta` 12, `--fs-small` 13, `--fs-row` 14, `--fs-body` 15, `--fs-title` 17, `--fs-bar` 18, `--fs-page` 22 |
| Radius | 28 sheet and bar, 16 card, 10 row, 6 key, capsule (unchanged) |
| Controls | `--ctl` 32 for every button in a sheet (Copy, Stop, Retry, Allow, Do it), `--ctl-s` 28 for title-bar icons and the step toggle |
| Heights | `--bar-h` 56, `--sheet-bar-h` 44 |
| Motion | unchanged: glide, morph, settle and the four durations |

Rules: one button shape in a sheet, 32 px capsule, 13 px semibold (the meta row's 28 px / 12 px buttons
were a second shape for the same job). Cards and the rows under the bar take `--r-card`; the question chip
too (it was 18). Code blocks take `--r-row`, inline code `--r-key`.

### Output windows (requests 7, 9, 11)

- **Free-floating, several at once.** Each answer, Find result or task trail is its own window
  (`output-<n>`). There is no docking and no answer ghost in the layout editor; the editor places the bar
  only. The sheet fills its window inside the 20 px shadow margin, so a window resized or maximised by the
  person is the sheet.
- **A title bar on every sheet.** 44 px, held to move the window: a kind chip (`Answer`, `Find`, `Task`),
  the conversation's name (its first question, or the task, or the search), and four 28 px icon controls:
  Copy conversation, History, New conversation, Close. A hairline appears under it only once the transcript
  scrolls beneath. This replaces the v0.4 `#chat-tools` row. v0.4's rule "no product name on a surface the
  product owns" holds: the title is the conversation, never "AevalBuddy".
- **No input in an output window.** The bar is the only text field. A task's question (`ask_user`) shows in
  its trail as "Waiting for your answer in the bar", in the accent; the bar's placeholder becomes "Answer the
  task's question" and its rim turns accent, and Enter sends the answer to that run. Plan approval and
  Continue stay as buttons in the trail, and an empty Enter in the bar does the same.
- **Reading measure.** Past 760 px the transcript centres instead of stretching, so a maximised output reads
  as a document.

Start over keeps the window and offers Undo for 10 s (`chat:reset {undoMs}`, `output:undo-reset`). Undo puts the old conversation back only if nothing new was asked since. The picked area is dropped with the old conversation.
Reason: Start over is one click away from Send and loses the whole transcript; a short Undo costs nothing and keeps the window where the person put it.

At most six output windows are open. A seventh closes the least recently touched window with no answer streaming into it, and the bar says `Closed "<title>". It is in History.`, or `Closed "<title>".` when history saving is off.
Reason: each window holds a webview and a conversation; past six they cover the screen they are about. Saying which one went, and where to find it, keeps the close from looking like a lost answer.

### The glob (request 8)

An output window opens by stretching out of the bar: a bud of glass leaves the bar's edge, grows into the
sheet on `--ease-morph`, joined to the bar by a neck that thins, pinches off at 60% and leaves two beads that
are absorbed by 84%; the words fade in from 55%. 440 ms in all. Closing runs it backwards toward the bar on
`--ease-glide` in 260 ms, then the window closes; with the bar collapsed it fades in 150 ms.

How: the bar and its outputs are separate windows, but Rust places an output's glass 12 px from the bar's,
which is inside both 20 px margins, so the output window can draw over the bar's edge. The shape is one SVG
path computed per frame (`renderer/glob.js`); it clips a real `.glass thick` layer, so the glob is the same
material as the sheet (backdrop, scrim), and a masked stroke draws its rim. Refraction is dropped while it
runs, as for every morph. Measured on the runner: well under a millisecond per frame for the path, no frame
over 34 ms (`[UI] globSmooth`). Reduced motion: a 150 ms fade, no shape.

| Departure | Reason |
| --- | --- |
| The sheet no longer grows by a clip-path reveal from the bar's edge (Motion, above) | request 8 asked for the liquid glob; the reveal was a rectangle uncovering itself, the glob is one piece of glass becoming another |
| The glob is a window-level entrance yet uses `--ease-morph`, not `--ease-glide` | it is a shape change first; the window itself does not move (Rust shows it at its final bounds) |
| No overshoot on the glob | windows never bounce (Superseded, above); the liquid reads from the neck and the beads |

### Liquid glass you set (request 5)

Glass no longer follows Windows' "Transparency effects" setting. Chromium reports that setting as
`prefers-reduced-transparency`, which used to force the solid tier; Rust also reported it as
`transparency-off`. Both are now ignored, and Settings, General, **Liquid glass** is the control: a slider
from None to Very very (stops None, Subtle, Liquid, Very, Very very; 0 to 1, default 0.5, which is exactly the
v2.1 material). It moves every window live.

| k | 0 None | 0.5 Liquid (default) | 1 Very very |
| --- | --- | --- | --- |
| Body | solid frosted (`--solid`), no backdrop | backdrop | backdrop |
| Blur thin / thick | none | 14 / 28 px | 8 / 20 px |
| Saturation thin / thick | n/a | 1.7 / 1.5 | 2.4 / 2.0 |
| Refraction band / scale thin / thick | none | 14 px / 22 / 28 | 22 px / 44 / 56 |
| Specular rim | 0.55 | full | full, 2 px, plus a top-left sheen |
| Scrim | n/a | measured as before | lighter over dark backdrops only |

The scrim rule is what keeps contrast: the adjustment is `(0.5 - k) x 0.24 x (1 - y)` thin and `x 0.16` thick,
which is zero over a white page (y = 1), where contrast is hardest, so the 4.5:1 floor holds at every
setting; only a dark desktop shows through more. None is the solid tier, so its contrast never depends on
the desktop at all.

| Departure | Reason |
| --- | --- |
| Windows transparency off no longer means solid (Material, Solid tier, above) | request 5: the owner wants glass whatever that setting says, and a control of his own |
| `prefers-reduced-transparency` no longer forces solid | it is the same Windows setting as seen by Chromium; None on the slider is the way to ask for a solid surface |
| The "Use a solid surface" switch is gone; the slider's None replaces it | two controls for one thing; a profile with Solid set reads as None |
| A contrast theme (forced colours), a build that cannot hide from capture, and repeated capture failures still force solid | those are about legibility and privacy, not taste |

### Suggestions, once (request 3)

The example chips under an empty bar are for someone who has not used it. They show until the first request
is sent (a question, a Find with Enter, or a task) and never again, on any later open. Kept in the settings
file as `hasSentFirstRequest` and in localStorage so they vanish at once.

### One place for the Ask / Find / Do keys (request 6)

The keys are taught by the token at the bar's left, and only there: its tooltip (`Ask ? or Ctrl+1 · Find / or
Ctrl+2 · Do > or Ctrl+3 · Alt+Down for this menu`) and the menu it opens, whose rows now carry both the sigil
and the Ctrl chord. Removed: the `/ Find  > Do` legend beside the empty input, the "Choose Ask, Find or Do in
the bar" row in Settings' shortcut list, and the "Type to find on screen." line in an empty Find view (it
now says "Looking…" until Find reports).

### Windows you can hold (requests 4, 10)

- **Edges and corners resize** on every resizable window: zones 10 px wide across the glass edge (7 px in
  the shadow margin, 3 px on the rim), 18 px corners, created by `renderer/frame.js` and started through
  Tauri's `startResizeDragging`. They carry `data-hit`, so the margin's click-through never swallows a
  grab. Output windows, Settings and History get all eight; the bar gets left and right (its height
  follows its content).
- **Drag** by the title bar on output windows, Settings and History, and anywhere on the layout editor's
  ghost. Double-clicking the Settings or History title maximises it.
- **Desktop, full screen.** Settings and History hold up maximised: past 900 px the rail widens to 220 px
  and pages keep a 720 px measure, centred; History's list keeps 760 px. Output windows keep a 760 px
  reading measure.

### The closing bar (request 7)

The bar folds back into the orb as deliberately as it grows: the words blur out over 90 ms, the rows under
it slide back into it, and the shape closes on `--ease-glide` in 200 ms (it was a 90 ms opacity cut on the
morph curve). 200 ms fits inside the window's own wait before it shrinks to the orb (T_COLLAPSE + 30).

### Implementation notes (not departures)

- Frames reach `glass.js` and `lens.js` as an `aeval-frame://` URL instead of JPEG bytes (ARCHITECTURE.md
  D4); both still accept bytes.
- `-webkit-app-region` is gone from the stylesheets; `data-drag` and `frame.js` do its job under WebView2.
- The bridge takes back WebView2 behaviour Electron never had: its context menu, reload, find bar, print,
  zoom and back keys, and navigation by a dropped file.

## Rust rewrite: decisions and notes from the lanes

Folded from the lanes' entries files at the final merges: entries for a section this document did not have.

### Decisions

D-03: The NSIS installer is one click like Electron's oneClick build. `src-tauri/installer.nsi` is a copy of tauri-bundler 2.10.4's template (tauri-cli 2.12.4) with the welcome, license, folder and finish pages removed; a run without `/S` or `/P` takes Tauri's passive path (progress bar, per-user folder) and starts the app when it finishes. Reason: an existing Electron user re-running the download expects no wizard, and the passive path is code Tauri already ships and tests. Re-copy the template whenever TAURI_CLI moves.

### Keyboard

Esc never stops an answer or hides an output window. Esc in the bar clears the field, then folds the bar; Esc in an output gives the foreground back to the app that had it before the bar opened (or the next app window below). Folding the bar also returns the foreground. While a task waits on the person, the first Esc in a bar holding typed text clears it and says "Cleared. Press Esc again to stop the task."; the next Esc stops the task.
Reason: output windows float on their own since REQUESTS #11 and free models can queue for 20 to 30 s, so an Esc that aborts throws away a wait the person already paid for. The blur half of Electron's collapse is kept so typing continues where it was.

### Accessibility

Text follows the Windows text size (Settings, Accessibility, Text size) times the person's own 100 to 200% setting, at most 250%. Pages scale their type by `--text-scale` from `ui:text-scale {scale}`.
Reason: the glass surfaces used fixed pixel sizes and ignored the system setting people with low vision rely on.
