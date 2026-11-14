# Lane brief: voice-transcribe

Branch `rust/voice-transcribe`, cut from `rust/voice-one-pass` (f35b788). Not merged, not released. The owner has not decided whether Parakeet becomes the default; this lane makes it selectable and measured and leaves the default alone.

## Goal

Move dictation's speech to text from whisper.cpp (`whisper-rs`) to transcribe.cpp (`transcribe-cpp` 0.4.2, the library Handy uses), so Parakeet can be offered beside Whisper. The two libraries cannot link into one exe: each brings its own ggml (LNK2005, `tools/voice-bench` on branch `bench/parakeet`).

## Files

`src-tauri/Cargo.toml` (the voice block), `src-tauri/build.rs`, `src-tauri/src/voice/{whisper,model,engine,stream,settings}.rs`, `src-tauri/src/voice/whisper.cmake` (comments only), `src-tauri/src/selftest/probes/voice.rs`, `tools/voice-cmake-env.mjs` (new), `tools/smoke-labels/voice.txt`, `.github/workflows/{rust,release}.yml`, `settings/settings.html`, `renderer/voice-settings.js`, `tools/ui-shots/lanes/voice.mjs`. `src-tauri/.cargo/config.toml` is deleted: it only set `CMAKE_PROJECT_INCLUDE` for whisper-rs-sys.

## Decisions

**D-voice-16: one library, transcribe.cpp, for Whisper and Parakeet.** `voice/whisper.rs` is still the only file that talks to the engine. A loaded model keeps one session. No timestamps. Whisper runs one greedy pass (`temperature_inc = 0`), with the text so far as its prompt; a pass with more words than the clip could hold is done again with the library's settings (`looped`, kept, Whisper only). The library stops a text that repeats or runs out of room and hands back what it had; that partial text is used.

**D-voice-17: models are GGUF files from huggingface.co/handy-computer, each pinned to a commit and a SHA-256** (`model.rs`, read 2026-11-14). Whisper tiny, base and small at Q5_K_M, English and multilingual; `parakeet-tdt_ctc-110m-Q5_K_M.gguf` (101 MB). A whisper.cpp `.bin` an earlier build fetched still loads in transcribe.cpp, so it runs until its GGUF has arrived in the background, then it is deleted (`model::legacy_for`, `engine::upgrade`). If such a file does not load, the GGUF is fetched at once.

**D-voice-18: the build flags reach the library through `TRANSCRIBE_CMAKE_ARGS`.** `whisper.cmake` is unchanged in what it sets (static C++ runtime, AVX2, FMA, F16C, no AVX-512, `GGML_NATIVE` off). cargo's `[env]` table cannot write an absolute path inside a `-D` argument, so `tools/voice-cmake-env.mjs` sets the variable in `rust.yml` and `release.yml`, and `build.rs` stops a build that lacks it. `voiceNoCppRuntime` is the proof the file was applied.

**D-voice-19: engine `parakeet-110m`.** English only: with another language chosen, Whisper base stands in, which keeps the Language setting true (`engine::spec`). Parakeet is handed each piece whole, silence left on (`engine::piece`): cut to the speech as Whisper's pieces are, it wrote the fixture's "tomorrow" as "to morrow" and left out every comma (`voiceSpeedParakeetTrimmed`).

## Smoke labels

