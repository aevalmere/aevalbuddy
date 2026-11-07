# Accessibility review: AevalBuddy v2 (WCAG 2.2 AA, Windows Narrator / NVDA)

Sources read: `renderer/*` (html, js, css), `settings/*`, `history/*`, `lib/lens.js`, `lib/region.js`,
`lib/cursor-buddy.js`, `lib/tray.js`, `main.js` (window focus, global shortcuts, Escape, agent flow), `DESIGN.md`.
All 36 screenshots on the `shots` branch were downloaded and viewed one by one. Nothing was run, and no screen
reader was driven. Evidence tags: (code) read from source, (shot) measured on a screenshot, (inferred) follows
from how Windows, Chromium or the screen readers behave and needs a real Narrator/NVDA pass to confirm.
No product files were edited.

Severity: **Blocker** = a keyboard or screen-reader user cannot complete the task. **High** = WCAG A/AA failure
with a workaround. **Medium** = AA failure of limited reach. **Low** = polish.

## Bottom line

Visually the app is careful (a 2px focus outline everywhere, `prefers-reduced-motion` blocks in five stylesheets,
labelled icon buttons, `role=alert` on errors, text-only history). Structurally it was built mouse-first:

1. In the bar, **Tab is hijacked** to cycle modes, so every other control in the bar is unreachable by keyboard.
2. The **answer / trail window is never reachable by keyboard**. It is shown with `showInactive()`, and nothing
   moves focus into it except a confirm card or the consent sheet. Stop, Retry, Copy, Do it and Find's results
   are mouse-only, and a screen reader is never told an answer arrived.
3. The agent's **confirm card auto-focuses Allow** in a window that has just stolen focus, and a **global Enter**
   is registered while the agent is paused.
4. Streaming, the step trail and the Find hit count are announced as **whole-subtree rewrites** inside a
   `role=log`, which is the worst case for NVDA and Narrator (repeats, or silence).
5. The area selector, the layout editor and the tour are **pointer-only**.
6. Several state markers (the accent `#823A80`) are **1.6 to 2.3:1**, and no stylesheet has a `forced-colors`,
   `prefers-contrast` or `prefers-reduced-transparency` branch.

The glass + one-input redesign can fix most of this by construction, but only if the requirements in
section 10 are treated as acceptance gates. As specified in DESIGN.md today it would make 3 and 6 worse.

---

## 1. Keyboard operability and focus order

### A1. Blocker. Tab in the bar is swallowed, so Settings, Close, Area, "Open settings" and the status buttons are unreachable
- `renderer/overlay.js:282-289`: `if (e.key === 'Tab' && !e.ctrlKey && !e.altKey) { e.preventDefault(); ... cycleMode(...) }`.
  Focus starts in `#query-input` (`overlay.js:163`), and Tab/Shift+Tab never leave it. `#mode-label`, `#region-btn`,
  `#settings-btn`, `#close-btn`, `#region-clear`, `#bar-status-action`, `#find-scope` (the "This window" toggle,
  whose only keyboard path is Tab-with-text, which is itself the hijacked key) and the no-key notice's
  "Open settings" button (`index.html:91`, the one thing a first-run user must press) have no keyboard path.
  WCAG 2.1.1, 2.1.2, 2.4.3. (code)
- Fix: delete the Tab handler. Keep Ctrl+1/2/3 (`overlay.js:277`) and add `Ctrl+Tab` / `Ctrl+Shift+Tab` (or
  `Alt+Down` on `#mode-label`) to cycle. Make the mode control an ARIA `radiogroup` or `tablist` with arrow keys so
  it is one Tab stop. Find's scope toggle moves to `Ctrl+Shift+S` plus its button. Put `aria-keyshortcuts`
  on the controls. Natural DOM order is then mode, input, area, settings, close, status actions. The same Tab
  line also drives the screen-reader "I pressed Tab and the label changed" confusion.

### A2. Blocker. The answer, trail and Find results window cannot be reached by keyboard
- `main.js:631-649` (`showOutput`: `outputWin.showInactive()`), `main.js:1297-1331` (no shortcut focuses it), and the
  only `outputWin.focus()` calls are `main.js:2031` (confirm/asking) and `2147` (consent). The panel stays focused
  while the answer streams (`main.js:563-588`), and `#chat` has no `tabindex`, though the CSS comment at
  `overlay.css:605` says it is focusable. Result: Stop (`output.js:109`), Copy (`:122`), Do it (`:129`), Retry
  (`:211`), Find rows (`trail.js:59`), the trail's Stop (`trail.js:135`), Start/Continue (`trail.js:268`) and the
  history/new-chat buttons are mouse-only. Esc in the bar collapses it, aborts the query and hides the answer
  (`main.js:590-596`), so "Esc then read" destroys what a keyboard user was about to read. WCAG 2.1.1, 2.4.3. (code)
- Fix: (1) add a panel-level key, `F6` (and `Ctrl+Down`), that calls `outputWin.focus()` and focuses `#chat`
  (`tabindex="0"`, `aria-label="Conversation"`); `F6` or `Esc` inside the answer returns focus to the bar
  (`overlayWin.focus()`, `#query-input`). (2) When the person is a screen-reader user (see C4) also move focus to
  the answer when it completes. (3) Preferred for the redesign: render the transcript inside the same window and
  document as the input, so there is one Tab order and one live-region owner (section 10).

### A3. High. Escape and Enter are global shortcuts
- `main.js:1318-1323` registers `Escape` globally whenever the bar is anything but a pill, and `main.js:1324-1330`
  registers `Enter` globally while the agent is `paused` or in `plan`. A global shortcut is consumed before any
  window sees it. While either is armed, Esc and Enter stop working in every other application, including the one
  the user is trying to take over after "Paused: you took over. Enter to continue" (`lib/agent.js:696`, `:1076`).
  Narrator and NVDA users press Enter and Esc constantly. WCAG 2.1.4 (character key shortcuts), 2.1.1. (code)
