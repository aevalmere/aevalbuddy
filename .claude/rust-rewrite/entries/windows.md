# Shared-document entries: windows lane

Appended in the same commit as the change each one describes. The merge
agent folds them into DESIGN.md, PARITY.md and ARCHITECTURE.md.

## ARCHITECTURE · 10.1 New channels · C1 (K-22, K-23)
Add:
The per-lane blocks (BACKLOG "Hotspots") follow the split (d) channels in both allowlists, in the order windows, capture, llm, agent, each between `// ---- lane: <name> ----` and `// ---- end: <name> ----`. `ipc::tests::lane_blocks_match_preload` keeps the two files' markers identical.
Windows lane, invoke: `settings:set-text-scale`, `output:blur`, `output:cycle-target`, `output:close-all`, `output:last-answer`, `output:undo-reset`, `output:glass`, `window:nudge`, `bar:dirty`, `bar:ready`, `bar:prefill-open`, `a11y:announce`. Listen: `output:meta`, `ui:text-scale`, `bar:clear`, `bar:note`, `a11y:say`.

## ARCHITECTURE · 10.2 Extended payloads · C1 (K-05, K-06, K-07, K-22, K-23, K-24)
Add:
- `overlay:state.target`: `{label, title, kind}` of the output the bar's next question continues, or null. `kind` is `chat`, `find` or `trail`; `title` is the first question (whitespace collapsed, 90 characters), else `New conversation`, `Find` or `Task`. `overlay:state.visibleOutputs`: how many output windows are open and not hidden with the app (K-05).
- `overlay:state` also carries whatever the `overlay_extra` hook returns, merged over the core fields; llm adds `recipients` (K-07).
- `output:meta {grow, front, target}` to each output when one of its values changes: `grow` is `down`, or `up` when the window sits above the bar, so the sheet grows away from it; `front` is true for the most recently touched open window; `target` for the window the bar continues (K-06).
- `ui:text-scale {scale}` to every window when the Windows text size or the person's factor changes, and to a page when it says `glass:hello`. `scale` = Windows factor (1.0 to 2.25) times the person's (1.0 to 2.0), at most 2.5, three decimals (K-22).
- `settings:get` gains `textScale` (the person's factor), `osTextScale` (the Windows factor) and `overlayPos` (the bar's anchor `{h, v, offX, offY}`) (K-22, K-24).
- `settings:set-text-scale {scale}` -> `{ok, textScale, osTextScale, scale}`; the factor is kept to one decimal between 1.0 and 2.0 (K-22).
- `history:list {query}` -> `{rows, enabled}`; `enabled` is whether history saving is on (K-24).
- `output:close {label?}` closes that window, default the calling output, else the bar's target; with no window at all the bar folds as before. `output:close-all` closes every window with a conversation; the warm spare stays (K-04).
- `output:last-answer {label?}` -> `{text}`: the window's last answer, '' if none. `output:undo-reset {label?}` -> `{ok}`. `output:cycle-target {dir: 1|-1}` -> `{label}`: the new target, or null. `output:blur`: the calling window gives the foreground back. `output:glass {top, height}`: the visible sheet inside the glass, in glass DIPs (K-23).
- `window:nudge {dx, dy, dw, dh}` -> `{ok}`: the calling window moves or resizes by DIPs inside its work area and size limits; false for the bar, which moves with `hud:nudge` (K-23).
- `bar:dirty {dirty}`, `bar:ready` (the bar's field has focus after a hotkey), `bar:prefill-open {text}` (K-23).
- `a11y:announce {text, priority}` -> `{to}`: relayed as `a11y:say {text, priority}` to the focused AevalBuddy window, else the open bar; `priority` is `polite` or `assertive`, text at most 300 characters (K-23).
- `bar:clear {}`: the bar empties its field. `bar:note {text}`: a one-line note under the bar.
- `chat:reset {undoMs}`: how long Undo is offered, 0 when there was nothing to undo.

## ARCHITECTURE · 11.1 Shared state and hooks · C1 (K-03, K-07)
Add:
`AppState.queries` maps an output label to the generation of the Ask streaming into it; `busy` is "any entry". The Ask calls `query_started(label, gen)` and `query_ended(label, gen)` (a stale end for an older generation leaves a newer Ask alone); a closed window's entry goes with `query_forget`. `Hooks.abort_query_for(label)` stops that window's Ask only; closing or resetting a window calls it only when that window's own Ask is running. Until llm reports per window, the single `busy` flag counts for the target window. `Hooks.overlay_extra` returns extra `overlay:state` fields; it is called before the state lock is taken, so it may read the state itself.

## ARCHITECTURE · 11.3 Output windows · C1 (K-04)
Add:
`outputs::fresh(app)` is always a new or spare idle window, never the focused chat (Find results and task trails open in it). `outputs::close_all(app)` closes every window with a conversation; each stops only its own Ask.

## DESIGN · Keyboard · D-01
Add:
Esc never stops an answer or hides an output window. Esc in the bar clears the field, then folds the bar; Esc in an output gives the foreground back to the app that had it before the bar opened (or the next app window below). Folding the bar also returns the foreground. While a task waits on the person, the first Esc in a bar holding typed text clears it and says "Cleared. Press Esc again to stop the task."; the next Esc stops the task.
Reason: output windows float on their own since REQUESTS #11 and free models can queue for 20 to 30 s, so an Esc that aborts throws away a wait the person already paid for. The blur half of Electron's collapse is kept so typing continues where it was.

## DESIGN · Output windows · D-31
Add:
Start over keeps the window and offers Undo for 10 s (`chat:reset {undoMs}`, `output:undo-reset`). Undo puts the old conversation back only if nothing new was asked since. The picked area is dropped with the old conversation.
Reason: Start over is one click away from Send and loses the whole transcript; a short Undo costs nothing and keeps the window where the person put it.

## DESIGN · Output windows · D-32
Add:
At most six output windows are open. A seventh closes the least recently touched window with no answer streaming into it, and the bar says `Closed "<title>". It is in History.`, or `Closed "<title>".` when history saving is off.
Reason: each window holds a webview and a conversation; past six they cover the screen they are about. Saying which one went, and where to find it, keeps the close from looking like a lost answer.

## DESIGN · Accessibility · D-24
Add:
Text follows the Windows text size (Settings, Accessibility, Text size) times the person's own 100 to 200% setting, at most 250%. Pages scale their type by `--text-scale` from `ui:text-scale {scale}`.
Reason: the glass surfaces used fixed pixel sizes and ignored the system setting people with low vision rely on.

## PARITY · POS-09 · D-01
Replace "Collapse: aborts query and hides output unless `keepOutput`; closes Find; blur, not focusable, click-through with forwarding, show inactive; window shrinks to the pill only after T_COLLAPSE+30 ms (0 with reduced motion) so the fold plays inside the bar-sized window." with:
Collapse (D-01): the answer keeps streaming and output windows stay; closes Find; the foreground goes back to the app that had it, then not focusable, click-through with forwarding, show inactive; window shrinks to the pill only after T_COLLAPSE+30 ms (0 with reduced motion) so the fold plays inside the bar-sized window.

## PARITY · IPC-02 · D-01
Replace "Exit HUD if in HUD, else collapse." with:
Exit HUD if in HUD, else collapse (POS-09 under D-01: no abort, no output hidden).

## PARITY · IPC-18 · B-026
Replace "Reopen conversation as live chat; {ok}." with:
Reopen conversation as live chat, about the whole screen: the picked area is cleared, and only the window it lands in stops its Ask; {ok}.

## PARITY · EVT-03 · B-095
Replace "| EVT-03 | `hud:docked` {docked} | output | main.js:851,1051,1063,1082 |" with:
| EVT-03 | `hud:docked` {docked} | removed from both allowlists: no page listens and the Tauri port never sends it (output windows float on their own, REQUESTS #11) | main.js:851,1051,1063,1082 |

## PARITY · EVT-18 · B-095
Replace "| EVT-18 | `buddy:move` | (allowlisted, currently unsent; keep in allowlist) | preload.js:116 |" with:
| EVT-18 | `buddy:move` | removed from both allowlists: never sent; `ipc::tests::every_listen_channel_has_a_sender` keeps unsent channels out | preload.js:116 |

## PARITY · EVT-19 · B-095
Replace "| EVT-19 | `settings:data` | (allowlisted, currently unsent; keep) | preload.js:117 |" with:
| EVT-19 | `settings:data` | removed from both allowlists: never sent; settings read `settings:get` | preload.js:117 |
