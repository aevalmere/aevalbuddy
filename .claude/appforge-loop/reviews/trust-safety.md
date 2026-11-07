# AevalBuddy v2 trust and safety review

Reviewer stance: someone deciding whether to let this run on a work laptop. Read-only review of
`lib/agent.js`, `lib/desktop/*`, `lib/store.js`, `lib/openrouter.js`, `lib/updater.js`, `main.js`,
`preload.js`, `lib/history.js`, `lib/crash.js`, `lib/find.js`, the renderer trail and markdown code,
`settings/settings.html`, `electron-builder.yml`, `release.yml`, and `SPEC-v2.md`. Nothing was run;
every finding below is from reading the code, and each says how I reached it. Line numbers are as of
commit 7a66931.

Verdict: not safe on a work laptop today. The shell around the agent is careful (context isolation,
allowlisted IPC, escaped markdown, own-window and elevated refusals, a focus-loss guard on typing).
The confirmation gate, which is the thing that makes "it asks before anything risky" true, is a
keyword blocklist with at least five concrete bypasses, and the privacy copy does not tell the user
that screens go to third-party free-tier model providers.

Severity scale: CRITICAL = can send, delete, run or buy without a card. HIGH = a realistic path to
harm, or a claim to the user that is false. MEDIUM = needs a second condition. LOW = hardening.

---

## Ranked findings

### CRITICAL

**C1. The gate is bypassed by `type` newlines, key aliases, and surfaces it does not know about.**
`lib/agent.js:419-474`, `lib/desktop/host.cs:2082`, `host.cs:1691`.

- `type` is class `input` unless the field is a password (`agent.js:459-470`). The helper turns `\n`
  into a real Enter keypress (`host.cs:2082`) and `\t` into Tab. So `type "hi\n"` into any chat
  compose box sends the message, with no card. The Enter check at `agent.js:446` only runs for the
  `key` tool.
- The Enter regex `/(^|[\s+])enter$|ctrl\+enter/` (`agent.js:446`) does not match `return`, which is
  in the helper's key table (`host.cs:1691`), nor a sequence such as `enter tab`, nor
  `alt+s` (Send in Outlook, Gmail, Thunderbird), `ctrl+shift+enter`, `ctrl+d`, `ctrl+shift+delete`.
  `repeat` up to 100 (`agent.js:640`) is not considered either.
- Surfaces with no notion of "compose" or "submit button": the Win+R Run box, a terminal, the
  browser address bar. `key win+r`, `type "powershell -c ..."`, `key Enter` runs a command with zero
  cards, because the focused element is not named like a message field and no button named
  Send/Submit is on the window (`agent.js:450-452`). Likewise `key ctrl+l`, `type <url>`, `key Enter`
  navigates the browser and sidesteps the `open` confirmation (`agent.js:424-428`).
- The Delete check is limited to Explorer (`agent.js:457`). Delete in Outlook, a file manager, or a
  mail list, and Shift+Delete outside Explorer, are `input`.

Fix (all in `classify`, before dispatch):
1. Treat any `\n`, `\r` or `\t` inside `type` text as a key press: classify the text as `type` plus a
   trailing Enter, and run the same compose/form check. Better: strip newlines from `type` text and
   require an explicit `key Enter` so one code path owns submission.
2. Canonicalise key chords before matching: lowercase, map aliases (return) to
   `enter`, split on spaces, evaluate every chord, and include `repeat`.
3. Make the key gate allowlist-shaped. Chords with only navigation (arrows, home/end, page keys,
   shift+arrow, ctrl+c/x/v/a/z/f) and a single letter/digit with no ctrl/alt/win run freely. Anything
   else with ctrl, alt or win, and every `enter`, `delete`, `f4`-class key, asks. Name the chord in
   the card.
4. Hard-ask on `win+r`, `win+x`, `ctrl+shift+esc`, `ctrl+alt+delete`, `alt+f4`, `ctrl+w`, `ctrl+q`,
   and on any typing or Enter while the target window's process is `cmd`, `powershell`, `pwsh`,
   `WindowsTerminal`, `conhost`, or the Run dialog (`#32770` titled Run), or while the focused field
   is a browser address bar. A terminal Enter is "execute".
