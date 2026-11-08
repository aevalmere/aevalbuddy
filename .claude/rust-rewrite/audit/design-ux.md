# AevalBuddy design review: ease of use, accessibility, power user, trust

Branch `rust-rewrite`, reviewed 2026-11-07. Read-only: renderer (`renderer/`, `settings/`, `history/`),
the Rust behind it (`src-tauri/src/`), `DESIGN.md`, `REQUESTS.md`, the `rust-d-ui/latest/` shots (Rust
renderer, mock bridge) and the Electron `latest/` set including the `m-*` contrast matrix. The app was not
run. Bar for every finding: better than Apple, for a non-developer who downloads the .exe.

Constraints honoured in every fix below: REQUESTS #3 (suggestions only until the first request), #6 (the
Ask/Find/Do key hints live on the left token only), #9 (no input in output windows; the bar is the only
field), #11 (free-floating output windows).

---

## 1. Verdict per lens

**Ease of use: not there yet.** The bar itself is good: one field, a live token that says what Enter will
do, a guessed task that can never run on Enter, honest queue messages. The failures are around it. Nothing
answers until the user leaves the app, makes an OpenRouter account and key, and flips a privacy switch on
openrouter.ai; the key test then says "Key works." even when that switch is still off, so the first
question fails. Esc, the most natural "get this out of my way" key, kills the answer that is still
streaming and hides every window. With several output windows open, nothing says which conversation the
next question joins, and a second question silently stops the first answer. The tour still prints
"Press  and type a word you can see." with a blank where the shortcut should be.

**Accessibility: solid foundations, five WCAG AA failures.** Good: two-tone focus ring, solid-colour
label tokens with a measured scrim (placeholder 5.3 to 9.7:1 across all nine `m-bar` shots), reduced
motion turns the glob into a fade, forced colours handled on most surfaces, the region selector works from
the keyboard, confirm cards take focus and need a deliberate Tab. Failures: text cannot be enlarged at all
(zoom blocked, px type, Windows Text size ignored: 1.4.4); the accent used as text measures 2.9 to 3.9:1
(1.4.3); a screen-reader user hears nothing when an answer arrives, because it is announced from a window
that never has focus; answer markdown has no headings, paragraphs or real links (1.3.1); History nests a
button inside a button and Settings' fifteen disclosures carry no expanded state (4.1.2).

**Power user: fast core, thin keyboard layer.** Prefixes, Ctrl+1/2/3, Alt+Down, Up-arrow recall, F6 and
hotkey conflict detection exist. Missing: new conversation, close/switch/copy/stop/retry from the
keyboard, a per-question model choice or "retry with another model", choosing between several providers,
History keyboard navigation and reuse, and any measurement of hotkey-to-bar latency (and nothing stops the
first keystrokes after the hotkey from being dropped).

**Trust: careful in the agent, weak at the edges.** The safety gate, the 700 ms confirm settle, the bold URL
host, the first-run consent sheet, the "AevalBuddy is using your computer, Esc to stop" chip and "Stopped.
Nothing else was done." are better than most shipping agents. But four disclosure strings hard-code
OpenRouter, so once an Anthropic key answers, the consent sheet tells the user their screen goes somewhere
it does not. Keys are never shown even masked, and if Windows cannot provide a key store they are saved in
plain text without a word. No answer says what was sent (screenshot or window text, which provider, which
model). The consent copy says moving the mouse stops a task; it pauses it.

---

## 2. Findings, most impactful first

Format: **ID. Title** (lens, severity). Surface and shot. Problem. Fix spec. Scope: renderer-only or Rust.
Severity: S1 blocks or misleads users, or fails WCAG 2.2 AA; S2 friction; S3 refinement.

Counts: **S1 13, S2 19, S3 20** (52 findings).

### S1

**U1. No answer is possible without an outside detour, and the key test misses the block that will fail
the first question** (Ease, S1)
Surfaces: tour step 1 (`28-onboard-1.png`), no-key bar (`02-panel-nokey.png`), Settings Account
(`rust 08-settings-account.png`).
Problem: `store::has_api_key()` gates every Ask (`host/ask.rs:368-371`). A non-developer must open
openrouter.ai, sign up, create a key, copy it, come back, paste it, and separately turn on "Free model
publication" (`settings.html:55-59`). The OpenRouter key test only reads the credit endpoint
(`llm/client.rs:554-560`), so it reports "Key works." while the privacy switch is still off; the first
question then fails with the DATA_POLICY text (`llm/errors.rs:104-107`). The tour forces the key first, so
the first-run experience is homework, not value.
Fix:
1. **Connect OpenRouter in one click (OAuth PKCE).** Settings and the tour show a primary button
   `Connect OpenRouter`. Rust (new `auth/openrouter.rs`): bind a loopback listener on `127.0.0.1:<free
   port>`, open `https://openrouter.ai/auth?callback_url=http://localhost:<port>/callback&code_challenge=<S256>&code_challenge_method=S256`
   in the default browser, read `code` from the callback, POST `https://openrouter.ai/api/v1/auth/keys`
   `{code, code_verifier, code_challenge_method:"S256"}`, store the returned `key` with
   `set_provider_key("openrouter", ..)`, answer the browser tab with a plain page "AevalBuddy is connected.
   You can close this tab.", emit `key-saved`. Timeout 5 min: `Didn't hear back from OpenRouter. Try
   again, or paste a key instead.` (Verify the current OpenRouter PKCE parameters before building.)
   Paste stays available as `Paste a key instead`.
2. **Test what will actually be used.** After the credit check, `test_key(OpenRouter)` sends one
   `max_tokens: 1` request to the default `:free` vision model. A data-policy refusal returns status
   `blocked` with the text in the copy deck and an `Open OpenRouter privacy settings` action; Settings shows
   the row state "Free models off" (section 3).
3. **Lead with something that works without a key.** Reorder the tour (U10): step 1 is the hotkey plus
   Find, which is local and needs no key; step 2 is "Connect a provider to get answers"; step 3 is Ask.
Scope: Rust (auth flow, test probe, tour order) and renderer (Settings button, states).

**U2. Several providers' keys cannot be held, seen or chosen; the one field mis-files keys** (Ease and
Trust, S1; REQUESTS #2 not met in the UI)
Surface: Settings Account (`rust 08-settings-account.png`, `settings.html:47-83`).
Problem: Rust stores four keys (`store/mod.rs:381-473`) and `settings:save-key` files any pasted key under
the provider its prefix names (`ipc.rs:511-535`), but the page has one field labelled "OpenRouter API key"
with placeholder `sk-or-v1-...`, a summary that only says "Connected", and Remove that only clears
OpenRouter (`settings.js:144-149`). Paste an Anthropic key and the UI claims OpenRouter is connected. The
provider order (`llm/settings.rs:48-57`) and per-provider models are unreachable ("no UI writes them yet",
ARCHITECTURE.md line 589).
Fix: the full spec is section 3. Scope: renderer and Rust (new IPC listed there).

**U3. Esc stops the answer that is still streaming and hides every output window** (Ease and Trust, S1)
Surface: bar and every output window (`rust 03-output-answer.png`, `05-outputs-multi.png`).
Problem: Esc on an empty bar, and Esc anywhere in an output window (`output.js:574-576`), call
`overlay:collapse` (`ipc.rs:375-381`), which runs `collapse_panel(app, false)`: `abort_query` and
`outputs::hide_all` (`windows/mod.rs:489-492`). The global Escape path does the same (`windows/mod.rs:679`).
With free models queueing for 20 to 30 s, a user who presses Esc to keep working loses the answer, and
every other conversation window vanishes too. Output windows are free-floating since REQUESTS #11, so
tying them to the bar is a leftover of the docked layout.
Fix:
- `collapse_panel` never aborts. Remove the `abort_query` call; streaming continues, the orb shows busy.
- Collapsing the bar leaves output windows where they are (drop `outputs::hide_all` from collapse; keep it
  for the `toggleVisibility` chord, whose job is "Hide the overlay").
- In an output window, Esc moves focus back to the bar if it is open (`ui:focus-bar`), else returns focus to
  the app that had it (new IPC `output:blur`: Rust `SetForegroundWindow` on the last non-AevalBuddy HWND).
  Esc never closes, hides or stops anything there. Close is Ctrl+W and the close button; stop is Ctrl+. and
  the Stop button (U19).
- Record the departure from PARITY POS-09 in DESIGN.md.
Scope: Rust (`windows/mod.rs`, `ipc.rs`) and renderer (`output.js:574-582`).

**U4. A new question silently cuts off the answer streaming in another window** (Ease and Trust, S1)
Surface: multiple outputs (`rust 05-outputs-multi.png`).
Problem: `submit_to` starts with a global `abort(app)` (`host/ask.rs:372`) over a single `current()` slot.
The interrupted window gets an abort, which maps to a bare `query:done {}` (`host/ask.rs:688-694`): the
answer stops mid-sentence with no explanation. Several windows promise several conversations.
Fix: key `current()` by output label (`HashMap<String, Current>`); `abort(label)` only cancels that
window's query; `query:cancel` from a window cancels its own; a new question in window B never touches A.
Allow two concurrent answers; a third waits with the stage label `Waiting for your other answer to
finish`. If single-flight has to stay for now, the cut window must get a note row (neutral style):
`Stopped because you asked something in another window.` with a `Continue` button (`query:retry` with that
label).
Scope: Rust (`host/ask.rs`, `state.rs` busy flag becomes a count) and renderer (note row).

**U5. With several output windows open, nothing shows where the next question goes** (Ease, S1)
Surface: bar plus three windows (`rust 05-outputs-multi.png`).
Problem: a question continues the last-focused window (`outputs::target`, `windows/outputs.rs:98-126`). In
the shot the bar is identical whichever window is the target. A follow-up can land in the wrong
conversation and be answered with the wrong context, which misleads.
Fix:
- Rust adds `target: {label, title, kind} | null` and `visibleOutputs: n` to `overlay:state`.
- The target window gets `body[data-target="1"]` while the bar is open; style
  `body[data-target="1"] #opanel > .gl::after { background: linear-gradient(var(--accent-text), var(--accent-text)); padding: calc(var(--px)*1.5); opacity:.9 }`.
