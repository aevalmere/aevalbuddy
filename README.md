# AevalBuddy

A small always-on-top overlay for Windows that talks about whatever is on your screen. Press a hotkey, type a question, and the answer streams into a floating conversation window; follow-ups continue the same conversation, and every message sees the screen as it is right then. The assistant can even point: when an answer involves a specific spot on screen, an animated pointer glides there with a label. The overlay is excluded from screen capture, so it stays invisible in screen shares and recordings.

Built with Electron and plain JS/HTML/CSS. No build step beyond `npm install`, and no AI SDK: requests go straight to OpenRouter's OpenAI-compatible endpoint.

## Run it

Requirements: Windows 10/11, Node.js 18+ and npm.

```bash
npm install
```

```bash
npm start
```

On the very first launch a pointer appears and walks you to the settings panel, where you paste an OpenRouter API key (create one free at openrouter.ai/keys). The walkthrough auto-runs only that once; dismiss it any time with its close button, Esc, or a click elsewhere, and replay it by typing `help me` in the query box. The key is stored on this machine only, encrypted through Windows credential storage when available; if encryption is unavailable it falls back to plain JSON in your user profile.

## Use it

| Shortcut | Action |
| --- | --- |
| `Ctrl+Shift+Space` | Capture the screen and open the query panel |
| `Ctrl+Shift+H` | Hide or show the overlay |
| `Esc` | Collapse the panel, close settings, exit the layout editor, or dismiss the walkthrough |

Type a question and press Enter, or press Enter on an empty box to send the default prompt ("What's on my screen and what should I do?"). Clicking the idle pill does the same thing as the capture shortcut.

It works like a messaging app, without looking like one. Your question appears as a small chip on the right, the answer streams in underneath as prose rather than in a coloured bubble, and typing again sends a follow-up in the same conversation (each message captures the screen fresh, so "what about now?" means now). The transcript scrolls inside the answer window instead of bouncing its size around. Hover an answer for its Copy button, select text for partial copies, right-click for copy and paste menus, and use the + at the top of the answer window to start over. Collapsing with Esc keeps the conversation; it comes back with the next capture. Nothing is saved to disk; quitting clears it.

Answers render markdown: headings, lists, bold, inline code, fenced code blocks, and links that open in your browser. And when an answer involves a specific place on your screen, the assistant sends the pointer there: an arrow glides to the exact pixel and a speech bubble types out what to do, with the tail pointing back at the arrow.

The bubble is the control. Click it once to finish the typing, click again for the next spot, and click the last one to dismiss; the line under the message says which. Drag it anywhere if it is covering something, or close it with its X or Esc.

The input bar and the answer are separate windows. By default the answer docks under the input bar; the layout editor can split them apart (see Layout).

The first two shortcuts are rebindable in settings: press Change, then press the new combo. New combos must include Ctrl, Alt, or Win. If another app already owns the combo, settings shows an inline error and keeps listening for a different one. Changes apply immediately, no restart. `Esc` is fixed. Shortcuts are listed in settings only; the query panel stays clean.

### Model

Everything in the picker is free by default. The list is the free vision
models OpenRouter currently offers, with live context sizes, and the default
is `nvidia/nemotron-nano-12b-v2-vl:free`, which is built for reading images
and is the best of the free ones at small text and dense interfaces. This
model's only job may be reading the screen for the brain, and a wrong answer
built on a misread variable name is not a fast answer. `google/gemma-4-26b-a4b-it:free`
is still there if you want speed over accuracy. Free models are queued and
rate limited, so they are slower to start than paid ones; that is the trade.

Flip "Show paid" to add a second group. Those bill per token at rates where a
question costs a fraction of a cent (`qwen/qwen3.7-flash` is the one to pick),
and nothing paid is ever selected for you. Every id is checked against the
live list before it is offered, so a retired model disappears from the picker
rather than failing mid-question.

### Separate brain

