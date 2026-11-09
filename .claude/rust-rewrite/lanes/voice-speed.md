# Lane brief: voice-speed

Branch `rust/voice-speed`, cut from `rust-rewrite`. Decisions in `.claude/rust-rewrite/entries/voice-speed.md` (D-voice-10 to D-voice-14).

## Goal

Andy: "transcription and stuff is super slow, how come whimpr flow (open source btw) does it so much better?" Dictation should put text in the field within about 300-500 ms of letting go of the key, for a 5-10 s utterance on an ordinary laptop CPU.

## Files

`src-tauri/src/voice/**` (new: `vad.rs`, `stream.rs`), `src-tauri/src/selftest/probes/voice.rs`, `renderer/voice-settings.js`, the Voice page in `settings/settings.html`, `tools/smoke-labels/voice.txt`, `tools/ui-shots/lanes/voice.mjs`. No Cargo or build config change.

## Smoke labels added

`voiceSpeed` (warm Whisper on the fixture within `SPEED_BUDGET_MS`), `voiceSpeedE2e` (release to text for a 17 s utterance with a pause, through the real pipeline, within `E2E_BUDGET_MS`). Logs: `voiceSpeedBefore`, `voiceSpeedAfter`, `voiceSpeedTiny`, `voiceSpeedTimings`.

## Progress

| Item | Status |
|---|---|
| Research: WhimprFlow (Blueturboguy07/WhimprFlow, MIT), Handy, OpenWhispr, Whispering | done, entries/voice-speed.md |
| q5_1 models, legacy f16 kept until replaced; model kept loaded while dictation is on, loaded at app start | pushed, CI pending |
| `audio_ctx` fitted to the clip, single segment, no timestamps, flash attention | pushed, CI pending |
| Energy VAD trims silence; pieces before pauses transcribed while the key is held | pushed, CI pending |
| AI cleanup off by default (stored as `aiCleanup`), 800 ms budget when on; Settings says the cost | pushed, CI pending |
| Clipboard restored off the critical path | pushed, CI pending |
| Stage timings (`voice::last_timings`, `[voice] timings` on stderr), before/after probe | pushed, CI pending |
| Not done: Parakeet engine, Vulkan GPU backend, whisper.cpp's Silero VAD (needs whisper-rs 0.15) | see entries/voice-speed.md, "Left" |