- When `visibleOutputs >= 2`, the bar shows `#target-chip` between the field and the area button: text
  `Continues: <title>` (28 characters, ellipsis, full title in `title`), and a 24 px x button with
  `aria-label="Start a new conversation instead"` that sends `chat:new`. One window: no chip, rim only.
- Input `aria-describedby` gains `Continues the conversation "<title>".`
- Keys: Ctrl+N new conversation, Ctrl+Tab / Ctrl+Shift+Tab cycle the target (new IPC
  `output:cycle-target {dir}`); clicking a window still retargets it (`outputs::touch`).
Scope: Rust (state field, cycle IPC) and renderer (`index.html`, `bar.js`, `overlay.css`, `output.js`).

**U6. The disclosures say OpenRouter even when another provider receives the screen** (Trust, S1)
Surfaces: agent consent sheet (`trail.js:427`), Privacy "Who sees your screen" (`settings.html:273`,
`rust 07-settings-fullscreen.png`), bar notices (`overlay.js:228`), tour step 1 (`buddy.rs:66`),
`settings:use-clip-key` error (`ipc.rs:503`).
Problem: with an Anthropic key at the top of the order, the consent sheet still reads "Each step sends a
screenshot and the window's text to OpenRouter". A consent text that names the wrong recipient is a false
statement about where the user's screen goes.
Fix: Rust adds `recipients: [{id, name, model, modelName, free}]` (the ready providers in try order, section
3) to `overlay:state`, `settings:get` and the `agent:consent-request` payload. Strings are built from it
(copy deck, rows C-30 to C-33). The Privacy paragraph becomes generated text, not static HTML.
Scope: Rust (payloads) and renderer.

**U7. A screen-reader user hears nothing when an answer arrives or a task needs them** (Accessibility, S1)
Surface: output windows (`output.js:84-92, 447`; `trail.js:268, 341, 352, 451`).
Problem: output windows are shown without focus by design (`output.js:50-52`), and every announcement goes
to that window's own `#announce`. NVDA and Narrator generally do not speak live regions of a background
window, and focus is in the bar where the person pressed Enter. The task's question ("Waiting for your
answer in the bar") is also only announced there. Separately, the answer announcement reads up to 600
characters of markdown-stripped text unprompted (`output.js:89, 447`).
Fix:
- Each page's `announce(text, priority)` speaks locally when `document.hasFocus()`, else invokes new IPC
  `a11y:announce {text, priority}`. Rust forwards it as `a11y:say` to the focused AevalBuddy window, else to
  the bar if open. Optional fallback when no AevalBuddy window has focus: `UiaRaiseNotificationEvent` on the
  overlay HWND (verify on the runner with NVDA and Narrator).
- Texts: answer done `Answer ready. <first sentence, at most 160 characters>. Press F6 to read it.`; error:
  the error text, assertive; task asks: `The task has a question: <question>. Type your answer in the bar.`
  assertive; step rows per U27.
- The bar's `#announce` gets a second, assertive region (`role="alert"`, `aria-live="assertive"`) for
  errors and task questions.
Scope: Rust (relay) and renderer.

**U8. Text cannot be made bigger anywhere** (Accessibility, S1; WCAG 1.4.4)
Surface: all windows.
Problem: the bridge blocks Ctrl+= / Ctrl+- / Ctrl+0 and Ctrl+wheel (`src-tauri/bridge/aeval-bridge.js:96-116`),
every size is px (`glass.css:80-91`), and Windows Settings, Accessibility, Text size is not read.
Fix:
- Rust reads `Windows.UI.ViewManagement.UISettings.TextScaleFactor` at boot and on its `TextScaleFactorChanged`
  event, multiplies by the user's own setting (`textScale`, 1.0 to 2.0, step 0.1, stored), and sends
  `ui:text-scale {scale}` to every window and in `glass:hello`.
- `glass.css :root { --text-scale: 1 }` and every type token becomes `calc(Npx * var(--text-scale))`;
  `--ctl`, `--ctl-s`, `--bar-h`, `--sheet-bar-h` scale too. The bar already follows its content height.
- Pages handle Ctrl+= / Ctrl+- / Ctrl+0 themselves (`keydown` before the bridge) and invoke
  `settings:set-text-scale`; Settings, General gets a `Text size` row with a slider 100% to 200% and the
  note `Starts from your Windows text size.`
- The smoke shots add one set at 200%.
Scope: Rust and renderer.

**U9. The accent used as text fails contrast** (Accessibility, S1; WCAG 1.4.3 and 1.4.11)
Surfaces: `.trow-note.waiting` "Waiting for your answer in the bar", 13 px semibold (`overlay.css:1549`,
`rust 05-outputs-multi.png`); copied icon `#copy-all[data-done]` (`overlay.css:619`); active area button
(`overlay.css:325`); area chip icon (`overlay.css:542`).
Problem: `--accent-bright #C56FC3` (relative luminance 0.272) against the glass body measured from the
matrix (answer and task sheets, 98th-percentile background luminance 0.032 to 0.063 over black, wallpaper
and white at 100/125/150%) gives 2.84 to 3.91:1; Settings over white at 125/150% drops to 2.57:1. Text needs 4.5:1, icons 3:1; over white backdrops both fail.
Fix: add `--accent-text: #EBC0E9` (luminance 0.612: 5.9:1 at background 0.063, 3.9:1 even on a `--fill-2`
chip) in `glass.css :root`; use it for every accent-coloured text and icon on glass (the four selectors
above, plus `caret-color`). Keep `--accent-bright` for fills, rims and the lens ring. Add the token to
DESIGN.md's palette table.
Scope: renderer-only.