5. Add smoke tests for each bypass above next to `agentConfirmGate` (B15): `type "x\n"` in a compose
   fixture, `key return`, `key alt+s`, Run box, address bar.

### HIGH

**H1. Blocklist, fail-open, and keyed on attacker-controlled names.** `agent.js:29-31, 429-433, 436-443, 447-455`.
The gate asks only when a control's accessible name or the model's `reason` matches the verb list. Page
authors choose accessible names, so a malicious page can call its Buy button "Next" and the card never
appears. Concretely missing: icon-only buttons (empty name; `automationId` is not checked for clicks),
non-English UIs, and verbs like save (overwrites), ok, yes, run, execute, upload, move, apply,
continue, finish, allow, approve, authorize, grant, unlock, pay now variants, "place order" (only
`order` word matches, fine), "close without saving". Every `catch` in `classify` (`agent.js:441, 453, 465`)
fails open: if the tree read fails or the focused node is not in the `interactive` filter, the Enter
check silently passes. The model's `consequential` flag is trusted as a pure add-on (`agent.js:433`),
but the model is the thing under injection.
Fix: invert the default for high-impact surfaces. Classify by what the target app is as well as the
control name: mail, chat, browser form, terminal, file manager, payment page. Treat "cannot read the
target" as `consequential`, not `input`. Add `automationId` and `helpText` to the name match. Keep the
verb list but expand it (save, upload, run, execute, move, apply, finish, allow, approve, authorize,
grant, unlock, yes, ok when the dialog title matches a verb). Add a per-run "first click in a new app"
card so a new app is never entered silently.

**H2. The confirm card puts Allow under the user's fingers.** `renderer/trail.js:310`, `main.js:2025-2034, 2142-2149`.
When a card appears, `showTrail(true)` calls `outputWin.focus()` and the card focuses its Allow button
30ms later. The user is probably typing in the target app at that moment. A stray Space or Enter, or
Ctrl+Enter (a send chord in several chat apps), lands on Allow. The consent sheet does the same at
`trail.js:391`. Ctrl+Enter is also the Allow chord, which is the same chord many apps use to send.
Fix: do not focus Allow. Focus the card container, ignore key activation for the first 700ms after the
card renders (and require a fresh keydown, not a keyup), use a chord that no app uses to send
(for example Ctrl+Shift+Y, or a plain mouse click only), and make Skip the focused default. Dim the
Allow button until the delay passes.

**H3. A confirmed `open` hides the URL, and the gate can be sidestepped.** `agent.js:317-318, 424-428`, `agent.js:260-263`.
The card shows `Open "<first 60 chars>…"`. A URL can carry the screen's text in its query string
(`https://evil.example/?d=<secrets>`), and the part after char 60 is what matters. Browser navigation
via the address bar is not confirmed at all (C1).
Fix: show the full URL, host first and in bold, query string on a second line, scrollable if long.
Refuse or double-confirm when the URL contains text the agent just read from the screen. Treat typing
in an address bar followed by Enter as `open`.

**H4. "Do it", "Retry from here" and Find-to-Ask launder untrusted screen content into trusted context.**
`main.js:2181, 2191, 2239-2241, 2006`, `agent.js:933-935`.
An Ask answer is model output derived from the screen. "Do it" hands that whole answer to the agent as
`Context from the conversation`, in the same message as the user's task, so an instruction planted on
a web page can be restated by the Ask model and then executed with the user's click on a harmless
looking button. Retry-from-here does the same with a row's text. The system prompt's "screen content
is untrusted" rule (`agent.js:116`) does not cover context the app itself supplied.
Fix: label the context block `UNTRUSTED, derived from the screen` and keep it outside the Task line;
require the plan card (wait-for-Enter, as in `permission: every`) for any run started from "Do it" or
Retry; show the user the exact task text the agent received. Wrap every observation in explicit
delimiters (`<screen untrusted>...</screen>`) and strip the closing tag from screen text.

