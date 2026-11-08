# Entries: voice lane

## ARCHITECTURE · Decisions · D-voice-1
Add:
**D-voice-1: speech to text is whisper.cpp on this PC, through `whisper-rs`; Gemini with the person's own key is the one cloud option.** Voice input has to be free by default and has to be provable on a CI runner with no microphone.

| Option | Free | Offline | Testable on CI | Quality | Verdict |
|---|---|---|---|---|---|
| whisper.cpp via `whisper-rs` 0.14 (base.en, 148 MB) | yes | yes | yes, feed it a WAV | good; tiny 78 MB faster, small-q5_1 190 MB better | **default** |
| Windows.Media.SpeechRecognition | yes | dictation grammar needs Microsoft's online service and the speech privacy switch | no: reads the mic only, so no fixture | weaker on free-form dictation | rejected |
| Gemini `generateContent` with inline audio | free tier, with the person's key | no | needs a key on CI | very good, and cleans up in the same call | optional engine |
| Groq / OpenAI Whisper API | needs a key the app does not have | no | no | very good | rejected |
| candle (pure-Rust Whisper) | yes | yes | yes | same models, slower on CPU, bigger build | rejected |

Models are downloaded on first use (`voice/model.rs`) from `huggingface.co/ggerganov/whisper.cpp`, streamed into `<userData>/voice/<file>.part`, checked against the SHA-256 pinned in `MODELS` and only then renamed; a `<file>.sha256` marker spares re-hashing on later starts. English uses the `.en` model, any other language the multilingual one. Progress goes to the window that asked (`voice:progress`). The loaded model is dropped after two idle minutes. Whisper needs AVX2, FMA and F16C; a CPU without them is told so and offered Gemini.

## ARCHITECTURE · Decisions · D-voice-2
Add:
**D-voice-2: whisper.cpp is built with the static C runtime and a fixed instruction set.** tauri-build links the VC runtime statically, so whisper.cpp must be compiled /MT as well or the exe would need msvcp140.dll. `src-tauri/.cargo/config.toml` sets `CMAKE_PROJECT_INCLUDE` to `src/voice/whisper.cmake`, which whisper-rs-sys passes to CMake: it swaps /MD for /MT, turns `GGML_NATIVE` off (a runner with AVX-512 would otherwise build a binary that crashes on most laptops) and AVX, AVX2, FMA and F16C on. OpenMP stays off (no vcomp140.dll). Bindings are generated at build time with bindgen (libclang from the runner's LLVM). The smoke label `voiceNoCppRuntime` reads the exe's import table and fails on any msvcp, vcomp or vcruntime import.

## ARCHITECTURE · Module map · D-voice-3
Add:
`src-tauri/src/voice/`: `capture` (cpal over WASAPI, 16 kHz mono, a live level, five minutes at most; `AEVAL_VOICE_WAV` or `capture::set_fixture` reads a WAV instead), `model`, `whisper`, `gemini`, `engine` (picks one), `cleanup` (the free-model tidy-up), `insert` (the text into the focused app), `ptt` (the hold-to-talk hook), `pill` (the dictation window), `settings` (`voice` in aevalbuddy.json), `ipc` (`voice:*`), `mod` (one utterance at a time: start, latch, finish, cancel, the pipeline). Setup calls `voice::setup` after `host::setup`; `ipc::dispatch` asks `voice::ipc::route` before the core router.

## ARCHITECTURE · IPC contract · D-voice-4
Add:
Invoke (bar and Settings, family `voice`): `voice:start`, `voice:stop`, `voice:cancel` (the bar's mic; `{ok, error?}`), `voice:get` (the Voice page: `settings`, `hotkeyLabel`, `devices`, `engines [{id, local, bytes, ready, available}]`, `languages`, `download`, `cleanupModel`, `gemini`, `cpuOk`), `voice:set` (a patch of `dictation`, `hotkey`, `cleanup`, `language`, `engine`, `device`; `{ok, settings, hotkeyLabel}` or `{ok: false, error}`), `voice:download`, `voice:capture-hotkey {on}` (the hook watches but never starts while Settings records a chord). Listen: `voice:state {state, origin, message?, text?, progress?}` with states `listening`, `handsfree`, `preparing`, `working`, `cleaning`, `done`, `error`, `idle`; `voice:level {origin, level 0..1}` about 20 times a second; `voice:progress {origin, progress | got, total}`. `origin` is `bar` or `dictation`; the bar gets its own, the pill (label `voice-pill`, caller Other, listen only) gets dictation's.

## ARCHITECTURE · Voice · D-voice-5
Add:
Cleanup (`voice/cleanup.rs`) sends the raw transcript, never the audio, to the first OpenRouter candidate whose text model ends in `:free`; a native provider is never used, so a paid key is never spent on it. The system prompt fixes punctuation, capitals and small grammar slips, drops filler words and keeps only the correction where the speaker corrects themselves, and forbids adding, answering, summarising or translating. The reply is refused when it is empty, chatty, or much longer than what was said. No free model, an error, a refusal or two seconds without a reply all give the raw text with um, uh and the like dropped. Off in Settings, the words come as heard.

## ARCHITECTURE · Voice · D-voice-6
Add:
Dictation's chord is a low-level keyboard hook (`voice/ptt.rs`), not a RegisterHotKey shortcut: it must see the release, and the default Ctrl+Win has no key. The hook swallows only the chord's own key (if any) and Space or Esc while held; modifiers pass through, and a Win chord taps an unassigned key (0xE8) so the Start menu stays shut. Another key joining the chord means it was some other shortcut: the recording is dropped quietly. Holds under 250 ms are taps. Space while holding latches hands-free; the chord again finishes. The text goes in by pasting: every clipboard format is saved, the text put in with the "not for history or cloud clipboard" marks, Ctrl+V sent, and the old clipboard put back unless something else wrote to it meanwhile. When the clipboard cannot be opened it is typed as Unicode keystrokes. Our own keystrokes carry `OWN_INPUT` so the hook ignores them.

## DESIGN · Bar · D-voice-7
Add:
**The mic.** An icon button before the area button. At rest, a stroked mic in `--label-2`. Listening, it takes `--accent-glass` with `--accent-text`, and the mic becomes a five-bar level meter, so the person can see they are heard. Writing it down, the mic breathes (no motion when reduced). An error tints it `--danger`, and the sentence takes the field's placeholder for four seconds. Click to start and again to stop, or hold and let go; Esc stops without keeping anything. The words land at the caret and the field's own input handling guesses Ask, Find or Do as for typing. Every state is said through the bar's live region.

## DESIGN · Dictation · D-voice-8
Add:
**The dictation pill.** A 200 by 52 capsule 20 px above the bottom of the work area, centred on the screen the person is working on. It never takes focus and lets clicks through, and it is kept out of screenshots. It carries its own ground (the lens chip's tint at 92%) rather than refracting, since it floats over any app. A thirteen-bar waveform in `--accent-text` scrolls with the voice; Writing, it greys and breathes; Done shows for 0.7 s; an error says why in two lines for 2.6 s.

## DESIGN · Settings · D-voice-9
Add:
**Settings, Voice** (after Tasks): Dictation (on/off, the hold-to-talk chord recorded by holding it, which may be two modifiers alone), Transcription (on this PC at three sizes, each with its download size or Downloaded, and Gemini, which says the recording goes to Google; a progress bar while the model downloads), Language, Microphone (Windows default first), Cleanup (on/off and which free model does it) and What is kept: "Nothing is recorded or kept beyond the utterance."
