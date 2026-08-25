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
