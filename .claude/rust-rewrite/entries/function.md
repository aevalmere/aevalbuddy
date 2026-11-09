## PARITY · IPC-06 · B-164
status: replaced by `output:glass {top, height}` (K-23); `output:resize` is gone from both allowlists, the route and output.js. The sheet's height still becomes the height a reopened window is placed at (`outputs::set_sheet`). evidence: UNIT `outputs::tests::content_growth_never_sets_bounds` (now reads `set_sheet`); ui-shots `sheetReportsGlassRect`

## ARCHITECTURE · Ask · B-121
Add:
`query:done` carries `sent {provider, providerName, model, modelName, free, image: "screen"|"region"|null, imageSize, windowText: {app, chars}|null, history, reader: {modelName}|null, msgId}` and `msgId` (the turn's `gen`). The pictures of the last three answers in each output window stay in memory only (`host::ask::sent_images`, pruned when a window is gone); `output:sent-image {msgId}` returns a `data:image/jpeg;base64,` URL or null once dropped (D-38).

## ARCHITECTURE · Ask · B-122
Add:
`models:list` with neither `kind` nor `provider` (the bar's `@` menu, an answer's "Retry with") also returns `providers: [{id, name, groups, current, ready}]`, every provider with a usable key in the person's order. `query:submit` and `query:retry` take `{provider, model}`; a pinned model answers by itself: no reading step by another model, no 404 healing and no failover.

## ARCHITECTURE · LLM · B-132
Add:
`llm::client::Meta.overall` lets one non-streamed call ask for a longer whole-call cap than `OVERALL_TIMEOUT` (90 s); the agent sets it to its own turn limit (180 s on a native provider, 60 s otherwise).

## PARITY · clipboard:write · function lane
status: replies `{ok}` with whether the text reached the clipboard (Electron always said `{ok: true}`); every Copy in the app says "Copied" only on `ok`. evidence: ui-shots `doneOffersNextActions`

## DESIGN · Keyboard · voice lane follow-up
Add:
An app shortcut equal to the dictation chord is refused while dictation is on ("Already used to hold and dictate"), and turning dictation on is refused while an app shortcut holds its chord.