- Fix: never register a bare Enter. In `paused`/`plan`, put focus on the trail's Continue/Start button
  (`trail.js:268`) and use `Ctrl+Enter` (matching the confirm card) as the only global key, registered only while the
  trail window is not focused. For Esc: register only while the agent runs or a selector is open; while the bar is
  open rely on the focused window's own handler (`overlay.js:331`) and collapse on blur.

### A4. High. Confirm card focuses the risky button and steals focus mid-keystroke
- `trail.js:310`: `setTimeout(() => allow.focus(), 30)`, after `showTrail(true)` calls `outputWin.focus()`
  (`main.js:2022-2031`). If the user is typing in the target app (or has just pressed Enter/Space), the next key
  lands on **Allow**, which is the control the safety gate exists to protect. `Ctrl+Enter` / `Ctrl+Backspace`
  (`trail.js:315-322`) are also not announced anywhere except in keycap text. WCAG 3.2.2, 3.3.4 (spirit), 2.4.3. (code)
- Fix: focus the card itself (`tabindex="-1"`, `role="alertdialog"`, `aria-labelledby` the sentence,
  `aria-describedby` the two reason paragraphs) or **Skip**, not Allow. Ignore Enter/Space for the first 400 ms after
  the card appears. Keep Ctrl+Enter / Ctrl+Backspace, add `aria-keyshortcuts="Control+Enter"` on Allow and
  `"Control+Backspace"` on Skip, and add `Esc` = Skip when the card has focus. After resolution, return focus to the
  trail Stop button (or to the target window, as the agent focus rule already does) and announce the result.

### A5. High. The area selector is a drag-only gesture
- `renderer/region.js:68-93`: mousedown, mousemove, mouseup, and Esc to cancel. No keyboard path. WCAG 2.1.1, 2.5.1,
  2.5.7 (dragging alternative). (code)
- Fix: arrow keys move a default rectangle, Shift+arrows resize, `Enter` confirms, `Esc` cancels (the window
  already has keyboard focus, `lib/region.js:70-87`). Add "This window" and "This monitor" choices in the bar
  (`Alt`-less buttons) so a region is optional. Put `role="application"` plus an `aria-live` size readout on
  `#size` (`region.html:31`), and make the `#hint` (`region.html:33`) readable text with a role of `status`.

### A6. High. Layout editing is drag-only; resizing is edge-drag only
- `overlay.css:989` and `output.html:34-44`: `-webkit-app-region: drag`; the "Done" and "Dock/Detach" buttons are the
  only non-drag controls. There is no keyboard way to move the bar or the answer, snap to a corner, or resize.
  WCAG 2.5.7, 2.1.1. (code)
- Fix: in the HUD add a 3x3 "Snap to" button group (top-left ... bottom-right) per window, arrow-key nudge (8 px,
  Shift = 32 px) while the HUD has focus, and width/height steppers in Settings > Window layout
  (`settings.html:381-387`). Persist through the existing `store` calls.

### A7. High. The guided tour is pointer-only
- `lib/cursor-buddy.js:379` and `:393`: the buddy window is `focusable: false` and click-through; its buttons
  (`buddy.js:182-192`, Skip, Try it, Later, close) cannot receive focus or keys. A keyboard-only person who starts
  the tour (and the tour starts on first run) has a message they cannot dismiss except by clicking elsewhere.
  WCAG 2.1.1, 2.2.1 (the typed message, see C3). (code)
- Fix: for card steps (`kind === 'card'`, `buddy.js:199`) make the window focusable and focus the primary button;
  bind `Esc` to skip globally for the tour's duration (it is already a registered global during the walkthrough,
  `main.js:1320`), and `Enter` to "Next" only when the card has focus.

### A8. Medium. Settings widgets: roles without the keyboard model
- Radio groups `settings.html:202-205`, `:228-231` and the skill list `settings.js:466-496` are `role=radio` on
  `<button>`s, each a separate Tab stop, with no arrow-key handling and no roving tabindex. Screen readers announce
  "radio button, 1 of 2" and then Arrow does nothing. WCAG 4.1.2, 2.1.1 pattern. Fix: roving tabindex +
  Arrow/Home/End handler in one helper used by all three groups.
- Every "Edit" button is named just "Edit" (`settings.html:50,91,113,...` eight to ten times) with no
  `aria-expanded` / `aria-controls`, and `openSection` (`settings.js:34-41`) rewrites the text to "Done". Fix:
  `aria-expanded`, `aria-controls="<sec>-body"`, and `aria-label="Edit API key"` (use the group heading via
  `aria-labelledby="key-heading this-id"`). The `.summary` (e.g. "Connected") should be tied with `aria-describedby`.
- Rail buttons (`settings.html:27-36`) smooth-scroll the column but do not move focus; the next Tab continues from the
  rail. Fix: after `showPage` focus the `<h1>` (`tabindex="-1"`). `aria-current="false"` is also emitted
  (`settings.js:411`); set the attribute only on the current one.
- Shortcut capture (`settings.js:735-826`): while recording, every key including Tab is `preventDefault`-ed
  (`:793-795`); the text "Press keys" is not in a live region and Esc is the only exit that is not announced.
  Fix: `aria-live="polite"` on the row, text "Press the new keys, or Esc to cancel", and let Tab through.
- Settings buttons that carry state as colour only: see section 4 (`.choice-option` selected has no mark).

### A9. Medium. History: nested interactive, delete without confirm, focus drops
- `history/history.js:69-109`: `.del` is a `span role=button tabindex=0` **inside** a `<button class="item">`. That
  is invalid nesting (screen readers flatten it and Narrator reads it as one control; the inner one is skipped in
  browse mode). Deleting a single conversation (`:169-188`) has no confirm and no undo, and
  the list is rebuilt (`render`, `:111`), so focus falls to `<body>`. WCAG 4.1.2, 3.3.4, 2.4.3. (code)