The strongest free models on OpenRouter are text only. GLM 5.2 cannot see an
image, so the only way to put it behind a screen assistant is to give it a
reading of the screen instead. That is what this switch does: the model above
reads the screen into a short structured report, and a second model answers
from that report.

The report is not prose. It names the app, the state, what has focus, the text
that matters copied character for character, and a list of controls with their
positions on a 0 to 1000 grid.

The brain never sees those numbers. It cannot check a coordinate it did not
observe, and when it was allowed to write one it copied the wrong row or made
one up, which is what put the pointer in the wrong place. It names a target
instead, and the app looks the name up in the reader's list. A name that
matches nothing points at nothing, on purpose: a pointer in the wrong place is
worse than none.

What it costs is worth knowing. Two calls run in sequence, so the wait is the
reader's full reply plus the brain's first token, and the answer window says
which stage it is on rather than sitting on an unexplained pause. The brain
also only knows what the reader wrote down: a detail the reader skipped is a
detail the brain cannot use. On by default, because the difference between a
12B model's answer and GLM 5.2's is most of the difference in answer quality;
turn it off for one fast call instead of two. The default brain is
`z-ai/glm-5.2:free`.

### Answer style

Automatic by default, and that is the setting to leave alone. Each request is routed to a procedure the
assistant follows before it replies: Debug an error, Explain this, Improve this writing, or the general
Screen assistant. The routing takes the reader's own word for what is on screen first, because it has
actually looked at it, and falls back to scoring the question. Anything unrecognised gets the general
one, which is the right answer for most questions.

The procedures are markdown files in `skills/`, sent with every request, which is what keeps a small
free model on the point instead of rambling. Pin one by hand in settings if you want the same treatment
every time. Adding your own is dropping a file into `skills/` with a `name` and `summary` in its front
matter; it appears at next launch.

### Reading a long answer

The transcript scrolls inside the answer window: wheel over it, or Page Up,
Page Down, Home and End. The window grows with the answer up to two thirds of
your screen height before the scrolling starts, and the bottom edge fades
while there is more below.

### Resizing and layout

Drag any window edge. The input bar and the answer window resize from 320 to 900 wide and remember it;
the input bar's height stays pinned to its content, because a one-line field has nothing to gain from
being taller. Drag the answer window's height and it stops sizing itself to the transcript and holds
what you set, until New chat hands that back. Settings resizes freely and reopens where you left it.

### Layout

Settings has "Edit layout". It turns both windows into draggable layout ghosts, the way games let you rearrange a HUD: an Input bar ghost and an Answer window ghost, each draggable anywhere, each snapping to the nine standard anchors (corners, edge midpoints, center) once you let go. Snapping waits for the drag to settle, so nothing jumps out of your hand mid-drag.

To separate the answer from the input bar, drag the answer ghost clear of the dock zone, or just press its Detach button; press Dock to input (or drop it back near the input bar) to reattach, shown by a "Docked to input" badge. A docked answer follows the input bar and grows away from it (downward from a top anchor, upward from a bottom one). Press Done or Esc to keep the layout. The pill, the input bar, the answer, and future launches all use the saved positions.

Quit from the settings panel (gear icon in the query panel, then Quit). No window of this app appears in the taskbar or in Alt-Tab: every one of them is a Windows tool window, so the hotkeys and the pill are the whole interface. The settings footer shows the app version, which is the quick way to confirm which build is running.

## Develop it (the live-reload loop)

```bash
npm run dev
```

This runs the app under a file watcher. Edit any source file (or have an AI assistant edit it) and the running app updates itself: changes to renderer HTML, CSS, or JS reload the windows in place; changes to main-process files restart the app in about a second. Leave it running while iterating; there is no manual quit-and-relaunch step.

Two rules that save confusion. Only one copy of the app can run at a time (a second launch just pings the first one), so close any `npm start` copy before starting `npm run dev`. And the packaged installer in `dist/` is a frozen snapshot from whenever `npm run build` last ran; it never updates itself, so rebuild it when you want the installable version to catch up.

### Looking at the UI

```bash
npm run shots
```