**U10. The first-run tour prints blank shortcuts and teaches retired hotkeys** (Ease, S1)
Surface: tour steps 3 and 4 (`28-onboard-3.png`, `28-onboard-4.png`).
Problem: steps 3 and 4 interpolate `findOnScreen` and `agentTask` (`buddy.rs:85, 92`), which are unbound by
default since v2.1 (`store/mod.rs:56-63`), producing "Press  and type a word you can see." They also teach
three hotkeys that the single inferring bar replaced, and use "Agent", a name DESIGN.md retired.
Fix: new steps in `buddy::walkthrough` (copy deck C-40 to C-44), all quoting only `captureQuery`:
1. Overlay, pointer on the orb: `Press Ctrl+Alt+Space, then type a word you can see on screen.` Advances on `find-hit`.
2. Card: `To get answers, connect an AI provider. OpenRouter has free models.` Buttons `Connect` (opens
   Settings, Providers) and `Later`. Advances on `key-saved`. Skipped if a key exists.
3. Overlay: `Now ask anything about your screen.` Advances on `first-answer`. Skipped without a key.
4. Overlay, pointer on the left token: `This shows what Enter will do. AevalBuddy guesses from what you
   type, and a task always asks before anything risky; Esc stops it.` Buttons `Got it`. The token's own
   tooltip keeps the keys (REQUESTS #6: the hint lives on that button, so the tour points at it rather
   than repeating it).
Never format a chord that is empty; a test asserts no step contains two spaces in a row.
Scope: Rust (`buddy.rs`, step prerequisites).

**U11. Answers have no structure for assistive tech, and links hide where they go** (Accessibility and
Trust, S1; WCAG 1.3.1)
Surface: answer body (`rust 03-output-answer.png`; `markdown.js:24-27, 63-98`).
Problem: headings render as `<div class="md-h">`, paragraphs as `<div class="md-p">`, code as `<pre>`
without `<code>`, and links as `<a href="#" data-href>`. Heading navigation does nothing, every link is
announced as "#", and a sighted user cannot see the destination before clicking.
Fix in `markdown.js`: `#` to `<h2>`, `##` to `<h3>`, `###`+ to `<h4>` (the sheet title is the `h1`), keeping
the `md-h md-hN` classes for style; paragraphs `<p class="md-p">`; fences `<pre><code>`; links
`<a href="URL" data-href="URL" title="URL">label</a>` (click is still intercepted, `output.js:564-569`), and
when the label is not the host, append ` <span class="md-host">(host)</span>` in `--label-2` 13 px. Each
`pre` gets a 28 px `Copy` button (`aria-label="Copy code"`).
Scope: renderer-only.

**U12. History rows nest a button inside a button, the delete target is 22 px, and deleting has no undo**
(Accessibility and Trust, S1; WCAG 4.1.2, 2.5.8)
Surface: History (`rust 09-history.png`; `history.js:69-110, 204, 216`; `history.css:261-273`).
Problem: each row is a `<button>` containing `<span role="button" tabindex="0">`, which is invalid and
announced as one button. The delete control is 22 x 22, inside another target, invisible until hover, and
deletes immediately.
Fix: row markup `<div class="item" role="listitem"><button class="item-open">…</button><button
class="del" aria-label='Delete "<title>"'>x</button></div>`, list container `role="list"`. `.del` 28 x 28,
visible at rest at `--label-3`, `--label` on hover/focus. Delete hides the row and shows a footer note
`Deleted "<title>". [Undo]` for 6 s; the renderer delays `history:remove` until the note expires or the
window closes (`pagehide` commits). Keyboard: Delete key on a row deletes (same undo).
Scope: renderer-only.

**U13. Settings rows hide their state from assistive tech: fifteen buttons named "Edit", no expanded
state** (Accessibility, S1; WCAG 4.1.2)
Surface: every Settings row (`rust 06-settings-glass-050.png`; `settings.js:34-41`; `settings.css:1096-1119`).
Problem: the chevron is a `.edit` button at `font-size: 0` whose name is "Edit" or "Done"; it toggles a
section but has no `aria-expanded` or `aria-controls`. Tabbing through Settings is "Edit, button" fifteen
times. Only the 28 px chevron is clickable although the row looks like one control.
Fix: give each `.body` an id; `.edit` gets `aria-expanded="true|false"`, `aria-controls="<body id>"`,
`aria-labelledby="<h2 id> <summary id>"` (so it reads "API key, Connected, collapsed"); `openSection`
updates `aria-expanded`. Make the whole `.row-head` the hit area with
`.edit::before { content:""; position:absolute; inset:0 }` on a `position:relative` row (the chevron stays
the visual). Same for `#edit-position` and `#replay-tour`.
Scope: renderer-only.

### S2

**U14. Errors tell the user to open Settings but only offer Retry** (Ease, S2)
Surface: error row (`14-answer-error.png`; `output.js:277-306`; `llm/errors.rs:146-217`).
Problem: "Open Settings, check the key, and save it again" with only a Retry button; the model id is raw;
no way to try another model or provider from the row.
Fix: Rust `query:error` payload becomes `{message, question, code, provider, model, modelName,
retryInMs?, actions:[{id,label}]}`. Codes and actions:
| code | actions |
| --- | --- |
| `bad-key` | `Fix key` (open Settings at `prov-<id>`), `Ask <next provider>` if one is ready |
| `no-credit` | `Add credit` (external), `Use a free model` (sets the default free model, retries) |
| `free-busy`, `rate-limited` | see U15 |
| `model-gone`, `no-vision` | `Choose model` (Settings, Models, that job) |
| `data-policy` | `Open OpenRouter privacy settings` (external), `Test again` |
| `offline` | none: waits and retries itself when Rust sees the connection return |
| `server`, `unknown` | `Retry`, `Copy details` (raw status and provider message) |
`addErrorRow(message, question, actions, tone)` renders actions as `.retry-btn` capsules in that order.
Scope: Rust and renderer.

**U15. A busy free model is shown as a red failure, with no automatic retry and no use of the user's other
providers** (Ease, S2)
Surface: `14-answer-error.png`.
Problem: queueing is normal on free models, but after three failovers the row is a danger-red error
("Every free model is busy…") and the user must retry by hand.
Fix: for `free-busy` and `rate-limited`, the row uses the neutral `.note-row` style with a clock icon, text
`Free models are busy right now. Trying again in 20 s.` (countdown updates the text every second, not
announced; announce at start and when it retries), actions `Try now` and, if another provider is ready,
`Ask Anthropic instead`. Rust waits `Retry-After` (default 20 s, then 40 s), at most two automatic retries,
then the final text `Free models are still busy. Try again in a minute, or add another provider for busy
times.` with `Retry` and `Add a provider`. Cross-provider failover per section 3.
Scope: Rust (retry loop, Retry-After) and renderer.

**U16. Nothing shows what was sent for an answer** (Trust, S2)
Surface: answer meta (`rust 03-output-answer.png`; `output.js:118-131`; `host/ask.rs:652-667`).
Problem: the usage line names a model id only. The user cannot tell whether a screenshot or only the
window's text was sent, whether it was the area or the whole screen, or to which provider.
Fix: Rust adds to `query:done` `sent: {provider, providerName, model, modelName, free, image:
"screen"|"region"|null, imageSize:[w,h], windowText:{app, chars}|null, history:n, reader:{modelName}|null}`
and keeps the JPEGs of the last three answers per window in memory only (served via `aeval-frame://`,
new IPC `output:sent-image {msgId}`). The meta row gets a quiet disclosure button `What was sent`
(`aria-expanded`) that opens:
`Sent to OpenRouter (Gemma 3 27B):` / `Your question` / `A screenshot of the whole screen, 1920 x 1080
[View]` or `The text of the Chrome window, 2,340 characters` / `4 earlier messages from this conversation`
/ (free only) `Free models' providers may keep and train on what they receive.` The usage line ends with
`<model name> via <provider>`.
Scope: Rust and renderer.