- Fix: make the row an `<li>` containing two sibling buttons (open, Delete "<title>"). After delete, focus the
  next row (or the search box), and announce "Deleted. Undo" in `role=status` with a 6 s undo (hold the write).
  `countEl` (`:124`) should be a `role=status` so search results announce "3 conversations".

### A10. Low. The pill is a dead focus target
- `index.html:13` gives the pill `role="button" tabindex="0"` and `overlay.js:250-255` handles Enter/Space, but the
  window is `focusable: false` (`main.js:426`), so Tab/UIA focus can never land on it. It is not harmful, but
  it advertises an affordance that does not exist and its name never changes. The real keyboard entry is the
  global shortcut (default `Ctrl+Shift+Space`) and the tray (Win+B). Fix: say so in onboarding for keyboard users,
  keep the shortcut discoverable in the tray tooltip (`lib/tray.js:173`), and make the pill's `aria-label`
  follow its visible text (C5).

---

## 2. Focus-visible

- Overlay: `overlay.css:81-86` is a 2px accent outline plus a 1px slate halo. The halo is what makes it pass. Good.
- **High. Settings and History have no halo**: `settings/settings.css:73-77` and `history/history.css:72-76`
  draw only `2px solid #823A80`. Measured: accent vs the settings background `#1B1C21` = **2.32:1**, vs the history
  panel about the same; vs the overlay skin `#262930` **1.99:1**. Fails 1.4.11 (3:1) for the focus indicator.
  Fix: use the same two-ring pattern everywhere (`outline: 2px solid #C9CFDD; box-shadow: 0 0 0 4px #823A80` or
  `outline: 2px solid var(--accent-bright)` which is 5.2:1 on `#1B1C21`, 4.5:1 on `#262930`, 3.2:1 on `#3E404A`).
- **High. Inputs deliberately remove the ring**: `#query-input:focus` (`overlay.css:336-341`),
  `input[type=password|text]:focus` (`settings.css:364-369`), `#search` (`history.css:147`), `.ask-input:focus`
  (`overlay.css:1285-1288`). Their only cue is a slate-at-0.3 border: measured **2.13:1** against the panel. The
  bar's caret is accent `#823A80` at **1.99:1** (`overlay.css:325`). WCAG 2.4.7, 1.4.11. Fix: border-colour
  solid `#C9CFDD` (6.6:1 on the top of the skin) when focused, plus a 2px bottom bar. A mid-grey such as `#7C8190`
  is only 2.65:1 on `#3E404A`, so do not pick a "just enough" grey.
- `.combo-option:focus` sets `outline: none` and a 0.09 slate background (`settings.css:494-499`): the keyboard
  cursor in the model list is a 1.1:1 tint (shot 10 shows it only on the selected row). Fix: 2px inset outline in the
  accent-bright.
- `:focus-visible { border-radius: 4px }` (`overlay.css:85`, `settings.css:76`) re-rounds 6/8/12 px controls on focus,
  which makes the focus ring jump shape. Drop the radius override.
- `.link`, `.edit`, `.retry-btn`, `.stop-btn` keep the global ring. `#chat-tools button` is `opacity: 0` until the
  panel is hovered (`overlay.css:572-579`); a focused one is revealed at `--chrome` (0.55), but a screen-magnifier or
  low-vision pointer user cannot find an invisible control. Keep the buttons at the 0.55 idle value, as every other
  chrome icon does.

---

## 3. Roles, names, live regions

### C1. High. Streaming answer is rewritten wholesale inside `role=log`
- `output.html:29` `#chat role="log" aria-live="polite"`, and `output.js:229-239` calls
  `renderMarkdownInto(liveMsg.body, liveMsg.raw)` every animation frame, which sets `innerHTML` for the whole body
  (`markdown.js:108-110`). Each frame removes and re-adds the entire answer. NVDA re-reads the growing text
  repeatedly or drops it (inferred, the well-known "innerHTML in a live region" failure); Narrator may announce
  only the first chunk. WCAG 4.1.3. (code)