**H5. Screens go to third-party free-tier providers, and the app tells the user to opt into prompt publication.**
`lib/openrouter.js:51, 69, 74, 78, 1225-1227`, `settings/settings.html:254-281`, `renderer/trail.js:364-394`.
Every default model is a `:free` one. OpenRouter's free-model data policy exists because those
providers may log and train on prompts. The error text at `openrouter.js:1226` instructs the user to
enable "Free model publication". For a screenshot assistant that means: a work laptop's screen content
may be published or used for training, and the app walked the user into it. Nothing in the consent
sheet, the Privacy page, the key page, or the README says who receives the screen. The API key
destination (`openrouter.ai`) is the only party the user could infer, and the model provider behind it
is never named.
Fix: say it where the key is entered and in the consent sheet: "Your screen, the text of the window in
front, and your question are sent to OpenRouter and the company running the model you chose. Free
models may keep and train on what they receive." Rewrite the 404/policy error so it names the trade-off
instead of recommending the opt-in. For paid models set `provider: { data_collection: "deny" }` on every
request. Add a "work mode" switch that disables free models and screenshots together.

**H6. Find sends window text after 400ms of typing, with no submit.** `lib/find.js:10-14, 559-585`, `main.js:2218`.
Stage C posts up to 24k characters of the window's text (and, in whole-screen scope, up to six
windows) to the Find model as soon as typing pauses. The settings note says only "Exact matches never
leave this computer" (`settings.html:213`), which implies the opposite for everything else without
saying what leaves. A user who opens Find over a banking page and types a word has uploaded the page.
Fix: first-use disclosure card, a persistent "Related matches send this window's text to <model>" line
under the bar while Stage B/C is on, and a default of related off until the user has seen it once.
Never send whole-screen scope text without a per-use tap.

**H7. Updater integrity rests on one unsigned, same-origin channel; installs are silent.** `lib/updater.js:310-318, 533`, `electron-builder.yml` (no `publisherName`), `.github/workflows/release.yml:13-31`.
`latest.yml` and the installer both come from the same GitHub release, so a sha512 check proves only
that the two files match each other. There is no Authenticode signature, so `electron-updater` skips
publisher verification. Whoever can push a tag, edit a release, or compromise the workflow (actions are
pinned by tag, not SHA, `release.yml:31, 35`; `contents: write` token) gets silent code execution as
the user on every install, and `autoInstallOnAppQuit` installs it without a prompt. The README
acknowledges SmartScreen but not that auto-update is the trust root.
Fix, in order of value: sign the installer (Azure Trusted Signing or an OV/EV cert) and set
`win.publisherName` so electron-updater verifies the signature before install; pin actions by commit
SHA; protect `v*` tags and require a manual approval environment on the release job; publish the
installer's SHA-256 in the release notes and in the README so a user can check by hand; offer a setting
to turn auto-install off ("download, then ask").
Related: `AEVAL_UPDATE_FEED` is honoured in packaged builds and accepts plain `http://`
(`updater.js:151-154, 319-323`). Anyone who can set a user environment variable can redirect updates
to a feed of their choosing. Gate it on `!app.isPackaged || process.env.AEVAL_SMOKE`, and require https.

