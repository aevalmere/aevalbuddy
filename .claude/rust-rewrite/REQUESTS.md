# Rust rewrite: Andy's requests

Started 2026-11-07. Andy asked to "rewrite in rust: super fast and smooth and elegant", via subagents only (don't do the work inline), and added "make sure you dont lose functionality or any quality". He asked for all of this to be written down so it survives a lost session. Repo copy: `.claude/rust-rewrite/REQUESTS.md`.

Decisions: Tauri v2 + existing HTML/CSS renderer (pure-Rust UI would cost liquid-glass quality). Work on branch `rust-rewrite`; main stays Electron until parity. No local Rust toolchain; compile/test in GitHub Actions (`rust.yml`), per test-in-cloud. Contract docs: `.claude/rust-rewrite/PARITY.md` and `ARCHITECTURE.md`.

Andy's requests (all must ship):
1. Rust rewrite, zero lost functionality or quality.
2. API keys: Claude (Anthropic), OpenAI, Gemini, OpenRouter (was OpenRouter only).
3. Suggestions show only until the user's first request, then never again.
4. Full-screen, desktop-oriented layout.
5. Liquid glass independent of the Windows transparency setting, with a slider from none to "very very" liquid.
6. Remove duplicated instructions; the Ask/Find/Do shortcut hint lives on the left button only, not in the chat.
7. Output windows and the closing bar get proper, consistent formatting; the whole app must look consistent, intentional and polished.
8. Output windows "glob out" from the chat bar with a liquid-glass morph animation.
9. No separate follow-up input in output windows; the input stays on the bar.
10. Drag edges/corners resize properly on every window.
11. No separate outputs layout: output windows are free-floating, draggable by their title bar, and multiple can be open at once.
12. A few polish loops at the end (designer, ease-of-use, accessibility, power-user and trust reviews).

**How to apply:** check PARITY items and this list before calling the rewrite done. See aevalbuddy-overlay, [[v2-ui-direction]], go-all-the-way.
