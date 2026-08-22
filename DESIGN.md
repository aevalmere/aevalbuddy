# AevalBuddy design constraints

Written before the v0.2 surfaces were built. Every visual change builds to this file; anything outside it
is a defect.

## What this app is remembered for

The assistant feels like part of Windows, not a website floating over it: you talk to it like a messaging
app, and it can physically point at your screen when an answer needs a "click here".

## The one pushed dimension

Motion. Color, type, and layout stay quiet and systematic; the personality lives in how surfaces arrive,
grow, and glide. Nothing bounces, nothing loops decoratively, everything decelerates.

### Motion tokens

| Token | Value | Used for |
| --- | --- | --- |
| `--ease-glide` | `cubic-bezier(0.32, 0.72, 0, 1)` | window-level entrances: panel, answer window (the macOS sheet curve) |
| `--ease-out` | `cubic-bezier(0.16, 1, 0.3, 1)` | in-surface micro motion: message rise, badges |
| `--d-fast` | 150ms | hover, press, opacity |
| `--d-med` | 240ms | message entrances |
| `--d-slow` | 380ms | panel and answer-window entrances |

Rules: animate transform and opacity only; entrances scale from 0.9 to 0.95, never from zero; exits run at
roughly half the entrance duration; every animation dies under `prefers-reduced-motion` except opacity.
The query panel grows out of the pill's corner: its transform-origin is the anchored corner, so expansion
reads as the pill becoming the panel.

## Palette (fixed, six values, nothing else)

`#000000` black, `#10254F` space indigo, `#2B2C33` shadow grey, `#823A80` grape soda, `#C9CFDD` pale
slate, `#303D6A` twilight indigo.

Distribution: shadow grey and black carry the surfaces (~60%), indigo and twilight carry fields, bubbles,
and cards (~30%), grape soda is reserved for the primary action, focus, and the pointer ring (~10%).
Secondary text is slate at 0.70 alpha minimum (measured 4.5:1 or better on every surface it sits on);
never lower for text under 14px.

## Type

Two families: Segoe UI (system) for UI, Consolas for code. Scale: 15 input text, 14 body, 13 secondary,
12 metadata (floor). Markdown headings inside answers: 16/15/14 semibold, at least a step apart.

## Spacing and radius

Spacing scale: 4, 8, 12, 16, 24, 32. No in-between values.
Radius scale: 4 keycaps and chips, 6 controls and inputs, 8 cards and bubbles, 12 floating windows.

Borders: default borderless; separate with whitespace, then a background shift. Exception with a reason:
the floating overlay windows keep a 1px twilight border, because they sit over arbitrary desktop content
and a shadow alone disappears on dark backgrounds (3:1 boundary requirement).

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