**U17. Pressing Esc to clear a typo while answering a task's question stops the whole task**
(Ease, S2)
Surface: bar while a task asks (`rust 05-outputs-multi.png`).
Problem: global Escape is registered whenever `agent.running` (`hotkeys.rs:316-318, 338-341`) and `on_escape` stops
the agent before anything else (`windows/mod.rs:654`). In the bar, Esc otherwise means "clear text"
(`bar.js:520-531`).
Fix: `bar.js` sends `bar:dirty {dirty}` on input (debounced 100 ms). `on_escape`: if the agent state is
`asking`, `plan` or `paused`, the overlay window is focused and the bar is dirty, emit `bar:clear` to the
overlay and announce `Cleared. Press Esc again to stop the task.`; otherwise stop as today.
Scope: Rust and renderer.

**U18. The token says Ask while Enter will answer the task** (Ease, S2)
Surface: `rust 05-outputs-multi.png` (token `Ask`, placeholder "Answer the task's question").
Problem: the token is the one promise of what Enter does; during `asking`, `plan` and `paused` it lies
(`bar.js:424-451`).
Fix: `renderToken` uses `agentWaiting()`: name `Reply` (asking), `Start` (plan), `Continue` (paused), Do
glyph, `data-locked="true"` styling; `aria-label="<name>. Enter sends this to the running task."`. If the
person explicitly locks Ask or Find (prefix, Ctrl+1/2, menu), honour the lock: `onEnter` routes to
`agent:continue` only when `!bar.lock`.
Scope: renderer-only.

**U19. Most actions have no keyboard shortcut** (Power user, S2)
Surface: bar, output windows.
Problem: only Ctrl+1/2/3, Alt+Down, Alt+Enter, Ctrl+Enter, Shift+Enter, F6, Up/Down recall exist.
Fix (pages see keys before the bridge blocks WebView2 defaults, `aeval-bridge.js:93-107`):
| Key | Bar | Output window | IPC |
| --- | --- | --- | --- |
| Ctrl+N | new conversation: the next question opens a new window | same, focus moves to the bar | `chat:new` from overlay (exists) |
| Ctrl+Tab, Ctrl+Shift+Tab | cycle the target window | focus the next/previous output | new `output:cycle-target` |
| Ctrl+W | close the target window | close this window | `output:close {label}` |
| Ctrl+Shift+W | close all output windows | same | new `output:close-all` |
| Ctrl+Shift+C | copy the target's last answer | copy this conversation | new `output:last-answer {label}` |
| Ctrl+. | stop the answer or task in progress | same | `query:cancel` / `agent:stop` |
| Ctrl+Shift+R | retry the target's last question | retry here | `query:retry {label}` |
| Ctrl+R | ask about an area | not bound | `region:pick` |
| Ctrl+, | Settings | Settings | `overlay:open-settings` |
| Ctrl+H | History | History | `overlay:open-history` |
| Esc | clear text, then close the bar (outputs stay, U3) | back to the bar or the previous app | |
| F6 | to the target answer (exists) | to the bar (exists) | |
Tooltips gain the key: `Close (Ctrl+W)`, `Copy conversation (Ctrl+Shift+C)`, `History (Ctrl+H)`,
`Stop (Ctrl+.)`; buttons get `aria-keyshortcuts`.
Scope: renderer and Rust (three new IPC rows).

**U20. No way to choose a model for one question, or retry with another** (Power user, S2)
Surface: bar, answer, error row.
Fix:
- `@` as the first character in the bar opens `#model-menu` (`role="listbox"`, same look as
  `#intent-menu`) listing the models of every ready provider, grouped by provider (`role="group"
  aria-label="Anthropic"`), filtered by the text after `@`. Enter or Tab picks; Esc closes and leaves the
  text. No match: `No model called "<text>". Esc to keep typing.`
- The pick becomes `#model-chip` after the token: `Sonnet 5.5` plus x, `aria-label="Model for this
  question: Claude Sonnet 5.5 via Anthropic. Press to remove."`. Backspace at position 0 removes the chip
  before the lock. Applies to Ask and Do; with Find the chip shows `title="Find uses the Find model"` and
  is ignored.
- `query:submit` and `agent:start` accept `{provider, model}`; a pinned model never fails over.
- Retry on answers and error rows becomes a split button: `Retry` and a 28 px chevron
  `aria-label="Retry with another model"` opening the same list; sends `query:retry {label, provider, model}`.
Scope: Rust (`SubmitOpts` override, retry payload) and renderer.

**U21. Keys are never shown, even masked, and unencrypted storage is silent** (Trust, S2)
Surface: Settings Account (`settings.js:86-93`; `store/mod.rs:409-417`).
Problem: "Connected" says nothing about which key or provider. If OSCrypt is unavailable, keys go into
`aevalbuddy.json` in plain text with no notice.
Fix: `settings:get` adds `keyHints: {openrouter: "sk-or-v1…a3f9", ...}` (existing `mask_key`, `ipc.rs:287-295`)
and `keyStorage: "encrypted"|"plain"`; the provider rows show the hint in `--font-mono` (section 3). Under
the list: encrypted `Keys are encrypted with your Windows account and only sent to their own provider.`;
plain `Windows couldn't provide a key store, so keys are saved unencrypted in your AevalBuddy settings
file. Anyone signed in to this Windows account can read them.`
Scope: Rust and renderer.

**U22. "New conversation" in an output window wipes it and makes it vanish** (Ease, S2)
Surface: `+` in every sheet title bar (`output.html:55`; `windows/outputs.rs:357-379`; `output.js:480-488`).
Problem: the tooltip promises a new conversation; the window's transcript is discarded and the sheet is
hidden (`opanel.hidden = true`), so the window seems to close. No undo, no hint that History kept it.
Fix: rename to `Start over` (`aria-label="Start over in this window"`). Rust keeps the previous `Chat` for
10 s (`outputs::undo_reset`). `chat:reset` keeps the sheet visible with one neutral row `Started over. The
last conversation is in History.` and `Undo`; the empty sheet reads `Your next question goes here.`
Scope: Rust and renderer.

**U23. Pressing Enter without a key does nothing visible** (Ease, S2)
Surface: `02-panel-nokey.png` (`bar.js:410-413`; `host/ask.rs:368-371`).
Problem: the notice was already showing, the text stays, nothing moves.
Fix: when `query:submit` returns `needs-key`, replay a 240 ms `nudge` on `#notice` (2 px horizontal, fade
under reduced motion), announce `Answers need an AI provider. Press Tab to connect one.`, and make the
notice button the next tab stop after the field.
Scope: renderer-only.

**U24. Find's empty result is a dead end** (Ease, S2)
Surface: `26-find-empty.png` (`trail.js:51-60`).
Problem: the Find window, where the eyes are, shows only "Nothing on screen matches ..."; the next step (`Ask instead`) is
back in the bar's status row.
Fix: under the empty text, action buttons `Ask about "zebra"` (`find:to-ask`) and, when scope is the
window, `Look on the whole screen` (`find:scope`). Actions only: no keys and no how-to text, so the
comment at `trail.js:57` and REQUESTS #6 still hold.
Scope: renderer-only.

