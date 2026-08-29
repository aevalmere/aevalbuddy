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

### The values that are not in it

"Nothing else" held until the app grew a surface with no surface of its own. Every window here supplies
its own background, so the palette is always measured against a known dark grey. The area selector is
not a window with a background; it is a sheet of glass over whatever the user happens to be pointing at,
which is as often a white document as a black editor.

| Token | Value | Where | Why the palette value does not work |
| --- | --- | --- | --- |
| `--accent-bright` | `#C56FC3` | the selection's corner ticks | grape soda is a dark violet by design, which is right on the app's own dark surfaces and a smudge on a black editor. The ticks are the one mark saying which pixels get sent |
| `--danger` | `#E4818C` | the too-small warning, Delete all | the accent means "this is the action". An error that borrows it says the opposite of what it means |
| `--danger-edge` | `#6B2E38` | that warning's border | the rim that keeps `--danger` legible against a pale background |
| `--glass-deep` | `#191A1F` | the bottom of a floating surface, the top of a document window | the deep end of the painted glass. Shadow grey is the top of the ramp, which is what the palette always said carried the surfaces; this is where it goes. Defined in `renderer/overlay.css` |
| `--glass-floor` | `#101116` | the bottom of Settings and History, and the selector's chips | the deep end of the thick material. See v1.2 |
| `--bubble-deep` | `#0C1B3B` | the bottom of the pointer's speech bubble | space indigo carried down its own hue, so the bubble is top-lit like everything else. The shared neutral floor would drain the colour out of the one surface whose colour is its identity. Defined in `renderer/cursor-buddy.css` |

So the rule that replaces "nothing else": a value outside the six is named as a token where it is
defined, carries its reason there, and is listed here. An unnamed hex sitting inline in a stylesheet is
still a defect, because it is a decision nobody can count. `renderer/region.css` had seven of them
under a header comment claiming it used the palette.

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

## v1.1: the checks the project kept describing and never wrote

v1 ended on a rule: a suite that only reports is a suite that watches things
break, so `must()` fails the run and takes the exit code with it. That rule was
applied to the smoke run and stopped there. Everything true of the *source*
rather than of a running app still had no way to fail, and three of the defects
below were sitting in plain sight inside comments that described a guard nobody
had written.

### A gate for the things that need no app

`npm run check` (`tools/check.js`). It exits non-zero, which is the only
property that separates a check from an observation. Four of them, each a class
this repository has actually shipped rather than a style opinion:

| Check | Why this one |
| --- | --- |
| syntax | No build step and no typechecker. A stray bracket in a branch nobody takes reaches a user before it reaches anyone else |
| palette | The rule above already says an unnamed inline hex "is still a defect, because it is a decision nobody can count", and `region.css` once carried seven of them under a header comment claiming it used the palette. A comment cannot enforce that |
| ipc | The preload allowlist drifts both ways. A renderer invoking an unlisted channel fails only at runtime, in a frameless window with no devtools open; a listed channel nothing invokes is the `query:cancel` defect exactly, every layer present except the one that calls it |
| line endings | The tree is CRLF. One LF file turns its next edit into a whole-file diff with the real change lost inside it |

It found `buddy:move` and `settings:data` on the listen allowlist on its first
run, present nowhere else in the repository. Nothing sent them, nothing listened
for them.

### The failover had the same blind spot it was built to cover

`listModels` refuses to offer an id the live catalogue does not have — that is
the v1 fix. `nextFreeModel`, twenty lines down, read its next id straight out of
the curated array and never looked. So the machinery written to survive a
retired model could hand the next attempt another retired one, and burn one of
only three tries on a guaranteed 404.

It prefers a live id now, and for vision one that still accepts an image, since
a model that lost image input answers a screenshot with a 400 and failover does
not retry those. The catalogue is consulted only when already in hand: fetching
inside a retry would put a round trip on the slow path, and a cold cache has to
mean "no opinion" rather than "nothing to try".

Which exposed why it was blind. The cache was only ever warmed by opening
Settings, so the ordinary path — launch, ask, hit a queued free model — was
precisely the one running without it. It is warmed once at boot.

### Three sentences the app could not have known were true

| The copy said | What had happened |
| --- | --- |
| 404: "and the free model tried after it was not either" | On a paid model no stand-in is ever tried |
| 429: "every free model this app knows about was busy too" | The cap is three attempts, which is never "every" |
| 5xx: "on this model and on the fallback" | Same: asserted whether or not one ran |

`withFailover` stamps the attempt count on the error, and the sentence follows
the count or is absent. This is the same rule as the pointer: dropping is the
important half, because something confidently wrong that the user cannot check
is worse than nothing.

### The windows were left on the primary display

