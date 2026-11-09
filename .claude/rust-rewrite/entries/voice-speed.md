# Voice speed (lane voice-speed)

Andy: "transcription and stuff is super slow, how come whimpr flow (open source btw) does it so much better?" This entry records why it was slow, what the open-source dictation apps do, and what changed. It extends D-voice-1 to D-voice-9 (lanes/voice.md).

## What "whimpr flow" is

[Blueturboguy07/WhimprFlow](https://github.com/Blueturboguy07/WhimprFlow), MIT licence (ideas taken, no code copied). A Tauri v2 + Rust proof of concept of a Wispr-Flow-style dictation app. On Windows it is:

| | WhimprFlow (Windows) | AevalBuddy before this lane |
|---|---|---|
| Speech to text | whisper.cpp via whisper-rs 0.16, `ggml-base.en.bin`, CPU only (no CUDA or Vulkan on Windows; Metal on macOS) | the same model and engine, CPU only |
| Model in memory | loaded once at app start, never let go | loaded on the first press, dropped after 2 idle minutes, so the first dictation after a pause paid the load again |
| Whisper parameters | `single_segment`, `no_context`, greedy | greedy, `no_context`, full 30 s window |
| Cleanup | default "Local" (Qwen3-4B via llama.cpp, a separate worker); without the optional GGUF, or set to None, the raw text goes in at once. Cloud cleanup is opt-in with the person's own key | on by default, a free OpenRouter model with a 2 s budget on every dictation |
| Streaming, VAD | neither | neither |
| Insertion | clipboard + `SendInput` Ctrl+V | the same, then waited 450 ms to restore the clipboard before reporting done |

So WhimprFlow was not faster at transcribing. On a typical Windows install it skipped the cleanup round trip entirely and never paid a cold model load. Those two account for most of what Andy felt: up to 2 s of waiting for a free model, plus a few hundred ms (more from a cold disk) of reloading after every idle spell.

## The other open-source apps

| App | Engine and model | GPU | VAD | Streaming | Cleanup |
|---|---|---|---|---|---|
| Handy (cjpais/Handy, Tauri + Rust) | transcribe-rs: Parakeet TDT 0.6B v2/v3 int8 via ONNX Runtime (CPU EP only, on every platform), or Whisper via whisper.cpp | Whisper only: Metal on macOS, Vulkan on Windows/Linux | Silero | no, batch on release | none built in (optional post-processing) |
| OpenWhispr (Electron) | whisper.cpp or Parakeet locally, or cloud APIs | whisper.cpp GPU builds | yes | no | optional LLM, cloud or local |
| Whispering (Epicenter, Tauri) | cloud APIs by default (Groq Whisper is fast), local whisper.cpp optional | via whisper.cpp | optional | no | optional "transformations" |
| VoiceInk (macOS) | whisper.cpp with Core ML / Metal, Parakeet via FluidAudio (ANE) | ANE / Metal | yes | no | optional |

Where their speed comes from, roughly in order of effect: (1) no network on the critical path by default; (2) the model kept loaded; (3) a faster model on the same CPU (Parakeet TDT int8 runs about 20x real time on a Zen 3 desktop, 5x on a 2015 i5, per transcribe-rs's own table; Whisper base with a 30 s window is slower than that on short clips); (4) a GPU for Whisper where the build has one; (5) VAD so silence is not encoded.

## Decisions

**D-voice-10: the text goes in as heard by default; AI cleanup is a switch, with an 800 ms budget.** A free model queues; waiting for it was the largest single delay. Off (the default), the words go in with hesitations ("um", "uh") taken out by `cleanup::tidy`, at once. On, the free model gets 800 ms (was 2 s), then the tidied words go in. The store key is now `aiCleanup`, because the `cleanup: true` earlier builds wrote was their default, not a choice; Settings and IPC still call it `cleanup`. The Settings note says what each side costs, and shows the last dictation's release-to-text time.

**D-voice-11: the model is kept loaded while dictation is on, and loaded at app start.** `whisper::pin` follows the dictation switch (`voice::refresh`, run at setup and on every Voice settings change); unpinned, a model is let go after 10 idle minutes (was 2). base.en q5_1 costs about 100 MB of memory.

**D-voice-12: q5_1 models, and Whisper sized to the clip.** The defaults are now the q5_1 files (base.en 60 MB, was 148 MB; tiny.en 32 MB, was 78 MB; small was already q5_1): faster to download and load, the same accuracy within a word or two in a hundred. An f16 file an earlier build fetched keeps working until its replacement has downloaded in the background, then is deleted. `audio_ctx` is set to the clip's length plus 64 frames (floor 256, Whisper's own 1500 for clips near 30 s or longer), with `single_segment`, `no_timestamps` and flash attention.

**D-voice-13: energy VAD and transcribing while the key is held.** `vad.rs` finds speech from loudness against the clip's own noise floor (padded 300 ms before, 400 ms after) and cuts the silence off before Whisper sees it. `stream.rs` watches the recording every 250 ms; at a pause of 0.5 s after at least 1.5 s of speech it transcribes everything before the pause right away, with the text so far as Whisper's prompt. At release only the part after the last pause is left. whisper.cpp's own Silero VAD needs whisper-rs 0.15 (whisper.cpp 1.7.6); the energy detector does the trimming that matters here without a second model.

**D-voice-14: every utterance is timed.** `voice::Timings` (stop, wait for an in-flight piece, transcribe, cleanup, insert, total from release to text in) is kept as `voice::last_timings()`, printed as `[voice] timings {...}` on stderr, and shown in Settings. The smoke run's `voiceSpeed` and `voiceSpeedE2e` labels fail the build over budget. The clipboard is now restored on a thread of its own after the paste, so "text in" is the moment Ctrl+V is sent.

## Numbers (CI runner, windows-latest, 4 vCPU, no GPU)

Filled in from the green run: see "Measured" below.

## Left

- **Parakeet TDT 0.6B (ONNX).** Faster than Whisper base on CPU and more accurate in English, and what Handy defaults to. Not done here: it needs ONNX Runtime, whose Windows builds link the dynamic VC++ runtime (msvcp140/vcruntime140), which `voiceNoCppRuntime` forbids; shipping or downloading onnxruntime.dll plus the runtime DLLs beside the exe, and a 0.5-0.7 GB model download, is a decision for Andy. With the changes above Whisper base already meets the budget on the runner.
- **Vulkan for whisper.cpp.** Would need the Vulkan SDK on the runner and links `vulkan-1.dll` at load time, which a PC without a Vulkan driver does not have, so the exe would not start there. Doing it right means building ggml's Vulkan backend as a separately loaded DLL (GGML_BACKEND_DL) and shipping it beside the exe; whisper-rs 0.14 does not expose that.
- whisper.cpp's Silero VAD, with a whisper-rs 0.15 bump.
