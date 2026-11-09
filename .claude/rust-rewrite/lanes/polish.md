# Lane brief: polish

Branch `rust/polish`, cut from `rust-rewrite` after Andy tested the first install. His words: "make it visually appealing, like the borders and stuff are not it. it has to be subtle and spaced well, relaxing, and relaxed. the color scheme should match my website EthanZhang. default the liquid glass to much more liquidy too, make the whole slider like a bigger range ... some of the menus got stuck cropped in half weirdly." and "it needs to be subtle and elegant, like apple."

## Goal

1. The app reads as the same family as ethanzhang's site: ink, cream and paper grounds, pastel and signal blue as the one accent, blush as a rare warm touch, the site's two easing curves.
2. Quiet edges: no outline rings on surfaces, plates, fields or menus. Surfaces separate by tone, spacing and soft ink shadows; the glass's specular edge is the only line.
3. Liquid glass is much more liquid by default, and the slider spans frosted-flat to molten.
4. No menu is ever cut by its window.

## Files

CSS: `renderer/glass.css`, `renderer/overlay.css` (colours, rims, menus), `renderer/region.css`, `renderer/lens.css`, `renderer/voice-pill.css`, `renderer/cursor-buddy.css`, `settings/settings.css`, `history/history.css`. JS, menus and glass only: `renderer/glass.js` (`AevalGlass.place`, the level curve, the key), `renderer/frame.js` (window menu), `renderer/output.js` (retry menu), `settings/settings.js` (combo placement, slider names), `history/history.js` (row menu). Rust: `store/mod.rs` (glass default and v2.1 migration), `backdrop.rs` (default bits), `windows/layout.rs` and `windows/mod.rs` (bar window cap, resize during the glide), `selftest/probes/polish.rs`. Tests: `tools/ui-shots/lanes/polish.mjs`, `tools/smoke-labels/polish.txt`.

## Decisions

- Typeface: Segoe UI Variable stays. Bricolage Grotesque (the site's face) is OFL and could be bundled, but at 13-15 px UI sizes on Windows the system face with its optical sizes reads cleaner and more native, which is the Apple-like quality asked for. The family resemblance comes from the palette, the curves and the spacing instead.
- Glass scale v2 (`glassScale: 2`): 0 None, 0.25 Frosted, 0.5 Soft (about v2.1's default), 0.75 Liquid (default), 1 Very liquid. Refraction follows k^1.5: band 8/17/27/40 px, bend thin 9/25/46/70, thick 11/32/58/90. Blur thin 24 to 8 px, thick 36 to 22 px. A v2.1 value migrates once: 0.5 (the untouched default) becomes 0.75; others keep their look (0.12 + 0.74 v). The page key moved to `aeval.glassLevel2` so a v2.1 value in localStorage is never read on the new scale.
- Menus: every popup hanging off a control or a point is placed by `AevalGlass.place` (position fixed, below or above, scrolls inside when neither fits, clamped across, corrected for transformed ancestors). The bar's menus stay in the page's flow and the window grows to them: the cap went from 280 to 400 px and scales with the text size, and a resize asked for during the opening glide is applied when the glide ends instead of being dropped. The app context menu (`ui:context-menu`) is a native popup and was never clipped.

## Design sources applied

- unslop (polish mode, light slop): 4 px grid for every margin, padding and gap (2 px only as a marked optical nudge); same thing, same size (sheet text at one 20 px inset in every kind, History rows one height and one gap); grouping by space (day heads 16 above, 8 below); one accent used rarely; borders gone or 7% hairlines; no bounce (`--ease-settle` lost its overshoot); motion only on transform and opacity for menus.
- webforge-taste: one depth cue per surface (rings removed, soft ink shadows only), the site's `cubic-bezier(0.23, 1, 0.32, 1)` as the one ease-out, menus 200 ms, reduced-motion fallbacks kept, AA contrast held (a resting delete icon at 60% stays above 3:1).
- Impeccable (pbakaus/impeccable README): its anti-patterns (gradient accents, gray text on colour, cards in cards, cramped padding) were checked against the shots; none remain in the polished surfaces. It gives no numbers.
- UI UX Pro Max: semantic tokens rather than raw colours in components (the CSS lint enforces it), visible focus states, reduced motion respected. No numbers either.
- VengeanceUI: a component catalogue with no published rules; taken only as the feel to aim for (soft, short, decelerating motion).
- Left alone on purpose: the cursor buddy's idle tap and the working-state rim orbit loop by design (function lane's area); the sheet's height still animates (B-164) because it follows streaming text.

## Progress

- Palette, quiet edges, glass curve, menu placement, migration, checks: first push.
- Fix (64a5661): the new popup placer was declared `place` inside glass.js and shadowed the pane placer, so every refracting pane was moved like a menu (it broke readingMeasure640 and made the first shots wrong). It is `placePopup` now, exported as `AevalGlass.place`. Glass default test updated to 0.75; slider thumb centred with `translate` to stay on the 4 px grid.
- Spacing pass (8d6ee5d): sheet words 20 px from the glass in every kind (textInsetIs20InEveryKind), title shares the inset; History search inset 16, day heads 16 px above, rows 64 px and 4 px apart, delete at 60% at rest. Retry menu re-placed once model names arrive. Merged the function lane.
- 54dab43: popups capped at their own max height and at the window when the anchor is past it; build fixed for the function lane's `collapse_panel(app)`; find mark centred in its padding.
