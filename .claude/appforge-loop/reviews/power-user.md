# AevalBuddy v2: power-user review

Reviewer stance: lives in Raycast, VS Code and a terminal; has used Cluely and Claude in Chrome.
Read: lib/agent.js, lib/find.js, lib/desktop/index.js, lib/openrouter.js, main.js (hotkeys, Ask flow,
Find/agent wiring), renderer/overlay.js, renderer/output.js (Do it), settings screenshots,
SPEC-v2.md. Looked at 25 of the 36 frames on the `shots` branch (01, 03, 04, 06, 08, 10, 12, 14,
16, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28-1, 29, 30 and a few others).

Evidence tags: [code] read in source, [shot] seen in a screenshot, [infer] my inference, [know] from
my own knowledge of other products; verify before acting on those.

---

## Part 1. Ranked findings

### P0: will lose a power user in the first hour

**1. A global `Enter` hotkey swallows Enter system-wide while the agent is paused.**
main.js:1327-1330 registers `Enter` with `globalShortcut` whenever agent state is `paused` or `plan`.
`paused` is entered the moment the user touches the mouse or keyboard (agent.js:686-697). So the
natural takeover flow is: user clicks into the doc, types a fix, presses Enter, and the Enter is eaten
by AevalBuddy and resumes the agent in the middle of their sentence. A global shortcut is consumed
before any app sees it. [code] Fix: never register bare Enter globally. Resume with a chord that is
registered only while paused (`Ctrl+Enter`) or with the pill/trail click, and show the chord in the
paused row.

**2. The five default global chords collide with VS Code, Chrome and Edge defaults.**
lib/store.js:8-20 (comment claims "G and U are unclaimed by the common Windows apps"). [know, verify]
- `Ctrl+Shift+Space`: VS Code "Trigger Parameter Hints" (a key a dev presses all day).
- `Ctrl+Shift+G`: VS Code "Source Control"; Chrome/Edge/Firefox "Find previous" right after a Ctrl+F.
  The feature is literally called Find, so this is the chord people will hit by reflex.
- `Ctrl+Shift+U`: VS Code "Output panel".
- `Ctrl+Shift+H`: VS Code "Replace in Files".
- `Ctrl+Shift+A`: Chrome/Edge "Search tabs".
Four global chords is the wrong shape for a one-input product. Ship ONE global (suggest
`Ctrl+Alt+Space`, or `Alt+Space` if the user has not got PowerToys Run/Raycast on it), and make Find,
Agent and Region inside-the-bar prefixes (Part 2). Keep the other four rebindable and unbound by
default. Settings already reports a failed register; also report "taken by <app>" is impossible, so
just make it cheap to change.

**3. Chosen text-only models are silently swapped, and a saved choice is rewritten.**
openrouter.js:316-329 `pickLive` treats a model as `usable` only if `kind === 'text'` or it accepts
images. But Find (`kind 'find'`) and Agent (`kind 'agent'`) are text-only jobs. Consequences:
- DEFAULT_FIND is `nemotron-3.5-lightning:free` (openrouter.js:74), chosen as "fastest text model".
  It fails `usable`, so Find silently runs on the first image-capable free model: Nemotron 3.5 Super
  VL, which openrouter.js:114 itself calls "slower to start". Screenshots 08, 10 and 30 show
  `Find model: NVIDIA: Nemotron 3.5 Super VL (free)`. [code + shot] That is a 120B model doing two
  serial calls per keystroke pause. This one bug probably explains most of the "related hits drift in
  late" feel.
- A user who picks the cheap fast paid agent brain (`deepseek/deepseek-v4-flash`, `z-ai/glm-5.3`,
  BRAINS list, openrouter.js:142-164) gets it replaced by the free default at run start
  (main.js:2160) and `store.healAgentModel` (store.js:385-387) rewrites their saved setting to match.
  Silent, and it moves a paid choice onto a slower free model.
Fix: `usable()` must depend on what the kind needs (`agent`: lists `tools`; `find`/`text`: anything),
not on image input.