This boots the app, drives every surface into a representative state, and writes a PNG of each one to `shots/`: the pill, the query panel with and without a key, a frame caught part way through the opening glide, the conversation, the thinking state, both layout ghosts, settings top and bottom, both model lists, the pointer with its bubble, and one shot of the real overlay composited over the desktop.

That last one matters: `capturePage` renders the page in isolation, so it can never show the acrylic backdrop or the system shadow, because neither belongs to the page. The live shot goes through `desktopCapturer` with content protection off for a single frame (and straight back on), cropped to the overlay plus a margin, so what lands on disk is what a person actually sees and nothing else from the desktop.

It works by calling `webContents.capturePage()`, which renders the page itself rather than reading the display, so it sees windows that are invisible to screenshots (see below). It runs against a throwaway settings profile, which means it does not disturb a copy of the app you already have running. It also prints `fit.<surface>=windowN:surfaceN` lines: those two numbers should match, and a gap between them is a band of empty window showing under the panel.

## Build the installer

```bash
npm run build
```

This produces `dist/AevalBuddy Setup 0.5.0.exe` (NSIS, per-user install, no admin prompt).

The installer is unsigned, so Windows SmartScreen shows "Windows protected your PC" on first run. That is expected for an unsigned app, not a defect. Click "More info", then "Run anyway". Code signing needs a paid certificate and is out of scope for this MVP.

## How invisibility works

The overlay, settings, and walkthrough windows all call `setContentProtection(true)`, which maps to `WDA_EXCLUDEFROMCAPTURE` on Windows. Screen shares, recordings, and screenshots taken through standard capture APIs will not include them. This is reliable on Windows 10 2004 and later, but not guaranteed for every capture backend (some hardware capture devices and older OS builds still see the window). The app keeps working either way; the overlay is just visible in those recordings.

A side effect worth knowing: the screenshots AevalBuddy sends to the model also exclude its own windows, so the model sees your screen, not the assistant.

Below Windows 10 build 19041 the call does nothing at all, so the app checks the build and says so in the settings footer rather than letting you believe the overlay is hidden when it is not.

## Design notes and assumptions

