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

## Progress

- Palette, quiet edges, glass curve, menu placement, migration, checks: first push.
