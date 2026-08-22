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

It works like a messaging app. Your message appears as a bubble, the answer streams into a bubble under it, and typing again sends a follow-up in the same conversation (each message captures the screen fresh, so "what about now?" means now). The transcript scrolls inside the answer window instead of bouncing its size around. Hover an answer for its Copy button, select text for partial copies, right-click for copy and paste menus, and press New chat in the answer window to start over. Collapsing with Esc keeps the conversation; it comes back with the next capture. Nothing is saved to disk; quitting clears it.

Answers render markdown: headings, lists, bold, inline code, fenced code blocks, and links that open in your browser. And when an answer involves a specific place on your screen, the assistant sends the pointer there: an arrow glides to the exact pixel and a speech bubble types out what to do, with the tail pointing back at the arrow. Drag the bubble if it is covering something, click it to skip the typing, press Next to walk through several spots, and close it with its X or Esc.

The input bar and the answer are separate windows. By default the answer docks under the input bar; the layout editor can split them apart (see Layout).

The first two shortcuts are rebindable in settings: press Change, then press the new combo. New combos must include Ctrl, Alt, or Win. If another app already owns the combo, settings shows an inline error and keeps listening for a different one. Changes apply immediately, no restart. `Esc` is fixed. Shortcuts are listed in settings only; the query panel stays clean.

### Model

The default is `qwen/qwen3.7-flash`. A screen assistant lives or dies on how fast the first words appear, and the free tier is queued and rate limited, which is what made earlier versions crawl. This one is not free, but at roughly $0.03 per million input tokens a question costs a small fraction of a cent, and a dollar of credit lasts a very long time.

The picker groups models by what they are for, with live prices and context sizes pulled from OpenRouter so the labels cannot go stale:

| Group | What it is |
| --- | --- |
| Recommended | the default: quick, reads a screen well |
| Fast | DeepSeek, GLM, ByteDance Seed and a small Qwen, all in the same speed class |
| Smarter, slower | GLM 5V and Qwen Max, for multi-step problems |
| Free | whatever OpenRouter currently gives away with image input; costs nothing, queued and rate limited |

Every curated id is checked against the live list before it is offered, so a retired model disappears from the picker rather than failing mid-question. "Use a different model id" accepts any other OpenRouter model that takes images; clear it and save to return to the recommended one. The dropdown is custom-drawn because native select popups misrender inside transparent frameless windows on Windows.

If you have no credit at all, pick something from the Free group; the error you get otherwise says exactly that and where to fix it. Two other OpenRouter quirks: free ids ending in `:free` rotate over time, and OpenRouter blocks them entirely until the account allows them. If an error mentions a data policy, open openrouter.ai, go to Settings then Privacy, and enable "Free model publication".

### How it answers

Settings has a third step: a procedure the assistant follows before it replies. Each one is a markdown file in `skills/` that travels with every request, so the shape of the answer is enforced outside the model. That is what keeps a small fast model on the point instead of rambling, and it is the difference between an answer that names the next action and one that describes the screen back to you.

Four ship with the app: Screen assistant (the default), Debug an error, Explain this, Improve this writing. Adding your own is dropping a file into `skills/` with a `name` and `summary` in its front matter; it appears in the picker at next launch.

### Layout

Settings has "Edit layout". It turns both windows into draggable layout ghosts, the way games let you rearrange a HUD: an Input bar ghost and an Answer window ghost, each draggable anywhere, each snapping to the nine standard anchors (corners, edge midpoints, center) once you let go. Snapping waits for the drag to settle, so nothing jumps out of your hand mid-drag.

To separate the answer from the input bar, drag the answer ghost clear of the dock zone, or just press its Detach button; press Dock to input (or drop it back near the input bar) to reattach, shown by a "Docked to input" badge. A docked answer follows the input bar and grows away from it (downward from a top anchor, upward from a bottom one). Press Done or Esc to keep the layout. The pill, the input bar, the answer, and future launches all use the saved positions.

Quit from the settings panel (gear icon in the query panel, then "Quit AevalBuddy"). The overlay has no taskbar icon, so the hotkeys and the pill are the whole interface. The settings footer shows the app version, which is the quick way to confirm which build is running.

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

This boots the app, drives every surface into a representative state, and writes a PNG of each one to `shots/`: the pill, the query panel with and without a key, a frame caught part way through the opening glide, the conversation, the thinking state, both layout ghosts, settings top and bottom, the model list open, and the pointer with its bubble.

It works by calling `webContents.capturePage()`, which renders the page itself rather than reading the display, so it sees windows that are invisible to screenshots (see below). It runs against a throwaway settings profile, which means it does not disturb a copy of the app you already have running. It also prints `fit.<surface>=windowN:surfaceN` lines: those two numbers should match, and a gap between them is a band of empty window showing under the panel.

## Build the installer

```bash
npm run build
```

This produces `dist/AevalBuddy Setup 0.3.0.exe` (NSIS, per-user install, no admin prompt).

The installer is unsigned, so Windows SmartScreen shows "Windows protected your PC" on first run. That is expected for an unsigned app, not a defect. Click "More info", then "Run anyway". Code signing needs a paid certificate and is out of scope for this MVP.

## How invisibility works

The overlay, settings, and walkthrough windows all call `setContentProtection(true)`, which maps to `WDA_EXCLUDEFROMCAPTURE` on Windows. Screen shares, recordings, and screenshots taken through standard capture APIs will not include them. This is reliable on Windows 10 2004 and later, but not guaranteed for every capture backend (some hardware capture devices and older OS builds still see the window). The app keeps working either way; the overlay is just visible in those recordings.

A side effect worth knowing: the screenshots AevalBuddy sends to the model also exclude its own windows, so the model sees your screen, not the assistant.

## Design notes and assumptions

- Visual constraints (palette usage, motion curves, spacing and radius scales, per-surface states) live in `DESIGN.md`; the stylesheets build to it. The signature motion is one decelerating curve, `cubic-bezier(0.32, 0.72, 0, 1)`.
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
lib/openrouter.js          OpenRouter client: SSE streaming, history, point directives, models
lib/store.js               electron-store wrapper: key encryption, model, skill, shortcuts, positions
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
