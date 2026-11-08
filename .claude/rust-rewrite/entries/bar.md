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