v0.6 moved capture and the pointer onto the cursor's display, and recorded why:
on a two-monitor desk the primary is a coin flip. Every window kept positioning
against `getPrimaryDisplay()`, so the defect survived in the half the user can
see — the app answered about the screen they were working on and opened the
panel on the other one.

`workArea()` follows a latched display, and pill, panel, answer, both ghosts and
the answer window's height ceiling all position through it. Latched rather than
read live: those functions run during a glide, and reading the cursor's display
on every call would walk the panel to another monitor because the user reached
for their mouse. It re-latches where the overlay is summoned, and a latch that
changes moves the windows before anything animates, because a glide interpolates
rectangles and growing a panel out of a pill on another monitor is a window
flying across the desk. `display-removed`, `display-added` and
`display-metrics-changed` re-place everything, since an unplugged monitor would
otherwise strand every window on a rectangle nobody has, with the only control
that could move them living in the window that went away.

### The field could not hold the question

The query field was `<input type="text">`. The questions this app exists for
arrive as a pasted stack trace, or as "this is what I tried, this is what
happened", and a text input silently swallows every newline in one. There was no
way to write the second line of a question at all.

Everything around it already assumed otherwise: `INPUT_MIN_HEIGHT` 62,
`INPUT_MAX_HEIGHT` 280, a window height pinned to its content, and a resize
handler that only ever refused to change the width. The whole apparatus for a
field that grows, wrapped around one that could not.

It is a textarea. Enter sends, Shift+Enter is a newline, which is the messaging
convention the surface was already built on. The type scale is unchanged and the
rhythm is kept exactly: one row is 32px, being 21px of line plus the padding
either side, so a one-line question sits on the same control height as the
buttons beside it and each further line adds exactly one line of panel. Seven
rows, then it scrolls, which stays under `INPUT_MAX_HEIGHT` with room for the
area chip and the notice that share the panel. The chrome aligns to the top
rather than the middle, so the gear and the close button stay on the first line
instead of drifting down the side of a long question.

Photographed as `03c-panel-multiline`, because the panel is no longer one fixed
rectangle and a surface that changes shape has to be looked at in both.

### A comment that cost 400 milliseconds an answer

`history.trim()` said: "only measure when the count alone could plausibly exceed
the budget; stringifying the whole list after every answer is not free." There
was no such guard. `JSON.stringify` ran over the entire history after every
answer, on the process that draws the UI, and again on every pop — at the 4MB
cap, a multi-megabyte serialise to discover that nothing needed dropping.

Sizes are summed instead. Measured on the same input, 60 saves of a 120KB
conversation: 428ms before, 1ms after, keeping the same 34 entries. An estimate
that read low would let the file grow past its cap, which is worse than the cost
it replaced, so the estimator reads high and the self-check measures the real
serialised length rather than trusting the estimate that produced it.

This is the third time a comment in this repository has described something the
code did not do — after the arrow tip and the "uses the palette" header. The
pattern is worth naming: the comments that lie are the ones stating a
*performance* or *geometry* property, because nothing ever fails when they stop
being true.

### The area selector never returned an area

Choosing an area is described in this file as the largest single lever on answer
quality, and `lib/region.js` opens with the arithmetic: a 600x300 selection goes
over the wire at 1:1 instead of 0.62x, and small text is simply legible.

It never worked. `finish()` closed the selector window before settling its
promise. `close()` calls `destroy()`, `destroy()` emits `closed`, and the
`closed` handler settles with `null` — the right safety net for a window that
goes away by some route nobody planned. Whether that mattered turned on one
question nobody had asked, so it was asked directly, with a 25-line probe:
`closed` fires *synchronously*, during `destroy()`. The net therefore caught
every successful selection as well. The promise resolved null, `pending` was
cleared, and the call carrying the actual rectangle was a no-op. `Ctrl+Shift+A`,
drag a box, release, and the app treated it as a cancel.

The fix is the order: settle, then close.

The interesting part is why it lasted. There were three assertions about the
selector and all three were about cancelling — it cancels on demand, Escape
dismisses it without taking the panel, and no scrim is left stranded. Every one
of them passed, on a feature that could not succeed. A suite can be detailed,
green, and still be describing only the ways a thing fails to happen.

That is the same shape as the v1 finding, and it is worth stating as a rule
rather than as a third anecdote: **test the path the feature exists for first,
and the failure paths second.** The failure paths are easier to write, which is
exactly why they accumulate.

## v1.2: the surfaces were coloured in, not lit

"A very very light liquid glass, to make it feel premium Apple." Photographed
at four times life size, the reason the surfaces did not was specific rather
than a matter of taste. Every one of them was a flat two-stop gradient with a
uniform 1px bevel across the top, which is a slab with a line drawn on it. The
one part that would have read as a material -- a large soft shadow separating
it from the desktop -- was pointed at a wall.

