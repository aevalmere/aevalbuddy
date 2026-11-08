# Entries from the bar lane

Folded into DESIGN.md, PARITY.md and ARCHITECTURE.md by the merge agent.

## DESIGN · Spacing and radius · D-22
Replace "Radius scale: 28 for sheets and the bar, 16 for cards and grouped lists, 10 for rows and fields, 6 for
keycaps, capsule (999) for chips, buttons and the search field. The bar is a 56 px capsule." with:
Radius scale: 28 / 16 / 12 / 8 / 6 / capsule, and the rule inner = outer − inset, so every radius is
concentric with the one around it. `--r-sheet` 28: the bar, outputs, Settings, History, the tour card.
`--r-card` 16: what sits 12 px inside a sheet (Find rows, the Settings rail), menus and popovers, the guess
card. `--r-plate` 12: what sits 16 px inside a sheet (transcript plates, code, tables, Settings and History
groups, the question chip) and menu rows (4 px inside a 16 menu). `--r-row` 8: rows 8 px inside a 16
container (rail rows, combo options), the lens ring and loupe. `--r-key` 6: keycaps and inline code.
`--r-cap` 999: buttons, chips, fields and the token. The bar is a 56 px capsule at every text size (its
radius is half its height). Reason: the old 10 px row radius sat 4 px inside a 16 px menu and 16 px plates
sat 16 px inside a 28 px sheet; neither was concentric (design-visual F37).

## DESIGN · Spacing and radius · B-054
Replace "Spacing scale: 4, 8, 12, 16, 24, 32." with:
Spacing scale on a 4-grid: `--sp-1..6` = 4, 8, 12, 16, 24, 32, plus `--stage` 20 (the shadow margin). 2 px only
for hairlines and optical nudges, commented where used. Insets: `--inset-text` 16 (text from any sheet edge),
`--inset-row` 12 (selectable rows from a sheet edge), `--inset-menu` 4.

## DESIGN · Type · B-054
Replace "Segoe UI Variable: Display for the bar and titles, Text for everything else (fallback Segoe UI), Cascadia
Mono or Consolas for code. The bar's text is 18/24. Answers and body text are 15/1.5. Then 14 for rows,
13 secondary, 12 metadata (floor). The first Markdown heading level inside answers is 17 semibold, the rest step down from there." with:
Segoe UI Variable with its optical sizes: Small for every 12 px style (`--font-small`), Display at 18 px and
up (the bar, page titles), Text everywhere else (fallback Segoe UI); Cascadia Mono or Consolas for code.

| Token | Size/line | Weight | Face | Use |
| --- | --- | --- | --- | --- |
| `--t-meta` | 12/16 | 400 | Small | usage, keycaps, timestamps, slider stops |
| `--t-caption` | 13/18 | 400 / 600 | Text | secondary text, sheet titles (600), buttons (600) |
| `--t-row` | 14/20 | 400 / 600 | Text | rows, trail steps, find snippets, settings rows |
| `--t-body` | 15/24 | 450 | Text | answers (measure 640 px, about 68 ch) |
| `--t-title` | 17/22 | 600 | Text | answer h1/h2, trail outcome, confirm title |
| `--t-bar` | 18/24 | 450 | Display | the bar input |
| `--t-page` | 22/28 | 600 | Display | Settings page titles |
| `--t-code` | 13/20 (inline 0.875em) | 400 | Cascadia Mono | code |

Each size has its line height as `--lh-<role>`. Weights: `--w-body` 450 and `--w-strong` 600; no 500, 650 or
700. Numerals: `tabular-nums` on counts, times, step numbers and tables. Every size, and the control heights
`--ctl` 32, `--ctl-s` 28, `--bar-h` 56 and `--sheet-bar-h` 44, is `calc(N px * var(--text-scale))`, so text
follows the Windows text size and the person's setting (D-24). The old names `--fs-*` remain as aliases.

## DESIGN · Palette · D-23
Add (a row after `--accent-bright`):
| `--accent-text` | `#EBC0E9` dark tone, `#823A80` light tone | every accent-coloured text and icon on glass: the caret, the active area button, the area chip's icon, a copied tick, a task's "Answer in the bar". 5.9:1 on the scrim where `--accent-bright` measured 2.6 to 3.9:1 (design-ux U9). `--accent-bright` stays for fills, rims and the lens ring |

## DESIGN · Motion · B-054
Add (rows to the Motion table):
| `--ease-spring` | `cubic-bezier(0.2, 0.9, 0.25, 1)` | the glob's body growth (a spring with response about 0.42 s, damping 0.88, no overshoot) |
| `--d-press` | 100ms | press feedback |
| `--d-reduced` | 150ms | every reduced-motion fade |
| `--d-reveal` | 140ms | content appearing in a surface that has landed |
| `--d-glob` | 480ms | the glob opening |
| `--d-glob-close` | 260ms | the glob closing |

