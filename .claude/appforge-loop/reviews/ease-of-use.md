# Ease-of-use review: AevalBuddy v2 (everyday-user lens)

Reviewer stance: a student or office worker who has never configured an API key and does not know what
"OpenRouter", "agent" or "model" mean. Sources read: README.md, SPEC-v2.md, main.js (first run, tour, tray,
shortcuts, agent start), lib/tray.js, renderer/*, settings/*, lib/openrouter.js (error copy), lib/updater.js,
and all 36 screenshots on the `shots` branch (viewed one by one). Nothing was run; findings marked
(code) come from reading source, (shot) from a screenshot, (inferred) from how Windows or OpenRouter behave.
No product files were edited.

Bottom line: the surfaces are polished and the individual states (error rows, confirm card, empty Find) are
well written. The problem is the front door and the mental model. A newcomer needs roughly 8 to 12 minutes and
25-plus clicks across three trust cliffs (unsigned installer, a third-party account, an API key) before the
first answer, and then meets a bar that asks them to pick one of three modes whose names they cannot guess.

---

## Ranked findings

Impact scale: P0 = people abandon here; P1 = people get stuck or distrust it; P2 = friction or polish.

### P0-1. The API key is the cliff, and the app helps with none of it

Journey today: tour pointer opens Settings and says "Paste an OpenRouter API key here and press Save. A free
key works." (shot 28-onboard-1). Nothing says what OpenRouter is, why a key is needed, where to get one, or
how long it takes.

Friction, in order:
1. "OpenRouter", "API key" and "key" are unexplained jargon. The only help is a small "Get a free key" link
   (code: settings.html) and, in the README, one sentence.
2. The settings window is pinned above everything at the screen-saver level (code: main.js pinOnTop, no
   blur-to-close). When the person clicks "Get a free key", the browser opens underneath AevalBuddy's own
   window. They must drag settings aside to read the OpenRouter page, and with no taskbar button or Alt-Tab
   entry (by design) there is no obvious way back to settings after they switch. (inferred; verify on a runner
   with a window screenshot.)
3. The OpenRouter side has its own steps the app never describes: create an account (often email
   verification), open Keys, Create, name it, copy. Honest estimate for a first-timer: 4 to 8 minutes.
4. Free models need an OpenRouter privacy opt-in ("Free model publication"). The app knows: it has a special
   error for it (code: friendlyError). But the first question is where the person learns this, after
   already believing the setup worked ("Key works" in the key test).
5. The tour dismisses on any click elsewhere (code: settings.js, overlay.js click handler to `buddy:dismiss`).
   Someone who clicks around the settings window to figure out what a key is cancels the tour, and the
   README says it never auto-runs again.
6. No clipboard assist. Everyone has to find the field, paste, press Save, then Test.

Proposals (cheapest first):
- Replace step 1 with a three-line card in plain words: "AevalBuddy needs a free pass to use AI models. It
  takes about two minutes and costs nothing." Button: **Get my free pass** (opens the key page). Hide the word
  "API" in the first run; show "key" only in the field label.
- Watch the clipboard while that card is open. When it sees text starting `sk-or-`, fill the field and say
  "Found your key. Save it?" with one button. This removes the paste, the field hunt and the Save/Test pair.
- Run the Test automatically on save, and fold the free-model opt-in into it: make one tiny free call; if it
  returns the data-policy error, show "One more switch on OpenRouter" with a button to that exact page and
  auto-retry when they return to the app window (focus event).
- Close or move Settings out of the way when it opens an external link (or drop it to normal z-order for the
  duration), and bring it back with the tray or a notification action.
- Give the tour a "pause" instead of "cancel" on click-away, and re-offer it once on the next launch if the
  key is still missing.
- Longer term: a "try without a key" path (Find works literally offline, but nothing teaches it; see P1-7).

### P0-2. Three modes is exactly the learning curve the owner wants gone

Evidence (shots 03, 18, 21; code: overlay.js MODES):
- The mode is a faint word at the left of the bar ("Ask", "Find", "Agent"). It is not styled as a control
  (SPEC 7.2 forbids a segmented control), has no arrow, and nothing says it is clickable beyond a tooltip that
  only shows on hover. A newcomer reads it as a label.
- Switching is Tab (on an empty bar), Ctrl+1/2/3, or three different global hotkeys. Tab does a different
  thing in Find once text exists (toggles "This window / Whole screen"), so the same key means two things.
- "Agent" is jargon. The tour does not mention it until step 4 of 4, and the README Use-it table is the only
  place the three verbs and five shortcuts are explained side by side.
- The placeholder is half a sentence that only works with the mode word: "anything about your screen"
  (shot 03). Without the faint "Ask" in front, it reads as a fragment. In Find it is "a word or an idea on
  screen" and in Agent "describe a task".
- Wrong-mode cost is high: typing "rename these screenshots by date" in Ask returns advice, not action; typing
  a question in Find shows a word search that finds nothing.
- Five global shortcuts to remember. Most people will remember at most one.

See the section "Unified input proposal" below. This is the headline change.

### P0-3. Nothing says what leaves the computer

The README's only privacy statements are: windows are hidden from screen share, history is text-only and local,
screenshots are not written to disk. It never says that each question sends a screenshot of the screen to
OpenRouter and on to a model provider, or that free models are free partly because providers may log or train
on prompts (OpenRouter's "Free model publication" opt-in is precisely that consent, and the app asks people to
turn it on while saying nothing about it). Settings > Privacy (shot 09) lists History and Screen share only.

An everyday user with an email, a bank page or a school portal on screen needs one plain paragraph before the
first capture. This also matters for the agent, whose screenshots may contain passwords on screen (the spec
redacts password fields in UI trees, but not in screenshots).

Proposal: a one-time line at the end of key setup ("When you ask, AevalBuddy sends what is on your screen to
OpenRouter's AI models. Free models may keep it. Use paid models if that matters.") plus a Privacy row that
says the same, and a "Pause capture" quick toggle in the tray. Add the same to the README above "Use it".

### P0-4. After install there is nothing to look at

Journey: README step 1 ("opens the app. No admin prompt, no wizard") is honest and good. But what the person
sees next is only a 72-ish px pill (shot 01) in a corner and a pointer that opens a settings window one second
later. No taskbar button, no Start-menu welcome, no window that says "AevalBuddy is running here".
- The tray icon lives in Windows' overflow (^) by default on a fresh profile. README tells people to look "by
  the clock", which is where it is not (inferred). Say "click the small arrow by the clock".
- Launch at login is off by default (code, shot 09). After the first reboot the app is simply gone, with no
  taskbar trace and no memory of how it got there. The person has to search the Start menu to find out it
  exists. For a no-taskbar tool this is the most likely way to lose a satisfied user.
- README has no screenshot, GIF or 15-second demo anywhere. A downloader deciding whether to trust an unsigned
  exe has only prose.

Proposals: ship a 20-second GIF under the download button; after the first successful answer ask once "Keep
AevalBuddy ready when you turn on your PC?" (Yes / Not now) instead of burying it in General; add a tray
balloon on first launch ("I'm in the tray. Click the arrow by the clock, or press Ctrl+Shift+Space").

### P1-5. SmartScreen and browser warnings (handled, but still scary)

The README copy is the best part of this journey: it names both screens and says "not a virus warning". Gaps:
- Edge and Chrome show a download warning with a second "Keep anyway" step on many installs. README says
  "choose Keep", one step. Show the "..." menu location or a picture.
- Nobody can verify the claim "not a virus warning" from the screen itself. Add the SHA-256 of the exe and
  one sentence on how to check it, or a link to the release checksums, for the careful minority.
- Show three small screenshots in the README for More info, Run anyway, Keep. People follow pictures, not
  prose, at this moment of doubt.
- The README is 33 KB and, after the download block, speaks developer ("perception sub-agent", "0 to 1000
  grid", "second model", "skills"). The download-to-first-question path is in the first 25 lines; keep it
  there and move everything below to a "How it works" page.

### P1-6. Free-model waits look like a freeze

Shots 05 and 15: three grey dots and a Stop button, nothing else. README itself says waits of half a minute
are ordinary. A person who sees dots for 20 seconds assumes it is broken and presses Stop or asks again.
The status line that names the fallback model (code) is not shown in these frames, and "model" names mean
nothing to this audience anyway.

Proposals: after 6 seconds show "Free models queue up. Usually 10 to 30 seconds." with a gentle elapsed count;
after a failover, "Switching to a faster one". Offer a one-click "Use a faster paid model" only after the
second wait over 20 seconds, with the cost in plain terms ("about 1 cent per question").

### P1-7. A global hotkey that fails to register is silent at launch

Code: main.js logs a failed registration with `console.warn` only; in a packaged GUI app that goes nowhere. The
tour's step 2 then says "Press Ctrl+Shift+Space and ask anything" and nothing happens if another tool owns it.
The "Taken by another app" warning exists only inside Settings > Shortcuts (shot 09 shows the shortcut row, not
the warning).

Proposal: if the main shortcut fails, say so in the tour and on the pill ("Ctrl+Shift+Space is taken. Click
here instead, or pick another key") and open the rebind row.

### P1-8. Error recovery: wording is good, but half the fixes are out of reach

| Situation | What shows today | Problem for an everyday user | Fix |
| --- | --- | --- | --- |
| No key (shot 02) | "No OpenRouter API key saved yet." + **Open settings** | Fine, but assumes they know what the key is. The bar looks ready, so the first Enter produces this. | Say "Not set up yet. Finish setup (2 min)" and re-run the guided key card, not the raw settings. |
| Rate limit (shot 14) | "Every free model is busy... retry in a moment, or add credit and pick a paid model in Settings." | Asks them to pay and then to know what "pick a paid model" means. Retry is the right action; the money suggestion lands too early. | Auto-retry once silently with a visible timer; mention paid only on the second failure. |
| 402 / 404 / 400 | Names the model id, "Free group", "Model list", and, in the 400 text, "Use a separate brain" (code: friendlyError) | "brain" is a stale term; Settings now says "Answer model". Model ids are noise. | Remove model names from user text; one verb: "Open model settings". Fix the stale word now. |
| Free-model privacy opt-in | "enable 'Free model publication'" | Correct but names a setting the user has never seen, on a website they have to navigate. | Deep link to the exact page; auto-retry when they return. |
| Offline | Bar: "No connection. Ask works again when you are back online." (code) | Reads as if only Ask is affected, and the bar notice is shown before they try. In a unified bar this should say what still works. | "You're offline. Finding words on your screen still works." |
| Find offline (shot 27) | "2 exact / Related matches need a connection." | Good. "exact/related" is jargon, see P2. | "2 matches here. Similar ideas need internet." |
| Agent blocked by admin window (shot 25) | "that window runs as administrator, and AevalBuddy does not. Start Outlook normally and retry." with a Retry button | Most people cannot start Outlook "normally" or know what "administrator" means, and Retry will fail identically. | "Outlook is open in a special mode that blocks me. Close it, open it from the Start menu, then try again." Disable Retry until the window changes. |

### P1-9. The Agent: scary parts are well handled, a few dead ends remain

What works: the consent sheet (code: trail.js) is short and plain; the confirm card (shot 23) shows the action,
the reason, and Allow / Skip with keys; Esc, the Stop button, the tray, and mouse movement all stop it. Good.

Gaps:
1. The task text is lost if they press "Not now" on the consent sheet (code: overlay.js clears the input on a
   `consent` result and `agent:consent` with allow=false discards `pendingAgent`). The person typed a sentence,
   was asked a question they did not expect, said "not now", and must type it again.
2. The first thing a person sees on pressing Enter is a permission dialog about "using your computer", before
   any evidence of value. Show the plan first (dry run: "Here is what I would do"), then ask for permission at
   the first click, with the plan as context.
3. The word "Agent" appears in the trail header (shot 22 "Agent Working · step 5"), settings, tray ("Do a
   task…") and tour. Three names for one thing (the team already logged this as D2). Pick one verb.
4. No undo. After "Done" (shot 24) there is no "Show me what changed" and no "Undo". At least say "Ctrl+Z in
   Notepad undoes the last step" or offer the opening of the saved file.
5. "Do it" under answers is hover-only (code: output.js, appears with Copy), so nobody finds it by looking.
6. Takeover pause ("Paused: you took over. Enter to continue") makes Enter a global shortcut while paused
   (code: applyRegistrations). If they have gone to type in another app, their Enter is swallowed. Keep, but
   surface a visible Resume button.
7. Steps use verbs such as "Read Notepad", "Switch to ..." and quoted control names, which is fine. The pill's
   "Step 5" is not (shot not available): it should say "Working... 5 of about 6".

### P1-10. Updates are silent in a way that can mean never

Code: electron-updater downloads in the background and installs on quit (`autoInstallOnAppQuit`). The user is
told by one silent toast ("It installs the next time you quit") and a tray item. This app has no taskbar
button, no window to close, and launch-at-login is off, so people almost never "quit" it; they shut down or
sleep. Windows fast startup and hibernate can keep the process alive across "shutdowns". Result: some users
run an old build for weeks and never know.

Proposals: when an update is ready and the bar is idle, show one line in the bar ("Update ready. Restart
now / Tonight"), and auto-apply on the next idle collapse after N hours. Keep the toast but make it a normal
(non-silent) one. After an update, the existing "updated to X" toast is good; add what changed in one line.

### P2-11. Settings: too many words for a tool with one required setting

Shots 08, 10, 10b, 29, 30:
- Eight sections (Account, Models, Answers, Find, Agent, Privacy, Shortcuts, General) with four model pickers,
  all showing NVIDIA model names the user cannot rank. "Screen model" says "Looks at your screen and answers"
  but "Answer model" (shot 10b: "Let the screen model read, and a bigger model answer") is a different job. An
  everyday user cannot tell which to touch and will touch none. That is fine; make it explicit.
- "Answer model: Off" is a real setting that reads like a missing feature.
- Everything is closed behind "Edit". It is calm, but every change is two clicks, and nothing signals that
  "Connected" is the one setting that matters.
- Quit is in the footer of Settings and in the tray; there is no "Help", "Send feedback", or "Report a
  problem". The README says `errors.log` is "what to send", with no address.

Proposals: three groups visible by default (Account, Privacy, General) and a single "Advanced" fold holding
all model pickers, Find tuning and Agent limits. Replace four pickers with one "Speed" choice (Free and
slower / Faster, a few cents) and move the rest to Advanced. Add Help (opens a one-page "what can I type"), and
a "Copy diagnostic info" button.

### P2-12. Find: dead ends and jargon

- Empty state (shot 26): "Nothing on screen matches 'zebra'. No related matches either." and no next step. It
  should offer "Ask about it instead" (the code has an `Ask instead` action for one state; the empty state
  should always carry it).
- "2 exact · 2 related" (shot 18), "This window" scope link that Tab toggles: jargon and a hidden shortcut.
  "2 matches · 2 similar" and a plain "Look at: this window / all windows".
- Enter in Find means "next match" and Ctrl+Enter means "ask about this". People hit Enter to submit; they get
  a ring jump. In a unified bar that collision must be resolved by intent (below).
- Related matches need a key and a model; offline or keyless, Find silently degrades. The bar should say so once.

### P2-13. Answer window and bar polish that affects first impressions

- Empty-Enter sends "What's on my screen and what should I do?" automatically (code). Nice for experts, but a
  new user who taps Enter to see what happens triggers a capture and a model call with no warning; say so in the
  placeholder ("Press Enter to ask about everything on screen").
- Region (crop) button: icon only, tooltip only. Text "Pick an area" on first run would help; the chip
  ("Asking about a 640 x 320 area", shot 13) is clear.
- Pill reads "Ask" (shot 01). In a unified model the pill should carry the mark only.
- Layout "HUD" editor and "Docked / Detach" (shots 06, 07) are power-user concepts; fine, but hidden under
  General > Window layout > Move. Keep hidden.

---

## Journey walk-through (friction log)

### J1. Download to first question

1. README opens with a one-line description, then a blue Download button. Good. (README)
2. Browser warns the file is uncommon: needs "..." > Keep > Keep anyway on Edge. README says "choose Keep".
3. SmartScreen: More info > Run anyway. README covers it, text only, no picture.
4. Installer: no UI, a few seconds, app starts. Nothing on screen explains where it went. (P0-4)
5. After about one second a pointer glides onto Settings, which opens by itself (code: startWalkthrough). It
   is a pleasant surprise for some and an alarming one for others, since a window appears that the person did
   not ask for.
6. "Paste an OpenRouter API key". They have none. Branch to the browser, account creation, copy. (P0-1)
7. Back in the app, find the field, paste, Save, Test. The tour moves on only when the key saves.
8. Tour step 2: "Press Ctrl+Shift+Space and ask anything about your screen." Bar opens with a screen capture
   already taken. They type, press Enter.
9. Wait 10 to 40 seconds under three dots (P1-6). Answer streams. Pointer may glide to something. First
   success, 8 to 12 minutes in.
10. Tour step 3 and 4 introduce Find and the agent by hotkey only. There is no practice target and no way to
    tell whether the Find step has a "word you can see" in the context of the person's open window.

Dead ends: click-away cancels the tour for good; missing hotkey is silent; closing the browser tab with the key
page loses the key (OpenRouter shows it once). Say so: "Copy it now. OpenRouter will not show it again."

### J2. Finding something on screen

Today: Ctrl+Shift+G (or Tab twice), type a word, rings appear, Enter jumps, Shift+Enter goes back, Ctrl+Enter
asks. Related matches arrive a second or two later with a reason chip (shot 19, 20). That is good.

Friction: knowing G exists; understanding "exact vs related"; Enter conflicts with Ask; the results list and
the lens overlap the content they are highlighting; and no way to say "where is the Save button?", which is
how people phrase it (that currently goes to Ask, which points, but it feels like a different feature).

### J3. Asking the agent to do a task

Today: Ctrl+Shift+U (or Tab to Agent), describe, Enter, consent sheet once, plan, steps with a moving pointer,
confirm card for risky steps, Done. The trail (shots 22 to 24) is understandable and calm. Friction is in
P1-9: lost task on "Not now", no undo, naming, hover-only "Do it", the Retry that cannot work (shot 25).

### J4. Recovering from errors

See the P1-8 table. The pattern to carry forward: every error names what happened, states the fix in actions
a non-technical person can take in the app, and offers one button. The current copy is close; it is the actions
that point to places the person has never been (OpenRouter settings, "Free group", administrator mode).

### J5. Finding settings, updating

Settings is reachable from the gear in the bar, from the tray, and from "Open settings" on the key notice. It
cannot be found from the taskbar, from the Start menu (after install it is only "AevalBuddy", which starts the
app), or from the pill. Updates are in Settings > General > Updates and the tray (P1-10). Check for updates in
the tray is fine. There is no "Help" or "About" anywhere.

---

## Unified input proposal: one box, no modes, no learning curve

### Principles

1. One surface, one hotkey: Ctrl+Shift+Space opens the bar. Everything else is typed. (Keep Ctrl+Shift+A for
   area; keep G and U only as optional power-user bindings, off the README front page.)
2. Guess locally and instantly, with no model call. A 30-line rule scorer runs on every pause in typing.
3. Cost-asymmetric defaults. Ask and Find change nothing, so a wrong guess costs seconds. Do changes things,
   so it is never the silent default: it must be earned by a clear imperative, and it still shows its plan
   first. When unsure, Ask.
4. The screen is a signal: if the typed text matches words on screen, the person probably meant Find.
5. Show the guess, never hide it, and make the correction a single keystroke with the text kept.

### How the guess works

| Signal | Points to | Example |
| --- | --- | --- |
| Ends with `?`, starts with what / why / how / who / when / which / can / is / does / should / explain / summarize / translate / tell me | Ask | "why is this red?" |
| Starts with find / where is / where's / show me / highlight / search for / look for / jump to, or is quoted | Find | "where is the Save button", `"invoice"` |
| Short (1 to 3 words), no verb, and literal hits exist on screen right now | Find | "invoice", "Total due" |
| Starts with an action verb (open, click, press, type, rename, move, delete, send, save, close, create, fill in, book, turn on/off, install, download, attach, log in) or contains "for me", "then", "and then" | Do | "rename these screenshots by date" |
| "do it", "yes do that", "go ahead" right after an answer that listed steps | Do (carries that answer as context) | follow-up to a how-to |
| Long, conversational, or none of the above | Ask | "I'm trying to write a cover letter and..." |
| Short phrase, zero hits on screen | Ask, with a note | "refund policy" |
| Tie or low confidence | Ask | everything else |

Tiebreakers: a follow-up inside a live Ask conversation leans Ask; a bare noun that also begins with an action
verb ("save" alone) is treated as Ask unless it has an object ("save as notes.txt"); "write me a haiku" is Ask
(text reply), "open Notepad and write a haiku" is Do (has "open ... and").

### What the person sees

- Left of the input, where "Ask / Find / Agent" is today, a quiet word that changes with the guess and
  crossfades 150 ms after typing pauses (not per keystroke, so it never flickers): `Ask`, `Find`, `Do`. Rename
  Agent to Do everywhere (tray, settings, trail).
- The Enter hint at the right edge of the bar changes with it: "Enter: answer", "Enter: next match",
  "Enter: show the plan". The hint is how the person learns what will happen before it happens.
- Find intent lights up matches on screen live, as typing happens, exactly like Ctrl+F, because it is local and
  harmless. That is the zero-click demonstration of the feature: type "invoice" and the screen answers.
- Do intent never types or clicks on guess. Enter shows a short plan card ("I would: open Notepad, type a
  haiku. Go / Just explain"). Go is the first-time consent as well; after the person has approved a few runs,
  low-risk tasks may start on Enter with the plan streaming in the trail as now, while risky steps always
  confirm.
- Clicking the word, or pressing Tab, cycles through the next-best guesses (ranked), not a fixed mode list.
  A tiny menu on click gives three plain descriptions: "Ask: get an answer", "Find: light it up on screen",
  "Do: have me do it".
- Typed prefixes for people who know: `?` Ask, `/find`, `/do`. Not taught in first run. Listed in Help.

### Placeholder copy

- Idle bar: "Ask, find, or tell me what to do" (works with no mode word, so it fixes the fragment problem).
- After the first week or when empty and idle for 4 s, fade through examples, one at a time, drawn from what is
  in front of the user (foreground window title/process via UIA is already available in v2): in a browser
  "Summarize this page"; in Excel "What is the total of this column?"; in an email "Draft a reply"; otherwise
  "What does this error mean?" / "Find 'invoice'" / "Rename these files by date".
- Empty-state chips under the bar for the first 5 opens, each clickable and each filling the box (so the chip
  teaches the guess): `What's on my screen?`  `Find "total"`  `Open Notepad and write a haiku`. They vanish after
  first use or on dismiss, and return from Help.

### First-run teaching (replaces tour steps 2 to 4)

1. Keep one guided step for the key (see P0-1), with the clipboard assist.
2. One practice step with the bar open: "Type what you want. I'll work out whether to answer, find it, or do
   it." Chips below as above. Advance on the first thing sent, whichever intent it was.
3. Teach each verb in context, once, the first time it fires, as a one-line toast under the bar (never a modal):
   - First Find: "Highlighted on your screen. Enter for the next match, Esc to clear."
   - First Do: "I'll show the plan first. Esc stops me any time."
   - First Ask: nothing; it is the default and needs no teaching.
4. After the first success, one question: "Keep AevalBuddy ready when you turn on your PC?"
5. Skip the pointer for steps 3 and 4. Teaching by doing beats a tour of shortcuts the person cannot practice
   against their own content.

### Handling wrong guesses (no dead ends, nothing retyped)

- Before sending: the guess is visible; Tab or a click on the word picks another; the text is untouched.
- After sending: results carry one quiet line at the top, in the answer's meta row: "Answered. Find this on
  screen instead" or "Highlighted 3 matches. Ask about it instead" or "Plan shown. Just explain". Each one
  re-runs with the same text and the other intent.
- Find finds nothing: roll to Ask automatically with "Nothing on screen says 'refund'. Asking instead..." (no
  empty state that ends).
- Ask returns steps: a persistent (not hover-only) "Do it for me" button under the answer.
- Do was a wrong guess: the plan card has "Just explain" as the second button; no action has run yet.
- A Do that already acted wrongly: Esc stops, the done card says what changed and suggests Ctrl+Z in that app.
- The scorer learns locally: if the person overrides the same leading verb twice, remember it. No telemetry.
- Log the override reason in the local store so later tuning has data (counts only, no text).

### Pill, tray, shortcuts to match

- Pill: mark only, or "AevalBuddy". Drop the "Ask" word, since there is no mode to announce. While a Do is
  running show "Working" and a step count, as now.
- Tray: "Ask or do something…" (opens the bar), "Ask about an area…", "Hide the overlay", "New chat",
  "History…", "Settings…", "Help", "Check for updates", "Quit". Find and Do disappear as separate items; the
  bar handles them. Keep them as optional bindings in Settings > Shortcuts under Advanced.
- README Use-it table: two rows (open the bar, pick an area) plus Esc. Everything else is "just type".

---

## What a person would expect that is missing

1. A demo: a GIF or short clip in the README, and a picture for each warning screen.
2. A privacy paragraph before the first capture (P0-3), and a one-click "pause capture" (tray) for sensitive
   moments, since the pill is always ready.
3. Not needing an account: a no-key mode that works with Find (it already can) and a clear label for what is
   unavailable, so the app is useful in the first minute.
4. A visible "I'm here" after install and after every reboot (launch at login prompt, tray balloon, a
   Start-menu entry that opens the bar rather than doing nothing visible).
5. Help: a "?" in the bar and tray that shows what to type, with the examples above. Today help is typing the
   literal phrase "help me" (README), which nobody can discover, and it only restarts the tour.
6. A way to report a problem: "Copy diagnostics" and a link, not "send errors.log".
7. Undo or at least a changelog for the agent's work, and a visible Pause button, not only Esc.
8. Voice input (hold a key and speak) is what many non-technical users expect from a screen assistant; at
   minimum an acknowledgement that it is not here yet. Also: drop a file or image into the bar, and copy-as-text
   for Find hits.
9. A plain-language model choice. Most people want "Free, slower" or "Faster, a few cents", not model names.
10. Cost and usage visibility if they ever add credit: "This month: 14 questions, 3 cents".
11. Where is my data: a "Delete everything and reset" row (Delete all is history only).
12. A clear statement of platform: Windows only is in the first line of the README; a Mac or Chromebook visitor
    still downloads before learning it does not apply. Put it on the button.

---

## Suggested order of work

1. Guided key flow with clipboard assist, privacy line, Settings z-order fix, and the stale "separate brain"
   text. (P0-1, P0-3) Removes the largest drop-off.
2. Unified bar: local intent scorer, live guess word, Enter hint, Tab cycles guesses, one hotkey, rename Agent to
   Do, replace tour steps 2 to 4 with practice chips and in-context toasts. (P0-2)
3. Install trail: README GIF and warning pictures, tray balloon, launch-at-login prompt after first success.
   (P0-4, P1-5)
4. Waiting and failure: elapsed message for free-model queues, silent auto-retry, actionable agent errors,
   Find empty state rolls to Ask, hotkey-failed notice. (P1-6, P1-7, P1-8, P1-9)
5. Update nudge in the bar, settings collapse to three groups plus Advanced, Help, diagnostics. (P1-10, P2-11)
