# Shared-document entries: sheet lane

Appended in the same commit as the change each one describes. The merge
agent folds them into DESIGN.md, PARITY.md and ARCHITECTURE.md.

## DESIGN · Answer sheet · D-05
Add:
The reading column is at most 640 px wide (`--measure`) and centres once the sheet is wider, in the transcript and the Find list alike. Code and tables may use the whole column.
Reason: DESIGN said 760, about 82 characters at 15/24 Segoe UI Variable Text. 640 is about 68, inside the range that stays comfortable to read, and a maximized window no longer sets a 1500 px line.

## DESIGN · Answer sheet · D-29
Add:
Under an answer: `<model name> via <Provider> · <total s>`, then `· <Provider> was busy` after a hop, and `free` only while paid models are shown. Time to first word, tokens, cost and the model id are in the tooltip.
Reason: telemetry at full length read as a log line under every answer. The two questions a person has at that moment are which model answered and whether the wait was normal; the rest is one hover away.

## DESIGN · Answer sheet · B-061, B-062
Add:
The glob is one piece of glass. It opens in 480 ms and closes in 260 ms after the words leave (80 ms): a drop leaves the bar, a neck with horizontal tangents pinches as the body reaches the sheet, the neck's halves recoil into the bar and the sheet, and the sheet's own glass takes over from t 0.86. Words stay clipped to the shape and appear from t 0.70. The bar's thin material runs through the join and fades over t 0.10 to 0.55. With the bar closed, a gap over 14 px, or under 48 px shared along the edge, the sheet scales 0.94 to 1 toward the bar in 280 ms instead (160 ms out). Reduced motion fades.
Reason: a shape stretched across a gap, or across a window that holds none of the bar, reads as a glitch. A plain scale says the same thing without pretending the glass is touching.

## DESIGN · Answer sheet · B-063, B-161
Add:
Answers are a document. `#` is an h2, `##` an h3, `###` and below an h4, all 17/22 or 15/22 semibold. Lists stay one list across blank lines and nest by indent. Tables keep their column alignment. Code sits in a plate with a one-click Copy that shows a check for 1.5 s. A link shows its host beside it when the label does not, and a click goes to Rust's `open-external`, never navigates. The whole answer text is escaped before any markdown rule runs, so answer text can never become markup.
Reason: a model's answer is untrusted text, and it is also the thing read most in this app.

## DESIGN · Answer sheet · B-160
Add:
A streaming answer appends: the blocks before the last blank line outside a fence are set once and left alone, so a selection survives and the text above the tail stands still. The tail carries a 6 px dot until the answer ends.
Reason: re-rendering on each delta lost the selection and shimmered the whole answer.

## DESIGN · Output windows · B-168
Add:
Start over keeps the window and says `Started over. The last conversation is in History.` with Undo for as long as Rust offers it (`chat:reset {undoMs}`). If Rust refuses the undo, the row says `Couldn't bring it back here. It is in History.` Then, or at once when there was nothing to undo, the sheet reads `Your next question goes here.`
Reason: Undo that can fail has to say so, and where the conversation went.

## DESIGN · Agent trail · B-165, B-170
Add:
The plan is the checklist. Each act sits under the plan item it serves as a detail line (`planIndex`); with no index an item checks off once that many acts have finished, and the acts wait behind the steps toggle. Each row is read with its state first (`Planned:`, `Now:`, `Done:`, `Skipped:`, `Failed:`). New acts are announced at most once every 2 s, the latest one waiting for the gap; a failure is announced at once.
Reason: a run of 20 small acts told one by one drowned out the three things the task was for, and a screen reader spoke over itself.

## DESIGN · Agent trail · B-007, B-183
Add:
A confirm card shows the exact text that will be typed, 8 lines then scrolling. When it was cut short in the sentence above, Allow waits until the text has been scrolled to its end or shown whole, then both buttons run a 700 ms fill before they answer. The how-to is announced, not shown.
Reason: a person cannot agree to text they have not seen, and a key already on its way must not answer a card that just appeared.

## DESIGN · Agent trail · B-171
Add:
Consent names who receives the screen, for example `Each step sends a screenshot and the window's text to OpenRouter (Gemma 3 27B).`, adds `Free models' providers may keep and train on what they receive.` only for free models, and ends with `Move the mouse to pause it. Press Esc to stop it.`
Reason: "the model" is not an answer to who sees your screen.

## DESIGN · Find · B-169
Add:
Nothing found offers the next step and no key hints: `Ask about "<query>"`, and `Look on the whole screen` when the search was the window only. When Find has no hit to hand over, the words become the question.
Reason: an empty result is the moment someone needs a way forward.

## PARITY · NEW-05 · B-061
status: done; evidence: SMOKE globMorphMs, ui-shots globWordsNeverOutsideShape, globOpenOnClock

## PARITY · NEW-06 · B-166
status: done; evidence: SMOKE noOutputInput, ui-shots paidHopAsksFirst
