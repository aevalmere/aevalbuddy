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

## Every surface ships four states

| Surface | Loading | Empty | Error | Offline |
| --- | --- | --- | --- | --- |
| Answer window | thinking dots inside the assistant bubble | window stays hidden until a first message exists | inline row naming the failure and the fix | network errors name the connection and the retry key |
| Model picker | "Loading models..." label | never empty: live list, else known-models fallback with a labelled note | inline note when the live fetch failed | fallback list keeps the picker usable |
| Settings key | saved state shown as a chip | "No key yet" chip plus the walkthrough | inline message next to the field | not applicable (local) |