**4. Ask does not appear until the screenshot is done, then the screenshot is thrown away.**
main.js:1921 `await triggerCapture()` runs before `expandPanel()` (main.js:1155 inside it), so
Ctrl+Shift+Space shows the bar only after desktopCapturer plus the JPEG loop (main.js:1126-1130,
re-encode until under 1.6 MB). Then `handleQuerySubmit` captures again on Enter (main.js:1482-1486), so
the hotkey capture only feeds the "capture failed" notice. [code] The keystroke-to-bar latency is
bound to capture time on a 4K display, and the capture costs again on Enter. Fix: show the bar first,
capture in parallel, and on Enter reuse the pre-capture if the foreground window handle and title are
unchanged (cheap 64x36 thumbnail hash for the rest). SPEC target is 260ms bar open (SPEC 6).

**5. No cost or latency transparency anywhere.**
`grep usage` in openrouter.js, main.js and renderer finds only the key-balance check
(openrouter.js:374). No request sets `usage: {include: true}`, no response `usage.cost` is read, no
token counts, no time-to-first-token, no per-run total. The only price info is the per-million line in
the picker (openrouter.js:199-205). For a product whose headline is "any OpenRouter model", a user
who flips "Show paid models" has no idea what a 20-step agent run cost. Add: cost and tokens per
answer (meta row under the answer), per agent run (trail footer `14 steps · 41s · $0.004`), a daily
tally in Settings and the tray, and a per-run spend cap that stops a run with a named limit row, like
the step and time caps (agent.js:21-22, 947-949). Also show `is_free_tier` and the free-request
budget from `/api/v1/key` (openrouter.js:363); free accounts have a small daily request allowance
[know, verify at openrouter.ai/docs/api-reference/limits], and an agent run spends one request per
step, so "free by default" quietly means "a handful of runs a day". The 429 copy ("take a few requests
a minute", openrouter.js:1242) does not say that.

### P1: makes the headline features feel slow or fragile

**6. Agent: the first model call is spent on a plan nobody waits for.**
RULES (agent.js:117) force `update_plan` first. In the default `risky` mode the plan is displayed and
immediately followed (agent.js:1075-1083), so it is one full free-tier round trip (5 to 15s [infer])
of pure ceremony, then the first action is a second call. Let the model emit the plan in `content`
alongside its first tool call, or make it optional for tasks that fit one sentence. Per-step cost is
the model call, not the engine (shot 22 shows 90 to 410 ms per step), so removing a call is the only
optimisation that matters.

**7. Agent: fixed 250 ms sleep plus a full tree re-read after every changing action.**
agent.js:1183-1188 sleeps 250 ms unconditionally, then `observe` (agent.js:513-543) re-sends the whole
interactive tree (up to 12,000 chars, about 3k tokens) after every click, type and key. The engine
already has a `generation` counter bumped on structure-changed events (desktop/index.js:127-128,
returned by readTree at :684) that the agent never reads. Replace with: wait until generation settles
(poll 40 ms, cap 600 ms), then send a delta (`+ added`, `- removed`, `~ changed`, plus the focused
element and any new dialog) instead of the full tree. Typing into a field should return the field's
new value, not 80 nodes. This is the observation-compression gap against Claude in Chrome, whose
`read_page` can be scoped by `ref` and which re-reads on demand rather than on every action.

**8. Agent: interactive-only observation hides dialog text, which the safety gate then needs.**
`observe` uses `filter: 'interactive'` (agent.js:519). A "Delete 14 items?" message is a static text
node, so the model never sees it; the executor's gate only checks the dialog *title* (agent.js:436-440)
and the control name. Include non-interactive text nodes for any window whose class is a dialog
(`#32770`) or that appeared since the last observation.

**9. Agent: no window argument on read tools, one target at a time, popups invisible.**
`read_page`, `find`, `get_page_text`, `screenshot` take no `window` (agent.js:63-72); only
`focus_window` changes `run.target` (agent.js:604-606). A "copy the total from Excel into the email"
task therefore requires focus ping-pong that also steals the user's foreground. Win32 context menus
and many dialogs are separate top-level windows (class `#32768`, `#32770`) that the target's tree does
not contain, so after a right-click the next observation shows the old window. [infer] Fix: add
`window` to every read tool, and after each action diff `listWindows` for new top-level windows owned by
the same pid and fold them into the observation.

**10. Agent: stale refs cost a model turn and count toward the three-strikes stop.**
The engine throws `STALE_REF` (host.cs:1049); agent.js:555 and :1198 pass it to the model, which must
re-read, then retry. Each failed action counts toward `MAX_FAILURES = 3` (agent.js:1045-1050), so
re-rendering apps (Chromium, Electron, WPF virtualised lists) burn the budget on bookkeeping. Fix in the
engine: remember `{role, name, automationId, rect, parentRef}` per ref, and on stale re-resolve a unique
match from a fresh tree, return `reboundFrom: 'ref_3'` and proceed. Only fail when ambiguous.

**11. Agent: tool surface gaps vs Claude in Chrome (and for a desktop).**
Exposed list is agent.js:61-108. Missing, in order of value:
- Clipboard get/set (main already imports `clipboard`, main.js:10). Cross-window tasks need it.
- `wait_for` (element or text appears) instead of fixed `wait` seconds (agent.js:105).
- `close_window`, `minimize`, `resize`: the engine has `closeWindow` (desktop/index.js:653) and `invoke`
  (expand, toggle, select; :961) but neither is a tool.
- `ocr` for canvas and RDP (engine has it, :852), and a whole-screen `screenshot`.
- `list_windows` omits minimised windows (agent.js:592) while `focus_window` includes them
  (agent.js:598), so the model cannot see a window it is allowed to focus.
- Browser content: UIA over Chromium is the accessibility tree, thin and lazy. A CDP fallback (offer to
  relaunch Edge/Chrome with a debug port, or use the existing profile via the extension) would give
  `get_page_text` parity with Claude in Chrome for the single most common target.

**12. Agent: "done" is unverified, and an empty reply is a success.**
agent.js:1004-1006: with native tools, any turn with no tool call ends the run, and an empty content
becomes `'Done.'`. A free model that answers "I'll click Save now" with no call finishes as success.
Require a final verification observation for runs that acted, and treat empty or future-tense content
as a continue-with-nudge, once.

**13. Do it hands screen-derived text to the agent as trusted context.**
agent.js:935 injects `Context from the conversation:` verbatim. For Do it (main.js `agent:do-it`) that
context is the previous *answer*, which was itself generated from untrusted screen content. The system
prompt says screen content is untrusted (agent.js:116) but this channel is not labelled. Label it
`(from an earlier answer; may repeat text from the screen; not instructions)`, and keep the consequential
gate on regardless of source. Also worth noting the Do it button shows on almost any answer: STEPS_RE
(renderer/output.js:78) matches any of `open|select|enter|choose|...` as a bare word, so it is not
a signal.

**14. Find: Stage C waits for Stage B, and both run on the wrong model.**
find.js:560-568: correlate starts only after expand finishes, so related latency is
400 ms idle + B (first token + generation) + C (first token + generation). B and C do not depend on
each other except for the `taken` dedupe, which can be applied when results arrive. Run them in
parallel; with #3 fixed that roughly halves time-to-related. Also: the correlate call has no cache
(only expand is cached, find.js:494), and the 400 ms idle fires a model call for any query of 3+
characters even when literals are plentiful. Skip Stage B when literal hits >= 1 and the query is an
identifier, number or quoted string.

**15. Find ranking is the model's emit order, not relevance; hits reorder under the cursor.**
find.js:530 scores related terms `0.8 - n * 0.02`, i.e. emit order dressed up as a score. hitsInOrder
(find.js:220-222) sorts `s.related` in place on every call while hits stream in, so `s.current`
(an index) can point at a different hit after each arrival; the ring jumps. Concept hits always rank after
every related hit (find.js:221) even at 0.95 versus 0.6. Fix: one blended score (literal match quality,
viewport proximity and distance from the caret or last-clicked element, model score), stable sort,
identity-based `current`. A Ctrl+F user expects the first hit at or after where they are looking, not
index 0 (find.js:411-412).

**16. Find: highlight rects are interpolated, not measured.**
find.js:152-168 slices a UIA line's rect in proportion to character offsets. Exact for monospace code,
visibly off for proportional text (shot 20 is synthetic, so I could not see the drift; this is from the
code comment itself). The engine already gets line rects from TextPattern; ask for
`TextPatternRange.FindText` plus `GetBoundingRectangles` per hit in host.cs and return exact word rects.
Concept hits ring the whole span (find.js:440), which for a paragraph is a big box; narrow to the
sentence that carries the answer.

**17. Find: missing the Ctrl+F basics a power user checks in ten seconds.**
No case-sensitive, whole-word or regex toggle; multi-word queries get no token or fuzzy matching
(`single` guard at find.js:201, :210); no "3 of 7" position counter (status only reads `2 exact ·
1 related`, find.js:252); no Up/Down to move through the list (overlay.js:292-296 only Enter and
Shift+Enter); no "copy all hits". The `/` prefix grammar below absorbs most of this: `/re:\d{4}`,
`/"exact phrase"`, `//literal only`.

**18. Find to Ask drops almost everything it knew.**
main.js:2000-2008 sends `Explain this from my screen: "<match>"` as plain text after `closeFind()`
(which also clears the lens). Lost: the hit's `why`, the surrounding span, the window, the rect, the `ref`.
For a concept hit `match` is the whole span text (find.js:437), which can be a paragraph in quotes. Ask then
re-captures the full screen and the model has to find the thing again. Better: set the hit's rect as the
Ask region (the region path already exists, main.js:1167-1186, and sends a tight, higher-fidelity crop),
attach the hit span as exact text, keep the lens up until Ask answers. There is also no Find to Agent
path at all; see the chip design in Part 2.

**19. Ask ignores the app's own text when deciding what to send.**
SPEC 7.4 says "Perception order is UIA, then OCR, then vision" but handleQuerySubmit always sends the
image (main.js:1599) and adds UIA text as a side block (main.js:1482-1486, 1727). For text-dense
windows (terminal, IDE, document, web page with a readable tree, >= ~800 chars) a text-only fast model
answers "what is this error" with no upload and a far smaller TTFT. Route to text when UIA text is rich and
the intent is textual; send the image only when it is not or the question is visual ("which button",
"what colour").

**20. The default Ask model is a reasoning model, and the stream hides the reasoning.**
DEFAULT_MODEL is `nemotron-3-nano-omni-30b-a3b-reasoning` (openrouter.js:51). `readSse` only forwards
`delta.content` (openrouter.js:993-999), so reasoning tokens produce no visible progress; the user sees
thinking dots until the first answer token. No `reasoning: {effort: 'low'}` or `exclude` is sent, and no
TTFT is measured, so nobody can see this regression. Either send a reasoning budget for Ask and Find, or
surface a one-line reasoning summary ("Reading the terminal output") as it streams. Add a hedge: if no
first token in ~3.5 s on a `:free` model, race the next free model and cancel the loser. Free queues make
tail latency the experience.

**21. Cold-start costs land on the first keystroke.**
Nothing starts the desktop helper before first use (no `desktop.start` call in main.js); readExactText
races a 4,000 ms timeout when cold (main.js:1731-1732) inside `Promise.all` with capture, so the first Ask
of a session can wait up to 4 s before the request is even built. startAgentRun awaits
`fetchCatalogue` (8 s timeout) and `resolveLive` before starting (main.js:2159-2161). Prewarm
`desktop.start()` (and the catalogue) on bar open, not at boot (boot time is a gate, SPEC B29), and
prefetch the foreground window text while the user types so Enter finds it ready.

### P2: keyboard-only flow and polish

**22. No Up-arrow history recall.** overlay.js:276-319 has no ArrowUp/ArrowDown handling; the history
window is mouse-first (shot 16) and stores conversations, not inputs. Add per-mode recall: Up/Down walks
the last 50 inputs (all modes, shown with their prefix), `Ctrl+R` opens a fuzzy search over them, and
Find queries are in there too. Store inputs separately from conversations so "Save history: off" does
not disable recall of commands (or keep them in memory only).

**23. Tab is a trap, and its meaning changes under you.** overlay.js:282-288 calls `preventDefault` on
every Tab: no keyboard path to the region, settings or close buttons (they are tab-stoppable but never
reachable), and the same key means "next mode" or, with text in Find, "toggle scope" with no visible hint
beyond a title tooltip (index.html `find-scope`). Tab with text in Ask/Agent switches mode, which is
surprising for anyone used to Tab = complete. Suggested: Tab accepts the ghost-text suggestion or the
classifier's runner-up (Part 2); `Alt+1/2/3` switches mode; `Alt+S` toggles scope with the key printed on
the scope button.

**24. Single-line input.** renderer/index.html uses `<input type="text">`. Pasting a stack trace or a
multi-line error flattens it, and Enter during IME composition submits (no `e.isComposing` guard,
overlay.js:290). Use an auto-growing textarea: Enter sends, Shift+Enter newline, guard composition.

**25. Do it, hit rows and trail rows are mouse-only.** Do it (renderer/output.js:132-141) has no key;
results rows have no Up/Down selection; the confirm card has chords (good). Add `Alt+Enter` = Do it /
act on this hit, and arrow selection in the results list.

**26. Screenshot 20 (lens) shows the agent control chip.** The frame is meant to show Find hits but
carries "AevalBuddy is using your computer . Esc to stop" and a top-edge frame line. Either the harness
leaked agent state into the Find shot, or the lens can show the frame during Find. Worth a check in
lib/lens.js. The why-chip in the same frame is two lines and covers the hit below it; clamp chip text to
~60 chars and place it on the side with more room.

**27. Agent error recovery is reactive.** Shot 25: the run plans three steps, then fails on the first action
because the target is elevated. `elevated` is already in listWindows (desktop/index.js:51-52). Check the
target at run start and say it before spending a model call.

**28. Four model slots with developer names.** Settings rows `Screen model`, `Answer model`, `Find model`,
`Agent model` (shot 30) plus `Show paid models`. For one input that works out intent, the user should
choose a speed/quality dial (Fast, Balanced, Best) with the cost shown, and a single Advanced section
for per-intent overrides. The current labels expose the pipeline (reader and brain) rather than the job.

**29. Trail does not say which model ran.** Shot 22/24 show verbs and ms but not the model, token use,
total time or cost, and there is no way to see what the model was shown (the observation). Add a
collapsed "what it saw" per step for debugging, and the model name in the footer.

---

## Part 2. One input that works out intent

### 2.1 Design rules

1. The text is the person's, the mode is a guess. Anything typed without a prefix is classified live;
   the guess is shown, never hidden, and costs one keystroke to change.
2. Local and instant first. The classifier is pure JS, budget 30 ms hard, no network. LLM only breaks
   ties, only off the critical path, and may only *suggest*.
3. Never auto-pick the dangerous one. Ask and Find are cheap and reversible, Agent is not. Agent
   starts only from an explicit `>` or from a high-confidence imperative that is then shown as a plan with
   a confirm before the first action.
4. Doubled prefix means "more careful or narrower": `//`, `>>`, `??`. One rule to learn.

### 2.2 Prefix grammar

Borrowed from places this user already types: `/` is search in Vim, less and browsers; `?` is a question;
`>` is "run a command" in VS Code's palette; `@` scopes in Slack and VS Code symbols.

```
input    := [stage] ( "|" stage )*
stage    := [prefix] [scope]* body
prefix   := "/"   Find        /invoice      /"due date"     /re:\d{4}-\d{2}
          | "//"  Find, literal only, zero model calls, works offline
          | "?"   Ask         ?why is this red
          | "??"  Ask, deep: best model, image forced
          | ">"   Agent       >rename these files by date
          | ">>"  Agent, confirm every action
          | none  classify locally
scope    := "@" name    @window (default) @screen @sel @clip @region @chrome @"Untitled - Notepad"
          | "#" name    #debug #rewrite (answer style)  #fast #best (model tier)
chip     := carried subject, not typed: [hit 2/5] [answer] [region] [selection]
```

Rules:
- A prefix is only a prefix at column 0 of a stage. `/etc/hosts` is a path: if the first token has a
  second `/` with no space it is classified as text, not Find; `/ etc` or `/"etc"` forces Find.
- `@app` resolves against `listWindows` titles and process names with the same scorer as
  `desktop.find` (desktop/index.js:772-795). Ambiguous names show the candidates as ghost text; Tab
  picks the first.
- Backspace at column 0 on a prefix removes it and returns the stage to "auto". Typing a prefix character
  at column 0 replaces the guess. Both are one keystroke.
- `#fast` `#best` choose a tier, not a model id; ids stay in Settings. `#nocost` (alias `#free`)
  pins free models only.
- Empty Enter keeps today's behaviour (default question, main.js:114).

### 2.3 Intent classifier: local heuristics first

Runs on every input event after the first character, in this order, first confident rule wins.
Measured cost target: median under 2 ms, hard cutoff 30 ms (`performance.now()` guard returns "ask" and
logs if exceeded).

| # | Signal | Result | Confidence |
| --- | --- | --- | --- |
| 0 | Explicit prefix | that intent | 1.0 |
| 1 | Ends with `?`, or starts with a wh-word (what, why, how, where, who, when, which), `is/are/does/do/can/should`, `explain`, `summarize`, `translate`, `tell me`, `meaning of` | Ask | 0.9 |
| 2 | Starts with an imperative app verb: open, launch, click, type, press, go to, close, rename, move, save, fill (in), send, create, delete, download, install, scroll, select, check/uncheck, enable/disable, `please`, `can you` + verb, `help me` + verb | Agent (plan + confirm, never silent) | 0.8 |
| 3 | Quoted string, identifier shape (`camelCase`, `snake_case`, `0x..`, UUID, hex, path, IP, version, money, date), number, or 1 to 3 tokens with no verb and no wh-word | Find candidate | 0.6 |
| 4 | **Present on screen**: the whole input (or each token) occurs in the prefetched foreground-window text index (the same snapshot Find and Ask-exact-text use). indexOf over 20k chars is under 1 ms | upgrades 3 to Find at 0.85. Not present and short: Ask 0.6 | 0.85 |
| 5 | `then`, `and then`, `->`, `|` in the text | split into stages, classify each | n/a |
| 6 | Personal overrides: leading-token to intent counts from past corrections (>= 2 corrections flips the rule) | per history | 0.9 |
| 7 | Everything else | Ask | 0.5 |

Signal 4 is the one a keyword grammar cannot do: the app knows what is on the screen. "invoice" typed over
an invoice is a Find; "invoice" typed over a terminal is a question.

Decision: `margin = best - runnerUp`. At or above 0.3 the guess is shown solid. Below that it is shown
dim with the runner-up beside it (`Ask ↵ · Find ⌃↵`).

When unsure (margin under 0.3 and the user pauses 600 ms or presses Enter):
- Enter runs the default (Ask, the safe one) immediately. No waiting on a model.
- In parallel, one 40-token LLM classification on the Find-tier model (cached by normalised input,
  prefix-free). If it returns a different intent at >= 0.8, a one-line suggestion chip appears
  (`Looks like a task. Do it ↵`), it never switches by itself.
- Agent can never be the result of an LLM classification alone.

### 2.4 Showing the guess and overriding it with one keystroke

- The mode word at the left (`Ask`, `Find`, `Agent`, overlay.js:42-61) becomes live: it updates as you type
  (cross-fade `--d-fast`), solid when confident, 55% alpha with the runner-up when not. It is the
  explanation and the control in one place.
- **Tab** while the guess is dim cycles to the runner-up. While solid, Tab accepts ghost text (scope or
  history completion).
- **Ctrl+Enter** runs the runner-up instead of the guess. (Today Ctrl+Enter in Find means "ask about this
  hit" and on a confirm card means Allow; those become `Alt+Enter` = act on this hit, and the card keeps
  Ctrl+Enter because its context is unambiguous.)
- **Typing a prefix character at column 0** replaces the guess; **Backspace at column 0** returns to
  auto. `Alt+1/2/3` still jump modes.
- After sending: the answer or results header carries `Not this? Find ⇥ · Do ⇥` for 8 s. Tab reruns the
  same text as the other intent on the **same capture and UIA snapshot** (no second screenshot).
- Learning: each override increments a counter for the first token; two corrections flip the local rule
  for that token (stored locally, resettable in Settings).

Worked examples:

| Typed | Screen | Guess | Why |
| --- | --- | --- | --- |
| `why is the build red` | Terminal | Ask (0.9) | rule 1 |
| `invoice` | Invoice in the page | Find (0.85) | rule 4, present on screen |
| `invoice` | Terminal | Ask (0.6), dim, runner-up Find | not present |
| `rename these files by date` | Explorer | Agent (0.8), plan card first | rule 2 |
| `/etc/hosts` | any | Ask about a path | second `/` without a space |
| `/re:\d{4}` | any | Find regex | explicit |
| `>> send the draft to Sam` | Outlook | Agent, confirm every action | explicit, doubled |
| `how do I rename these files` | Explorer | Ask, with a Do it chip | rule 1 beats rule 2 |

### 2.5 Chained intents

Two ways, same model: each stage's output becomes the next stage's *subject*.

**A. Chips (no syntax, the default).** After a Find, the current hit becomes a chip at the left of the
input: `[hit 2/5 "Invoice 2291"]`. Whatever you type next is about that subject:
- `?` or no prefix: Ask about it. The Ask uses the hit's span text as exact text and its rect as the
  region (finding #18), so the answer is about those pixels, not the whole screen.
- `>`: Agent receives the hit's `ref` and window handle as a hint ("the user highlighted ref_41 in
  window w2B3"), not a paraphrase.
- Backspace on the chip dissolves it. Esc closes Find as today.
Other chips: `[answer]` after an Ask (typing `>` means Do it, without the button), `[selection]` when the
bar was opened with text selected, `[region]` (existing chip).

**B. Pipes, for people who think in shells.** `/invoice | ?sum these | >paste it in Excel`.
- `/` stage emits `{hits[]}`; `?` stage gets all hit texts as exact text (sums are exact because the
  strings are exact); `>` stage gets the previous answer as **untrusted** context (finding #13).
- Feel: one rail of pills under the bar, `/ invoice ✓ 3 hits` then `? sum these…` then `> paste…`, each pill
  a live status. Stage 1 is instant (local Find), so the rail visibly advances without waiting on a model.
- Edit any pill to rerun from there. Esc stops the current stage and keeps the earlier ones.
- Safety: an Agent stage inside a pipe always shows its plan and confirms the first mutating action, even
  in `risky` mode, because its context came from other stages.
- Failure: a stage with zero output stops the pipe and says so (`/invoice` found nothing, so `?sum these`
  did not run).

---

## Part 3. What would make this clearly better than Cluely

Cluely's strengths (from the spec and my use): invisible overlay, one hotkey, a screenshot plus live meeting
audio. AevalBuddy already matches the first two. Features that Cluely structurally cannot follow, in
priority order:

1. **Answers that show their source.** Because the app reads UIA text, every answer can carry source chips
   (`Terminal line 41`, `Invoice 2291`). Click a chip: the lens rings the exact span on screen. A
   screenshot-only tool cannot prove where it read something. Needs: exact word rects (finding #16) and
   the spans passed into the answer prompt with ids.
2. **Find, Ask, Do as one pipeline** (chips and pipes above). Cluely is Ask-only; Claude in Chrome has no
   Find. The composition is the product.
3. **Honest cost and speed on every action.** Per-answer and per-run cost, TTFT, free-request budget,
   spend cap, Fast/Balanced/Best dial. Cluely is a subscription; yours is transparent pay-as-you-go or free.
4. **Meeting help without audio: read live captions through UIA.** Teams, Zoom and Meet render live
   captions as accessible text. `?what did Sam just ask me` with `@captions` reads the caption lines
   directly: free, exact, no STT model, no WASAPI. SPEC 6 defers audio to v2.1; this is the v2 answer and
   it respects the free-model constraint.
5. **Run journal and Undo.** Each mutating agent action records its inverse where one exists (type: the
   previous value or Ctrl+Z; setValue: old value; click: none, flagged). `Undo run` replays inverses
   in reverse. Claude in Chrome cannot do this; for a desktop agent it is what makes "do it" safe to say
   yes to.
6. **Record a successful run as a macro.** Save the steps with refs stored as `{role, name, automationId}`
   (the stale-ref re-bind in #10 is the same machinery) and replay with no model calls: zero cost, about
   1 s per step. Palette entry `>@morning-report`. This turns the slow free-tier loop into a one-time
   cost.
7. **Select, then one key.** Open the bar with text selected (read via UIA TextPattern selection): the
   bar shows `[selection]` and Tab cycles Explain, Translate, Rewrite, Fix, Find elsewhere. No typing, no
   screenshot.
8. **Watch queries.** Pin a Find (`/"build failed"`) so the lens stays live and a toast fires when it
   appears. Combined with a macro, an unattended reaction. The poll loop already exists (find.js:645-647).
9. **Local-first guarantees you can show.** A status line per request: `Sent: 1 screenshot, 3.2 KB of
   window text` / `Sent: nothing (local Find)`. Pair with `//` literal-only for zero-network use. Privacy is
   a feature for the person sharing a screen, and the app already hides itself from capture.
10. **Speed that you can feel.** Bar opens before capture, helper and catalogue prewarmed on open, window text
    prefetched while typing, image reused across follow-ups, text-only routing when UIA text is rich, hedged
    free-model requests. Target: Enter to first token under 1.5 s on a paid fast model and under 4 s p50 on
    free.
11. **One global hotkey, everything else by prefix.** Fewer collisions, easier to teach, matches how
    Raycast and the VS Code palette are used.
12. **Multi-window agent tasks** (window argument on read tools, clipboard, popups folded into the
    observation). Where Claude in Chrome is limited to a tab, this is the reason to exist.

---

## Part 4. Suggested order of work

1. Findings 1, 3, 4, 21 (small, high impact, mostly one-liners: Enter hotkey, `pickLive`, bar-before-capture,
   prewarm).
2. Finding 5 (usage, cost, TTFT) because it makes every later speed claim measurable; add `usage:
   {include: true}` to `request()` bodies (openrouter.js:28-41) and a `ttft` field to every call.
3. Findings 6, 7, 14 (remove a model call, delta observations, parallel Find stages), measured with the
   new cost and TTFT numbers.
4. The single input: prefix grammar, local classifier with the on-screen-text signal, live mode word,
   Ctrl+Enter runner-up, history recall (finding 22).
5. Chips and pipes, then source chips, then macros and Undo.

Things done well, worth keeping: executor-side safety gate (agent.js:419-474), the confirm card with
visible chords (shot 23), the control frame and Stop reachability, content protection on every window,
the trail verbs (shot 24), the "no connection" and "free models busy" copy (shots 14, 27), and a literal
Find that works offline and with no key.
