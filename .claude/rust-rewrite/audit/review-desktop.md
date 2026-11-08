# Review: desktop, capture, backdrop, region, lens, buddy (rust-rewrite @ 544f982)

Scope: `src-tauri/src/desktop/**`, `capture.rs`, `capture/**`, `backdrop.rs`, `backdrop/**`, `region.rs`,
`lens.rs`, `buddy.rs`, `selftest.rs`, `lib/desktop/host.cs`, `lib/desktop/host.ps1`, and the callers in
`host/` as far as needed. Everything here comes from reading the code. Nothing was run. Findings marked
PLAUSIBLE depend on platform behaviour (DXGI or Tauri internals) that reading alone cannot settle.

Counts: **P0: 1, P1: 6, P2: 18**

---

## P0

### P0-1. Mixed-DPI layouts give overlapping DIP rectangles, so an area capture can crop the wrong monitor
- **Where:** `capture/screen.rs:141-148` (`to_dip`: origin = physical origin / own scale), `capture/screen.rs:240-250` (`matching`), used by `capture.rs:156-158` (`shot_image`), `region.rs:225-226` (`pick`), and `host/find.rs:352-357` ("Ask about this" sets the region).
- **Defect:** when a display to the right of (or below) another has a higher scale factor, the two DIP rectangles overlap. `matching()` then breaks the tie with `Iterator::max_by`, which returns the *last* maximum. Displays are sorted primary first, so a rect on the primary that falls inside the overlap is assigned to the secondary.
- **Scenario:** the primary is 1920x1080 at 100% (DIP 0,0,1920,1080). A 4K monitor at 150% sits to its right at physical x=1920, which gives DIP (1280,0,2560,1440). The user drags an area on the primary at DIP (1500,200,300,150). Both overlaps equal 45000, so the secondary wins. `crop_plan` cuts physical (330,300,450,225) out of the **4K monitor**, and the model answers about an unrelated part of the other screen. Every later question in that conversation reuses `ACTIVE` and gets the same wrong crop. Electron never hit this because Chromium's DIP layout never overlaps, so this is a regression. The existing unit test (`matching_picks_most_overlap_by_id_not_order`) only covers the non-overlapping orientation (the higher-DPI screen on the left).
- **Fix:** make every DIP rect that is mapped back later carry its display. `region::Pending` already knows `target`. Store `target.device` (or the physical rect from `target.rect_to_phys`) alongside `ACTIVE`/`PENDING_SHOT`, and have `shot_image` take that display instead of calling `matching()`. Do the same for Find's padded rect: it starts in physical pixels, so keep it physical. Add a unit test for the overlapping layout. For the longer term, pick one canonical DIP space that cannot overlap (Electron's layout), or work in physical pixels end to end. `windows/screen.rs:145-152` (`matching`, used by `host/mod.rs:76-82 active_region_px`) has the same `max_by` tie, so the region's text and OCR read also targets the wrong rectangle (outside my files; same fix).

---

## P1

### P1-1. A request that times out in the helper keeps running and sends input later
- **Where:** `lib/desktop/host.cs:304-317` (`Handle`: `Join(timeout)` replies TIMEOUT and abandons the thread), with the input paths `Click` 2010-2056, `TypeCmd`/`TypeInto` 2104-2154, `KeyCmd` 2156-2178, and `SetValueCmd` 2304-2316. Rust timeouts: `client.rs:977-1014` (`type`: max(4 s, 2 s + 2 ms/char); `key`: 4 s).
- **Defect:** the abandoned request thread is never cancelled, so it can still move the cursor, click or type after the agent has been told the op failed.
- **Scenario:** the agent calls `type {ref}` into a Chromium/Electron editor. `FocusableNear` (1859-1892) walks up to 200 elements with cross-process UIA calls. On a busy app that takes more than 4 s, so the helper replies `TIMEOUT` and the agent retries `type`. Then the first thread finishes its focus walk and types too. The text lands twice, or the agent's next `key Enter` arrives in the middle of it. `click {ref}` does the same thing with `PointFor` (about 25 live property reads, 1948-1965): the click fires seconds late at a point that may be stale, after the agent has moved on, and the physical-input watch cannot stop it. `key` with repeat x chords above about 250 overruns 4 s on its own (Sleep(15) is about 15.6 ms each).
- **Fix:** give each request a cancellation flag. `Handle` sets it on timeout, and every input primitive (`SendInputs`, `SetCursorPos`, `MoveTo`, `SetFocus`, pattern `Invoke`/`SetValue`/`Toggle`, `FocusWindow`) checks it before acting. Alternatively, on the Rust side, treat TIMEOUT on any input op as an unknown outcome and kill the helper (`kill_tree`) before replying, so a late input is impossible. Size `key`'s timeout from repeat x chords, as `type` already does.

