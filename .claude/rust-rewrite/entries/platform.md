# Platform lane entries

Folded into DESIGN.md, PARITY.md and ARCHITECTURE.md by the merge agent.

## PARITY · STORE-14 · D-16, B-087
Replace "| STORE-14 | `inputWidth` | 420 | Clamped 320..900. | store.js:79,302-318 |" with:
| STORE-14 | `inputWidth` | 600 on a new profile (D-16; Electron 420). A stored width is kept. | Clamped 320..900. | store.js:79,302-318 |
status: done; evidence: UNIT store::tests::fresh_file_gets_every_default_in_order, store::tests::stored_input_width_is_kept

## DESIGN · Spacing and radius · D-16
Add:
New profiles start with a 600 px bar (the `inputWidth` default; windows' `PANEL_WIDTH` matches). At 420 px the close button was hidden and a question had about 200 px after the token and the icons; Spotlight is about 680 px and Raycast 750. A width a person already stored is kept.

## PARITY · STORE-44 · B-001, B-002
Replace "| STORE-44 | Write is atomic (electron-store uses write-file-atomic). | electron-store | UNIT |" with:
| STORE-44 | Write is atomic and durable: one save at a time with the snapshot taken under the save lock, a unique temp name, fsync, then `MoveFileExW(REPLACE_EXISTING \| WRITE_THROUGH)`; the previous good file is kept as `aevalbuddy.json.bak`. A file that does not parse is copied to `.corrupt` and `.bak` is used in its place; a file that exists but cannot be read (after three retries) is never written over that session; invalid UTF-8 is decoded lossily, as Node did. | electron-store | UNIT |
status: done; evidence: UNIT store::tests::concurrent_sets_never_lose_or_corrupt, store::tests::parse_failure_recovers_from_bak, store::tests::invalid_utf8_is_read_lossy_not_reset, store::tests::unreadable_file_disables_saving, durable::tests::replaces_and_leaves_no_temp

## PARITY · STORE-40 · B-004
Replace "Decrypt failure -> '' (user re-enters), never a crash." with:
Decrypt failure -> '' (user re-enters), never a crash. A new key is minted only when `Local State` is missing or is a JSON object with no `os_crypt.encrypted_key`; a file that cannot be read or parsed is left alone (copied to `Local State.corrupt`) and keys are stored in plain text that session. A UTF-8 BOM is skipped. The file is rewritten durably with every other key kept.
status: done; evidence: UNIT crypto::tests::unreadable_local_state_is_left_alone, crypto::tests::bom_local_state_keeps_its_key, crypto::tests::local_state_is_created_then_reused

## PARITY · FILE-03 · B-003, D-12
Replace "written via `.tmp` then rename; 400 ms debounce." with:
written durably (unique temp, fsync, write-through replace) 400 ms after the first change by one writer thread (a throttle, as history.js:55-63 did; later changes ride along); a file that does not parse is renamed to `history.json.corrupt-<unix ms>` first.
status: done; evidence: UNIT history::tests::corrupt_file_is_kept_aside_not_overwritten, history::tests::steady_saves_still_write_within_400ms, history::tests::flush_now_and_timer_never_overlap

## PARITY · HIST-05 · B-003
Replace "| HIST-05 | Unreadable/missing file = empty list, never a failed start. Write failures ignored. |" with:
| HIST-05 | Missing file = empty list; a file that does not parse is renamed aside, then an empty list; a file that exists but cannot be read = empty list and no writes this session. Never a failed start. A failed write is retried on the next change and on quit. |
status: done; evidence: UNIT history::tests::corrupt_file_is_kept_aside_not_overwritten, history::tests::unreadable_file_is_never_written

## PARITY · HIST-06 · D-12
Add (note to the row):
D-12: history writes throttle from the first change (400 ms, never postponed by later changes) on one long-lived writer thread; `flush_now` on quit takes the same write lock, so the two never overlap. Reason: Electron's history.js throttled; the Rust port had become a debounce that a steady stream of saves could postpone forever, with a thread per change.
