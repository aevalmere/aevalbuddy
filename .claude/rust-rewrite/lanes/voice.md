# Lane brief: voice

Branch `rust/voice`, cut from `rust-rewrite`. Decisions and the IPC contract are in `.claude/rust-rewrite/entries/voice.md` (D-voice-1 to D-voice-9).

## Goal

Speak instead of typing: a mic in the bar that fills the field, and Wispr-Flow-style dictation (hold Ctrl+Win anywhere, speak, let go, cleaned-up text is typed where the cursor is). Free by default: Whisper runs on this PC; cleanup uses a free model and falls back to the raw words.

## Files you own

`src-tauri/src/voice/**`, `src-tauri/.cargo/config.toml`, `src-tauri/src/selftest/probes/voice.rs`, `renderer/voice.{js,css}`, `renderer/voice-pill.{html,css,js}`, `renderer/voice-settings.js`, `tools/smoke-labels/voice.txt`, `tools/ui-shots/lanes/voice.mjs`.

## Shared edits (each marked `voice lane`)

- `src-tauri/Cargo.toml`: the `lane: voice` block (cpal, whisper-rs) and three windows-sys features.
- `src-tauri/src/lib.rs`: `mod voice;` and `voice::setup(&handle)` after `host::setup`.
- `src-tauri/src/ipc.rs` and `preload.js`: `// voice lane` blocks after the agent blocks in both lists (not `// ---- ` markers, so `lane_blocks_match_preload` still counts 16), the `"voice" => APP` family, the route call in `dispatch`, `voice/ipc.rs` in the route scan, voice.js and voice-settings.js in `pages_may_call_what_they_call`.
- `src-tauri/src/selftest/probes/mod.rs`: the `voice` lane last, a Notepad lane; 10 lanes.
- `renderer/index.html`: the mic button, voice.css, voice.js. `settings/settings.html`: the Voice rail button and page, voice-settings.js. `settings/settings.css`: `.voice-select`.
- `tools/ui-shots/lanes.mjs`: the voice import; `tools/ui-shots/bridge.test.mjs`: 10 lane files.

## Smoke labels

`voiceNoCppRuntime`, `voiceModel`, `voiceTranscribe`, `voiceBarField`, `voicePill`, `voiceDictation`, `voiceClipboard`. Recordings read `src/voice/fixtures/dictation.wav` ("Um, so I think we should meet at three, no, four pm tomorrow, to go over the budget.").

## Progress

| Item | Status |
|---|---|
| Engine decision and build (D-voice-1, D-voice-2) | in progress |
| Capture, model download, Whisper, cleanup, insertion, hook | in progress |
| Bar mic, pill, Settings Voice | in progress |
| Probe, labels, ui-shots lane | in progress |