### P1-2. After the first Find, the backdrop captures every display at 2 Hz forever, even when only the orb is showing
- **Where:** `host/mod.rs:408` (`backdrop_pace`: `!crate::lens::windows(app).is_empty()` makes the pace Fast), `lens.rs:162-171` (`draw` hides lens windows and never destroys them), `lens.rs:288-294` (`windows()` ignores visibility), and `lens.rs:296-303` (`destroy` is dead code).
- **Defect:** once lens windows exist they stay alive but hidden, and `backdrop_pace` counts them as "something you read", so the pace never drops back to 5 s.
- **Scenario:** the user runs one Find and closes it. From then on, with only the pill or orb visible (the overlay is a glass page, so `shown` is not empty), `tick` runs `grab_all()` every 500 ms on every monitor until the app restarts. Combined with P1-3, that is a constant CPU and battery drain at idle.
- **Fix:** have `lens::windows()` (or `backdrop_pace`) count only visible lens windows (`flags::is_visible(hwnd)`, Win32, no main-thread hop), or destroy lens windows in `lens::clear()` when no hits or frame remain.

### P1-3. Each backdrop tick pays a full-resolution CPU pipeline for every display, with no change detection
- **Where:** `backdrop.rs:370-404` (`grab_all`: every display, `frame.rgb()` then `resize`), `encode.rs:32-45` (`from_bgra`: per-pixel `extend_from_slice` over the whole frame), `encode.rs:74-75` (`resize` clones the whole source buffer first), `dxgi.rs:198` (a new zeroed `w*h*4` Vec for every copy), and `Cargo.toml` (`opt-level = "s"` for these loops).
- **Defect:** to produce a 1/6-size JPEG, every tick converts the whole frame to RGB (8.3 M pixels at 4K), copies it again, and Lanczos-resizes it. This happens for displays that host no glass window, and also when DXGI reports no change (`Dup::frame` returns the same `Arc` on WAIT_TIMEOUT).
- **Scenario:** with two 4K monitors and the bar open, each tick moves about 4 x 33 MB of buffers per display plus a Lanczos pass. My rough estimate (not measured) is 40-80 ms of CPU per 4K display per tick, which at 2 Hz is a steady 10-30% of a core while the screen is static. Add P1-2 and this runs at idle too.
- **Fix:** (1) capture only the displays that host a visible glass window (`display_for` already computes them). (2) Keep the last frame `Arc` and JPEG per display, and skip conversion and encoding when `Arc::ptr_eq` says DXGI handed back the same frame. (3) Resize BGRA directly as `U8x4` (fast_image_resize with `use_alpha(false)`) or with a borrowed `ImageRef`, then convert the small image. Or downscale on the GPU (CopySubresourceRegion to a mip chain or render target) and read back only about 1/36 of the pixels. (4) Reuse the readback buffer. (5) Set `[profile.release.package.fast_image_resize] opt-level = 3`, and use opt-level 3 for the crate's capture code.