**U25. Keys typed right after the hotkey can be lost, and the latency is unmeasured** (Power user, S2)
Surface: bar open (`overlay.js:97-103`; `windows/mod.rs:476-477`).
Problem: the field is focused only after `overlay:state` renders; keystrokes in between go nowhere. Every
open also unregisters and re-registers all hotkeys synchronously (`hotkeys::apply`). No gate measures
hotkey to focused field.
Fix: in `overlay.js` add a capture-phase `keydown` on `document`: if `state.mode === 'panel'` and focus is
not in the field and the key is printable without Ctrl/Alt, `input.focus()` and let the key through. Rust
records `t0` in `on_shortcut` and `t1` on a new `bar:ready` ping sent after the field is focused; the smoke
run logs `barOpenMs` with a budget of p95 100 ms on the runner. Call `hotkeys::apply` from `expand_panel`
only when the set of wanted shortcuts changes (Escape), off the show path.
Scope: Rust and renderer.

**U26. The bar's focus indicator is a half-pixel rim change** (Accessibility, S2)
Surface: bar (`overlay.css:295-303`). DESIGN.md line 69 promises "a 3 px accent halo".
Fix: `#bar:focus-within > .gl { box-shadow: var(--shadow), 0 0 0 2px rgb(16 18 28 / .9), 0 0 0 4px rgb(235 192 233 / .95); }`
(the dark inner ring holds 3:1 against a white desktop, the light ring against a dark one). Keep the rim
change. Forced colours: `outline: 2px solid Highlight`.
Scope: renderer-only.

**U27. Task progress is invisible to screen readers** (Accessibility, S2)
Surface: trail (`22-agent-running.png`; `trail.js:175-228, 265-269`).
Problem: done, planned, running and failed rows differ only by `aria-hidden` marks; steps are never
announced, only state changes ("Task: Working").
Fix: each row gets a visually hidden prefix (`.sr-only`): `Planned:`, `Now:`, `Done:`, `Skipped:`,
`Failed:`. Announce each new act row when it starts, throttled to one per 2 s (`Step 5: Type "notes.txt" in
Notepad`), failures immediately; confirm cards stay assertive. Give the confirm card
`aria-describedby` pointing at `.confirm-reason`.
Scope: renderer-only (relay per U7).

**U28. Find results claim to be a listbox but arrows do nothing** (Accessibility, S2)
Surface: Find view (`output.html:76`; `trail.js:61-99`).
Fix: keep `role="listbox"` and implement it: rows `tabindex="-1"` except the current one (`0`), Up/Down/
Home/End move focus and `find:step {index}`, Enter goes to the hit, `aria-activedescendant` not needed with
roving focus. Or drop the roles and use `<ul>` of buttons with `aria-current="true"` on the current hit.
Scope: renderer-only.

**U29. In a contrast theme the working indicator disappears** (Accessibility, S2)
Surface: orb and sheets (`glass.css:209-218, 277-289`).
Problem: forced colours hides `.gl::after`, which is the rim light that says "working"; the orb's busy and
agent states and the sheet's streaming state vanish.
Fix: `@media (forced-colors: active) { .glass.orbit { outline: 2px solid Highlight; outline-offset: -2px }
#pill.orbit { outline-style: dashed } .pill-dot { forced-color-adjust: none; background: Mark } }`.
Scope: renderer-only.

**U30. Windows can only be moved or resized by dragging** (Accessibility, S2; WCAG 2.5.7)
Surface: output windows, Settings, History (`renderer/frame.js`).
Fix: Alt+Space or a right-click on a title bar opens a window menu (`role="menu"`): `Move`, `Size`,
`Maximize`/`Restore`, `Close`. In Move or Size, arrows move or resize 16 px (Shift 64), Enter keeps, Esc
reverts. New IPC `window:nudge {dx, dy, dw, dh}`. Check that Win+Arrow snapping works on the runner; if it
does, say so in Settings, Shortcuts.
Scope: renderer and Rust.

**U31. The consent sheet says moving the mouse stops a task; it pauses it** (Trust, S2)
Surface: consent (`trail.js:422`), Settings Tasks note (`settings.html:258`); the run pauses
(`agent/run.rs:52`).
Fix: copy deck C-31 and C-52. A safety promise has to describe what actually happens.
Scope: renderer-only.

**U32. With history off, History says "Nothing is saved until you ask something"** (Trust, S2)
Surface: History empty state (`history.js:150`).
Fix: Rust `history:list` returns `{rows, enabled}`; when disabled the empty text is `History is off, so
nothing is being saved. Turn it on in Settings, Privacy.` with a `Turn on` link (`settings:set-save-history`).
Scope: Rust (one field) and renderer.

### S3

**U33. Settings, Shortcuts lists only the global chords** (Power user, S3). Add a read-only group `In
AevalBuddy windows` listing U19's keys plus F6, Up/Down recall and Backspace. Leave the Ask/Find/Do keys
out: they stay on the token (REQUESTS #6). F1 in any window opens this page. Renderer-only.

**U34. Model ids where people read names** (Ease, S3). Stage labels `Busy — asking gemma-3-27b-it`
(`host/ask.rs:506, 601, 627`), the usage line (`host/ask.rs:656`) and errors (`llm/errors.rs:163-205`) use
ids. Use the catalogue display name (`Gemma 3 27B`); keep the id in `title` and in `Copy details`. Rust.

**U35. The Models page speaks in jobs a non-developer has no model of** (Ease, S3). "Screen model",
"Answer model: Off" (`rust 08-settings-account.png`). Add a top row `Choose models: Automatic
(recommended)` / `Choose for each job`; the four pickers show only in the second. Renderer-only.

**U36. History's time labels contradict their group** (Ease, S3). `rust 09-history.png` shows "yesterday"
under "This week": `when()` uses 24 h deltas, `dayGroup()` calendar days (`history.js:30, 113`).
Rows show `20 min ago` / `3 h ago` today, else the time (`14:05`) under its day group, and `12 Oct` under
Earlier. Renderer-only.

**U37. History is mouse-first and read-only** (Power user, S3). Up/Down move between rows, Enter opens,
Delete deletes (U12 undo), any printable key focuses search with that character, Esc clears then closes.
Row context menu: `Ask again` (new IPC `bar:prefill-open {text}` over `windows::open_bar(app, None,
Some(text))`), `Copy question`. Renderer plus one Rust IPC.

**U38. Windows pile up** (Ease, S3). Tray `Close all answer windows` and Ctrl+Shift+W (U19). When a sixth
window would open, the least recently touched idle one closes and the bar status says `Closed "<title>". It
is in History.` Rust and renderer.

**U39. "Do it" appears on almost every answer** (Ease, S3). `STEPS_RE` (`output.js:82`) matches any list
or any of eight verbs. Show it only when at least two list lines match
`^\s*(\d+[.)]|[-*])\s+(click|double-click|right-click|press|open|type|select|choose|go to|drag|enter)\b`.
Renderer-only.

**U40. Settings' Delete all relies on a 4-second re-press** (Trust and Accessibility, S3;
`settings.js:767-788`). Use the History window's inline confirm: `Delete all 4 conversations?` `Delete`
`Keep`, focus on Keep, result in the `role="status"` line. Renderer-only.

**U41. Settings radio groups and pickers miss parts of their ARIA pattern** (Accessibility, S3;
`settings.html:213-243`, `settings.js:195-233`). Choice groups: roving tabindex, arrows move and select.
Model lists: group headers become `role="group" aria-label="Free"` wrappers instead of `aria-hidden`
items; the combo button gets `aria-controls`. `showPage` uses `behavior: 'auto'` under reduced motion
(`settings.js:444`). Renderer-only.

**U42. The area selector is silent** (Accessibility, S3; `region.html`, `region.js`). Body
`role="application" aria-label="Choose an area of the screen"`, a polite live region announcing `Area 960
by 540. Arrows move it, Alt+arrows resize, Enter asks, Esc cancels.` once on first key and the size on
change (throttled 500 ms). Add `Alt+arrows resize` to the visible hint. Renderer-only.

