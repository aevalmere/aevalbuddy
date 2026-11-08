# Shared-document entries: settings lane

## DESIGN · Tauri rewrite: the requests that changed the surfaces · B-069
Add:
### Providers (request 2)

Settings' first page is **Providers** (it was Account): one grouped list, in the order AevalBuddy tries
them, of OpenRouter, Anthropic, OpenAI and Google Gemini. Answers come from the first ready one, which wears
a `Default` capsule. Each row is one line and one button: a drag handle, the position, the name over the
masked key (in the mono face) or what the provider offers, the status as an icon and a word, the capsule,
and a chevron; its accessible name is `OpenRouter, 1 of 4, Ready, default`. One row is open at a time.

Every state is a word plus an icon, and each says what happened and what to do, in design-ux §3.2's words:
Not added (OpenRouter leads with `Connect OpenRouter` and offers `Paste a key instead`; the others show a
labelled key field, `Show`, `Save` and where to get a key), Checking… (field and buttons disabled, 15 s
then Not checked), Ready (`Works. Checked just now.`, OpenRouter adds the credit left), Key rejected, No
credit, Busy (with the time it frees and where questions go meanwhile), Free models off, Not checked, and
the inline Remove confirmation (`Keep` has focus). A pasted key whose prefix names another provider is not
saved: the row says `This is an OpenAI key.` and offers `Save it under OpenAI`, which moves the key to that
row. A key copied from a provider's site is offered when the window comes back (`Key on the clipboard:
sk-ant-a…9Q2c`, `Use this key`); the page only ever receives the masked form.

Order is the default. Drag a row (6 px before it lifts; the others slide aside; Esc puts it back; the row
settles into its slot), or use `Move up`, `Move down` and `Use first` in an open row, or Alt+Up and
Alt+Down on a focused row. Each move is spoken (`Anthropic is now first. It answers by default.`) through
the one status line under the list, which is heard and not shown: the rows show state. Focus returns to
the row after a move, Remove or Replace.

Under the list: `When one is busy or fails, try the next`, and inside it `Ask me before switching from free
models to a paid provider`, then the storage line (C-58) from `keyStorage`.

| Departure | Reason |
| --- | --- |
| No credit and Not checked also offer a quiet `Remove` | §3.2 lists only `Add credit`/`Test again` and `Test again`; a key that cannot be checked or has no credit is the one a person most wants gone, and Remove still asks first |
| The status line under the list is visually hidden | the open row already shows the new state; a second visible copy of it would be noise for sighted users |
| Status glyphs (ring, spinner, circled check, cross, alert, clock, dash) and the drag grip are drawn inline, on the sprite's grid and stroke; chevrons, close, check, search and trash are sprite spans (`ico i-<name>`) | the K-40 sprite has no status set yet; these become `i-<name>` spans when it gains one |

## DESIGN · Tauri rewrite: the requests that changed the surfaces · B-070
Add:
### Settings rows

A Settings row is one line, the name on the left and its value on the right, and the whole line is the
disclosure button: the chevron is a real button whose `::before` stretches over the line, named by the
heading and the value (`Liquid glass, Subtle, collapsed`), with `aria-expanded` and `aria-controls` on its
body. Hover fills the line with `--fill-1`, taking the plate's corners from its position. The body opens
from `0fr` to `1fr` over `--d-med` on `--ease-morph`, and stops clipping once it lands so a model list can
hang below it. Rows that do one thing (Bar position, Tour) have a trailing text button with the action's
name (`Move…`, `Replay`) and a value (`Top right`, `4 steps`) instead of a chevron. Groups are r12 plates;
rows inside are separated by a hairline from the text's left edge.

## DESIGN · Liquid glass you set (request 5) · B-072
Add:
The slider's five stop names sit on their stops (the first and last flush with the track's ends), the track
has a tick at each stop, the current stop's name is `--label` and the others `--label-2`. A drag that ends
within 0.04 of a stop lands on it; arrow keys and Page keys go stop to stop; clicking a name moves there.

## DESIGN · Tauri rewrite: the requests that changed the surfaces · B-073
Add:
### Settings and History frames

Settings and History use the shared sheet bar (title left, 28 px close), sit inside the 20 px stage margin
(`body { padding: var(--stage) }`), and are `glass thick refract`. Maximized (`html[data-maximized]`), the
margin and the sheet's radius go to zero, so the sheet is full bleed. Past 900 px the Settings rail is
220 px and pages keep a 720 px measure; History's search, list and footer keep 760 px.

## DESIGN · Tauri rewrite: the requests that changed the surfaces · B-074
Add:
Settings, General has **Text size**: 100% to 200% in steps of 10, noted `Starts from your Windows text
size.` (and Windows' own percentage when it is not 100%). It is the person's factor on top of Windows';
Ctrl+=, Ctrl+- and Ctrl+0 in any window move the same value.

## DESIGN · Tauri rewrite: the requests that changed the surfaces · B-173
Add:
### Models per provider

The Models page starts with one choice: `Automatic (recommended)` (AevalBuddy picks a model for each job:
free ones on OpenRouter, the recommended one elsewhere) or `Choose for each job`. Only the second shows the
four job pickers, and with more than one provider holding a key a segmented `Models for` above them picks
whose models they edit (default: the Default provider). `Show paid models` exists only on OpenRouter's
segment; the others say `Every model here is billed per question.` Choosing Automatic puts every job back
on its recommended model. The pickers are the shared menu in its solid tier (never glass on glass), grouped
by tier with `role="group"`, with type-ahead.

## DESIGN · Tauri rewrite: the requests that changed the surfaces · B-174
Add:
### History

History opens with the search focused but no ring until the first key or Tab. Conversations are grouped by
calendar day (Today, Yesterday, the weekday for the rest of the week, Earlier) and a row's time never
contradicts its group: `20 min ago` or `3 h ago` today, the time under a day, the date under Earlier. A row
is two buttons side by side, the conversation and a 28 px Delete that is visible at rest in `--label-3`.
Delete hides the row and says `Deleted "<title>". Undo` for 6 s; only then, or when the window goes, is the
conversation removed. Up and Down move between rows (Up from the first goes back to the search), Enter
opens, Delete deletes, any printable key goes to the search, Esc clears the search and then closes. A row's
menu (right-click, Shift+F10 or the menu key) has `Ask again`, `Copy question` and `Delete`. One row is in
the tab order at a time, with its Delete. `Delete all` is a quiet text button that confirms inline (`Delete
all 4 conversations?`, `Delete` / `Keep`, focus on Keep). With history off the empty list says so (C-80)
with `Turn on`; with older conversations still listed, a line above them says so.

## DESIGN · Tauri rewrite: the requests that changed the surfaces · B-184
Add:
Settings, Shortcuts lists the keys that work inside AevalBuddy's windows under `In AevalBuddy windows`
(read-only), and F1 on any Settings page goes there. A shortcut another app holds offers free alternatives
as one-click buttons. Answers has `Open styles folder` with `Add a .md file to make your own style.`, and a
style longer than 8 KB says `Only the first 8 KB is used.` General has **Bar position** (`Top right`,
`Move…`).

## PARITY · RKEY-07 · B-176
status: changed; evidence: ui-shots checks radiosRoveWithArrows, providerRowsHaveExpandedState (Enter saves a key in every provider field; listbox arrows/Home/End/Enter/Escape/Tab plus type-ahead; shortcut recorder Escape cancels and needs a modifier, C-55)

## PARITY · RKEY-08 · B-174
status: changed; evidence: ui-shots checks historyArrowsMove, deleteIsUndoable (Escape still clears the search then closes; Up/Down, Enter, Delete and type-to-search added)

## PARITY · WIN-06 · B-073
status: renderer half done; evidence: ui-shots scenes settings-maximized and check oneTitleBarSystem (frame sizing rules are the windows lane's)

## PARITY · WIN-07 · B-073
status: renderer half done; evidence: ui-shots scene history-maximized