### P1-4. The buddy window is destroyed asynchronously and then "reused", so points and agent cursors vanish (PLAUSIBLE)
- **Where:** `buddy.rs:325-333` (`destroy_window` calls `w.destroy()`), `buddy.rs:268-271` (`create_window` returns early when `window()` is `Some`), `buddy.rs:511-525` (`show_points`: `cancel()`, then `create_window`), `buddy.rs:371-381` (`start`), `buddy.rs:386-399` (`agent_point`: waits up to 5 s for `ready`).
- **Defect:** off the main thread, Tauri's `destroy()` only posts a message, and the manager keeps returning the window until `Destroyed` is processed. `create_window` therefore "moves" the dying window and builds nothing. Meanwhile `destroy_window` has already set `ready = false`.
- **Scenario:** an answer shows points. The user asks again before dismissing them, and the new answer has points too. `host/ask.rs:330` calls `show_points` from a tokio task: `cancel()`, then `create_window`, which finds the dying window and returns. The payload is queued forever and no pointer appears (it works again on the next answer). In the agent, the first `agent_point` after a tour or points session (or right after `agent_clear`) waits the full 5 s, delaying the first action, and still shows no pointer.
- **Fix:** do not destroy and recreate. Hide the window, reset its page state and reuse it. Or run the destroy synchronously on the main thread (`run_on_main_thread` plus a wait), or track a `destroying` generation so `create_window` builds a new window when the existing one is going away.

### P1-5. A fresh DXGI duplication accepts its first frame even when it is pointer-only, then serves it as "unchanged" (PLAUSIBLE; P0 if confirmed)
- **Where:** `dxgi.rs:111-115` and `dxgi.rs:129-134`. Pointer-only frames (`LastPresentTime == 0`) are skipped only when `last` already exists. The first frame of a new duplication is copied regardless, cached in `last`, and returned on every later WAIT_TIMEOUT (`dxgi.rs:121-124`).
- **Defect:** the first-frame-black problem. A first frame that carries only a pointer update (or comes before the duplication has produced a desktop image) can be black or stale. It becomes `last`, and a static screen then keeps returning it.
- **Scenario:** a duplication is recreated after ACCESS_LOST (UAC prompt, lock and unlock, mode change, a full-screen app switch), or opened for the first time on a monitor. The mouse is moving at that moment, so the first acquired frame is pointer-only. The question goes out with a black or old screen, and so does every later one until something on that monitor repaints. WebRTC's DXGI capturer skips the first frames for exactly this reason. The CI capture check cannot see it (P2-5).
- **Fix:** for a duplication with no `last`, loop within the 900 ms budget until a frame has `LastPresentTime != 0` (or `AccumulatedFrames > 0`). If none arrives, return an error so the call falls back to GDI rather than caching. Optionally reject an all-zero first frame. Make the DXGI probe in `checks.rs` a `must` on a freshly recreated duplication.

### P1-6. The same DIP overlap puts Find highlights, the loupe and the agent cursor on the wrong monitor
- **Where:** `lens.rs:254` (`set_hits`: `screen::matching` picks the later display on a tie), `capture.rs:211-222` (`crop_dip` for the loupe, same `matching`), `host/agent.rs:333-337` (`point_at`: `cap::nearest_point`, where distance is 0 to both overlapping displays and `min_by` returns the *first*, the primary), `desktop/client.rs:650-664` (`window_shape` assigns the `display` id through `matching`).
- **Defect:** same root cause as P0-1. Rects and points that start in physical pixels are converted to DIP and then matched back ambiguously.
- **Scenario:** with the layout from P0-1, a Find hit at DIP x 1300-1900 on the primary is drawn by the secondary's lens at (x−1280), on the wrong screen, and the loupe magnifies the wrong pixels. An agent action on the 4K monitor inside its first 640 DIP glides the buddy cursor on the primary. The click itself lands correctly, because it is physical.
- **Fix:** carry the display id from the physical side (`matching_phys` is unambiguous) through to the lens, loupe and buddy, rather than re-deriving it from DIP. Same structural fix as P0-1.

---

## P2