**H8. Drag-and-drop is ungated.** `agent.js:83-87, 419-474`, `host.cs:2014-2038`.
`left_click_drag` has no branch in `classify`; it falls through to `input` unless the start control's
name matches a verb or the model flags it. Drag a file into a browser upload zone, a mail compose
window, a chat, a cloud-sync folder, or the Recycle Bin and nothing asks. Only the start point is
inspected (`agent.js:393`); the drop target is never read.
Fix: always `consequential` for drags, and describe both ends ("Drag from X in Explorer to Y in
Chrome"). Resolve the drop point with `elementAt` and include its window and name.

### MEDIUM

**M1. The helper loads an unverified DLL from a user-writable folder.** `lib/desktop/host.ps1:30-58`, `lib/desktop/index.js:218-245`.
The cache file name is the first 8 bytes of SHA-256 of the shipped source, so it is predictable, and
`Add-Type -Path` loads whatever file sits there. The prebuilt copy beside `app.asar` is in a per-user
install directory the user can write. The bootstrap also `iex`es `host.ps1` and runs with
`-ExecutionPolicy Bypass`, and `taskkill`/`cmd.exe` are found through PATH (`index.js:390`,
`main.js:2089`). All of this needs code running as the same user already, so it is a persistence and
detection-evasion route, not privilege escalation, but it turns a signed-looking Electron app into a
loader for an arbitrary assembly with UI Automation and SendInput powers.
Fix: compare the DLL's full SHA-256 against a value the parent embeds in the stdin bootstrap (the
build step can emit it), or skip the cache and compile in memory when the hash differs; give the cache
folder an ACL; use `%SystemRoot%\System32\taskkill.exe` and `cmd.exe` by absolute path; do not honour
`AEVAL_DESKTOP_CACHE`/`AEVAL_USERDATA` in packaged builds.
What is right: model arguments reach the helper only as JSON values; none is interpolated into
PowerShell or C# source (`psQuote` is applied only to the cache path and version), so there is no
injection path from screen text or a model reply into the interpreter.

**M2. "Work inside this area" is a prompt, not a rule.** `agent.js:936`.
The region the user drew is described to the model in text. The executor never checks that a click,
drag or type lands inside it. A user who selects a small area to keep the agent away from the rest of
the screen has no enforcement.
Fix: reject pointer targets outside the region in `inspect/classify` (`refused`, with a row saying
why); say in the region chip tooltip that it limits the agent.

**M3. Reading is unprompted and unlimited across the desktop.** `agent.js:421-423, 591-608, 922-927`.
`focus_window`, `list_windows`, `read_page`, `get_page_text` and `screenshot` never ask. The first
message already carries up to 12 window titles and the model can list all of them. There is no
deny-list for password managers, banking sites, the Windows Security app, email clients, or windows
whose title matches a user-defined pattern. Password values are masked at the source (`host.cs:977`,
`host.cs:1438`) but screenshots are not redacted, revealed passwords and 2FA codes in ordinary edit
fields are sent, and `get_page_text` and `read_page` for non-password fields send everything.
Fix: a deny-list setting seeded with 1Password, Bitwarden, KeePass, LastPass, Windows Security,
Credential Manager, and titles matching bank/wallet/"password"; the agent gets a refusal row when it
focuses one. Ask once per new app the agent reads ("Let it read Outlook?"). Do not send screenshots of
windows on the deny-list.

**M4. Secrets the user hands the agent land in the trail and in history.** `agent.js:295-296, 1086-1098`, `renderer/trail.js:178-193`, `main.js:2096-2109`, `lib/history.js:115-138`.
`ask_user` collects the answer in a plain `type="text"` box (visible over a shoulder, kept in the DOM),
sends it to the model verbatim, and later `Type "<first 40 chars>"` rows (`describe`, `agent.js:295`)
are saved by `trailText` into `history.json`, which defaults to on and is unencrypted JSON. The
redaction helper (`agent.js:504-511`) only masks a `value="..."` on lines ending with `(password)`.
Fix: when the question or the target looks like a credential, use a password input, send the answer to
the helper out-of-band without including it in the model transcript where possible, and write
`Type "••••"` to rows and history. Never store `type` text for password-flagged fields. Say in the
Privacy page that history is plain text on disk.

**M5. API key and consent state at rest.** `lib/store.js:98-135, 33-36, 416-424`.
The key is stored with `safeStorage` (DPAPI), which any process running as the same Windows user can
decrypt; the code falls back silently to a plaintext key if encryption is unavailable (`store.js:126-129`)
with no indication to the user. `agentConsent`, `agentPermission` and the rest of the settings are plain
JSON beside it; any same-user process can set `agentConsent: true` before the first run. This matches
what most Electron apps do and is fine against casual snooping, but it should be described honestly and
the plaintext fallback should be visible.
Fix: on fallback, show "Your key is stored unencrypted on this machine" in Account; store the
consent flag inside a DPAPI-encrypted blob so a plain JSON edit cannot grant it; document that the key
is readable by other programs run by the same user.

