# Decisions

- Screenshots publish to orphan branch `shots` (force-pushed single commit, 15 newest SHA folders + latest/) so they are readable without gh auth.
- D16 root cause: lens readiness tracked on the window (aevalReady), not webContents.isLoading(), which is still true inside did-finish-load.
- Gate retries (iter 5): only 5xx and timeouts, bounded at 3 attempts; the final answer goes to the unchanged assert.

## v2.1 Phase A (2026-11-06)

- Single input replaces the three modes. Token shows the live guess; prefixes / ? > // and Ctrl+1/2/3, Alt+Down lock it. Tab is never taken. A guessed Do shows a card: Enter answers, Alt+Enter or a click runs. Rejected: a runner-up Enter chord (too easy to run a task by accident).
- Smoke asserts replaced, not loosened: `modeCycle` (asserted the Tab cycle the owner asked to remove) became `barKeys` (Tab not taken, Ctrl+2/3/1 lock find/do/ask, text survives, Backspace unlocks). `shortcutsFive` (any boolean) became `shortcutsOneChord` (main chord registered true, the other four unbound and null): stricter. `trayOrder` keeps its exact-list check with the new first label `Ask, find, or do…` and the added `Help and feedback…`.
- One global chord Ctrl+Alt+Space; migration moves only chords equal to their old default.
- Escape global only for agent run, tour, area selector, layout editor. Enter never global.
- Find related: ask (default) / auto / off; old `findRelated:false` migrates to off; setFindSettings({related:true}) maps to auto so the desktop smoke still exercises the model stages.
- intent.js lives in renderer/ so the bar loads it under script-src 'self' without a cross-directory file URL.
- DLL hash lives in lib/desktop/prebuilt.json inside app.asar (generated, gitignored); packaged builds never use the user cache.
- Feed override: packaged only with AEVAL_SMOKE or AEVAL_UPDATE_TEST=1; https or loopback http; no credentials.

## v2.1 Phase B (2026-11-06)

- D-B1 Glass is drawn in the page from a desktopCapturer frame (lib/backdrop.js, renderer/glass.*), not with backgroundMaterial. Acrylic and mica need transparent:false and change the window style; that is the v0.4 failure (falling behind, unclickable collapse, desktop artifacts). Our windows are excluded from capture, so the frame never contains us. When they cannot be excluded, the solid tier is used.
- D-B2 One stage window was not built. The layout editor places input and output independently and about 54 call sites own their bounds. Fallback used (designer's own): a 20 px transparent click-through margin per window (lib/stage.js wraps get/setBounds and min/max sizes so callers keep speaking in glass rects), with the morphs done in CSS inside each window.
- D-B3 styleStable masks WS_EX_TRANSPARENT (0x20) and WS_EX_LAYERED (0x80000) in GWL_EXSTYLE, and WS_VISIBLE in GWL_STYLE. setIgnoreMouseEvents owns those two ex-style bits (it is how the margin is click-through); show and hide own WS_VISIBLE. Every other bit must hold. Raw values are logged as styleSamples on every smoke run.
- D-B4 AEVAL_SHOT_OFFSCREEN removed. It held every window at opacity 0 with setOpacity, which toggles WS_EX_LAYERED; noSetOpacity now forbids any `.setOpacity(` call in main.js and lib/. The shots run is CI-only anyway.
- D-B5 Shots use a rendered stand-in for the desktop (lib/backdrop-fixture.html: white document, black editor, wallpaper) fed to the glass as its frame, and the same picture as the page background, so each PNG is a composite. 12-live-overlay still goes through the real desktop.
- D-B6 Contrast gate (lib/contrast.js): two captures, the second with every glyph transparent; text colour composited over each background pixel; the 5th-percentile ratio per text run must be >= 4.5 (>= 3 for 24 px, or 18.66 px bold). Any failing frame fails the shots run. Text inside opacity < 0.6 is skipped (mid-fade or decorative).
- D-B7 topmostHolds runs keepOnTop on its real 4 s interval for the gate (it is off in smoke runs otherwise) against a foreign WinForms TopMost window, 60 s, sampled every 3 s, the first 6 s excused.
- D-B8 Find's loupe cuts the pixels under the current hit with backdrop.crop at full resolution, once per current-hit change, never on a timer.
- D-B9 The owner approved a 5% settle (--ease-settle), amending "nothing bounces". Used only for small arrivals: token lock, loupe, switches.
- D-B10 styleStable: WS_EX_NOACTIVATE (0x08000000) is owned by setFocusable, which has made the pill unfocusable since the MVP so it never takes focus from the app in front. It is not masked as free: the gate now requires it set on every pill sample and clear on every bar and answer sample. The old gate compared it to the first (pill) sample and so failed every pill to bar transition.
- D-B11 Contrast gate measures what a sighted person can see. Each text run is clipped by its ancestors (overflow, clip rect, clip-path inset(50%)) before sampling, so the sr-only live regions (#announce, #intent-desc) and rows scrolled under an edge are skipped, and the visible remainder of a half-clipped run is still held to 4.5:1. The hide pass now also clears ::selection text and text-decoration colours (a link underline in the accent read as its own background, 3.02), and selected input text is measured in its ::selection colour.
- D-B12 Shots under --force-device-scale-factor: Electron reports the display at DIP divided by the factor a second time (455x341 at 150% on a 1024x768 runner) while windows lay out on the full 683x512. The fixture is built and sent at the corrected size when AEVAL_SHOT_SCALE is set and matches the reported factor. Shot harness only; real DPI scaling is not affected.
- D-B13 The matrix fails any frame where the probe measured no visible text (it caught the vacuous iteration-2 gate). The orb is the one scene marked text:false: it shows a glyph and a badge and never a word, and its earlier runs=1 was the hidden live region. If the orb ever shows text it is still measured.
