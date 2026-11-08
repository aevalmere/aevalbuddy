# Platform lane entries

Folded into DESIGN.md, PARITY.md and ARCHITECTURE.md by the merge agent.

## PARITY · STORE-14 · D-16, B-087
Replace "| STORE-14 | `inputWidth` | 420 | Clamped 320..900. | store.js:79,302-318 |" with:
| STORE-14 | `inputWidth` | 600 on a new profile (D-16; Electron 420). A stored width is kept. | Clamped 320..900. | store.js:79,302-318 |
status: done; evidence: UNIT store::tests::fresh_file_gets_every_default_in_order, store::tests::stored_input_width_is_kept

## DESIGN · Spacing and radius · D-16
Add:
New profiles start with a 600 px bar (the `inputWidth` default; windows' `PANEL_WIDTH` matches). At 420 px the close button was hidden and a question had about 200 px after the token and the icons; Spotlight is about 680 px and Raycast 750. A width a person already stored is kept.