**M6. IPC trust is flat.** `preload.js:8-115`, `main.js:2229-2238, 2303-2307, 2408`, `store.js:422`.
Every window (overlay, settings, history, buddy, lens, region) loads the same preload with the whole
allowlist, including `agent:confirm`, `agent:consent`, `settings:set-agent` (which can set
`consent: true`) and `settings:save-key`. Handlers never check `event.senderFrame`/URL, and there is no
`will-navigate` or `setWindowOpenHandler` anywhere in `main.js`. Today CSP (`script-src 'self'`) and the
escaped markdown (`renderer/markdown.js:10-16`, all tags built in code) make exploitation unlikely,
but a single XSS in any window would be able to approve every confirmation. Confirm ids are small
sequential integers (`agent.js:477`).
Fix: split preloads by window with minimum channels (only the answer window may invoke
`agent:confirm`/`agent:consent`); verify `event.sender` against the expected `BrowserWindow` in those
handlers; remove `consent` from `settings:set-agent`; add `will-navigate` deny and
`setWindowOpenHandler(() => ({action: 'deny'}))`; use unguessable confirm ids.

**M7. "Hidden from screen share" is stated more strongly than it holds.** `README.md:3, 280-284`, `settings/settings.html:277-279`, `settings/settings.js:914-918`.
The README's first paragraph says the overlay "stays invisible in screen shares and recordings". The
caveats (hardware capture, some backends) sit in a later section. The Settings chip says "Hidden" in a
flat tone. It does handle the build-below-19041 case correctly and says so, which is the right instinct.
What is missing: (a) the agent's effects are not hidden, because the audience sees the cursor move and
text appear in real apps with no overlay explaining it; (b) the "AevalBuddy is using your computer"
chip and control frame are also hidden from the audience; (c) screen exclusion hides the window from
capture APIs, it does not hide the app from endpoint monitoring, process lists, or a screen reader of
the GPU output; (d) on a work or exam machine, concealment from other people may break a policy, and
the app should not suggest otherwise.
Fix: Privacy copy: "AevalBuddy's windows are left out of screen shares and recordings made by
Windows. What the agent does in your apps is not hidden. This does not hide AevalBuddy from your
employer's software, and some capture hardware still sees it." Put that sentence in the first-run tour,
not only in Settings. Add a per-run option "Show agent frame in screen share" for demos.

**M8. Coordinate and unnamed clicks read as "Click here".** `agent.js:327-329`, `agent.js:392-404`.
When the target has no name (canvas, icon button, coordinate click), `describe` returns "Click here in
Chrome". The user cannot judge it, and the target may have moved between plan and Allow. The re-resolve
loop only protects ref targets (`agent.js:1132-1143`); coordinate clicks are not re-checked after the
card.
Fix: for an unnamed target, read the nearest label or the element's tooltip/automationId, or pull a
cropped screenshot into the card. For coordinate targets, re-run `elementAt` after Allow and
re-confirm if the element's name or window changed.

### LOW

- **L1. History is plaintext and on by default.** `store.js:74`, `history.js:19-75`. Answers can
  contain emails and secrets read off the screen. Copy "Text only, on this machine" should add
  "unencrypted". Consider off by default on first run with an explicit choice.
- **L2. Crash and updater logs.** `crash.js:35-52` writes stack traces only (no screen content), which
  is fine; `DesktopError` messages include window titles (`host.cs:675`), so a titled message could land
  in `errors.log` through an unhandled rejection. Strip titles from logged messages.
- **L3. Cleartext links.** `main.js:2303-2307` allows `http://` through `open-external`. Prefer
  https-only, and show the host before opening.
