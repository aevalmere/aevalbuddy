# AevalBuddy v2.1 plan: one input, safe by construction

Merged from the five reviews in `reviews/` (designer, ease-of-use, accessibility, power-user,
trust-safety), SPEC-v2.md and DESIGN.md. Owner direction, verbatim: "one text input with like prefix
or some way to auto figure out what they want to do instead of 3 seperate or toggle yk? it has to be
like elegant." Liquid glass is Phase B and lands on the hooks Phase A leaves.

## The idea in one paragraph

One hotkey opens one capsule input. A local scorer (`lib/intent.js`, pure, under 5 ms) reads what is
being typed and the text of the window in front, and a token chip at the left of the field shows the
guess as a word: `Ask`, `Find`, `Do`. Prefixes lock it (`?`, `/`, `>`, `//`). Ask and Find are
read-only, so a wrong guess costs a second and is recovered with one link. Do changes things, so it is
never the silent result of a guess: an inferred Do stops on an editable guess card that Enter cannot
confirm. The executor's safety gate stays independent of all of this and fails closed.

## Decisions (conflicts between reviews, resolved)

| Question | Reviews said | Decided | Why |
| --- | --- | --- | --- |
| Name of the third verb | ease-of-use, power-user, accessibility: "Do"; designer: keep "Agent" as a noun | **Do** on the token, tray, trail and settings copy. The settings page and model row become **Tasks** / **Task model** (nouns, per DESIGN's "settings rows are nouns") | three of five reviews; "Agent" is jargon to the everyday user, and "Do" reads as the verb it is |
| Tab | designer and power-user: Tab swaps to the runner-up; accessibility: Tab must move focus | **Tab moves focus, always.** Override is Ctrl+1/2/3 and Alt+Down (opens the intent list) | Tab is the only key a keyboard user has to leave the field; the task brief forbids Tab |
| Runner-up chord | power-user: Ctrl+Enter runs the runner-up; trust: no Ctrl+Enter anywhere near acting | No runner-up chord on Enter variants. Alt+Down picks; result rows carry "Ask instead / Find this instead" | Ctrl+Enter is a send chord in chat apps; it must never be one keystroke from an action |
| Inferred Do | designer: infer at >= .85 else stay Ask with a ghost; trust and accessibility: never silent | Inferred Do shows the token `Do?` and, on Enter, a guess card in the bar: **Just answer** (default, Enter), **Run task** (click or Alt+Enter while the card shows), **Find** | Enter cannot confirm; the default is the non-acting option; the text stays editable |
| Explicit Do (`>`, Ctrl+3, tray) | all | runs, subject to consent (first run) and the executor gate | explicit intent is consent to try, not to every step |
| Do it / Retry / Find to Do | trust H4 | the run starts **plan-gated**: the first call must be `update_plan`, non-read calls before Start are refused by the executor, Start is a click in the trail; the handed-over text travels as `<untrusted>` data outside the Task line | screen-derived text cannot become an instruction by a click on a friendly button |
| Find with no hits | ease-of-use: roll over; designer: offer the alternate | inferred Find with zero exact hits on Enter rolls over to Ask with a one-line note; a locked `/` Find offers "Ask instead" and stays | the lock is the person saying "search", the guess is not |
| Find's model stages | trust H6: off until disclosed; power-user: they are slow because of the wrong model | new setting **Related matches: Ask first** (default) / **Automatic** / **Off**. In Ask-first, the status row offers `Look for related (sends 3,214 characters to <model>)`; Automatic shows that disclosure line while it runs; B and C run in parallel on the right model | nothing leaves the machine on a keystroke pause unless the person turned that on, and the count is always shown |
| Global chords | power-user: one global; ease-of-use: keep Ctrl+Shift+Space | **one global, Ctrl+Alt+Space.** Area, Find, Do and Hide are unbound by default and rebindable. Existing installs that still hold an old default are migrated once; custom chords are kept | Ctrl+Shift+Space is VS Code's parameter hints, G is Chrome/VS Code find-previous and Source Control, U and H are VS Code panels, A is Chrome's tab search |
| Esc | designer: Spotlight behaviour; accessibility: never global while the bar is open | Esc clears text, then a lock, then collapses, handled by the focused window. The only global Esc is while a Do run is going (Stop) or the tour is running | a global key is consumed before any app sees it |
| Settle easing | designer asks; owner approved | `--ease-settle: cubic-bezier(.34, 1.3, .64, 1)` (about 5%), used only for the token's lock and small shape changes, never on windows or sheets | owner approval, recorded in DESIGN.md as an amendment to "nothing bounces" |
| Ask with rich window text | power-user #19, SPEC 7.4 | text-only request when the window text is at least 800 characters and the question is not visual (which/where/colour/icon/button/look); otherwise the image as now | perception order UIA, then pixels, was already a rule and was not followed |
| Launch at login | ease-of-use: on; store comment: off | on by default **for fresh installs only**, announced once by a toast with a one-click off; existing installs keep what they had | a disappearing app after the first reboot is the likeliest loss of a happy user; turning it on silently for people who already chose is what people uninstall for |
| Update feed override | trust H7: https only, not in packaged builds | honoured unpackaged, or packaged with `AEVAL_SMOKE` or `AEVAL_UPDATE_TEST`; https, or http only to loopback | the update-check gate serves a local http feed on 127.0.0.1; loopback cannot be pointed at another machine |
| Prebuilt helper DLL | trust M1 | the build writes the DLL's SHA-256 into `lib/desktop/prebuilt.json` (inside app.asar); the bootstrap loads the bytes only if they hash to it, via `Assembly.Load(bytes)`; packaged builds never load the user cache and ignore `AEVAL_DESKTOP_CACHE` | the hash lives somewhere the DLL's folder cannot rewrite in the same step |

## Ranked by impact

1. The executor gate (trust C1, H1, H8): it is what "it asks first" means. Fail closed.
2. The global Enter/Esc swallow (power P0-1, a11y A3): breaks every other app the user touches.
3. The unified input (owner direction, ease P0-2): the headline change.
4. Key onboarding and presence (ease P0-1, P0-4): where people abandon.
5. Disclosure (trust H5, H6, ease P0-3): who receives the screen.
6. Keyboard and screen-reader blockers (a11y A1, A2, A4, C1, C3).
7. Speed bugs that are one-liners (power P0-3, P0-4, #21, #14) and the visible TTFT/cost readout.
8. Agent loop efficiency and correctness (power #6, #7, #9, #10, #11, #12).
9. Supply chain (action SHAs, DLL hash, feed override).

## Phase A (this build)

### A1. Unified smart input
- `lib/intent.js`: `score(text, ctx) -> {intent, scores, locked, prefix, body, why, risky}`; signals:
  question words and `?`; find/where-is phrasing; imperative verbs (with "how do I" cancelling them);
  literal occurrence in the prefetched window text; shape (quotes, identifiers, numbers, 1 to 3 words).
  Ties go to Ask. A pure function the smoke run asserts on a 40+ row table and times.
- Prefixes: `?` Ask, `/` Find, `//` Find literal-only (no model calls), `>` Do. Consumed into the
  token on input; the first Backspace at caret 0 turns the lock back into the typed character, so
  `/etc/hosts` stays askable. `/x/y` with a second slash before a space is a path, not a prefix.
- Token chip `#intent` (class hooks: `.intent-token`, `[data-intent]`, `[data-locked]`,
  `[data-confidence]`), linked to the field with `aria-describedby`; a separate polite announcer
  speaks the guess only when it changes, about 700 ms after typing stops.
- Override: Ctrl+1/2/3 lock Ask/Find/Do; Alt+Down opens a three-item listbox on the token. Tab is
  never taken.
- Up/Down at the ends of the field walk the last 50 inputs (kept in memory and, if history is on, in
  `inputs.json`, text only). Enter is ignored while an IME composition is open. Esc clears text, then
  the lock, then collapses.
- Wrong-guess recovery: every answer carries `Find this instead`; every Find status carries
  `Ask instead`; an inferred Find with no exact hits rolls over to Ask on Enter.
- Placeholder `Ask, find, or tell me what to do`; under an empty field, up to three example chips
  drawn from the app in front (browser, Excel, mail, editor, terminal, Explorer, fallback).
- The bar always opens unlocked (never in the last mode). The pill shows the mark only.
- One global chord by default, with a one-time migration; registration failures are shown on the pill
  tooltip, in the bar's notice row, and in the tour.

### A2. Trust and safety (every CRITICAL and HIGH)
- `type` text with `\n`, `\r` or `\t` is gated as a submit and named in the card.
- Keys are canonicalised (aliases like `return`, sequences, `repeat`), and the key gate is
  allowlist-shaped: navigation, clipboard, undo, plain characters and Shift+letter run freely;
  every Enter, Delete, F-key, and any Ctrl/Alt/Win chord outside the allowlist asks; Win+R, Win+X,
  Ctrl+Shift+Esc, Alt+F4, Ctrl+W/Q always ask and say so.
- Surfaces: terminals (cmd, powershell, pwsh, WindowsTerminal, conhost), the Run dialog, and browser
  address bars make typing and Enter consequential; `ctrl+l`/`alt+d`/`f6` then Enter in a browser is
  treated as navigation.
- Drag is always consequential and names both ends.
- Fail closed: an unreadable target, a tree read that throws, or a missing focused node is
  consequential with the reason "could not read what this would touch".
- Structural signals beyond names: dialog text (`#32770` static text), `automationId`, `helpText`,
  browser URL text for payment/checkout/compose pages, mail and chat apps by process; the verb list
  grows (save, upload, run, execute, move, apply, finish, allow, approve, authorize, grant, unlock,
  yes/ok inside a verb-titled dialog).
- Confirm card: focus lands on the card, not Allow; Skip is the default; key activation is ignored for
  the first 700 ms; the Ctrl+Enter Allow chord is removed (Esc or Ctrl+Backspace skip; Allow is a
  click or Alt+Shift+A after the delay); `open` shows the full URL with the host first.
- Do it, Retry and Find to Do pass their text as untrusted data and start plan-gated.
- Privacy disclosure naming OpenRouter and the model's provider, at consent, at key entry, on the
  Privacy page, and in the README; the free-model opt-in error names the trade-off.
- Find's model stages only after explicit intent or the Automatic setting, with the character count.
- `AEVAL_UPDATE_FEED` gated and https-only (loopback excepted); workflow actions pinned to SHAs.
- Helper DLL verified by SHA-256 before load.
- The bar never reopens in the last mode.

### A3. Power-user bugs
- Remove the global Enter; paused and plan waits resume from the trail's button or the pill.
- `pickLive` judges a model by what its kind needs; a saved choice is only rewritten when the model is
  gone from the catalogue.
- One capture per question: the bar opens before capture, the capture runs in parallel, and Enter
  reuses it when the window in front is unchanged and it is under 20 s old.
- Prewarm the desktop helper and the catalogue on bar open (not at boot), and prefetch the window
  text while the person types.
- Ask sends text only when the window text is rich and the question is not visual.
- Agent: no mandatory `update_plan` call (plan text rides with the first action, except plan-gated
  runs); wait on the engine's `generation` counter (poll 40 ms, cap 600 ms) instead of a fixed 250 ms;
  `window` argument on read tools; new top-level popups of the same process folded into the
  observation; a stale ref re-read and rebound once without counting a strike; `clipboard_read`,
  `clipboard_write`, `wait_for`, `close_window` tools; an empty final reply is nudged once, then
  ends as "did not say", never "Done".
- Find: stage C runs alongside B; the current hit is tracked by identity, so arrivals never move the
  ring; Find to Ask keeps the hit's rect (as the Ask area), its why, and its window.
- Every answer and every run shows time to first token, tokens and cost (`usage: {include: true}`).

### A4. Accessibility blockers and HIGHs
- Tab is free; F6 moves between the bar and the answer window; Esc in the answer returns to the bar.
- One announcer (`#sr-announcer`, polite, atomic) in each window; streaming sets `aria-busy` and the
  log is not live; trail rows update in place with a visually hidden state word.
- Area selector: arrow keys move, Shift+arrows resize, Enter confirms, Esc cancels.
- Layout editor: arrow nudge (8 px, Shift 32 px) and a snap-to-corner keyboard path.
- Tour: card steps take focus; Esc skips; Enter on a focused button.
- Targets at least 24 px; the contrast fixes that are not redesign (solid secondary text, accent marks
  in `--accent-bright`, focus halo in Settings and History, input focus border, error icon in danger).
- `forced-colors` and `prefers-contrast` blocks in each stylesheet; reduced motion is read live.

### A5. Ease-of-use P0s
- Key onboarding: a free-pass card in plain words, clipboard detection of `sk-or-` while it is open,
  an automatic test on save, a deep link to OpenRouter's privacy page when the free-model opt-in is
  off, and Settings dropped from always-on-top while an external link is open.
- Presence: launch at login on for fresh installs with a first-run toast; tray guidance ("click the
  arrow by the clock").
- Waits: after 6 s a line says free models queue and counts the seconds; after a switch it says so.
- A failed hotkey registration is shown in the UI.
- The typed task survives "Not now" on the consent sheet.
- Help and Report a problem in the tray and Settings (Copy diagnostics, open the issues page).

## Phase B (the next agent)

- Liquid glass material per designer finding 1 and 4: captured, blurred backdrop inside the page,
  adaptive scrim, specular rim, refraction on the bar and sheet only, the fallback ladder, and the five
  gates (`styleStable`, `topmostHolds`, `collapseClickable`, `noSetOpacity`, `snapCost`).
- Stage window (designer 3) and the 20 px shadow margin (designer 5).
- Type, radius and spacing re-scale (designer 6): 56 px capsule, 18 px field, Segoe UI Variable.
- Orb pill, ghost alternate, token glyphs, morph timings (designer 2.1, 2.5, 9).
- Find visuals (rect swatches, magnifier lens), trail as checklist, glass confirm sheet, settings and
  history as grouped lists, tour card without the mascot (designer 7 to 9).
- Contrast sampler in `npm run shots` and multi-backdrop frames (designer 11, a11y R8).
- a11y R1 to R8 and the `prefers-reduced-transparency` tier; interface size setting (a11y 8).
- Settings collapse to three groups plus Advanced and a speed dial (ease P2-11, power #28).
- Pipes and chips (`/invoice | ?sum | >paste`), `@scope`, `#tier`, macros, undo journal, watch
  queries, source chips (power Part 2.5 and Part 3).
- MEDIUM and LOW trust items not listed above: region enforcement (M2), read deny-list (M3), secret
  handling in ask_user and history (M4), at-rest notes (M5), per-window preloads and sender checks
  (M6), screen-share copy (M7), unnamed-target crops (M8), signing, fuses, test hooks.

## Gates

- Local: `node --check` on every changed JS file. The app is never run on this desktop.
- Cloud, every push to main: `installer.yml` (build, desktop-check, install, AEVAL_SMOKE, AEVAL_SHOT,
  uninstall, shots) and `update-check.yml`. Read through the public API.
- Visual: fetch `shots/latest/*.png` and look at the bar, find, trail, settings and tour frames.
- New smoke asserts: `intentTable` (40+ phrases), `intentFast`, one `gate:<bypass>` per trust-safety
  bypass, `noGlobalEnter` and `noGlobalEscWhileBarOpen`, plus the ones each change needs. No assert
  is weakened; asserts that pinned removed behaviour (`modeCycle` Tab cycling, five registered
  globals) are replaced by stricter ones that pin the new behaviour, recorded in DECISIONS.md.