## DESIGN · Material · B-054
Add (after "Shadow and margin"):
- **Elevation.** Three shadows: `--shadow-control` for the bar, chips and the orb (thin glass),
  `--shadow-sheet` for outputs, Settings and History (thick glass; it fits the 20 px margin) and
  `--shadow-menu` for menus. Plates inside glass (the confirm card, the consent sheet, code) carry no shadow:
  the rim and the fill are enough. Reason: one shallow shadow for everything flattened the hierarchy, and a
  shadow inside glass reads as a web card (design-visual F30).

## DESIGN · One system · B-054
Add (after the Rules paragraph):
Shared components live in `glass.css` so no page draws its own: `.sheet-bar` (the one title bar, 44 px, title
left aligned in 13/18 semibold, 28 px tools 4 px apart, the last 8 px from the glass, a hairline only when
content scrolls beneath), `.g-menu` (thin glass, r16, 4 px inside, rows 36 px high at r12, hover `--fill-1`,
selected `--fill-2` with the shared check, in 220 ms on `--ease-morph`, out in a 120 ms fade; `.solid` inside a
page that is already glass) and `renderer/icons.svg`, one icon family on a 20 px grid with a 1.5 px stroke
that stays 1.5 px at any size, at 18 px in a 32 px button, 16 px in a 28 px button and 14 px inline. Focus is
a 2 px light outline 2 px out plus a dark 5 px halo drawn as a second shadow, so a control keeps its own rim
(`--own-shadow`). The bar's focus is its rim brightening plus a 2 px dark and 2 px `#EBC0E9` ring outside it
(design-ux U26). Reason: two title-bar systems, two menu materials, five icon stroke weights and a focus ring
that erased control rims were the inconsistencies request 7 named (design-visual F7, F20, F21, F40).

## DESIGN · Material · D-21
Replace "thin glass `blur(14px) saturate(1.7) brightness(0.94)`, thick glass `blur(28px) saturate(1.5) brightness(0.9)`." with:
at the default Liquid stop, thin glass `blur(14px) saturate(1.55) brightness(0.94)`, thick glass `blur(28px) saturate(1.5) brightness(0.9)`. The slider moves both (see the level table under "Liquid glass you set").

## DESIGN · Material · D-21
Replace "thin `a = 0.46 + 0.32 Y`, thick `a = 0.72 + 0.16 Y`." with:
thin `a = 0.58 + 0.24 Y`, thick `a = 0.72 + 0.16 Y`, so thin reads as the same glass as thick, a touch lighter (0.76 against 0.84 at Y = 0.75). Above Liquid on the slider `Y` is the 98th-percentile luma, because the thinner blur lets bright detail through. Above Liquid the outer refraction band also gets a lighter scrim than the interior (up to 0.26 lighter at Very very), so the band shows the desktop while text, inset at least 16 px, keeps the measured value: the interior is a feathered layer inset by half the band that adds the difference (`.gsc` in `glass.css`).

## DESIGN · Liquid glass you set · D-21
Replace the table that begins "| k | 0 None | 0.5 Liquid (default) | 1 Very very |" and the paragraph after it ("The scrim rule is what keeps contrast: ... does not depend on the desktop at all.") with:
Liquidity lives at the edge, where text never sits, and the reading interior stays frosted. This is how the glass reads clear at Very very and stays legible.

| | 0 None | 0.25 Subtle | 0.5 Liquid (default) | 0.75 Very | 1 Very very |
| --- | --- | --- | --- | --- | --- |
| Body | `--solid` | backdrop | backdrop | backdrop | backdrop |
| Blur thin / thick | – | 16 / 30 | 14 / 28 | 12 / 26 | 10 / 24 |
| Saturation thin / thick | – | 1.33 / 1.30 | 1.55 / 1.50 | 1.78 / 1.70 | 2.0 / 1.9 |
| Scrim thin (y = p90 luma, p98 above 0.5) | – | `0.58+0.24y` +0.08(1−y) | `0.58+0.24y` | −0.08(1−y) | −0.16(1−y) |
| Scrim thick | – | `0.72+0.16y` +0.075(1−y) | `0.72+0.16y` | −0.075(1−y) | −0.15(1−y) |
| Edge band scrim (outer `--band`) | – | = interior | = interior | interior − 0.13 | interior − 0.26 |
| Refraction band / scale thin / thick | none | 9 / 11 / 14 | 14 / 22 / 28 | 23 / 39 / 48 | 32 / 56 / 68 |
| Refracting surfaces | – | bar, outputs, Settings, History | same | same | same |
| Rim width / peak | 1 px / 0.55 | 1 px / 0.62 | 1 px / 0.62 | 1.5 px / 0.73 | 2 px / 0.85 |
| Sheen (162°, top-left) | 0 | 0 | 0 | 0.08 | 0.16 |
| Thickness shade (bottom inset) | 0 | 0 | 0 | 0.18 | 0.35 |
| Tone | dark | auto light/dark | auto | auto | auto |