- **L4. Markdown link labels can lie.** `renderer/markdown.js:24-27` lets a model write
  `[docs.python.org](https://evil.example)`. Show the real host on hover and in a status line, or open
  links through a "Go to evil.example?" step.
- **L5. Test hooks ship in production.** `AEVAL_FAKE_LLM`, `AEVAL_OFFLINE`, `AEVAL_USERDATA`,
  `AEVAL_UPDATE_FEED`, `AEVAL_DESKTOP_CACHE`, `tools/fixtures/**` (`electron-builder.yml`). Gate on
  `!app.isPackaged` or a build flag.
- **L6. Keyboard hook visibility.** The agent enables a `WH_KEYBOARD_LL`/`WH_MOUSE_LL` hook while
  running (`host.cs`, `index.js:536-545`). It reports no key identity, which is good, but corporate EDR
  commonly flags low-level keyboard hooks as keylogger behaviour. Document it in the README so IT is
  not surprised.
- **L7. Stop depends on Esc being registered.** Esc is a global shortcut taken only while something is
  active (`main.js:1293-1302`). If another app holds it, Settings reports a failed combo, but a running
  agent has no warning. Verify that the trail shows "Esc not available, use the Stop button" when
  registration fails.
- **L8. No Electron fuses / ASAR integrity.** `package.json` has no `@electron/fuses` step; the app code
  in `resources/app.asar` can be swapped by anything running as the user. Flip
  `EnableEmbeddedAsarIntegrityValidation`, `OnlyLoadAppFromAsar`, and disable `RunAsNode` and
  `NODE_OPTIONS`.

---

## What is already good (keep)

- `contextIsolation: true`, `nodeIntegration: false`, narrow channel allowlist in `preload.js`, CSP
  `script-src 'self'` on every window.
- Markdown is escaped before any transform and every tag is built in code (`markdown.js:10-16`).
- Own-window refusal on every pointer point and on keyboard targets; `ELEVATED_TARGET`; secure-desktop
  and lock-screen refusal (`host.cs:1777-1786`); `FOCUS_LOST` mid-typing (`host.cs:1873-1877`).
- Password values masked at the source in trees and text (`host.cs:977, 1438`), plus a second redact.
- `open` is scheme-allowlisted, app names are character-restricted, and always confirmed.
- Step, failure and wall-clock limits; takeover pause; Stop settles and answers pending calls.
- The ref re-resolve after the card catches moved targets (`agent.js:1132-1143`).
- Windows builds below 19041 are told plainly that the window is not hidden.
- Screenshots and readings are not written to disk; history is text only and deletable.

---

## Trust UX: what a user would conclude

**Does the user always know what is sent, and to whom? No.**
- Ask: every Enter (including a bare Enter, which sends "What's on my screen and what should I do?",
  `main.js:114`) captures the active display and the foreground window's UI Automation text and sends
  both with the prior turns (`main.js:1483-1485`). No bar chip, no thumbnail, no "sent to X" line.
- Agent: window titles, accessibility trees, field values and sometimes screenshots go out every step
  (`agent.js:529-537` sends a screenshot whenever the tree has under 15 nodes, which is automatic). The
  consent sheet (`trail.js:370-374`) lists what the agent can do and says nothing about where the screen
  goes.
- Find: window text leaves on a typing pause (H6).
- Recipient is never named: the user sees "OpenRouter" at key entry and a model id in the picker. The
  company behind a free model, and its retention terms, appear nowhere.

**Confirmation copy.** The structure is good (what, why, keys). The wording needs work:
- "Asking because the model marked it consequential" speaks about internals; say "The agent says this
  could send or delete something."
- "Why: <model text>" is unverified model output next to a safety control; label it "The agent says:".
- For Enter and submit steps it does not show the message or recipient. Show the first 200 characters
  of the field, the recipient/subject if readable, and the app name. Without that the user is approving
  a verb, not a message.
- For open, show the whole URL (H3). For unnamed targets, show a crop (M8).
- Allow is the focused default and shares a chord with "send" in several apps (H2).
- The consent sheet should have three more lines: where the screen goes, what is never done without a
  card (and that the list is a heuristic, not a guarantee), and how to revoke. "It asks first before
  anything that sends, deletes, buys, signs or opens a link" is a promise the code does not keep (C1).