The material is painted. That is not a preference; v0.4 and v0.5 already
settled it, and nothing has changed:

- `backdrop-filter` cannot see past a window boundary. These are transparent
  windows over the desktop, so there is nothing behind them for CSS to blur
  and the property silently does nothing.
- `backgroundMaterial` acrylic is incompatible with `transparent: true`. It
  shipped once and cost the overlay its topmost behaviour, its collapsed
  click target, and the desktop's repaints.
- Depth is a material, not a compositor feature. This section is that sentence
  taken further than a flat gradient took it.

### The material, in four parts

| Part | What it is | Why it is that and not something else |
| --- | --- | --- |
| fill | `linear-gradient(var(--grey), var(--glass-deep))` | shadow grey is the palette value the distribution rule always assigned to surfaces; `--skin` used an unnamed `#3E404A` instead. The deeper ramp also buys contrast: slate at 0.70 on the old top stop measured 4.14:1, under this file's own floor for text at 12px, and 4.55:1 on the new one |
| sheen | slate at 0.085, fading out over the top **18px** | anchored to the edge in pixels, not as a percentage of height. Edge-lighting is a property of the edge; a percentage would give a 62px query bar a 20px bloom and a 780px transcript a 250px one, which is a spotlight and not a material |
| specular | a 1px pseudo-element on the top edge, slate at 0.3, fading out over the 12px corner radius at each end | `box-shadow: inset 0 1px 0` runs at one strength all the way into the corners, where the sheet has turned away from the light. That is a bevel. A gradient can fade along the span; a shadow cannot, which is why this is the one part that needs an element of its own |
| floor | `inset 0 -1px 0` black at 0.5, plus a soft `inset 0 -18px 22px -20px` | the underside of the sheet. Measured on the render, the answer window ends `#141519`, then `#090A0C` at the contact line, then the border |

Two thicknesses of the same material, which is the only thing about it that is
not uniform. The floating surfaces -- pill, query bar, answer, both layout
ghosts -- run shadow grey to `--glass-deep`. Settings and History run
`--glass-deep` to `--glass-floor`, so the thick material starts exactly where
the thin one ends. Every edge is identical. Apple ships regular and thick glass
for the same reason: a floating control and a scrolling document want different
amounts of light through them, and these two windows carry a type ladder down
to 11px that a brighter surface would cost.

That claim is measured, not asserted. Twelve labels in those two windows,
before against after: title 6.43 -> 6.79, h2 6.95 -> 7.30, summary 4.91 ->
5.08, Edit 4.16 -> 4.27, note 5.02 -> 5.14, status 8.29 -> 8.61, history
preview 3.69 -> 3.77, history timestamp 2.56 -> 2.57. Ten improve, two are
unchanged to two decimals, none regress.

Six of those are under 4.5:1 both before and after. That is a real violation of
this file's "0.70 alpha minimum, never lower for text under 14px", it predates
this work, and it is not a glass problem: fixing it means rebuilding the
hierarchy those two windows are scanned by, which is its own change and should
not be smuggled in behind a lighting one.

### Raised and cut, instead of another border

| Token | Value | For |
| --- | --- | --- |
| `--rim` | `inset 0 1px 0` slate 0.13, `inset 0 -1px 0` black 0.22 | anything raised out of the surface: buttons, keycaps, the model dropdown, the question chip, a switch that is on, a hovered history row |
| `--well` | `inset 0 1px 2px` black 0.22, `inset 0 -1px 0` slate 0.06 | anything cut into it: text fields, the search box, a code block, a switch that is off |

A raised control catches the light on its top lip and a well catches it on the
bottom one. That is two pixels doing the work a border would have done, and it
keeps the rule this design has held since v0.4: default borderless, separate
with whitespace and then a background shift.

The accent gets the rim and nothing else. Slate on grape soda measures 4.69:1,
which is 0.19 above the body-text floor; a sheen down the face of the button
takes the text midline to 4.05:1 and the top of the button to 3.47:1. There is
no budget there for a gradient, and one lit pixel above every glyph costs
nothing, because no glyph reaches it.

### A shadow aimed at a wall

`--lift` was `0 1px 2px, 0 18px 44px` and it was on the pill, the query bar and
the answer window. `#panel` and `#opanel` sit at `inset: 0` in windows sized
exactly to the surface -- `lib/shots.js` asserts that on every run and calls
any difference a dead band -- so a 44px blur on either of them was rasterised,
clipped by the window frame, and shown to nobody. It is off both.

`#pill` keeps a shadow, because it has somewhere to put one: `inset: 3px` in a
104x40 window, so three pixels of margin. It is now sized to that three pixels
rather than to forty-four. Photographed against the shot backdrop, the visible
result is the same to within two levels at the outer pixel -- `#181A1E`,
`#23252C`, `#2B2E36` under the pill against `#181A1E`, `#202228`, `#25272E`
before -- for a seventh of the blur area.