Added and gated: `voiceParakeet` (Parakeet downloads, loads and hears the fixture's "tomorrow"). Logs added: `voiceSpeedParakeet`, `voiceSpeedParakeetShort`, `voiceSpeedParakeetTrimmed`, `voiceSpeedParakeetPadded`, `voiceSpeedParakeetTimings`, `voiceSpeedParakeetHeard`, `voiceSpeedHeard`, `voiceLegacyBin`, `voiceSystem`. Logs gone: `voiceSpeedBefore`, `voiceSpeedBreakdown` (they timed whisper.cpp settings that no longer exist). `voiceSpeed` and `voiceSpeedE2e` keep their budgets, 900 and 1100 ms.

## Numbers

Runner: windows-latest, 4 vCPU of an AMD EPYC 9V74, no GPU. Fixture `dictation.wav`, 7.9 s, 7.5 s once the silence is cut. Before is whisper.cpp at f35b788. After is three runs, in order: 43594967001 (42684a1), 43597308478 (3add819), 43599526371 (fb12386). In the first run Parakeet's pieces were still cut to the speech.

| What | Before | After |
|---|---|---|
| Whisper base.en, fixture (`voiceSpeedAfter`) | 645 ms | 807, 830, 809 ms |
| Whisper base.en, first 3 s (`voiceSpeedShort`) | 561 ms | 707, 724, 720 ms |
| Whisper tiny.en, fixture (`voiceSpeedTiny`) | 297 ms | 423, 435, 428 ms |
| Whisper base.en, 17 s utterance with a pause, release to text (`voiceSpeedTimings`) | 695 ms | 832, 849, 862 ms |
| Parakeet, whole fixture (`voiceSpeedParakeet`) | | 251 ms, 227 ms on a second pass (third run) |
| Parakeet, fixture cut to the speech (`voiceSpeedParakeetTrimmed`) | | 203, 190, 193 ms |
| Parakeet, first 3 s (`voiceSpeedParakeetShort`) | | 98, 98, 101 ms |
| Parakeet, 17 s utterance with a pause, release to text (`voiceSpeedParakeetTimings`) | | 195 (pieces cut), 235, 237 ms |
| Parakeet, load from disk | | 72 to 73 ms |
| Installer | 5.27 MiB | 5.93 MiB |

Whisper is slower than it was by 130 to 185 ms on every clip. transcribe.cpp encodes Whisper's whole 30 s window whatever the clip's length and has no setting for a shorter one; whisper.cpp's `audio_ctx`, fitted to the clip, was where the earlier time came from. Both gates still pass, `voiceSpeed` with 70 to 93 ms to spare, so a slower runner can fail it.

In the second run Parakeet's first pass on the whole fixture logged 555 ms, 331 ms of it under "decode" while the library's own decoder line for that pass says 6.9 ms. The pipeline took 238 and 233 ms a piece on the same audio in that run, and the third run measured 251 and 227 ms, so the 555 ms was one slow pass, cause not found. The probe runs the pass twice and logs both.

Text, same fixture:

- Whisper base.en: "Umm, so I think we should meet at 3, no, 4pm tomorrow, to go over the budget."
- Parakeet, whole clip: "Um, so I think we should meet at three no, four PM tomorrow, to go over the budget"
- Parakeet, cut to the speech: "Um so I think we should meet at three no four PM to morrow to go over the budget"
- Parakeet, first 3 s: "Um so I think we should meet at three"

First download: base 64 MB (was 60), tiny 44 MB (was 32), small 194 MB (was 190), Parakeet 101 MB.

## What still works, and the gaps

`PARITY.md` has no voice rows (voice is newer than the Electron build it describes) and is unchanged. Against the behaviour the lane was told to keep:

| Behaviour | State |
|---|---|
| Pieces transcribed at pauses while the key is held | kept, both engines (`voiceSpeedTimings`, `voiceSpeedParakeetTimings`: 1 piece early) |
| The text so far as the prompt, for casing across pieces | kept for Whisper. **Gap: Parakeet takes no prompt.** Each piece is heard on its own; in the 17 s run its second piece began lowercase and had no full stop before it |
| Old processors refused (`cpu_ok`: AVX2, FMA, F16C) | kept, both engines; Settings' message no longer says Whisper |
| Unload ten minutes after the last use; kept while dictation is on | kept (`whisper.rs`, same code path) |
| Language setting | kept for Whisper (named only to a multilingual model; `auto` left to the model). **Gap: Parakeet is English only**; another language runs Whisper base instead and Settings says so |
| Short encoder window for Whisper | **Gone.** No such setting in transcribe.cpp; this is the 160 ms above |
| Files earlier builds fetched | run until replaced, then deleted |

## Not verified

- No run on a real PC or with a microphone; every recording is the fixture, on the CI runner.
- An old processor: `cpu_ok` is unchanged and the flags file is applied, but nothing ran on a PC without AVX2 or on one with AVX-512.
- Of the earlier builds' files only `ggml-tiny.en-q5_1.bin` was loaded (`voiceLegacyBin`); the base, small and f16 files go the same way but were not run. The background swap from an old file to its GGUF was not exercised.
- The multilingual and small GGUF files: their hashes are Hugging Face's own, none was downloaded here. A language other than English was not run.
- `release.yml` did not run (no release).
- Parakeet's accuracy beyond the one fixture; the benchmark on `bench/parakeet` has more clips.

## For the owner

- Whether Parakeet becomes the default. It is 3 to 7 times faster than Whisper base here and its text is plainer.
- Whether Whisper being about 160 ms slower is acceptable. If this merges, the `voiceSpeed` budget (900 ms) needs setting again from the new numbers or it will fail on slower runners.
- The Parakeet model is CC-BY-4.0 (NVIDIA, converted by Handy): it needs an attribution line somewhere in the app or its notices. The Whisper repositories are marked Apache-2.0.
- "On this PC, fastest" is still the label of Whisper tiny, which Parakeet now beats.