In code: blur thin `18px − 8px·k`, thick `32px − 8px·k`; saturation thin `1.1 + 0.9k`, thick `1.1 + 0.8k`; `spec = max(0, 2k − 1)`; band `round(4 + 20k + 8·spec)`; scale `round(base·2k + 12·spec)` with base 22 thin and 28 thick; scrim slope `(0.5 − k)·0.32·(1 − y)` thin and `·0.30` thick. The displacement is dropped while a window moves, as it is during a morph.

The contrast floor holds at every stop: the scrim slope is 0 at y = 1, so a white page keeps the 4.5:1 floor; the edge band never carries text (sheet text is inset 16 px, at least half the widest band); and the light tone takes over on bright content (D-20). None is the solid tier, so its contrast never depends on the desktop at all. Reason: at the old values Very very looked the same as Liquid on the sheets (design-visual F6), and the bar and the sheet read as two materials (F11). Settings and History opt in to refraction with `refract` on `#app` (settings lane, B-073).

## DESIGN · Material · D-20
Add (after the Scrim bullet):
- **Tone.** The tint is not always dark any more. Each window measures the median luma (y50) under its largest glass layer and turns light above 0.72 for two frames running, and dark again below 0.55; the gap keeps a page in the middle from flickering. The change is a 300 ms crossfade, 150 ms under reduced motion. The light tone: `--tint: 246 247 250`, `--label: #1C1C1F`, `--label-2: #3C3D44`, `--label-3: #4F515A`, black fills at 0.05 / 0.08 / 0.12, hairline and chip rim black at 0.10, `--accent-text: #823A80`, a black 0.12 outline outside the specular rim, and on thin glass a `0 1px 0 rgb(255 255 255 / 0.4)` text shadow. Its scrim thins as the page behind brightens: thin `a = 0.42 + 0.30 (1 − Y)`, thick `a = 0.62 + 0.20 (1 − Y)`. The solid tier stays dark whatever is behind it. Reason: one dark tint over a white page read as a grey slab; iOS 26 and macOS Tahoe glass flips to a light rendering over light content, which is how it can be clear and legible at once (design-visual F10). This departs from the dark tint the rest of this section assumes.

## DESIGN · Keyboard · D-34
Add (a new section):
The bar has its own keyboard layer, and its keys are taught where the control is: in tooltips and `aria-keyshortcuts`.

| Key | Does |
| --- | --- |
| Ctrl+N | Start a new conversation with the next question |
| Ctrl+Tab, Ctrl+Shift+Tab | Choose which open answer the next question continues |
| Ctrl+W, Ctrl+Shift+W | Close that answer; close them all |
| Ctrl+Shift+C | Copy that answer's last reply |
| Ctrl+. | Stop what is running |
| Ctrl+Shift+R | Retry that answer |
| Ctrl+R | Ask about an area of the screen |
| Ctrl+, | Settings |
| Ctrl+H | History |
| F6 | Move to the answer |

Reason: power users had to reach for the mouse for everything past Enter (design-ux U19). Ctrl+R means "ask about an area" because a reload is meaningless here and the bridge already blocks it, so no browser habit is broken.

## DESIGN · Keyboard · D-35
Add (to the same section):
Alt+Space, or a right-click on a title bar, opens a window menu on outputs, Settings and History: Move, Size, Maximize or Restore (where the title bar maximizes on double-click, so Settings and History), and Close. In Move and Size the arrows go 16 px at a time, 64 with Shift; Enter keeps the result and Esc puts the window back. Reason: everything a drag does has to be reachable without a pointer (design-ux U30), and these windows draw their own title bars, so Windows does not supply the menu. The bar has no title bar and has the layout editor instead.

## DESIGN · Accessibility · D-24
Add (the pages' half):
Every page applies `ui:text-scale {scale}` as `--text-scale` on `:root`, and every size and control height in `glass.css` is a `calc()` of it, so text and the controls around it grow together. Ctrl+=, Ctrl+- and Ctrl+0 step the setting by 0.1 within 1.0 to 2.0 from any page and save it through `settings:set-text-scale`. The bar's field, chips and menus are all sized from the same tokens, so nothing clips at 200%.