- Fix: while streaming, set `aria-busy="true"` on `#chat` and do not live-announce the body. Keep a separate,
  visually hidden `<div role="status" aria-live="polite" id="sr-announcer">`. On `query:start` write
  "Thinking about your screen". On `query:done` write "Answer ready." and, if the user chose it (setting "Read
  answers as they finish", default on when a screen reader is detected, C4), set the announcer text to the first
  300 characters and move focus to the answer on request (A2). Render markdown with DOM diffing (append text nodes,
  only replace the block that changed) so even a non-live read does not flicker.

### C2. High. The thinking dots and stages are silent or duplicated
- `output.js:183` `dots.setAttribute('aria-label', ...)` is on a `<span>` with no role, which is ignored. With no
  stage label (`setThinking(entry, true)` at `output.js:345`) a screen-reader user gets no feedback after
  pressing Enter. The input is also cleared on submit (`overlay.js:222`) with no spoken confirmation. WCAG 4.1.3.
- Fix: announcer text "Asking" on submit; `role="status"` on `.think-dots` is not needed once the announcer
  exists. Use `aria-hidden="true"` on the dots and put the stage text (`stage-label`) in the announcer.

### C3. High. Agent step trail is announced twice, and state is not exposed
- `trail.js:220-236` `putRow` does `old.replaceWith(li)` for every update, inside the same `role=log`. A step is
  announced once as "running" and again as "done" with the duration ("240ms") because the whole `<li>` is new.
  `.trow-mark` is `aria-hidden` (`trail.js:161`) and the state word is only drawn for failed/skipped
  (`STATE_MARK`, `trail.js:155`); "done" and "running" have no text, only the accent dot (1.9:1) and the three
  dots. Status headline "Working · step 5" (`trail.js:254`) is another text mutation in the log. WCAG 1.3.1, 4.1.2, 4.1.3. (code)
- Fix: each row is `<li>` with a visually hidden prefix (`<span class="sr-only">Running: </span>`, `Done:`,
  `Failed:`, `Skipped:`); update the existing `<li>` in place (`textContent` of the state span) instead of
  replacing it; take the trail out of `role=log` and add the announcer from C1. Announcer lines, one per step,
  short and in the order they happen: "Step 5: typing notes.txt in Notepad", "Waiting for you: Click Save in
  Notepad. Control Enter to allow, Control Backspace to skip.", "Agent finished. Saved notes.txt to your
  desktop." Mark the trail `aria-label="Agent steps"`, and the headline with `aria-live="off"`.

### C4. High. No screen-reader detection, so nothing changes for the people who need it
- Electron exposes `app.isAccessibilitySupportEnabled()` and the `accessibility-support-changed` event; the code
  never reads them (grep: none). Windows move without focus, never announce arrival, and `showInactive()` is used
  for everything (`main.js:440, 605, 646, 797`). (code)
- Fix: read that flag in main and send `a11y:{sr:true}` with `overlay:state`. When true: focus the answer window on
  `query:done` only if the bar has focus (A2), speak the announcer text, hold the answer window open on Esc
  (collapse bar only), and slow the buddy typing (C6). Add a Settings > General "Screen reader mode" override
  (Auto / On / Off), because detection by Chromium is by UIA client connection and is not 100 percent.
- Unverified (inferred): whether Narrator or NVDA speak a live-region change in a topmost, non-focused window. NVDA
  accepts some topmost-window events, Narrator generally follows the focused app. Test (section 12) before relying
  on `aria-live` alone for agent narration; the fallback is C4's focus move plus an earcon.

### C5. Medium. Names that drift from the visible label (2.5.3, 4.1.2)
- Pill: visible text is "Ask" / "Find" / "Agent" / "Step 5" / "Paused" (`overlay.js:145-149`); the accessible name
  is always "Ask AevalBuddy about your screen" (`index.html:13`), and "chat / busy / agent / error" live only in a
  6 px dot (`overlay.css:156-195`). Fix: set `aria-label` to `${visible text}, ${state}` such as "Step 5, agent
  running. Open the trail" and give the dot a text equivalent.
- `#mode-label` is named "Mode: Ask. Click to switch." (`overlay.js:48`): visible word is not first, and it
  teaches "click". Fix: `aria-label="Ask, mode"` with `aria-haspopup`/`aria-keyshortcuts`.
- `#region-btn` lit state (`overlay.css:484-487`) is colour only; add `aria-pressed="true"` and text in the chip
  (already "Asking about a 640 x 320 area", fine).
- `consent` sheet (`trail.js:364-394`) has no role or label: make it `role="alertdialog"` with
  `aria-labelledby` the title and `aria-describedby` the list.
- `#find-view role=listbox` holds `button role=option` rows (`trail.js:59-63`) plus a `<p>` when empty
  (`:52-56`): invalid listbox children, and no arrow-key handling or `aria-activedescendant`.

### C6. Medium. Typed-out tour and buddy text
- `buddy.html:28` `#callout role="status"`; `buddy.js:153-161` sets `textContent = text.slice(0, i)` every 16 ms.
  A live region receiving a new string every 16 ms is announced as a stream of fragments or not at all. WCAG 4.1.3,
  2.2.2 (moving text with no pause). (code)
- Fix: write the full text to the live region once (`calloutMessage.textContent = text`) and draw the typing
  effect on an `aria-hidden` copy; skip the effect when `accessibilitySupportEnabled`. `prefers-reduced-motion`
  is already honoured (`buddy.js:21`).

### C7. Medium. Find: the hit count is not a usable announcement
- `renderStatus()` (`overlay.js:75-100`) writes `"2 exact · 2 related"` plus stage lines into `#bar-status
  role=status` on **every keystroke** (`overlay.js:321-326`, `queryFind` with no debounce on the status). Narrator
  repeats intermediate states ("Searching related...", then the count). Stepping with Enter
  (`overlay.js:292-296`) announces nothing: the ring moves on a screen the SR user cannot see, `aria-selected`
  flips on a row that has no focus (`trail.js:63`), and the `#why` chip is `role=status` in a window that never
  has focus (`lens.html:15`, `lens.js:69-70`). The literal/related distinction is a filled dot vs a ring that is
  `aria-hidden` (`trail.js:45`), so it is lost. WCAG 1.3.1, 4.1.3. (code)
- Fix: (1) debounce count announcements to 500 ms after the last keystroke and announce only when the count
  changes: "4 matches: 2 exact, 2 related". (2) On Enter/Shift+Enter write to the announcer "Match 2 of 4, exact:
  Invoice 2291 for March" or "Match 3 of 4, related: Please settle by Friday. Settling relates to invoice." (3) Make
  the input `role="combobox" aria-expanded="true" aria-controls="find-view" aria-activedescendant="hit-N"` while
  in Find so the standard "option, 2 of 4" speech comes for free; give rows ids; add a visually hidden "Exact
  match" / "Related match" in each row. (4) Keep the lens `#why` as `aria-hidden` and duplicate its text in the
  result row (it already is, `trail.js:83-88`), so there is a single announcement path.

### C8. Medium. Status and update lines
- `settings.js:658` re-renders `#update-line` (`role=status`) every 30 s with a recomputed "Checked 3 min ago",
  so a SR announces it every minute while Settings is open (`settings.html:370`). The download text
  "Downloading 1.2.0: 12 of 40 MB, 30%" changes on every progress event in the same live region. Fix:
  `aria-live="off"` on the line; announce only transitions (checking, downloading started, ready, failed) via a
  separate status; give the bar `role="progressbar"` with `aria-valuenow` (`settings.html:371` has none).
- `makeStatus` (`settings.js:61-74`) hides the element (`hidden`) and shows it again, and auto-clears after 3 s.
  `role=status` content added at the same time as display is changed is commonly not announced. Fix: keep the
  region in the layout (visually collapsed with CSS), set text after a 100 ms delay, clear after 8 s (2.2.1).
- `clear-history` (`settings.js:686-707`): the label silently becomes "Delete everything?" for 4 s. Fix: use a real
  confirm (`<dialog>`/in-place with Delete and Keep, as `history.html:45-49` already does), announce it, and do not
  time out.
- `error-row role="alert"` inside a `role=log` (`output.js:199`, `trail.js:348`): fine for errors, but the
  "agent finished" note-row has no role, so success is silent (C3).

---

## 4. Contrast (measured on the screenshots)

Method: for each region, the most common colour is the background and the pixel farthest from it in luminance is
the foreground, ratio per WCAG. Thin 12px anti-aliased text under-reports by a few tenths, so treat the number as a
lower bound. Theoretical values from the CSS tokens (alpha blended on the skin gradient `#3E404A` to `#262930`) in
brackets.

| Item | Where | Measured | Verdict |
| --- | --- | --- | --- |
| Query placeholder (slate 0.45) | shot 03 | 2.77:1 [3.1 / 2.6] | Fail 1.4.3 (it is the hint for what the mode does) |
| History search placeholder (0.38) | shot 16 | 2.47:1 | Fail |
| Settings summary while its group is open (0.35) | shot 10, `settings.css:207-209` | 2.47:1 | Fail |
| Region icon / region button lit (accent) | shots 13 | 1.62 to 1.73:1 | Fail 1.4.11 |
| Error icon (accent `#823A80`) | shot 14, `overlay.css:829-832` | 1.78:1 | Fail 1.4.11; uses the "action" colour for an error |
| Running-step mark (accent) | shot 22 | 1.88:1 | Fail 1.4.11 |
| Pill state dot (slate 0.30 / accent) | shot 01, `overlay.css:156-195` | 2.17:1 idle | Fail 1.4.11; state is only in the dot |
| Idle chrome icons (0.55 opacity) | shot 03 | 3.44 to 3.83:1 | Pass 1.4.11 (marginal at the top of the gradient: 3.18) |
| Secondary text slate 0.70 on `#3E404A` (top of skin) | theory | [4.14:1] | Fail 1.4.3; DESIGN.md:38-40 claims "4.5:1 on every surface". Needs alpha at least 0.76 there, or solid `#B2B8C6` |
| Confirm "Skip" link on twilight | shot 23, `overlay.css:419-426` | 4.31:1 [4.32] | Fail 1.4.3 |
| Primary text on skin | shots 04, 22 | 7.25 to 8.8:1 | Pass |
| Secondary text on settings/history dark (0.70) | shots 08, 16 | 5.7 to 5.8:1 | Pass |
| Trail done / read rows, ms column | shot 22 | 4.89 to 4.98:1 | Pass |
| Allow / Open settings / Try it / Save (slate on accent) | shots 02, 23, 28 | 4.69:1 | Pass (barely; 13px semibold is "normal" size) |
| Danger `#E4818C` on skin / on white | theory | 5.42 / **2.69** | Pass on dark; the `--danger-edge` rim is the only help on a pale backdrop |
| `--accent-bright` ring on white / on mid grey | theory | 3.26 / 1.21 | Needs the rim (it has one) |
| Find current row tint (accent 22%) | shot 19 | tint vs panel **1.13:1** | Selected state is tint only |
| Settings selected radio row (accent 20%) | shot 29 | **1.14:1**, no mark on `.choice-option` | Fail 1.4.1 / 1.4.11 |
| Switch off track vs page | shot 08 | 1.47:1 (thumb 7.4:1) | Pass (thumb position carries state) |
| Window edge `--edge` (0.17) | all | 1.52:1 | DESIGN.md:70-72 says the edge exists for a "3:1 boundary requirement"; it is 1.5:1 |
| Focus outline accent in Settings/History | `settings.css:73` | 2.32:1 | Fail (section 2) |

Fix list: (1) text tokens become **solid** hex (no `rgba` text), e.g. `--text-secondary: #AEB4C2` (measured by formula: 4.96:1 on `#3E404A`, 7.0:1 on `#262930`, 5.05:1 on twilight
`#303D6A`, which also fixes the Skip link); placeholders use the same value (the hint is information).
No token below 4.5:1 on its own surface. (2) Introduce `--accent-ui: #C56FC3` (already `--accent-bright`, 4.5:1 on the dark skin) for
every accent **mark** (icons, dots, rings, focus), and keep `#823A80` only as a fill behind slate text.
(3) Error icon uses `--danger`. (4) Selected states get a non-colour cue: a check, a filled radio dot, or a 2px
leading bar at 3:1 (`.choice-option`, `.find-row.current`). (5) Add a contrast assertion to `npm run shots`
(sample the same regions this review sampled) so DESIGN.md's "measured 4.5:1" claim is enforced. (6) `history.css:253`
`.meta` is 11px, below the 12px floor DESIGN.md:676-678 says was applied.

---

## 5. Target size (2.5.8: 24x24 CSS px, or spacing)

- `#chat-tools button` 20x20, 2px gap (`overlay.css:556-570`): fails (circles overlap). Make them 24x24.
- `.del` in History is 22x22 and lives inside another button (`history.css:265-277`): fails. 24x24 and separate (A9).
- `#callout-close` 20x20 (`cursor-buddy.css:294-300`): fails; 24x24.
- `.copy-msg` and `.do-it` ("Copy", "Do it"): 12px text, `padding: 2px 8px`, about 20px high, 4px apart
  (`overlay.css:741-747, 1380`): fails. Use `min-height: 24px; padding: 4px 10px`.
- `.edit` (`settings.css:179-187`), `.link` (`:576-583`), `#region-clear`, `#bar-status-action`, `#find-scope`: about 18-20px high but
  isolated; they pass only through the spacing exception. Any future crowding breaks that; set `min-height: 24px`.
- Pass: pill 104x40, panel icon buttons 32x32 (`--control`), Stop, Retry, Allow/Skip rows, settings controls 32px, rail 29px.

---

## 6. Reduced motion and reduced transparency

- Good: `@media (prefers-reduced-motion: reduce)` in overlay, settings, lens, cursor-buddy and region CSS. The buddy
  skips the typing effect and the glide (`buddy.js:21-23, 80, 135`). The lens shows hits at once (`lens.css:147-156`).
- Medium: it **kills every animation including opacity** (`overlay.css:1054-1061` uses `animation: none !important`),
  whereas DESIGN.md:29-30 promises "every animation dies except opacity". Entrances therefore pop. Fix: keep opacity
  transitions at 100 ms and kill only transform and size.
- Medium: window motion reads the preference **once at boot** (`main.js:447-450`). Toggling "Show animations in
  Windows" mid-session is ignored until restart. Fix: subscribe via `systemPreferences.getAnimationSettings()`
  (Electron 38 has it) on `nativeTheme.updated`, or re-query on `window.matchMedia(...).addEventListener('change')`
  in the overlay and send `motion:reduced` on each change.
- Low: `history/history.css` has no reduced-motion block (only 150 ms colour transitions); add the same rule.
- Low: the pill dot `pill-breathe` (`overlay.css:170-195`) is the only infinite decorative loop; it is removed by
  the blanket rule, but then "busy" is only a static accent dot. Pair it with a text change ("Working") so removing
  motion does not remove information.
- Reduced transparency: nothing to honour today because surfaces are opaque (DESIGN.md:173-176). This becomes a
  requirement in section 10. There is no `prefers-reduced-transparency`, `forced-colors` or `prefers-contrast`
  query anywhere (grep: none).

---

## 7. Windows high contrast / forced colours

No `@media (forced-colors: active)` or `prefers-contrast` anywhere. Chromium turns forced colours on in Electron
when a Windows contrast theme is active (inferred; Electron 38 also exposes `nativeTheme.shouldUseHighContrastColors`
and `nativeTheme.inForcedColorsMode`). Likely behaviour (inferred, test on the runner with the Aquatic/Night sky
theme):
- **Lens hits may turn into opaque rectangles.** `.hit.literal` is only `background: rgba(...)` plus `box-shadow`
  (`lens.css:58-61`); box-shadow is dropped and background colours are forced, so the box that is supposed to
  highlight text may cover it. This is the most serious forced-colours risk in the app.
- Selected rows (`.find-row.current`, `.choice-option[aria-checked]`, `.skill-option[aria-checked]`, `.combo-option.selected`)
  are tints only: the selection disappears.
- The custom switch (`appearance: none`, `settings.css:308-337`) loses its on/off colour; only the thumb position remains.
- `--skin` gradients are replaced by Canvas, which is fine; the 1px hairlines are `rgba` borders and are forced to
  `ButtonText`, which is fine; the `#frame` control border (`lens.css:113-120`) loses its inner rim.
- Fix (one block per stylesheet):
  ```css
  @media (forced-colors: active) {
    .hit { forced-color-adjust: none; background: transparent; border: 2px solid Highlight; }
    .hit.literal { border-style: solid; }  .hit.related, .hit.concept { border-style: dashed; }
    #ring { forced-color-adjust: none; border: 3px solid HighlightText; outline: 2px solid Highlight; }
    #frame { forced-color-adjust: none; border: 3px solid Highlight; }
    [aria-checked="true"], .find-row.current, .combo-option.selected { outline: 2px solid Highlight; }
    .switch input:checked { background: Highlight; } .switch input::after { background: ButtonText; }
    :focus-visible { outline: 2px solid Highlight; box-shadow: none; }
    .accent-btn, .primary { border: 1px solid ButtonText; }
  }
  ```
  Dashed vs solid replaces fill vs outline as the literal/related cue.

---

## 8. Text scaling and DPI

- Everything is px (font sizes 11-16, windows pinned by main): pill `PILL = {104, 40}` (`main.js:83`), input height
  clamped 62-280 (`:87-89`), output 90 up to a fraction of the work area. Nothing in the repo responds to a UI
  scale, `zoomFactor`, or Windows "Text size" (grep: none), and Chromium web content generally ignores that Windows
  setting (inferred). People at 150-200 percent text get either unscaled text or clipping: the pill label "Step 5" /
  "Paused" in a 104x40 box (`overlay.css:100-113`), `.summary` truncated with an ellipsis (`settings.css:168-177`,
  `white-space: nowrap`), the `.sc-row` grid, and history rows (`white-space: nowrap`, `history.css:228-257`). WCAG 1.4.4,
  1.4.10, 1.4.12. (code)
- Only 1x screenshots exist (the answer shot is 420 px wide); no 125/150/200 percent evidence.
- DPI handling in the lens is deliberate (`screen.screenToDipRect`, `main.js:1989`), which is right for per-monitor DPI.
- Fix: (1) add Settings > General > "Interface size" (100 / 125 / 150 / 200 percent) that calls
  `webContents.setZoomFactor` on every window and multiplies `PILL`, the min/max limits and the bounds helpers
  by the same factor. (2) Replace `white-space: nowrap` with wrapping or a tooltip plus `title` on the truncated
  summary. (3) Let the pill size follow its content (`overlay:resize` already exists for the panel). (4) Add
  shots at 150 and 200 percent plus a "large text" variant to `npm run shots` and view them.

---

## 9. Screen-reader announcement of agent actions and confirmations

Summary of the path today: agent step to `agentEmit` to `outputWin.webContents.send` (`main.js:2011-2018`) to a DOM
mutation in a **non-focused, topmost** window, with the target app (Notepad, Outlook) holding focus and being
driven by UIA invoke/typing. The user hears the target app's focus changes and nothing from AevalBuddy except,
at a confirm, a stolen focus. The pill's "Step 5" and the control frame chip "AevalBuddy is using your computer
- Esc to stop" (`main.js:2048`) are visual only: `#chip role=status` lives in the never-focused lens window
(`lens.html:17`) and is mutated while `opacity: 0` (`lens.js:113-114`).

Requirements:
1. One announcer (C1/C3) with the lines in C3, spoken at: run start ("Agent started. Escape stops it"), each step, each
   pause/plan, each confirm, run end (done / stopped / did not finish, with the final message).
2. Confirm and ask-the-user prompts take focus (they already do) **and** are named and described (A4), spoken
   with the buttons and their keys: "Click Save in Notepad. Why: save the file. Asking because the control is
   named Save. Allow, Control plus Enter. Skip, Control plus Backspace."
3. The control frame chip's text is mirrored into the announcer at run start; the global Esc is documented there.
4. A stop key that works without a pointer exists (Esc), but "moving the mouse stops it"
   (`settings.html:245`) is a hazard for people using Mouse Keys, switch/eye-gaze pointers or a trackpad
   tremor; add a setting "Do not stop when the mouse moves" and keep Esc.
5. Interaction with Narrator itself: Narrator is also a UIA client driving focus. Test that agent keystrokes and
   Narrator's own cursor do not fight (inferred risk; section 12).

### Are the confirmation dialogs accessible? Answer: partly
- Good: `role="alertdialog"` with an `aria-label` equal to the action (`trail.js:286-287`), real `<button>`s, a
  documented keyboard shortcut, `Allow` and `Skip` both disabled after first press (`trail.js:299-300`), the safety gate
  is code not a prompt (`DESIGN.md:610-615`).
- Not accessible: focus starts on Allow (A4); no `aria-describedby` so the reasons are not read; `aria-modal` is not
  set and the card sits inside a `role=log` live region, so it is announced twice; the consent sheet has no role;
  shortcuts are announced only as keycap text; the card has 4.31:1 for the Skip link; focus is not restored after the
  answer; and Enter/Esc interplay with the global shortcuts in A3. The card must also be reachable and answerable
  without ever touching the mouse, which it is once A2 is fixed.

---

## 10. Requirements for the liquid-glass redesign (translucent surfaces)

Facts from this repo that constrain the design: all surfaces are opaque today by decision (DESIGN.md:165-176)
because `backgroundMaterial` acrylic is incompatible with `transparent: true` and broke z-order and repaint; real
backdrop blur can only be an OS material or a drawn imitation. Text colours are `rgba` slate alphas, which become
**lower contrast the more translucent the surface is**. Measured on a worst-case backdrop (pure white), a dark tint
`#26292F` needs these opacities for slate text `#C9CFDD`:

| Text | Tint opacity needed over white (4.5:1) | Over yellow `#FFEB3B` | For 7:1 |
| --- | --- | --- | --- |
| Primary, solid slate | 0.78 | 0.75 | 0.91 |
| Secondary, slate at 0.70 alpha | **0.93** | 0.92 | not reachable |
| White text | 0.64 | 0.59 | n/a |

So with the current text tokens the "glass" can only be about 7 to 22 percent translucent on a bright document.
Translucency is therefore decoration that has to be bounded, not a layout driver.

R1. Fallback tiers, chosen in main and set as `data-surface` on `<html>` of every window:
  - `solid` (today's `--skin`, no blur): used when `prefers-reduced-transparency` matches (verify it flips with
    Windows "Transparency effects" on the target build; if not, read
    `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\EnableTransparency` in main and push it),
    when forced colours / high contrast is on, when a user setting "Surface: Solid" is chosen, when the OS material is
    unavailable (Windows build below 22621 for `backgroundMaterial`), when battery saver is on, and when a contrast
    check fails (R3).
  - `glass` (translucent plus blur) only when none of those apply.
  - Both tiers share one token set so nothing is designed only for glass. Ship `solid` as the default for any
    first run on an unrecognised build; a missing fallback is a release blocker.
R2. **Text never sits directly on glass.** Every text run sits on a plate whose background is computed to hold at
  least **4.5:1 (7:1 for text under 14px) against the worst-case backdrop** (white, yellow, mid-grey, and a
  high-frequency black-on-white text backdrop), not against the nominal desktop. With blur the tint floor is about
  0.80 for primary text (table above); use that or an adaptive tint. Convert all secondary/placeholder text to solid
  hex values that meet 4.5:1 on the plate; **no alpha text**.
R3. Adaptive option (recommended): the windows are excluded from capture (`setContentProtection`), so main can sample
  the real backdrop through `desktopCapturer` at 2 Hz while the bar is open and raise the tint (never lower
  than the floor in R2) when the sampled luminance or variance is high. A deterministic floor is the contract;
  adaptation is an improvement.
R4. Non-text contrast: controls, the edge, the focus ring and state marks hold **3:1 against both the plate and an
  arbitrary backdrop**. Use a two-tone focus ring (2px `#C9CFDD` inside, 2px `#101114` outside) and a two-tone window
  edge (1px light, 1px dark) instead of a single translucent hairline (today 1.5:1). Keep every state on shape or text,
  not on blur, tint or specular highlight.
R5. Forced colours: glass collapses to `Canvas`/`CanvasText`; `Highlight` for focus and selection; `forced-color-adjust: none`
  only for the lens marks, with explicit system colours (section 7).
R6. Motion and light effects: refraction, specular sweep and "morph" transitions run only in `data-motion="full"`;
  under reduced motion they become a 100 ms opacity change; nothing loops; nothing flashes more than 3 times per second.
  Window bounds animation reads the live preference (section 6).
R7. Sizes: glass padding and shadow must not shrink hit targets (24 px minimum, 32 px for chrome) or clip focus rings.
R8. Test matrix baked into `npm run shots` (and `installer.yml`): every surface rendered over white, black,
  `#808080`, yellow, a saturated red photo and a black-on-white text page, in `glass` and `solid`, at 100 and 200
  percent, plus forced colours. Fail the run when the contrast sampler used in this review reports under 4.5 (text) or
  under 3.0 (UI). This replaces the unmeasured claim in DESIGN.md:38-40.

## 11. Requirements for the single smart input that infers intent

Today the three verbs have different side effects (Ask reads the screen, Find highlights, Agent controls the
computer), different Enter semantics (send / next match / start, `overlay.js:290-318`), and different safety
levels. Inferring between them hides all of that. Requirements:

N1. **Intent is visible, programmatic and announced before it matters.** A text chip ("Ask", "Find on screen", "Do a
  task") sits next to the input, never colour or icon only, on a solid plate (R2). It is linked to the input with
  `aria-describedby`. A visually hidden live region (`role="status"`, polite, `aria-atomic`) announces "Interpreting as:
  Find on screen. Enter goes to the next match. Control 1, 2 or 3 to choose." only when the inferred intent
  **changes**, debounced about 700 ms after the last keystroke, never per character. On submit it announces what is
  starting: "Asking about your screen", "Finding invoice, 4 matches", "Starting the task: plan ready, waiting for you".
N2. **Override without a pointer, without the Tab key.** Keep Ctrl+1/2/3 (already in `overlay.js:277`), add
  `Alt+Down` on the chip to open a listbox of intents (combobox pattern), `Ctrl+Enter` = "run as Ask" and an
  after-the-fact "Run as <other intent> instead" action in the status line (the "Ask instead" button at
  `overlay.js:80-95` generalised), reachable by Tab. Document them in `aria-keyshortcuts` and in Settings >
  Shortcuts.
N3. **Predictable, not twitchy.** Once the person picks an intent manually it is locked until the input is cleared.
  No re-inference while the person is typing within 500 ms, never by a model round trip before Enter works (use
  deterministic local rules first, a model only as a tie-break after Enter, and say so). Inference must not move focus,
  resize the bar or swap the placeholder after the user has started reading it.
N4. **Never infer into side effects silently.** An inferred Agent intent always lands on the existing plan step
  ("Press Enter to start", `agent.js:1076`) regardless of the "Ask for risky steps" permission, and the plan
  is read out. Ask and Find are read-only and may run immediately. The agent's confirm gate (code, not prompt) stays
  as is.
N5. **The input's accessible name is stable** ("Ask, find or do something"); the intent lives in the chip and the
  announcer. Today `input.aria-label` is rewritten on every mode change (`overlay.js:50`), and a SR re-reads the whole
  label only on focus.
N6. **One output region with per-intent structure.** Headings or `role="region"` with names ("Answer", "Matches on
  screen", "Agent steps"), one announcer, focus moves by `F6`/`Ctrl+Down` (A2), and the hit count / current match /
  step lines from C3 and C7 are the same strings in the announcer, on screen and in the pill's accessible name.
N7. **Errors and ambiguity are text.** "Could be Ask or Find; showing Ask. Alt+Down to change." is exposed as text
  in the status line, not as a tooltip or a colour change. Wrong guesses are recoverable in one keystroke.
N8. **Voice and dictation.** Dictation (Win+H) and Voice Access type into the field through input events: the
  inference must tolerate partial words, must not auto-submit, and the visible chip text must equal the spoken name
  (2.5.3).
N9. **The collapsed pill carries the same state** (intent, "thinking", "step 5", "paused", "answer ready") in its
  accessible name and in a tray tooltip, so a user who never sees the pill still hears it.

---

## 12. Verification plan (do not rely on this review alone)

Because nothing here was run with a screen reader, items tagged (inferred) must be confirmed on a Windows 11
GitHub runner (`installer.yml`, installed exe, per the project rule) and once by hand with each tool:
1. Narrator and NVDA (latest): press the global shortcut, type, Enter, wait for the answer: does it speak "Asking",
   then "Answer ready"? Can F6 reach the answer? Does the streaming text repeat? (C1, A2)
2. Agent run in Notepad: are step lines, the confirm card and the end state spoken? Does Esc and Ctrl+Enter work with
   Narrator running? Does focus return to Notepad? (C3, C4, A4)
3. Find: type, Enter, Shift+Enter: is "Match 2 of 4" spoken? (C7)
4. Windows contrast themes (Aquatic, Desert, Dusk, Night sky) with the lens visible over text: are hits opaque? (section 7)
5. Keyboard only, no mouse: first run to a saved key, one answer, one Find, one agent run, history delete, shortcut
   change, layout move. Every step must be completable. (A1 to A9)
6. 125, 150, 200 percent display scaling and Windows Text size 150 percent, on a laptop at 1366x768. (section 8)
7. Run axe-core against each HTML page in the shots harness (Playwright-for-Electron) for names, roles, ARIA validity,
   nested-interactive and contrast; the sampler used here covers what axe cannot (the lens, canvas-like marks).

## 13. What to fix first (ordered)

1. A1 (Tab trap), A2 (reach the answer), A4 (confirm focus), A3 (global Enter/Esc): the keyboard blockers.
2. C1, C3, C4, C7: one announcer, no wholesale live rewrites, SR detection, Find announcements.
3. Contrast tokens: solid text hex, `--accent-ui` for marks, non-colour selected states, focus ring on Settings/History.
4. A5, A6, A7: region selector, layout editor, tour without a pointer.
5. Forced colours block, reduced-motion fidelity and live updates, interface size setting, target sizes.
6. Build the glass tier on top of this, behind R1 to R8 and N1 to N9.
