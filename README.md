# AevalBuddy

A small always-on-top overlay for Windows that answers questions about whatever is on your screen. Press a hotkey, it grabs a screenshot, sends it with your question to OpenRouter (a free vision model by default), and streams the answer into a floating panel. The overlay is excluded from screen capture, so it stays invisible in screen shares and recordings.

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

The input bar and the answer are separate windows. By default the answer docks under the input bar; the layout editor can split them apart (see Overlay position). Answers render markdown (headings, lists, bold, inline code, fenced code blocks, links that open in your browser), have a Copy button that copies the raw text, stay selectable for partial copies, and right-click gives copy and paste menus everywhere, including the key field in settings.

The first two shortcuts are rebindable in settings: press Change, then press the new combo. New combos must include Ctrl, Alt, or Win. If another app already owns the combo, settings shows an inline error and keeps listening for a different one. Changes apply immediately, no restart. `Esc` is fixed. Shortcuts are listed in settings only; the query panel stays clean.

### Model

The default model is `dots-studio/dots-3-note-preview:free`, picked because it costs nothing and its modality is exactly image plus text. Settings shows a dropdown of the free vision models currently on OpenRouter (fetched live, with a static fallback when offline); picking one saves immediately, and a text field below accepts any other OpenRouter model id. The dropdown is custom-drawn because native select popups misrender inside transparent frameless windows on Windows.

Two OpenRouter quirks to know. Free ids (ending in `:free`) rotate over time, so if queries fail with "model unavailable", pick a fresh one from the dropdown. And OpenRouter blocks `:free` models entirely until the account allows them: if errors mention a data policy, open openrouter.ai, go to Settings then Privacy, and enable "Free model publication". The inline error tells you this when it happens.

### Overlay position

Settings has "Edit overlay position". It turns both windows into draggable layout ghosts, the way games let you rearrange a HUD: an Input bar ghost and an Answer window ghost, each draggable anywhere, each snapping to the nine standard anchors (corners, edge midpoints, center) when released nearby. Drop the answer ghost near the input bar to dock them, shown by a "Docked to input" badge; a docked answer follows the input bar and grows away from it (downward from a top anchor, upward from a bottom one). Press Done or Esc to keep the layout. The pill, the input bar, the answer, and future launches all use the saved positions.

Quit from the settings panel (gear icon in the query panel, then "Quit AevalBuddy"). The overlay has no taskbar icon, so the hotkeys and the pill are the whole interface.

## Develop it (the live-reload loop)

```bash
npm run dev
```

This runs the app under a file watcher. Edit any source file (or have an AI assistant edit it) and the running app updates itself: changes to renderer HTML, CSS, or JS reload the windows in place; changes to main-process files restart the app in about a second. Leave it running while iterating; there is no manual quit-and-relaunch step.

Two rules that save confusion. Only one copy of the app can run at a time (a second launch just pings the first one), so close any `npm start` copy before starting `npm run dev`. And the packaged installer in `dist/` is a frozen snapshot from whenever `npm run build` last ran; it never updates itself, so rebuild it when you want the installable version to catch up.

## Build the installer

```bash
npm run build
```

This produces `dist/AevalBuddy Setup 0.1.0.exe` (NSIS, per-user install, no admin prompt).

The installer is unsigned, so Windows SmartScreen shows "Windows protected your PC" on first run. That is expected for an unsigned app, not a defect. Click "More info", then "Run anyway". Code signing needs a paid certificate and is out of scope for this MVP.

## How invisibility works

The overlay, settings, and walkthrough windows all call `setContentProtection(true)`, which maps to `WDA_EXCLUDEFROMCAPTURE` on Windows. Screen shares, recordings, and screenshots taken through standard capture APIs will not include them. This is reliable on Windows 10 2004 and later, but not guaranteed for every capture backend (some hardware capture devices and older OS builds still see the window). The app keeps working either way; the overlay is just visible in those recordings.

A side effect worth knowing: the screenshots AevalBuddy sends to the model also exclude its own windows, so the model sees your screen, not the assistant.

## Design notes and assumptions

- The screenshot covers the primary display only and is downscaled to at most 1280px wide, then JPEG-compressed to stay under ~1MB per request.
- Screen capture, the API key, and all model calls live in the Electron main process. Renderers talk to it through a small allowlisted IPC bridge and never see the key.
- The walkthrough pointer glides on a requestAnimationFrame interpolation loop, so it moves continuously instead of stepping, and its callout carries a close button. The pointer window is click-through everywhere except over that callout, so it never blocks your work.
- Markdown rendering is a small built-in renderer: model text is HTML-escaped before any transform, so answers cannot inject markup, and links only open http or https URLs through the system browser.
- Answer-window height updates are throttled and rounded during streaming so the window grows calmly instead of jittering per token.
- `Esc` is registered as a global shortcut only while the panel, the layout editor, or the walkthrough is open. While the overlay is idle, `Esc` behaves normally in every other app.
- The overlay position is stored as an edge anchor plus offset, so the pill and the panel always hug the same corner and a bottom-anchored panel grows upward.
- Rebinding rejects Shift-only combos on purpose: `Shift+A` would fire while typing capital letters.
- `electron-store` is pinned to v8, the last CommonJS release; v10+ is ESM-only and cannot be `require`d from a CommonJS main process.
- The default model was taken from OpenRouter's live model list on the day this was written (one of three `:free` ids that accept image input). The list shifts, which is why settings fetches it live and the model is a setting rather than a constant.
- A model that streams no answer text (some free models emit only reasoning traffic through certain providers) produces a clear inline error instead of an empty panel.
- No conversation history: each capture and question is a single stateless request.

## Files

```
main.js                    windows, hotkeys, capture, HUD editor, docking, IPC, query streaming
preload.js                 allowlisted contextBridge between main and renderers
lib/openrouter.js          OpenRouter client: SSE streaming, model listing, error mapping
lib/store.js               electron-store wrapper: key encryption, model, shortcuts, positions
lib/cursor-buddy.js        guided-walkthrough state machine (main process)
renderer/index.html        the input window: pill, query bar, input layout ghost
renderer/overlay.js|css    input window logic and the shared stylesheet
renderer/output.html|js    the answer window: markdown response, copy, answer layout ghost
renderer/markdown.js       built-in markdown renderer (escape first, then transform)
renderer/buddy.html|js     the pointer window
renderer/cursor-buddy.css  pointer and callout styling
settings/                  settings window: API key, model picker, position editor, shortcuts
electron-builder.yml       NSIS packaging config (output in dist/)
```
