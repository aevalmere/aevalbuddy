# chatfix lane

Three bugs Andy hit on the first Tauri install, fixed on `rust/chatfix`.

## The answer window vanished under the pointer

Seen with the bar at the bottom left; holding Ctrl+Alt+Space kept it.

- Cause: output windows are built hidden and shown with `flags::show_inactive`, a raw
  `ShowWindow(SW_SHOWNOACTIVATE)`, so tao's own VISIBLE flag stays false. Click-through went through
  tao's `set_ignore_cursor_events`, and tao applies any flag change by rebuilding the whole window
  style from its flags and calling `ShowWindow(SW_HIDE)` when VISIBLE is not set. The first time the
  pointer reached the window's clear part, the page asked for click-through and tao hid the window.
- Why bottom left: since B-027 an answer window is full height and its sheet grows inside it. Under
  a top bar the clear part is below the sheet and the pointer arrives from the bar straight onto the
  sheet. Over a bottom bar the window grows up, the clear part is above the sheet, and the pointer
  comes down through it. Holding the hotkey re-ran `show_with_chat`, which showed it again on every
  repeat.
- Fix: `flags::set_click_through` sets WS_EX_TRANSPARENT and WS_EX_LAYERED on the window itself
  (the same two bits tao sets) and the subclass keeps them through tao's later rewrites. Nothing in
  `windows/` calls `set_ignore_cursor_events` now (unit test `click_through_never_goes_through_tao`).
- Not fixed here: `buddy.rs` and `lens.rs` still use tao's click-through on windows shown raw
  (`desktop::win::show_inactive`), so the tour card may have the same problem.
- Smoke: `hoverKeepsAnswer` and `marginClickThrough` move the real pointer over an answer with the
  bar in four corners (sheet, clear part, sheet, clear part, outside, then the hotkey path) and require
  the window shown, unmoved and with its glass inside the work area; the clear part passes clicks
  through and the sheet takes them.

## No way back to the pill

- Collapse (Esc, the bar's X) folded only the bar and left the answers floating (D-01); an answer's
  X ends its conversation. `windows::minimize` (`overlay:minimize`) hides the answer windows and
  collapses the bar; the next open shows every conversation with content again (`show_with_chat`).
- The bar's X is now Minimize (a minus glyph, `i-minimize`, appended to the sprite), each answer's
  title bar has Minimize beside Close, Ctrl+M works in the bar and in answers, and the window menu
  lists Minimize. Esc is unchanged.
- Smoke `minimizeToPill`; ui-shots `minimizeButtonFolds` and Ctrl+M in `barShortcutsInvoke`.

## The continued conversation

- An earlier answer's action pills (Copy, Retry, What was sent, Find this on screen instead) were
  invisible at rest but kept their 40 px row, so each later turn opened on a dead gap; at a narrow
  width the last answer's row ran past the column.
- An earlier answer's row now takes no room unless it has focus or its sent panel is open; the
  row wraps; the usage line takes only leftover room.
- ui-shots `sheet-continued` (`continuedTurnsRead`): no overlap, the same gap between every turn,
  questions right-aligned, nothing wider than the column, earlier rows 0 px tall.