### P2-1. Failed DXGI opens are retried on every grab, and each output gets its own D3D11 device
- `dxgi.rs:217-233`, `dxgi.rs:45-104`. Any `open()` or `frame()` error drops the duplication, and the next grab does `CreateDXGIFactory1`, `D3D11CreateDevice` and `DuplicateOutput` again. When duplication is unavailable (DXGI_ERROR_NOT_CURRENTLY_AVAILABLE because other apps hold duplications, E_ACCESSDENIED on a locked or secure desktop, unsupported adapters), the backdrop creates and tears down a D3D11 device per display every 500 ms, then falls back to GDI anyway. Three monitors on one GPU also get three devices.
- **Fix:** cache open failures per device name with a backoff (for example 10 s, cleared on WM_DISPLAYCHANGE) and go straight to GDI during it. Share one device per adapter (key by adapter LUID).

### P2-2. HDR desktops go through legacy `DuplicateOutput` (PLAUSIBLE)
- `dxgi.rs:89-92`, `dxgi.rs:163-165`. With HDR on, the legacy API hands back an OS-converted 8-bit image. SDR content composed above 80 nits can come out clipped or washed out, which hurts text contrast for the model. The GDI fallback has the same problem.
- **Fix:** verify on a physical HDR machine (a runner cannot). If confirmed, use `IDXGIOutput5::DuplicateOutput1` with `R16G16B16A16_FLOAT`, divide by the SDR white level (`DISPLAYCONFIG_SDR_WHITE_LEVEL`), and sRGB-encode. Or use Windows.Graphics.Capture for HDR outputs.

### P2-3. Capture calls wait on the capture thread with no timeout
- `capture.rs:88-97`. `rx.recv()` blocks forever if DXGI or D3D11 hangs (TDR, a driver reset in `D3D11CreateDevice`). Every later capture then hangs too. The ask flow's `capture_screen_async` never resolves, so CAP-06's text-only fallback never happens, and the backdrop thread freezes.
- **Fix:** use `recv_timeout` (for example 3 s). On timeout, swap in a fresh worker (replace the sender in `worker()`) and return Err.

### P2-4. A panic inside `tick` freezes the glass for the rest of the session
- `backdrop.rs:477`, `backdrop.rs:538`, `backdrop.rs:328`. `busy = true` is never reset if `tick` panics after line 477, and `run_loop` neither reschedules after `catch_unwind` nor clears `busy`. Every later tick takes the `busy` early return, so pages keep a stale frame under a "live" tier. I found no current panic site in that window, so this is latent.
- **Fix:** reset `busy` with a drop guard, and in `run_loop` call `schedule(None)` after a caught panic.

### P2-5. CI does not gate the production capture path (DXGI)
- `desktop/checks.rs:299-304` and `311` only *log* DXGI exclusion, blanking and unavailability. `captureExcludesOwn` passes when GDI alone passes, and the whole-screen and area shots (326-423) check size and encoding but not content. A black or stale DXGI frame (P1-5), or DXGI being unavailable on the runner, would leave CI green while every user takes the DXGI path.
- **Fix:** when `last_backend()` is `dxgi`, require the probe result. Assert the whole-screen shot shows the fixture (mean brightness, no black). Log loudly when the runner cannot run DXGI, so a green run is not mistaken for DXGI coverage.

### P2-6. After a timeout the client keeps treating a dying helper as ready, and a start timeout never counts as a death
- `client.rs:587-604`. After a reply timeout the helper is killed, but `ready` and `proc` stay set until `on_exit`, so the next queued op is sent to the dying process and fails with HELPER_DOWN.
- `client.rs:454-468`. A start timeout marks the helper deliberate (`stopping`), so it never counts toward the three-deaths rule. If the helper hangs at start (AV scanning a `csc` compile, see P2-8), every desktop call waits 25 s while holding `queue`, indefinitely.
- **Fix:** set `ready = false` (and take `proc`) right at the timeout. Count start timeouts as deaths.

### P2-7. `taskkill` is resolved from the app folder first, and the helper is not in a job object
- `client.rs:194-207`. Rust's `Command::new("taskkill")` searches the exe's own directory before System32. The install is `currentUser` (`%LOCALAPPDATA%\AevalBuddy`, user-writable), so a `taskkill.exe` dropped there runs on every helper timeout or stop. This is not a privilege boundary, but it is a free persistence hook. On quit, the spawned taskkill thread may never run before the process exits; the helper then exits only on stdin EOF.
- **Fix:** put the PowerShell child in a Job object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (an immediate tree kill that also covers our own crash), and use an absolute `%SystemRoot%\System32\taskkill.exe` if it stays. `host/agent.rs:409` (`Command::new("cmd.exe")`, outside my files) has the same search-order issue.