**"Hidden from screen share".** Mostly honest in the README body, overclaimed in the opening line and
in the Settings chip, and silent about the three things a user on a work laptop most needs to know
(M7).

---

## Rules for "one smart input that works out intent"

When a single field decides between Ask, Find and Agent, the cost of a wrong guess is no longer a bad
answer, it is an action on the user's computer. These rules should be requirements, with tests.

1. **Agent is never the silent result of inference.** An ambiguous imperative ("send this to Sam",
   "delete the old ones", "open Notepad and write a haiku") resolves to a visible, editable guess:
   "Do this on your computer? [Run task] [Just answer] [Find]". The default focus is the non-acting
   option. Only an explicit signal starts an agent run: the user chose Agent (mode chip, shortcut, tray
   item), or confirmed the guess card. Enter never confirms the guess card; it needs a click or a
   distinct chord (not Ctrl+Enter).
2. **Say what the guess is and what each choice sends.** The guess card names the mode, the model, and
   the data each mode sends ("Answer: sends a screenshot and the window text to <model>. Task: lets it
   click and type in <app>").
3. **Hard-code the verbs that never infer Agent.** send, delete, pay, buy, order, submit, post, share,
   install, uninstall, format, reset, transfer, sign, reply, merge, deploy: when present, the guess
   card appears even if the classifier is 99% sure. A classifier is not a consent mechanism.
4. **Bias toward the cheaper mode on a tie.** Order of caution: Find (local first), Ask, Agent. A
   question mark, "how", "why", "what" go to Ask. Quoted words with no verb go to Find.
5. **Remember the intent, not the mode.** Do not reopen the bar in the last mode
   (`main.js:2208, 2616`; `store.js:426-434`). A user whose last task was Agent should not find their
   next question running as Agent. The mode chip resets to the neutral state on every open.
6. **Consent stays separate and un-inferable.** The first Agent run still needs the consent sheet, and
   it cannot be triggered from a guess card the user did not read. `agentConsent` cannot be set by any
   path except the sheet's own button (M6).
7. **No acting from content.** Intent classification reads only the typed text, never the screen or an
   earlier model answer. "Do it" under an answer opens the guess card, not a run (H4).
8. **Show what was inferred afterwards.** The trail's first row says "Started because you typed
   'rename these files by date' and chose Run task", and it is a stored row, so history shows how the
   run began.
9. **Voice and paste are not intent.** Pasted text that contains a newline, or text arriving from a
   non-keyboard source (paste, dictation, drag), cannot start an agent run on Enter; the user has to
   confirm the guess card first. A newline in pasted text must not submit (C1 applies to input fields
   too).
10. **Make the gate independent of the router.** Whatever mode got the run started, the executor-side
    rules from C1/H1/H8 apply unchanged. The classifier decides what the user is offered; it never
    lowers what the gate asks.
11. **Log the decision locally and make it reviewable.** One line per guess: typed text, guess,
    user choice. No screen content. Offer "Always ask before running tasks" as a setting that cannot be
    turned off by the guess card itself.
12. **Test it adversarially.** Add smoke cases: an imperative typed with a trailing newline, an
    imperative pasted from an email, a long-press Enter, a question that contains an imperative, a
    locale with a different verb, and an answer that tells the user to press "Do it".

---

## Suggested order of work

1. C1 (all five parts) and H8: executor-side, small, testable in the existing smoke harness.
2. H2 (Allow focus and chord) and H3 (full URL): copy and renderer changes.
3. H5, H6, M7 and the consent sheet copy: disclosure; no behaviour risk.
4. H4 and the smart-input rules, before the single input ships.
5. H1 (default for unreadable targets, app-aware classes) and M3 (deny-list).
6. H7 (signing, SHA pinning, tag protection) and M1 (DLL verification).
7. M2, M4, M5, M6, M8, then the LOW items.