- Visual constraints (palette usage, motion curves, spacing and radius scales, surface tokens, per-surface states) live in `DESIGN.md`; the stylesheets build to it. The signature motion is one decelerating curve, `cubic-bezier(0.32, 0.72, 0, 1)`.
- Acrylic was tried and reverted. Windows 11 does give a real blurred backdrop, and it looked better in an isolated test, but an opaque window stops behaving like a reliable topmost overlay and needs the layered style added back the moment `setOpacity` touches it. Depth comes from the material instead: a top-lit gradient, one crisp edge, a contact shadow and a deep soft one. The surface is opaque because at 97% a bright terminal behind it was still legible through the text on top.
- Window bounds animate; opacity does not. On Windows `setOpacity` toggles the layered window style, and driving it at frame rate put repaint artifacts on the desktop. Fades happen in CSS inside the window, and the animation runs at 60Hz rather than 120.
- Resizability is expressed through minimum and maximum sizes, never `setResizable`: on a frameless window, clearing that flag strips the thick-frame style and setting it back does not restore it. Equal limits pin a window just as firmly and are reversible. Every programmatic move unlocks the limits first, re-applies them after, and stamps itself so a drag can be told apart from the app moving its own window.
- One surface per window: the query bar has no field inside it, because a bordered input inside a bordered panel is the loudest tell that a UI was assembled rather than designed. Focus is the caret plus the panel border lifting, never a ring.
- An answer is a document, so it renders as prose directly on the surface. Only the question keeps a container, a quiet chip, which is what preserves the sense of a conversation without the app looking like a toy chat client.
- Opening animates the OS window, not just its contents. CSS can only move pixels inside a window, so a CSS scale played after Electron paints the window at full size reads as a snap followed by a wobble; that was the flash. `lib/motion.js` glides the window bounds themselves on the same curve, and the stylesheets pin each panel to the edge the window grows away from, so the growth uncovers a finished layout instead of reflowing one. Under `prefers-reduced-motion`, read once from the renderer at boot, every window transition collapses to a single setBounds.
- The pointer's arrow has its tip at its own element origin, so the transform is the target pixel with no centring maths in between. Coordinates map from image pixels to screen position by ratio, which sidesteps DPI scaling.
- The conversation is held in memory only. The model sees up to the last 12 turns as text; only the newest message carries the screen image, so request size stays flat.
- The pointer directive is plain text in the stream (`[[point x=.. y=.. | label]]`), stripped before display. Models vary in how reliably they emit it; a garbled one renders as ordinary text rather than crashing anything.
- Skills are read from disk once at boot and appended to the system prompt as a procedure, keeping their line breaks. A skill file that disappears between versions falls back to the default rather than silently sending nothing.
- The screen view covers the primary display only and is downscaled to at most 1280px wide, then JPEG-compressed to stay under ~1MB per request.
- Screen capture, the API key, and all model calls live in the Electron main process. Renderers talk to it through a small allowlisted IPC bridge and never see the key.
- The pointer glides on a requestAnimationFrame interpolation loop, so it moves continuously instead of stepping. Its window is click-through everywhere except over the bubble, so it never blocks your work; hit-testing also stays on for the whole of a drag, otherwise letting the cursor slip off the bubble would drop it mid-gesture.
- Markdown rendering is a small built-in renderer: model text is HTML-escaped before any transform, so answers cannot inject markup, and links only open http or https URLs through the system browser.
- The answer window is sized to the transcript exactly, with an 8px deadband so it does not twitch. It refuses to shrink while an answer is streaming, so the bottom edge does not chase the text as it arrives.
- `Esc` is registered as a global shortcut only while the panel, the layout editor, or the walkthrough is open. While the overlay is idle, `Esc` behaves normally in every other app.
- The overlay position is stored as an edge anchor plus offset, so the pill and the panel always hug the same corner and a bottom-anchored panel grows upward.
- Rebinding rejects Shift-only combos on purpose: `Shift+A` would fire while typing capital letters.
- `electron-store` is pinned to v8, the last CommonJS release; v10+ is ESM-only and cannot be `require`d from a CommonJS main process.
- The curated model list is hand-picked but never trusted: every id is checked against OpenRouter's live list before it is offered, and the price and context labels come from that same response, so nothing in the picker can quietly go out of date.
- A model that streams no answer text (some free models emit only reasoning traffic through certain providers) produces a clear inline error instead of an empty panel.

## Files

```
DESIGN.md                  design constraints: palette usage, motion, scales, states
main.js                    windows, hotkeys, capture, chat state, HUD editor, docking, IPC
preload.js                 allowlisted contextBridge between main and renderers
lib/openrouter.js          OpenRouter client: SSE streaming, the reader stage, history, point directives, models
lib/store.js               electron-store wrapper: key encryption, model, brain, skill, shortcuts, positions
lib/skills.js              loads skills/*.md and appends the chosen one to the system prompt
lib/motion.js              window-bounds animation: the glide curve, anchored start rects
lib/cursor-buddy.js        pointer state machine: walkthrough steps and model-directed points
lib/shots.js               visual self-check: photographs every surface (npm run shots)
skills/*.md                answering procedures; drop a file in to add one
tools/shots-run.js         runner for npm run shots (scratch profile, output in shots/)
renderer/index.html        the input window: pill, query bar, input layout ghost
renderer/overlay.js|css    input window logic and the shared stylesheet
renderer/output.html|js    the answer window: conversation transcript, answer layout ghost
renderer/markdown.js       built-in markdown renderer (escape first, then transform)
renderer/buddy.html|js     the pointer window: arrow, speech bubble, typing, drag
renderer/cursor-buddy.css  pointer and bubble styling
settings/                  settings window: API key, model picker, layout editor, shortcuts
electron-builder.yml       NSIS packaging config (output in dist/)
```