### P2-8. Non-ASCII or smart-quote install paths break the bootstrap's prebuilt path
- `protocol.rs:155-157`, `protocol.rs:171-191`, `client.rs:151-171`. The prebuilt dirs and cache go into the bootstrap line as raw `'…'` literals, unlike the sources, which are base64. `-Command -` decodes stdin with the console's OEM code page, so a per-user install under `C:\Users\José\…` or a CJK name reaches `Test-Path` garbled. The SHA-pinned DLL is skipped and every helper start compiles host.cs in memory with csc, which costs 1-3 s and is blocked under some AV/ASR policies. On a UTF-8 code page, `ps_quote` does not escape U+2018/U+2019, which PowerShell treats as quote characters, so a path containing ’ ends the literal early and the bootstrap fails to parse.
- **Fix:** pass `$AevalCache` and `$AevalPrebuilt` base64-encoded with the same `[Text.Encoding]::UTF8.GetString(...)` wrapper. The `desktop-<hash>.dll` key is unchanged, since it hashes `$AevalCs` only.

### P2-9. Every desktop op makes a blocking main-thread round trip per app window
- `client.rs:540-551` calls `capture::hwnd_of(&w)` (`capture.rs:265-267`, `WebviewWindow::hwnd()`, a synchronous main-thread getter off the main thread) for every window on every `call()`, while holding the op queue on a tokio worker thread. When the main thread is busy (WebView2 creation, a long sync IPC handler), every agent or Find op stalls for that long.
- **Fix:** use `windows::flags::hwnd_for(label)` (already cached; `backdrop_pace` does this), and fall back to `hwnd()` only for unregistered windows.

### P2-10. Click and drag button events resend an absolute move, undoing the cursor correction
- `host.cs:2044-2048`, `host.cs:2086-2096`, `host.cs:1681-1688`. `MoveTo` corrects the 1-px rounding of normalized coordinates with `SetCursorPos`, but each down and up is sent through `SendMouseAbs` with `MOVE|ABSOLUTE`, which moves the cursor back to the rounded spot. Clicks on wide virtual desktops can land 1 px off.
- **Fix:** after `MoveTo`, send the button events as plain `MOUSEINPUT` with only the button flag (no MOVE/ABSOLUTE).

### P2-11. One slow app can stall every tree read (PLAUSIBLE)
- `host.cs:1261-1297`. `Generation` holds `genLock` across `Automation.AddStructureChangedEventHandler`, a cross-process registration with the target app. A slow provider holds the lock for up to UIA's transaction timeout, and every `readTree`/`getText`/`find` for *any* window queues behind it and replies TIMEOUT.
- **Fix:** register outside the lock. Insert a placeholder generation under the lock, then register and publish the handler afterwards.

### P2-12. The region selector can hang, and a stale close handler can resolve a newer pick
- `region.rs:90-106`. If `region.html` never finishes loading, the hidden window stays forever, `pick()` never returns, and `is_active()` stays true, so every later pick returns `None` until restart.
- `region.rs:145`. The old window's `Destroyed` handler calls `settle(None)` unconditionally, so if it fires after a new `select()` has set `PENDING`, it resolves the new pick with `None` while the new scrim stays open.
- **Fix:** add a load timeout that destroys the window and settles. Tag `Pending` with a generation and make the handler settle only its own.

### P2-13. The frame protocol answers with `Access-Control-Allow-Origin: *`
- `backdrop.rs:220-222`. Screen frames and the full-resolution loupe are readable by any origin loaded in our webviews. `csp` is `null` in `tauri.conf.json`. `glass.js` needs CORS only for `http://tauri.localhost`.
- **Fix:** send `Access-Control-Allow-Origin: http://tauri.localhost`, and optionally check the request's `Origin`.