**U43. Small semantic gaps in the bar** (Accessibility, S3). `#examples` has `aria-label` without a role
(`index.html:103`): `role="group" aria-label="Suggestions"`. Guess card: `role="group"
aria-labelledby="guess-text"`; buttons carry `aria-keyshortcuts="Enter"` / `"Alt+Enter"` and hide their
`<kbd>` from the name. `#region-clear` `aria-label="Stop using the area and ask about the whole screen"`.
`#close-btn` name `Close the bar`. Renderer-only.

**U44. The token's key hints are only a hover tooltip** (Ease and Accessibility, S3; `index.html:33-35`).
Keyboard users never see `title`. Show the same text in a small glass tooltip under the token on
`:focus-visible` after 600 ms and on hover after 400 ms; still only on the token (REQUESTS #6).
Renderer-only.

**U45. The confirm card ignores the first 700 ms of input silently** (Ease, S3; `trail.js:326-329`). Show
it: buttons `aria-disabled="true"` with a 700 ms fill sweep (static under reduced motion), then enabled.
Renderer-only.

**U46. Two failure texts say nothing useful** (Trust, S3). `The agent stopped: an internal error.`
(`agent/run.rs:739`) and `Request failed for an unknown reason.` (`llm/errors.rs:109`). Copy deck C-60,
C-61, plus `Copy details`. Rust and renderer.

**U47. "Window layout · Drag edges to resize" opens the bar position editor** (Ease, S3;
`settings.html:419-425`). Row `Bar position`, summary from `overlayPos` (`Top right`), visible button
`Move…`. Rust adds `overlayPos` to `settings:get`. Renderer plus one field.

**U48. A taken shortcut gets no suggestion** (Power user, S3; `hotkeys.rs:459`). Probe three alternates
(`Ctrl+Alt+A`, `Win+Shift+Space`, `Ctrl+Shift+Space`) and return the free ones:
`Another app already uses Ctrl+Alt+Space. Free: Ctrl+Alt+A, Win+Shift+Space.` with one-click buttons.
Rust and renderer.

**U49. Custom answer styles exist but have no door** (Power user, S3). `skills.rs` loads a user folder; the
Answers page has no way to it. Add `Open styles folder` and a one-line note `Add a .md file to make your own
style.` New IPC `skills:open-folder`. Rust and renderer.

**U50. Windows' Transparency effects setting is ignored without a word** (Accessibility, S3). REQUESTS #5 is
kept; when Rust reports `transparency-off`, the Liquid glass section adds `Windows transparency effects are
off. AevalBuddy keeps its glass; choose None for solid panels.` Renderer-only.

**U51. The contrast matrix covers one glass level** (Accessibility, S3). All `m-*` shots are at k = 0.5.
At Very very the thin blur is 8 px, so bright detail behind text survives the 90th-percentile scrim. Add
k = 0 and k = 1 to the matrix (wallpaper and white, 100% and 150%), and in `glass.js` `measure()` use the
98th percentile when k > 0.5. Test tooling and renderer.

**U52. Smooth scrolling ignores reduced motion in Settings** (Accessibility, S3). Covered in U41;
listed for the count.

---

## 3. Multi-provider keys in Settings (REQUESTS #2)

Goal: hold OpenRouter, Anthropic, OpenAI and Gemini keys at once; show which one answers and why; let the
person order them; fail over predictably and never bill a paid account by surprise; every state is words
plus an icon; complete from the keyboard and with a screen reader.

### 3.1 Layout

Rail item `Account` becomes `Providers`; page `#pg-providers`, `h1` `Providers`.

```
Providers
Answers come from the first ready provider in this list. Drag to change the order.

 ⋮⋮  1  OpenRouter      sk-or-v1…a3f9                ✓ Ready     Default   ⌄
 ⋮⋮  2  Anthropic       sk-ant-a…9Q2c                ✓ Ready               ⌄
 ⋮⋮  3  OpenAI          GPT models                   ○ Not added           ⌄
 ⋮⋮  4  Google Gemini   Gemini models                ○ Not added           ⌄

 [on]  When one is busy or fails, try the next
       [x] Ask me before switching from free models to a paid provider
 Keys are encrypted with your Windows account and only sent to their own provider.
```

Row (collapsed, min height 52 px, one group card with hairlines like other Settings groups): drag handle
(16 px, `aria-hidden`), position number (`--label-2`, tabular), provider name (14 px, 600, `--label`),
subtitle (13 px, `--label-2`: the masked key in `--font-mono` when present, else what the provider offers:
`Free and paid models`, `Claude models`, `GPT models`, `Gemini models`), status (icon plus word, 13 px),
`Default` capsule (12 px, `--fill-2`) on the provider that answers now, chevron. The whole row is the
disclosure button (U13 pattern). One row open at a time, like the other groups.

Expanded body content by state is in 3.2. Every expanded ready row also has `Move up`, `Move down` and
`Use first` buttons (the keyboard and single-pointer alternative to dragging, WCAG 2.5.7).

### 3.2 States

| State | When | Status | Body text | Actions |
| --- | --- | --- | --- | --- |
| Not added, OpenRouter | no key | `○ Not added` | `Free models, no card needed. Connecting takes about a minute.` | `Connect OpenRouter` (primary, OAuth, U1); link `Paste a key instead` reveals the field |
| Not added, other | no key | `○ Not added` | `Use your own Anthropic account. Each question is billed to it.` | field `Anthropic API key` (placeholder `sk-ant-…`), `Show`, `Save`; link `Get a key at console.anthropic.com` (OpenAI: `platform.openai.com/api-keys`, Gemini: `aistudio.google.com/apikey`) |
| Clipboard offer | window regains focus, clipboard holds a key for a provider with none | unchanged | `Key on the clipboard: sk-ant-a…9Q2c` | `Use this key` |
| Wrong provider | pasted prefix belongs to another provider | unchanged | `This is an OpenAI key.` (nothing saved, text stays) | `Save it under OpenAI` |
| Testing | after Save, Connect or Test again | `Checking…` (spinner; static ring under reduced motion) | `Checking the key with Anthropic…` | field and buttons disabled; 15 s timeout goes to Can't check |
| Ready | test passed | `✓ Ready` | `Works. Checked just now.` OpenRouter adds `About $7.50 of credit left.` or `Free models only.` | `Test again`, `Replace key`, `Remove`, `Use first` |
| Rejected on save | 401/403 (Gemini 400) during Save | `○ Not added` | `Anthropic rejected this key, so it wasn't saved. Check it and paste it again.` | field keeps focus |
| Key rejected | a saved key starts failing 401 | `× Key rejected` (`--danger`) | `Anthropic rejected this key. It may have been revoked or mistyped. AevalBuddy is skipping Anthropic until you replace it.` | `Replace key`, `Remove` |
| No credit | 402, or credit check at 0 | `! No credit` | OpenRouter: `The key works, but the account has no credit, so only free models answer.` Others: `The key works, but the Anthropic account has no credit. AevalBuddy is skipping Anthropic.` | `Add credit` (external), `Test again` |
| Busy | 429 | `◷ Busy` | with another ready: `OpenRouter's free models are busy. Your key is fine. Questions go to Anthropic until about 14:32.` alone: `…Questions wait and retry until about 14:32.` | `Try now` |
| Free models off | OpenRouter data-policy refusal (U1 probe or a real request) | `! Free models off` | `Your OpenRouter privacy setting blocks free models. They're offered on the condition that their providers may keep and train on what they receive, which includes your screen. Turn on "Free model publication" to use them, or add credit and pick a paid model.` | `Open OpenRouter privacy settings`, `Test again` |
| Can't check | network error or timeout | `– Not checked` | `Can't reach Anthropic right now. The key is saved; AevalBuddy checks again when you're online.` | `Test again` |
| Removing | Remove pressed | unchanged | `Remove the Anthropic key from this PC? This doesn't revoke it at Anthropic.` | `Remove`, `Keep` (focus on Keep); after: status `Not added`, live `Anthropic key removed.`, focus back on the row |

Status words are always present; icons are `aria-hidden`. Status changes are spoken through one
`role="status"` line under the list.

### 3.3 Choosing the default provider and models

- **Order is the default.** The first row whose state is Ready answers; it wears `Default`. Reorder by drag,
  by `Move up` / `Move down` / `Use first`, or Alt+Up / Alt+Down on a focused row. Announce
  `Anthropic is now first. It answers by default.` Stored as `providerOrder` (replaces the hidden
  `provider` override, which migrates to the top of the order).
- **Models per provider.** The Models page gets a segmented control at its top, `Models for:` with one
  segment per provider that has a key (default: the Default provider). Each job row (Screen, Answer, Find,
  Task) edits that provider's pick: OpenRouter uses the existing keys, the others `nativeModels.<provider>.<kind>`
  (already in the store). `Show paid models` shows only on the OpenRouter segment; the others show
  `Every model here is billed per question.`
- **One question, another model:** the `@` picker (U20).

### 3.4 Failover

- Candidates: providers in list order, skipping Not added, Key rejected, No credit (native), Busy until its
  cooldown ends, and Free models off (OpenRouter, unless a paid OpenRouter model is picked).
- Inside OpenRouter's free models: the existing up to three free models (`llm/failover.rs`).
- To the next provider: only before any answer text has been shown, on 429, 5xx, 408, 404 (model gone),
  401, 402 or a timeout while online. Never on 400, a refusal, Stop, after text started, or when the person
  pinned a model with `@`.
- Free to paid: with `Ask me before switching from free models to a paid provider` on (default), the answer
  window shows a card instead of switching: `OpenRouter's free models are busy. Ask Anthropic (Claude
  Sonnet 5.5) instead? Anthropic bills your account for it.` `Ask Anthropic` / `Keep waiting`, and a
  checkbox `Don't ask again` (clears the setting). Keep waiting returns to U15's countdown.
- While switching: stage label `OpenRouter is busy. Asking Anthropic…`. After: the usage line reads
  `Claude Sonnet 5.5 via Anthropic · OpenRouter was busy`, and U16's panel names the provider that received
  the data. A task picks its provider at start and never switches mid-run.
- Cooldowns: 429 marks Busy for Retry-After or 60 s; 401 marks Key rejected until the key changes or a test
  passes; 402 marks No credit and re-tests silently after 10 min. A newly rejected key raises one bar
  notice: `Anthropic rejected your key, so answers come from OpenRouter for now.` `Fix key`.
- Privacy text lists the order (U6): `Your question and screen go to the first ready provider: OpenRouter,
  then Anthropic.`

### 3.5 Rust work

- `settings:get` adds `providers: [{id, name, order, hasKey, keyHint, status, statusText, checkedAt,
  busyUntil, credit}]`, `failover: {acrossProviders, askBeforePaid}`, `keyStorage`.
- `settings:save-key {provider, key}` tests before storing; returns `{ok, status, message, keyHint}` or
  `{ok:false, error, detected}` (`detected` is the provider the prefix names).
- `settings:test-key {provider}` returns `{ok, status, message, credit}` (with the U1 free-model probe).
- `settings:set-provider-order {order}`, `settings:set-failover {acrossProviders, askBeforePaid}`,
  `settings:set-native-model {provider, kind, model}`, `auth:openrouter-connect` (U1).
- Event `provider:status {id, status, statusText, busyUntil}` to Settings and the overlay.
- `llm::settings::active` becomes `candidates()` returning the ordered list; `host/ask.rs` and the agent
  start loop over it with the rules above; `query:stage`, `query:done.usage.provider`,
  `query:done.usage.via` carry the outcome.
- `clip_key_of` recognises all four prefixes; `settings:clip-key` returns `{found, masked, provider}`.

### 3.6 Accessibility of this page

`<ol id="providers" aria-label="Providers, in the order AevalBuddy tries them">`; each row button's name
is `OpenRouter, 1 of 4, Ready, default`; `aria-expanded` and `aria-controls` on it; key fields have
visible labels; drag has the button alternatives; focus returns to the row after Remove, Replace or a move.

---

## 4. Copy deck

Old to new. File and line give the source of the old string. New copy has no em dashes.

### Bar

| # | Where | Old | New |
| --- | --- | --- | --- |
| C-1 | `overlay.js:229` | No OpenRouter API key saved yet. Finding words on screen works without one. | Answers need an AI provider. Finding words on screen works without one. |
| C-2 | `overlay.js:228` | Doing tasks needs an OpenRouter API key. | Tasks need an AI provider. |
| C-3 | `index.html:133` (no-key case) | Open settings | Connect a provider |
| C-4 | `overlay.js:223` | `${x}` is taken by another app, so it does nothing. Pick another shortcut in Settings. | Another app already uses `${x}`, so it can't open AevalBuddy. Choose another shortcut. |
| C-5 | `index.html:133` (hotkey case) | Open settings | Change shortcut |
| C-6 | `bar.js:209` | This reads like a task. Enter answers it; AevalBuddy can also do it on your computer. | This sounds like a task. Enter just answers it; AevalBuddy can also do it on your computer. |
| C-7 | `index.html:97` | Just answer | Answer |
| C-8 | `index.html:98` | Run as a task | Do it for me |
| C-9 | `index.html:99` | Find it | Find on screen |
| C-10 | `bar.js:210` | This reads like a task. Enter answers it. Alt+Enter runs it. | This sounds like a task. Enter answers it. Alt+Enter does it for you. |
| C-11 | `bar.js:425` | Answer the task’s question | Type your answer for the task |
| C-12 | token names (new, U18) | (token stays "Ask") | Reply / Start / Continue |
| C-13 | `index.html:78` | Collapse; tooltip Collapse (Esc) | Close the bar; tooltip Close (Esc) |
| C-14 | target chip (new, U5) | none | Continues: `<title>` |
| C-15 | needs-key announce (new, U23) | none | Answers need an AI provider. Press Tab to connect one. |
| C-16 | U17 announce (new) | none | Cleared. Press Esc again to stop the task. |

### Output windows

| # | Where | Old | New |
| --- | --- | --- | --- |
| C-20 | `output.js:108` | Waiting for the model · N s. Free models queue at busy times; Stop is beside this. | Waiting for `<model name>` · N s. Free models queue when they're busy. |
| C-21 | `host/ask.rs:627` | Busy — asking `<id>` | That model is busy. Trying `<model name>`… |
| C-22 | `host/ask.rs:506` | Busy — reading with `<id>` | That model is busy. Reading with `<model name>`… |
| C-23 | `host/ask.rs:601` | Busy — thinking with `<id>` | That model is busy. Thinking with `<model name>`… |
| C-24 | `output.html:55` | Start a new conversation; tooltip New conversation | Start over in this window; tooltip Start over |
| C-25 | `output.html:62` | tooltip Close | Close (Ctrl+W) |
| C-26 | `output.html:40` | tooltip Copy conversation | Copy conversation (Ctrl+Shift+C) |
| C-27 | `output.html:47` | tooltip History | History (Ctrl+H) |
| C-28 | `output.js:206` | Have the agent do what this answer describes | Do the steps in this answer on your computer |
| C-29 | usage line `output.js:128` | `… · gemma-3-27b-it` | `… · Gemma 3 27B via OpenRouter` |
| C-34 | reset note (new, U22) | none | Started over. The last conversation is in History. |
| C-35 | empty sheet (new, U22) | none | Your next question goes here. |
| C-36 | cut-off note (new, U4) | none | Stopped because you asked something in another window. |
| C-37 | answer announce `output.js:447` | Answer: `<up to 600 characters>` | Answer ready. `<first sentence>`. Press F6 to read it. |

### Task trail and consent

| # | Where | Old | New |
| --- | --- | --- | --- |
| C-30 | `trail.js:427` | Each step sends a screenshot and the window's text to OpenRouter and the model set for tasks. A free model's provider may keep and train on what it receives. | Each step sends a screenshot and the window's text to `<Provider>` (`<model name>`). (free only:) Free models' providers may keep and train on what they receive. |
| C-31 | `trail.js:422` | Press Esc, or move the mouse, and it stops. | Move the mouse to pause it. Press Esc to stop it. |
| C-32 | `settings.html:273` | Asking sends a screenshot, or the window's text, and your question to OpenRouter, which passes them to the model you picked. Tasks send one each step. … | Asking sends your question and a screenshot, or the window's text, to the first ready provider: `<OpenRouter, then Anthropic>`. Tasks send one each step. Free models are offered on the condition that their providers may keep and train on what they receive. Paid models don't. Exact Find never leaves this computer; related Find sends the window's text to the Find model. |
| C-33 | `ipc.rs:503` | The clipboard no longer holds an OpenRouter key. | The clipboard no longer holds a key. |
| C-38 | `trail.js:200` | Waiting for your answer in the bar | (unchanged; colour moves to `--accent-text`, U9) |
| C-39 | step announce (new, U27) | Task: Working | Step `<n>`: `<step text>` |
| C-60 | `agent/run.rs:739` | The agent stopped: an internal error. | The task stopped because of a problem inside AevalBuddy. Nothing else was done. |
| C-62 | `agent/run.rs:651` | An agent run is already going. | A task is already running. |
| C-63 | paid-hop card (new, 3.4) | none | OpenRouter's free models are busy. Ask Anthropic (`<model name>`) instead? Anthropic bills your account for it. |

### Tour (`buddy.rs:59-110`)

| # | Old | New |
| --- | --- | --- |
| C-40 | Paste an OpenRouter API key here and press Save. A free key works. | (moves to step 2) To get answers, connect an AI provider. OpenRouter has free models. Buttons: Connect / Later |
| C-41 | Press `<chord>` and ask anything about your screen. | Now ask anything about your screen. |
| C-42 | Press `<blank>` and type a word you can see. | Press Ctrl+Alt+Space, then type a word you can see on screen. |
| C-43 | Press `<blank>` and say what to do. Agent asks before anything risky, and Esc stops it. | (pointing at the token) This shows what Enter will do. AevalBuddy guesses from what you type, and a task always asks before anything risky; Esc stops it. |
| C-44 | Buttons Try it / Later | Got it (last step) |

### Settings

| # | Where | Old | New |
| --- | --- | --- | --- |
| C-50 | rail `settings.html:29` | Account | Providers |
| C-51 | `settings.js:88` | Connected / No key yet | `<Provider> · <masked key>` / Not added (per row, section 3) |
| C-52 | `settings.html:258` | Esc, the Stop button, the tray, or moving the mouse stops it. Screenshots are never saved. | Esc, the Stop button or the tray stops it; moving the mouse pauses it. Screenshots are never saved. |
| C-53 | `settings.html:419-425` | Window layout · Drag edges to resize · (Move) | Bar position · `<Top right>` · Move… |
| C-54 | `settings.js:773` | Delete everything? | Delete all `<n>` conversations? (inline, with Delete / Keep) |
| C-55 | `settings.js:885` | Include Ctrl, Alt, or Win in the combo. | Add Ctrl, Alt or Win to the shortcut. |
| C-56 | `hotkeys.rs:459` | That shortcut is already in use - try another. | Another app already uses that shortcut. Try another. (plus free alternatives, U48) |
| C-57 | `llm/client.rs:555` | Key works. | Works. Checked just now. |
| C-58 | storage line (new, U21) | none | Keys are encrypted with your Windows account and only sent to their own provider. / Windows couldn't provide a key store, so keys are saved unencrypted in your AevalBuddy settings file. Anyone signed in to this Windows account can read them. |
| C-59 | Text size row (new, U8) | none | Text size · Starts from your Windows text size. |
| C-64 | transparency note (new, U50) | none | Windows transparency effects are off. AevalBuddy keeps its glass; choose None for solid panels. |
| C-65 | OpenRouter probe (new, U1) | Key works. | Works, but your OpenRouter privacy setting blocks free models. Turn on "Free model publication", or add credit and pick a paid model. |

### Errors (`llm/errors.rs`)

| # | Old | New |
| --- | --- | --- |
| C-61 | Request failed for an unknown reason. Retry, or ask again. | The request failed and `<Provider>` didn't say why. Retry, or copy the details if it keeps happening. |
| C-70 | `<p>` rejected the API key. Open Settings, check the key, and save it again. | `<Provider>` rejected your key. It may have been revoked or mistyped. (action: Fix key) |
| C-71 | Every free model is busy. Free models take a few requests a minute; retry in a moment, or add credit and pick a paid model in Settings. | Free models are busy right now. Trying again in `<n>` s. (final:) Free models are still busy. Try again in a minute, or add another provider for busy times. |
| C-72 | Rate limited by the provider. Wait a moment, then retry. | `<Provider>` is limiting requests right now. Trying again in `<n>` s. |
| C-73 | `<model>` is not available right now, and the free model tried after it was not either. OpenRouter says: `<detail>`. Pick another model in Settings. | `<Model name>` isn't available on OpenRouter right now, and neither was the backup. (action: Choose model; detail in Copy details) |
| C-74 | The model `<model>` does not accept images. Pick another Screen model in Settings, or turn on the Answer model so a screen model reads for it. | `<Model name>` can't read images. Choose a screen model that can. (action: Choose model) |

### History and tray

| # | Where | Old | New |
| --- | --- | --- | --- |
| C-80 | `history.js:150` (history off) | Conversations you have will be listed here. Nothing is saved until you ask something. | History is off, so nothing is being saved. Turn it on in Settings, Privacy. |
| C-81 | delete (new, U12) | none | Deleted "`<title>`". Undo |
| C-82 | `history.html:47` | Delete everything? | Delete all `<n>` conversations? |
| C-83 | `tray.rs:145` | Stop agent | Stop the task |
| C-84 | `tray.rs:167, 399` | New chat | New conversation |
| C-85 | tray (new, U38) | none | Close all answer windows |

---

## Appendix: contrast evidence

Method: for each `m-*` shot, background luminance of the glass body taken from low-variance tiles inside
the sheet (98th percentile, to find the brightest backdrop showing through), then the CSS token's contrast
against it. Text tokens are solid colours, so this is the contrast a glyph gets at its worst spot.

| Surface (worst backdrop) | Glass L (p98) | `--label-3` | `--label-2` | `--label` | `--accent-bright` |
| --- | --- | --- | --- | --- | --- |
| answer, thinking, error, agent (white, 100%) | 0.062 to 0.063 | 4.96 to 5.04 | 5.93 to 6.03 | 8.6 to 8.7 | 2.84 to 2.88 |
| find (white) | 0.062 | 5.03 | 6.01 | 8.68 | 2.88 |
| settings (white, 125/150%) | 0.075 | 4.50 | 5.38 | 7.77 | 2.57 |
| history (white) | 0.038 | 6.36 | 7.60 | 10.98 | 3.64 |
| bar placeholder, measured in the text box | 0.053 (white) | 5.45 | | | |
| any sheet, black backdrop | 0.017 to 0.048 | 5.73 to 8.37 | 6.85 to 10.00 | 9.89 to 14.44 | 3.28 to 4.78 |

The bar's p98 of 0.082 (white, 100%) and 0.12 (125/150%) comes from the token's `--fill-1` capsule, where the text is `--label`
13 px semibold (5.7:1, passes). Lens and tour rows on wallpaper were dominated by the desktop around the
small card and are not counted. Settings at 125/150% over white sits exactly at 4.5:1 for `--label-3`;
keep `--label-3` off 12 px text there (Settings `.edit` chevrons are icons, 3:1 applies).
