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

The first two shortcuts are rebindable in settings: press Change, then press the new combo. New combos must include Ctrl, Alt, or Win. If another app already owns the combo, settings shows an inline error and keeps listening for a different one. Changes apply immediately, no restart. `Esc` is fixed. Shortcuts are listed in settings only; the query panel stays clean.

### Model

The default model is `thinkingmachines/inkling:free`, picked because it costs nothing and accepts images. Change it in settings to any OpenRouter model id. Two things to know: free ids (ending in `:free`) rotate over time, so if queries start failing with "model not found", grab a current id from openrouter.ai/models; and the model must be vision-capable to actually read the screenshot, otherwise you get a clear inline error naming the problem.

### Overlay position

Settings has "Edit overlay position". It turns the overlay into a draggable layout ghost, the way games let you rearrange a HUD: drag it anywhere, and it snaps to the nine standard anchors (corners, edge midpoints, center) when you release nearby. Press Done or Esc to keep the spot. The pill, the query panel, and future launches all use the saved position, hugging the same corner or edge.

Quit from the settings panel (gear icon in the query panel, then "Quit AevalBuddy"). The overlay has no taskbar icon, so the hotkeys and the pill are the whole interface.

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
- `Esc` is registered as a global shortcut only while the panel, the layout editor, or the walkthrough is open. While the overlay is idle, `Esc` behaves normally in every other app.
- The overlay position is stored as an edge anchor plus offset, so the pill and the panel always hug the same corner and a bottom-anchored panel grows upward.
- Rebinding rejects Shift-only combos on purpose: `Shift+A` would fire while typing capital letters.
- `electron-store` is pinned to v8, the last CommonJS release; v10+ is ESM-only and cannot be `require`d from a CommonJS main process.
- The default model was taken from OpenRouter's live model list on the day this was written (one of three `:free` ids that accept image input). The list shifts, which is why the model is a setting rather than a constant.
- No conversation history: each capture and question is a single stateless request.

## Files

```
main.js                    window management, hotkeys, capture, HUD editor, IPC, query streaming
preload.js                 allowlisted contextBridge between main and renderers
lib/openrouter.js          OpenRouter chat-completions client, SSE streaming, error mapping
lib/store.js               electron-store wrapper: key encryption, model, shortcuts, overlay position
lib/cursor-buddy.js        guided-walkthrough state machine (main process)
renderer/index.html|js|css the overlay: pill, query panel, streamed response, layout ghost
renderer/buddy.html|js     the pointer window
renderer/cursor-buddy.css  pointer and callout styling
settings/                  settings window: API key, model, position editor, shortcuts
electron-builder.yml       NSIS packaging config (output in dist/)
```
