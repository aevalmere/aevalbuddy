# Aevaleron

**One shortcut, and your PC answers back.** Press `Ctrl+Alt+Space` in any app and type. Aevaleron sees what is on your screen, so you can ask about it, find things in it, or hand it the task and watch it work.

[![Download Aevaleron for Windows](https://img.shields.io/badge/Download-Aevaleron%20for%20Windows-0a84ff?style=for-the-badge)](https://github.com/aevalmere/aevaleron/releases/latest/download/Aevaleron-Setup.exe)

Free to use with free models. Windows 10 or 11. No account with us, no admin prompt.

## What it does

- **Ask about what you are looking at.** "What does this error mean?" "Summarise this page." The answer streams into a small glass window beside your work, and follow-ups keep the conversation. When the answer is about one spot on screen, a pointer glides there and shows you.
- **Find anything on screen.** Type a word and every match lights up, in any app, including text inside images. Ask for an idea instead of a word ("what do I owe?") and it highlights the lines that mean it, each with a one-line reason.
- **Let it do the task.** "Rename these files", "fill in this form", "turn on dark mode". Aevaleron shows its plan, then clicks and types for you, one visible step at a time. A safety check reads every risky step against what you asked for; money, passwords, sign-ins, installs and command lines always wait for your Allow. Move the mouse or press Esc and it stops.
- **Talk instead of typing.** Hold a key and dictate into any app; the words are cleaned up as they land.
- **Keep an eye on your coding agents.** Claude Code, Codex, Gemini CLI, Cursor and others report in. A card tells you when one finishes or needs you, and the PC stays awake while they work.

## Why people keep it

- **It stays out of the way.** No window in the taskbar and nothing on screen until you call it. One input works out whether you are asking, finding or giving a task.
- **It is private where it counts.** Your keys are encrypted on your PC. Aevaleron's windows are left out of screen shares and recordings, so nobody on a call sees your questions.
- **It costs nothing to start.** The default models are free on OpenRouter. Bring a key from Anthropic, OpenAI or Google when you want more.
- **It is small and fast.** Version 2 is rebuilt in Rust, for a much smaller download and a bar that opens the moment you press the shortcut.

## Download

**[Aevaleron-Setup.exe](https://github.com/aevalmere/aevaleron/releases/latest/download/Aevaleron-Setup.exe)** for Windows 10 or 11. That link is always the newest version. Older versions are on the [releases page](https://github.com/aevalmere/aevaleron/releases).

Aevaleron was called AevalBuddy before version 2. Installing it over AevalBuddy keeps your settings, keys, history and shortcuts.

1. **Run it.** It installs in a few seconds and opens the app. No admin prompt, no wizard, nothing to choose.
2. **If Windows says "Windows protected your PC"**, click **More info**, then **Run anyway**. That screen appears for any installer that has not paid for a code-signing certificate; it is not a virus warning. Your browser may also say the file is not commonly downloaded: choose **Keep**.
3. **Paste an API key.** A pointer walks you to the settings panel. Get a key free at [openrouter.ai/keys](https://openrouter.ai/keys); the default models cost nothing, so you do not need to add credit. Copy the key and Settings offers to use it. Free models also need **Free model publication** on at [openrouter.ai/settings/privacy](https://openrouter.ai/settings/privacy), which lets their providers keep and train on what they receive, your screen included.

Then press `Ctrl+Alt+Space` and type. Aevaleron starts with Windows from then on (turn that off in Settings, General). There is no window in the taskbar; the tray icon by the clock is how you find Aevaleron again.

It updates itself from then on. To remove it: Windows Settings, Apps, Installed apps, Aevaleron, Uninstall.

On the very first launch a pointer appears and walks you to the settings panel, where you paste an OpenRouter API key (create one free at openrouter.ai/keys). The walkthrough auto-runs only that once; dismiss it any time with its close button, Esc, or a click elsewhere, and replay it by typing `help me` in the query box. The key is stored on this machine only, encrypted through Windows credential storage when available; if encryption is unavailable it falls back to plain JSON in your user profile.

## Use it

One shortcut opens one input. Type a question, a word that is on screen, or a
task; the word at the left of the input says what Enter will do (Ask, Find or
Do), and you can settle it yourself.

| In the bar | Action |
| --- | --- |
| `Ctrl+Alt+Space` | Open the bar, from any app |
| `?` at the start | Ask, whatever the words look like |
| `/` at the start | Find on screen; `//` for exact words only |
| `>` at the start | Do it as a task on your computer |
| `Ctrl+1/2/3`, `Alt+Down` | Choose Ask, Find or Do without typing a prefix |
| `Backspace` on an empty bar | Go back to guessing |
| `Enter` / `Shift+Enter` | Send; in Find, next or previous match |
| `Alt+Enter` | Run a task the bar only guessed was one |
| `Up` | Earlier entries |
| `F6` | Move between the bar and the answer |
| `Esc` | Clear the text, then close the bar; stops a running task |

A task the bar only guessed never starts on Enter: it shows a card, Enter
answers it as a question, and running it is a click or `Alt+Enter`. The
per-action shortcuts from earlier versions (area, Find, Do, hide) are unbound
by default and can be set in Settings, Shortcuts; ones you had changed are
kept.

**Find** reads the app's own text where it shares it (and recognises text in
the pixels where it does not), so exact matches work offline and without a
key. Related matches explain themselves: `"refund" relates to "chargeback":
both are payment reversals`. The highlights are hidden from screen shares like
every other Aevaleron window.

**Do** (tasks) works with any OpenRouter model you pick; ones that support tool
calls use them, others write their actions out as JSON. It shows a plan, then
each step in the answer window, and the pointer glides to whatever it is
about to click. Anything that sends, deletes, buys, signs, types a password or
opens a link is a risky step, and so is any Enter, Delete, Alt
or Win chord, typing into a terminal, the Run box or an address bar, and any
drag. By default a safety check reads each risky step against what you asked
for: it runs the step when it is clearly part of your request, blocks it when
it works against you, and asks you when it is not sure. Money, passwords,
sign-ins, installs and command lines always wait for your Allow. Settings,
Tasks can make every risky step, or every action, ask you first. Esc, the Stop button, the tray, or
simply moving your mouse stops it. It asks for permission once, before its
first task.

**Who sees what.** Asking sends a screenshot, or the window's text when that
says enough, plus your question to OpenRouter and the model you picked. Tasks
send one per step. Free models' providers may keep and train on what they
receive; exact Find never leaves the computer.

There is a tray icon too: left-click it to open the bar,
right-click for every action plus History, Settings and Quit. It is the way back in if you hide the
overlay and forget the shortcut, because none of these windows appear in the
taskbar or in Alt-Tab.

Type a question and press Enter, or press Enter on an empty box to send the default prompt ("What's on my screen and what should I do?"). Clicking the idle pill does the same thing as the capture shortcut.

It works like a messaging app, without looking like one. Your question appears as a small chip on the right, the answer streams in underneath as prose rather than in a coloured bubble, and typing again sends a follow-up in the same conversation (each message captures the screen fresh, so "what about now?" means now). The transcript scrolls inside the answer window instead of bouncing its size around. Hover an answer for its Copy button, select text for partial copies, right-click for copy and paste menus, and use the + at the top of the answer window to start over. Collapsing with Esc keeps the conversation; it comes back with the next capture.

While an answer is on its way there is a Stop beside it, which matters because
the default models are free and therefore queued: waits of half a minute are
ordinary. If a request fails, the error says what happened and offers Retry,
which re-sends the question that failed rather than starting a new one.

Most failures never reach you. Free models are rate limited by design, so a
429, a 404 on a retired model, or a provider hiccup moves the request to the
next free model automatically and says which one in the status line. Only a
rejected key, no credit, or a genuinely malformed request stops the request,
because no other model would answer those differently.

### Asking about part of the screen

The crop button in the panel, or a shortcut you set in Settings. The screen dims, you drag a
box, and the question is about what is inside it.

This is the single biggest thing you can do for answer quality. A whole 2560px
desktop has to be squeezed into the app's 1600px budget before it is sent, so
12px interface text arrives at about seven pixels and the model is guessing at
glyphs. A 600x300 selection fits inside the same budget untouched, so it goes
at full resolution and small text is simply legible. It is also faster and
cheaper: a small image is a fraction of the tokens.

The selection sticks to the conversation, not to the one message, so a
follow-up looks at the same box. The panel shows a chip saying which area is in
force; "Whole screen" on that chip, or New chat, clears it. When the assistant
points at something it points inside the area, not at the equivalent spot on
the whole screen.

### Finding an answer again

Conversations are kept so you can come back to them. Press the clock icon at
the top of the answer window, or pick History from the tray, and you get a
searchable list: the question, the opening of the answer, and when it was.
Search reaches the whole conversation, not just the titles. Click one to load
it back into the transcript, or delete rows individually.

Text only, on this machine, in your user profile. Screenshots are never
written to disk and neither is the reader's description of one. Settings has a
switch to stop saving and a Delete all beside it; turning saving off stops new
writes and leaves what is already there until you delete it deliberately.

Answers render markdown: headings, lists, bold, inline code, fenced code blocks, and links that open in your browser. And when an answer involves a specific place on your screen, the assistant sends the pointer there: an arrow glides to the exact pixel and a speech bubble types out what to do, with the tail pointing back at the arrow.

The bubble is the control. Click it once to finish the typing, click again for the next spot, and click the last one to dismiss; the line under the message says which. Drag it anywhere if it is covering something, or close it with its X or Esc.

The input bar and the answer are separate windows. By default the answer docks under the input bar; the layout editor can split them apart (see Layout).

The first three shortcuts are rebindable in settings: press Change, then press the new combo. New combos must include Ctrl, Alt, or Win. If another app already owns the combo, settings shows an inline error and keeps listening for a different one. Changes apply immediately, no restart. `Esc` is fixed. Shortcuts are listed in settings only; the query panel stays clean.

### Model

Everything in the picker is free by default. The list is the free vision
models OpenRouter currently offers, with live context sizes, and the default
is `nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free`, which describes
itself as a perception sub-agent: a model whose job is to look at something
and report what is there, for a stronger model to act on. That is exactly the
job here: it reads the screen and answers in one call, and
a wrong answer built on a misread variable name is not a fast answer; only 3B
of its 30B parameters are active per token, so accuracy does not cost the wait
it looks like it should. `google/gemma-4-26b-a4b-it:free`
is still there if you want speed over accuracy. Free models are queued and
rate limited, so they are slower to start than paid ones; that is the trade.

Flip "Show paid" to add a second group. Those bill per token at rates where a
question costs a fraction of a cent (`qwen/qwen3.7-flash` is the one to pick),
and nothing paid is ever selected for you. Every id is checked against the
live list before it is offered, so a retired model disappears from the picker
rather than failing mid-question.

### Second model

Off by default. One vision model reads the screen and answers, which is one
call, one wait, and one thing to pick. Turning an image into text needs a
vision model either way, so this is the simple path and the one the app ships
on.

The switch exists because the strongest free model on OpenRouter is text
only. Nemotron Ultra cannot see an image, so the only way to put it behind a
screen assistant is to give it a reading of the screen instead. Turn on "Second
model" and the model above reads the screen into a short structured report,
and the second model answers from that report.

The report is not prose. It names the app, the state, what has focus, the text
that matters copied character for character, and a list of controls with their
positions on a 0 to 1000 grid.

The second model never sees those numbers. It cannot check a coordinate it did
not observe, and when it was allowed to write one it copied the wrong row or
made one up, which is what put the pointer in the wrong place. It names a
target instead, and the app looks the name up in the reader's list. A name
that matches nothing points at nothing, on purpose: a pointer in the wrong
place is worse than none.

What it costs is worth knowing. Two calls run in sequence, so the wait is the
reader's full reply plus the second model's first token, and the answer window
says which stage it is on rather than sitting on an unexplained pause. The
second model also only knows what the reader wrote down: a detail the reader
skipped is a detail it cannot use. The default second model is
`nvidia/nemotron-3.5-super-vl:free`; GLM 5.3 is in the paid group.

If OpenRouter retires the model you picked, the app notices on the next
question and moves you to the default, or to the first free model in the list
that is still offered, and settings shows the new choice.

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

To separate the answer from the input bar, drag the answer ghost clear of the dock zone, or just press its Detach button; press Dock (or drop it back near the input bar) to reattach, shown by a "Docked" badge. A docked answer follows the input bar and grows away from it (downward from a top anchor, upward from a bottom one). Press Done or Esc to keep the layout. The pill, the input bar, the answer, and future launches all use the saved positions.

### When something goes wrong

An app with no taskbar button cannot go dark to tell you it died, so it tries
not to. An unhandled error is written to `errors.log` in the settings folder,
announced once as a Windows notification, and the app keeps running rather
than disappearing. A window whose renderer dies is reloaded, because an
always-on-top blank rectangle is worse than no window. None of this should
ever fire; if it does, `errors.log` is what to send.

Quit from the tray icon, or from the settings panel (gear icon in the query panel, then Quit). No window of this app appears in the taskbar or in Alt-Tab: every one of them is a Windows tool window, so the tray, the hotkeys and the pill are the whole interface. A fresh install starts Aevaleron when you sign in to Windows, and says so once; turn it off in Settings. Upgrades keep the setting they had. The settings footer shows the app version, which is the quick way to confirm which build is running.

### Updates

An installed copy updates itself. Twenty seconds after launch, and every few hours after that, it asks GitHub for the newest release of this repository; if there is one, the installer downloads in the background. When it has arrived the tray menu grows a "Restart to update" item and one notification says so. Restart when you like, or never: the update is applied on the next quit either way. The link beside the version in the settings footer checks on demand and shows where things are. Nothing happens under `npm start`, because there is no installed copy to replace.

## Run from source

Built with Electron and plain JS/HTML/CSS. No build step beyond `npm install`, and no AI SDK: requests go straight to OpenRouter's OpenAI-compatible endpoint.

Requirements: Windows 10/11, Node.js 18+ and npm.

```bash
npm install
```

```bash
npm start
```

## Develop it (the live-reload loop)

```bash
npm run dev
```

This runs the app under a file watcher. Edit any source file (or have an AI assistant edit it) and the running app updates itself: changes to renderer HTML, CSS, or JS reload the windows in place; changes to main-process files restart the app in about a second. Leave it running while iterating; there is no manual quit-and-relaunch step.

Two rules that save confusion. Only one copy of the app can run at a time (a second launch just pings the first one), so close any `npm start` copy before starting `npm run dev`. An installed copy counts: it uses the same `%APPDATA%\aevaleron` folder as the source tree, so it shares the API key, settings, layout and history with `npm run dev`, and if it is running a dev launch quits at once and opens a capture in the installed copy instead. Quit it from the tray before developing. And the packaged installer in `dist/` is a snapshot from whenever `npm run build` last ran; a local build never updates itself, so rebuild it when you want the installable version to catch up.

### Shipping a release

```bash
npm version minor
git push --follow-tags
```

The tag starts `.github/workflows/release.yml`, which builds the installer on GitHub, creates a release named after the tag, and attaches the installer, its blockmap and `latest.yml`. That last file is what installed copies read to learn a newer version exists, so a release without it updates nobody. The tag has to match the version in `package.json`, which is why the version bump comes first.

The workflow creates the release as a draft before the build starts, and publishes it only after checking that all three files are attached. Creating it first matters: electron-builder uploads with more than one publisher at once, and each one that finds no release creates its own, which once split v1.1.0 into two releases with the installer in one and the blockmap in the other. Publishing at the end matters too, because a draft is invisible to installed copies and to the Download link at the top of this file. That link is `releases/latest/download/Aevaleron-Setup.exe`, which GitHub redirects to the newest release's asset of that name. That only works because the installer's file name has no version in it, so do not put one back.

### Testing the real installer

`.github/workflows/installer.yml` installs `Aevaleron-Setup.exe` on a clean Windows machine in GitHub's cloud and runs the app from where it landed, not from the source tree. It checks the install (exit code, Add/Remove Programs entry and version, both shortcuts, that a silent install does not launch the app), runs the smoke test and the screenshot run against the installed exe, then uninstalls and checks the files, the registry entry and the shortcuts are gone. Every push to main tests a fresh build of that commit. Every finished release tests the file the Download link serves, after checking it against `latest.yml`.

Results are on the run's summary page, with the screenshots attached as an `installer-check` artifact. The script is `tools/installer-check.ps1`. Do not run it on your own machine: it installs over and then uninstalls whatever copy is there.

### Looking at the UI

```bash
npm run shots
```

This boots the app, drives every surface into a representative state, and writes a PNG of each one to `shots/`: the pill, the query panel with and without a key, a frame caught part way through the opening glide, the conversation, the thinking state, both layout ghosts, settings top and bottom, both model lists, the panel with an area chip, an error with its Retry, a request in flight with its Stop, the history list, the area selector mid-drag, the pointer with its bubble, and one shot of the real overlay composited over the desktop.

That last one matters: `capturePage` renders the page in isolation, so it can never show the acrylic backdrop or the system shadow, because neither belongs to the page. The live shot goes through `desktopCapturer` with content protection off for a single frame (and straight back on), cropped to the overlay plus a margin, so what lands on disk is what a person actually sees and nothing else from the desktop.

It works by calling `webContents.capturePage()`, which renders the page itself rather than reading the display, so it sees windows that are invisible to screenshots (see below). It runs against a throwaway settings profile, which means it does not disturb a copy of the app you already have running. It also prints `fit.<surface>=windowN:surfaceN` lines: those two numbers should match, and a gap between them is a band of empty window showing under the panel.

Because that is a page render rather than a screen read, the run does not need the desktop to itself. `AEVAL_SHOT_OFFSCREEN=1 npm run shots` holds every window at zero opacity and click-through while it works, so you can keep using the machine and nothing of the app appears on screen. The single shot it cannot take that way is the composited `12-live-overlay`, which goes through `desktopCapturer` and therefore needs the window to actually be on the display; it is skipped, and the log says so rather than writing out a blank frame.

## Build the installer

```bash
npm run build
```

This produces `dist/Aevaleron-Setup.exe` (NSIS, one-click, per-user install, no admin prompt). The version is in the installer's metadata and in `dist/latest.yml`, not in the file name; see Shipping a release for why.

The application icon is generated, not committed. `npm run build` runs `tools/make-icon.js` first, which reads the pointer arrow straight out of `renderer/buddy.html` and `renderer/cursor-buddy.css`, rasterises it at seven sizes and writes `build/icon.ico`. So the icon is the pointer: move the path and the icon moves with it, there is no second drawing to keep in sync, and the repository stays text-only. Without it electron-builder falls back to Electron's own logo and says so on every run.

The installer is unsigned, so Windows SmartScreen shows "Windows protected your PC" on first run. That is expected for an unsigned app, not a defect. Click "More info", then "Run anyway". Code signing needs a paid certificate and is out of scope for this MVP.

## How invisibility works

The overlay, settings, and walkthrough windows all call `setContentProtection(true)`, which maps to `WDA_EXCLUDEFROMCAPTURE` on Windows. Screen shares, recordings, and screenshots taken through standard capture APIs will not include them. This is reliable on Windows 10 2004 and later, but not guaranteed for every capture backend (some hardware capture devices and older OS builds still see the window). The app keeps working either way; the overlay is just visible in those recordings.

A side effect worth knowing: the screenshots Aevaleron sends to the model also exclude its own windows, so the model sees your screen, not the assistant.

Below Windows 10 build 19041 the call does nothing at all, so the app checks the build and says so in the settings footer rather than letting you believe the overlay is hidden when it is not.

## Design notes and assumptions

- Visual constraints (palette usage, motion curves, spacing and radius scales, surface tokens, per-surface states) live in `DESIGN.md`; the stylesheets build to it. The signature motion is one decelerating curve, `cubic-bezier(0.32, 0.72, 0, 1)`.
- Acrylic was tried and reverted. Windows 11 does give a real blurred backdrop, and it looked better in an isolated test, but an opaque window stops behaving like a reliable topmost overlay and needs the layered style added back the moment `setOpacity` touches it. Depth comes from the material instead: a top-lit gradient, one crisp edge, a contact shadow and a deep soft one. The surface is opaque because at 97% a bright terminal behind it was still legible through the text on top.
- Window bounds animate; opacity does not. On Windows `setOpacity` toggles the layered window style, and driving it at frame rate put repaint artifacts on the desktop. Fades happen in CSS inside the window, and the animation runs at 60Hz rather than 120.
- Resizability is expressed through minimum and maximum sizes, never `setResizable`: on a frameless window, clearing that flag strips the thick-frame style and setting it back does not restore it. Equal limits pin a window just as firmly and are reversible. Every programmatic move unlocks the limits first, re-applies them after, and stamps itself so a drag can be told apart from the app moving its own window.
- One surface per window: the query bar has no field inside it, because a bordered input inside a bordered panel is the loudest tell that a UI was assembled rather than designed. Focus is the caret plus the panel border lifting, never a ring.
- An answer is a document, so it renders as prose directly on the surface. Only the question keeps a container, a quiet chip, which is what preserves the sense of a conversation without the app looking like a toy chat client.
- Opening animates the OS window, not just its contents. CSS can only move pixels inside a window, so a CSS scale played after Electron paints the window at full size reads as a snap followed by a wobble; that was the flash. `lib/motion.js` glides the window bounds themselves on the same curve, and the stylesheets pin each panel to the edge the window grows away from, so the growth uncovers a finished layout instead of reflowing one. Under `prefers-reduced-motion`, read once from the renderer at boot, every window transition collapses to a single setBounds.
- The pointer's arrow has its tip on its own element origin, so the transform is the target pixel with no centring maths in between. The stylesheet pulls the svg back by the offset between the path's apex and its box, because the apex is at (3.38, 0.29) and assuming it was at the origin put every point three pixels to the right.
- One coordinate system end to end: every model, whether it reads the screen or answers from a reading, gives positions on a 0 to 1000 grid, and the app maps that to the display by ratio, which sidesteps DPI scaling. Asking a vision model for pixels instead was the single largest source of misplaced pointers; it is never told how large the view it received is, so it answered on the grid it was trained on and the app divided those numbers by the image size.
- The conversation is held in memory only. The model sees up to the last 12 turns as text; only the newest message carries the screen image, so request size stays flat.
- The pointer directive is plain text in the stream (`[[point x=.. y=.. | label]]`), stripped before display. It is read loosely, because models drift: a bare pair, parentheses, a decimal, a dash before the label all resolve. One that drifts too far to read is swallowed rather than printed, since showing the user raw brackets loses the pointer and shows the machinery as well.
- Skills are read from disk once at boot and appended to the system prompt as a procedure, keeping their line breaks. A skill file that disappears between versions falls back to the default rather than silently sending nothing.
- The screen view covers the display the mouse pointer is on, and the pointer window opens on that same display. Both used to be the primary one unconditionally, which on a two-monitor desk meant reading one screen and pointing at it while the user was looking at the other. The view is downscaled to at most 1600px on its long edge, then JPEG-compressed to stay under ~1.6MB per request.
- Screen capture, the API key, and all model calls live in the Electron main process. Renderers talk to it through a small allowlisted IPC bridge and never see the key.
- An area is captured differently from the whole screen. The whole screen is asked of `desktopCapturer` at the long-edge budget directly, because decoding a 4K desktop at full size to shrink it is wasted work. An area is grabbed at the display's native resolution and cropped, so the selection keeps every pixel it had; only a selection larger than the budget is shrunk at all. The shot carries the rectangle it covers, and pointer coordinates map into that rectangle rather than onto the display.
- Model failover is one request's business only. It never writes the substitute to settings, and it never applies to a paid model: silently moving a billed request onto a different billed model is not the app's decision. It stops after three attempts, and it stops immediately once any answer text has reached the screen, because half an answer cannot be replaced by another model's half.
- History is a single JSON file rewritten whole, written through a temp file and a rename so a crash mid-write cannot leave half of one. Two hundred conversations of prose is a few megabytes; there is no case for a database, and the write is debounced so a ten-turn conversation does not rewrite the file ten times.
- The tray exists because every window here sets `skipTaskbar` and the Windows `toolbar` type, which is right for an overlay and wrong for the only copy of an application: hidden with its shortcut and with the shortcut forgotten, the app had no surface left to click. Its icon is the pointer's own arrow, rasterised from the same polygon and carried as base64, so the repository stays text-only.
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
lib/store.js               electron-store wrapper: key encryption, model, second model, skill, shortcuts, positions
lib/updater.js             GitHub release auto-update: background download, tray item, applied on quit
lib/skills.js              loads skills/*.md and appends the chosen one to the system prompt
lib/motion.js              window-bounds animation: the glide curve, anchored start rects
lib/cursor-buddy.js        pointer state machine: walkthrough steps and model-directed points
lib/region.js              area selection: the scrim window and the rectangle it returns
lib/tray.js                tray icon and menu; the way back in when the overlay is hidden
lib/history.js             conversation store: one JSON file, atomic write, debounced
lib/crash.js               catches what would otherwise end the process without a trace
lib/shots.js               visual self-check: photographs every surface (npm run shots)
skills/*.md                answering procedures; drop a file in to add one
tools/shots-run.js         runner for npm run shots (scratch profile, output in shots/)
tools/make-icon.js         generates build/icon.ico from the pointer arrow (npm prebuild)
tools/installer-check.ps1  installs the real installer, runs the installed app, uninstalls (cloud only)
renderer/index.html        the input window: pill, query bar, input layout ghost
renderer/overlay.js|css    input window logic and the shared stylesheet
renderer/output.html|js    the answer window: conversation transcript, answer layout ghost
renderer/markdown.js       built-in markdown renderer (escape first, then transform)
renderer/buddy.html|js     the pointer window: arrow, speech bubble, typing, drag
renderer/cursor-buddy.css  pointer and bubble styling
renderer/region.*          the area selector: scrim, drag rectangle, dimensions
settings/                  settings window: API key, model picker, layout editor, shortcuts
history/                   history window: searchable conversation list
electron-builder.yml       NSIS packaging config (output in dist/)
```