### P2-14. Query captures pay avoidable copies
- `encode.rs:74-75` (`resize` clones a 25 MB source on every call) and `capture.rs:161` (converts the whole 4K frame to RGB just to crop a small area).
- **Fix:** resize from a borrowed `ImageRef`, and crop the BGRA rows before converting (`Rgb::from_bgra` already takes a stride, so pass an offset slice).

### P2-15. A glass window on a display whose grab failed gets another display's frame with its own bounds
- `backdrop.rs:503-512`. The fallback takes `frames.keys().next()` but sends `d.bounds` of the window's own display, so the blurred backdrop is misaligned (it shows the other monitor's content).
- **Fix:** skip that window this tick, or send the matching bounds.

### P2-16. If the buddy page dies while hit-testing is on, a full-screen window keeps eating clicks (PLAUSIBLE)
- `buddy.rs:715-720`. `ignoring = false` is set only from the page's hover logic. If the renderer crashes or reloads while the cursor is over the callout, the full-display buddy window stays click-capturing on that monitor until a tour or points step ends.
- **Fix:** on page load (`on_page_load` Finished) and on the WebView2 ProcessFailed path, call `set_click_through(true)`. Also add a watchdog: if no `buddy:set-ignore` arrives within N seconds while `ignoring == false` and the cursor is outside the callout's last rect, restore click-through.

### P2-17. The stdout reader stops on the first non-UTF-8 line
- `client.rs:423-426`. `lines.next_line()` returns `Err` on invalid UTF-8 and the loop ends. host.ps1's `fatal` path writes through `[Console]::Out` in OEM encoding (localized exception text), so the fatal reason is lost. If it ever happened after ready, every reply would go unread until timeouts killed the helper.
- **Fix:** read with `read_until(b'\n')` and use `String::from_utf8_lossy`.

### P2-18. Fidelity left unused (parity-preserving, product decision)
- `encode.rs:132-139`. The whole screen is sized at DIP, so 1920x1080 at 150% is sent at 1280x720 even though the 1600 px budget would allow 1600x900 from physical pixels (25% more linear resolution for small text).
- `encode.rs:100`. 4:2:0 chroma at q88 smears coloured UI text (red errors, blue links). 4:4:4 fits easily within 1.6 MB at these sizes.
- `capture/screen.rs:284-293`. With CAP-01 the screen is chosen by the pointer, so a hotkey pressed while the pointer is parked on the other monitor captures the wrong screen. Consider the foreground (non-AevalBuddy) window's monitor first, with the pointer as fallback.
- These match Electron (CAP-01/02); they are listed because answer quality is bounded by capture fidelity.

---

## Checked, no defect found
- Both helper pipes are drained (stdout reader and stderr reader tasks), stdin has its own writer task, and every op has a Rust-side timeout. The helper exits on stdin EOF, so it does not outlive a crashed app.
- Model and agent strings never reach the PowerShell parser: after `ready` they are JSON lines read by host.cs's own UTF-8 `StreamReader`. `type` uses `KEYEVENTF_UNICODE` (no SendKeys syntax), and `key` takes only the parsed chord grammar (unknown keys get BAD_ARGS).
- The prebuilt DLL is loaded with `Assembly.Load(bytes)` from the same bytes that were hashed (no TOCTOU), the SHA is fixed at compile time, and in release the user-writable cache is never read. `powershell.exe` is launched by absolute path with `-NoProfile`.
- Key-ups are always sent: chords go in one atomic `SendInput`, and modifiers and Alt are released in `finally`.
- GDI capture releases its DC, bitmap and screen DC on every path. DXGI always calls `ReleaseFrame`, reuses the staging texture, honours `RowPitch`, drops alpha before resizing, and recreates the duplication on any acquire error.
- Our sheets (region, lens, buddy) are created hidden with `content_protected`, re-excluded with WDA_EXCLUDEFROMCAPTURE before every show, and never use WDA_MONITOR.
- UIA runs on MTA request threads with per-request timeouts in the helper.
- The backdrop pace hook and the window getters run with the backdrop lock released. No lock in these files is held across a main-thread round trip that the main thread could need.
- Capture and encoding never run on the event-loop thread.