The speech bubble keeps a real cast shadow, because it is the one surface that
floats in a window the size of a whole display.

### Not laggy, with a number

Frame timing on the surface that repaints continuously. The answer window's
transcript streams and the window grows as it does, so the worst honest case is
`#opanel`'s whole background invalidated on every animation frame: two gradient
layers, three inset shadows and the specular, re-rastered sixty times a second.

A/B in one process, counterbalanced, the "before" arm being the flat skin and
the 44px shadow restored through `insertCSS`. Counterbalancing matters, and the
first run proved it: running glass first in every pair made it look 30% more
expensive, and the giveaway was `ScriptDuration` -- the benchmark's own
JavaScript, byte for byte identical in both arms, came out 20 to 40% higher in
whichever arm ran first.

| Surface | | fps | frames over 20ms | main thread, per 3000ms |
| --- | --- | --- | --- | --- |
| 420x620 | glass | 60.0 | 0.0 | 429ms |
| 420x620 | flat | 60.0 | 0.3 | 320ms |
| 900x900 | glass | 60.1 | 0.0 | 486ms |
| 900x900 | flat | 59.8 | 1.0 | 498ms |

900x900 is the largest surface the app allows: widths are capped at 900 and a
docked answer window can fill the work area's height. At that size the two are
indistinguishable. At 420x620 the glass costs about 110ms of main thread per
three seconds of continuous full-surface invalidation -- roughly 0.6ms of a
16.7ms frame -- and it is spent on style and layout rather than raster, because
the specular is an extra box to lay out whenever the surface changes size.
Neither arm dropped a frame that mattered, and the glass arm dropped none at
all.

Rejected on cost rather than on looks: a radial specular in the top-left corner
(a third gradient layer on every surface, and it reads as a blob before it
reads as light); a 1px inner ring down the left and right edges (uniform, so it
draws a second border immediately inside the first, which is the tell it was
supposed to avoid); anything using `filter` or generated noise on a surface
that repaints or moves. The area selector's size chip is moved with
`translate3d` on every mousemove of a drag across the whole desktop, so its
glass is non-blurred insets only.

### Three things the photographs caught that reading the CSS would not

`#opanel::after`, the fade that says there is more transcript below, had
`--skin`'s base stop written out as a literal `rgba(38, 41, 48, ...)`. Moving
the token without it would have put a visible seam across the bottom edge of
the answer window. It follows `--glass-deep` now, with a note saying why it
cannot be a variable.

`.bubble-body` needed `position: relative` to be a containing block for its
specular. That moved it into the positioned paint layer, above its sibling
`.bubble-tail`, and the bubble's own left border then drew straight across the
join: the tail read as a separate arrow floating beside a card instead of part
of one shape. Invisible at life size and obvious at ten times it. The tail
carries `z-index: 1` now, with the reason beside it.

`#callout-hint` was slate at 0.55 alpha, 12px, on space indigo: 3.91:1. Under
the 4.5 floor, and under this file's own rule that slate never goes below 0.70
for text under 14px. It is the line telling you the bubble can be clicked away,
and it sits over whatever happens to be on the user's desktop. At 0.70 it
measures 5.44:1 and it is still the quietest thing in the bubble, because it is
small and italic and nothing above it is either.

### A fourth comment that described something the code did not do

`settings/settings.css` and `history/history.css` both open by claiming "the
same surface tokens as the overlay". Neither used them: both drew their own
background out of two unnamed values at an alpha nothing else in the app has,
and their `--bevel` was a different number from the overlay's. After the arrow
tip, the "uses the palette" header, and the `history.trim()` guard, that is the
fourth. The pattern named in v1.1 holds -- the comments that lie are the ones
stating a property nothing ever fails on -- and "same tokens as X" belongs on
that list beside performance and geometry.

### What deliberately did not change

The radius scale. 4 for keycaps, 6 for controls, 8 for cards, 12 for floating
windows, and the pill's 11 against the panel's 12, which is what makes the
expand glide read as the pill becoming the panel. Rounder corners are the
obvious move towards this look, and they would have spent a documented piece of
continuity on a stylistic one.

Nothing about motion. Every animation is still transform and opacity, entrances
still scale from 0.9 to 0.95, and `prefers-reduced-motion` still kills all of
it. Glass is a static material here: it does not shimmer, travel, or respond to
a cursor it cannot see.

And the border. Every floating window keeps its 1px slate edge even though
stacking a hairline with a shadow is a known tell, because the reason recorded
under "Spacing and radius" still holds -- these windows sit over arbitrary
desktop content, a shadow alone disappears against a dark one, and the 3:1
boundary requirement is not negotiable for an overlay.
