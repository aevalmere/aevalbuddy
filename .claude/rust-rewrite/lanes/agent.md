# Lane brief: agent

Branch `rust/agent`, cut from `rust-rewrite`. 22 items. Cross-lane tables (ownership, hotspots, contracts, decisions, merge order) are in `.claude/rust-rewrite/BACKLOG.md`; everything you need to do your items is in this file.

## Goal

The agent cannot do anything irreversible without the person seeing exactly what will happen: the confirm card carries the full text, keyboard activation and unknown buttons are gated, taking over the mouse stops the pending step, a timed-out helper request can never act late, and Stop cancels typing in progress. Around that: native models, a provider fixed at start, retries that honour Retry-After, a helper that dies with the app, and task copy that says what happened once.

## Files you own

`src-tauri/src/agent/**`, `src-tauri/src/host/agent.rs`, `src-tauri/src/host/desktop_ops.rs`, `src-tauri/src/desktop/client.rs`, `src-tauri/src/desktop/protocol.rs`, `lib/desktop/host.cs`, `lib/desktop/host.ps1`, your block in the IPC allowlists (after windows C1), and (after harness C1) `src-tauri/src/selftest/probes/agent.rs`, `tools/smoke-labels/agent.txt`.

## Contracts you provide (C1 first)

- **K-20** `agent:confirm-request.{value, truncated}` (B-006, real in C1) and `agent:step.planIndex` (field present; B-144 fills it).
- **K-21** `agent:consent-request.{provider, model, modelName, recipients}` (from today's settings until K-16 and K-18 land).

## Contracts you consume

- **K-04** `outputs::fresh` (windows); **K-13** `buddy::agent_point_phys` (capture).
- **K-16** candidates and `model_for`, **K-17** `ApiError` fields, **K-18** recipients, **K-26** fake LLM fixtures (llm).
- **K-30** (harness).

## Smoke labels you port

`gate*`, `agentRun`, `agentFocusKept`, `agentJsonMode`, `agentStopMs`, `agentTakeover`, `agentMaxSteps`, `agentConfirmGate`, `passwordRedacted`. Globs expand against `.claude/rust-rewrite/audit/parity-ipc.md` §3.

## Shared-document entries you write

D-27, D-30; PARITY evidence for AGENT and DESK-protocol rows your items close, and the new gate verdict tables (AGENT-05).

## Lane notes

- `lib/desktop/host.cs` is compiled at runtime by the helper; there is no local toolchain, so CI's desktop selfcheck and your probe are the only proof. Keep the C# change small and log each new op's result in the helper's own log line.

## Standing item: port your smoke labels

Every PARITY label under "Smoke labels you port" and every new label below is printed by `src-tauri/src/selftest/probes/agent.rs` through the K-30 API, with the Electron semantics from `.claude/rust-rewrite/audit/parity-ipc.md` §3 and `main.js` on `main`, and listed in `tools/smoke-labels/agent.txt` in the same commit that makes it print `=ok`. Acceptance: the rust workflow's smoke log shows each label `=ok` and the labels gate passes.

New labels your items add, beyond the PARITY list above (you print these): `agentNativeModel`, `cancelStopsTyping`, `clickExact`, `confirmFullText`, `gateKeyActivation`, `helperTimeoutNoLateInput`, `takeoverStopsStep`.

## Items

| ID | Severity | Title |
|---|---|---|
| B-006 | P0 | The confirm request carries the full text that will be typed |
| B-008 | P0 | Keyboard activation of a focused control goes through the gate |
| B-047 | P1 | The agent uses native model ids, picks its provider at start, and honours a pinned model |
| B-048 | P1 | Taking over the mouse stops the step that was about to fire |
| B-049 | P1 | The safety gate is a real allowlist |
| B-050 | P1 | A timed-out helper request can never act later, and Stop cancels typing in progress |
| B-051 | S1 | The consent request names the real recipient |
| B-130 | P2 | Two quick starts cannot leave an unstoppable run |
| B-131 | P2 | Failover never switches tool dialect under an old system prompt |
| B-132 | P2 | Overloads and Retry-After are retried properly; native thinking turns get time |
| B-133 | P2 | Coordinates are validated against the screenshot they came from |
| B-134 | P2 | A process name can never panic the run |
| B-135 | P2 | Finding the last JSON object is linear |
| B-136 | P2 | `get_page_text` is redacted like every other read |
| B-137 | P2 | The helper client never talks to a dying helper and survives odd output |
| B-138 | P2 | The helper dies with the app, and system tools are run by absolute path |
| B-139 | P2 | Install paths with accents, CJK or smart quotes keep the prebuilt helper |
| B-140 | P2 | Desktop ops look up window handles without main-thread hops |
| B-141 | P2 | Clicks land on the corrected pixel |
| B-142 | P2 | One slow app cannot stall every tree read |
| B-143 | S2 | Task copy says "task" and gives the remedy once |
| B-144 | S2 | Each act says which plan item it serves |

Work in severity order (P0, then S1 and P1, then S2 and P2, then S3), after your C1 if you have one.

### B-006 · P0 · agent · The confirm request carries the full text that will be typed
- **Sources:** review-llm-agent P0-1 (Rust half).
- **Files:** `src-tauri/src/agent/run.rs`, `src-tauri/src/agent/describe.rs`.
- **Fix:** Add `"value": <full text>` to the `agent:confirm-request` payload for `type`, `form_input` and `set_value` (passwords stay dotted, as `describe` does today). Keep the described `text` label. Add `"truncated": true` when the label's quote is shorter than the value. Contract K-20.
- **Acceptance:** UNIT `agent::run::tests::confirm_payload_carries_full_value` (a 200-character type step: payload `value` equals the input, `truncated == true`); SMOKE+ `confirmFullText` (agent probe asserts the emitted payload).

### B-008 · P0 · agent · Keyboard activation of a focused control goes through the gate
- **Sources:** review-llm-agent P0-2.
- **Files:** `src-tauri/src/agent/gate.rs`, `src-tauri/src/agent/run.rs` (`inspect`).
- **Fix:** When a `key` chord contains `space` or `enter`, or `type` text contains a space or newline, `inspect` reads the focused node from `lazy.tree()` and the gate runs the click checks on it: `name_verbs` on name, automation id and help text; an unnamed button is unreadable (ask); a mail or chat app with a clickable role asks; a risky page asks. `space` and `tab` leave `FREE_KEYS`: `space` is free only when the focused role is an editable text or document role. `tab` stays free (it only moves focus) but the next activation is checked as above.
- **Acceptance:** UNIT `gate::tests::space_on_focused_send_button_asks`; UNIT `gate::tests::typed_space_on_button_asks`; UNIT `gate::tests::space_in_text_field_is_free`; SMOKE+ `gateKeyActivation`.

### B-047 · P1 · agent · The agent uses native model ids, picks its provider at start, and honours a pinned model
- **Sources:** review-llm-agent P1-1 (agent half); parity-features #1 (AGENT-15); design-ux U20 (agent half), §3.4.
- **Files:** `src-tauri/src/host/agent.rs`, `src-tauri/src/agent/run.rs`.
- **Fix:** `start_run` picks the first candidate (K-16) once: native → `model_for(settings, provider, Kind::Agent)`; OpenRouter → `resolve_live`; never `heal_agent_model` a native id. The run keeps that provider for its whole life (no mid-run switch). `agent:start` and `agent:do-it` accept optional `{provider, model}`; a pinned model never fails over. The trail opens in `outputs::fresh` (K-04).
- **Acceptance:** UNIT `host::agent::tests::native_only_run_uses_native_model`; UNIT `host::agent::tests::pinned_model_never_fails_over`; SMOKE+ `agentNativeModel`.

### B-048 · P1 · agent · Taking over the mouse stops the step that was about to fire
- **Sources:** review-llm-agent P1-2.
- **Files:** `src-tauri/src/agent/run.rs`.
- **Fix:** Immediately before `acting = true`, and between calls in the per-turn loop, if `paused` then `wait_while_paused().await`; on resume re-run `inspect` and the gate (the screen may have changed), or return `Outcome::text("Not executed: the user took over.")` and drop the turn's remaining calls.
- **Acceptance:** UNIT `run::tests::pause_during_dodge_drops_the_action`; SMOKE `agentTakeover`, SMOKE+ `takeoverStopsStep` (fake LLM returns two calls; pause after the gate; dispatched count stays 0).

### B-049 · P1 · agent · The safety gate is a real allowlist
- **Sources:** review-llm-agent P1-6.
- **Files:** `src-tauri/src/agent/gate.rs`, `src-tauri/src/agent/run.rs`.
- **Fix:** No card only for known-harmless roles (menubar and menu items without risky verbs, tabs, tree items, scroll bars, text fields, documents) or a small harmless-name list; any other clickable without a harmless reason asks. `double_click` on list or tree items in `explorer` or the desktop is treated as "opens or runs the file" and asks. Read modal text for any window or tree node with role `dialog`, not only class `#32770`. Add the missing verbs (shut down, restart, trash, clear all, clear history, unsubscribe, cancel subscription, deactivate, close account, end task) with Unicode word boundaries, plus localized equivalents for the shipped UI languages (de, fr, es, it, pt, ja, zh: send, delete, buy, pay, post, remove). Fix the module doc so it describes what the code does. Update the smoke-v21 gate tables (`gateBypasses`, `gatePassesPlainInput`, `gateFailsClosed`, `gatePlanFirst`, `gateChords`) to the new verdicts.
- **Reconciled:** AGENT-05's verdict tables change; PARITY records the stricter gate. Decision D-27.
- **Acceptance:** UNIT `gate::tests::unknown_named_button_asks`; UNIT `gate::tests::explorer_double_click_asks`; UNIT `gate::tests::web_dialog_text_is_read`; UNIT `gate::tests::localized_send_asks`; SMOKE `gateBypasses`, `gateFailsClosed`.

### B-050 · P1 · agent · A timed-out helper request can never act later, and Stop cancels typing in progress
- **Sources:** review-desktop P1-1; review-llm-agent P2-1.
- **Files:** `lib/desktop/host.cs`, `src-tauri/src/desktop/client.rs`, `src-tauri/src/desktop/protocol.rs`, `src-tauri/src/agent/run.rs`.
- **Fix:** Give each helper request a cancellation flag; `Handle` sets it on timeout, and every input primitive (`SendInputs`, `SetCursorPos`, `MoveTo`, `SetFocus`, pattern `Invoke`/`SetValue`/`Toggle`, `FocusWindow`) checks it before acting; `TypeInto` and `KeyCmd` check it per chunk. Add a `cancel` op that sets the flag of every in-flight request. Rust: treat TIMEOUT on any input op as an unknown outcome (re-read state before any retry), size `key`'s timeout from repeat x chords as `type` does, and check `ended()` immediately before every desktop op that acts (after `window_set().await`, after a STALE_REF re-read, between the two ops of `focus_window` by title). Esc sends `cancel`.
- **Acceptance:** UNIT `client::tests::key_timeout_scales_with_repeat`; SMOKE+ `cancelStopsTyping` in `selftest/probes/agent.rs` (type 2000 characters into Notepad through `desktop::client`, send `cancel` after 200 ms; text length < 2000 and stable 500 ms later; CI has Notepad); SMOKE+ `helperTimeoutNoLateInput`, SMOKE `agentStopMs`.

### B-051 · S1 · agent · The consent request names the real recipient
- **Sources:** design-ux U6 (agent half), C-30.
- **Files:** `src-tauri/src/host/agent.rs`.
- **Fix:** `agent:consent-request` gains `recipients` (K-18, from `llm::recipients`) and `provider`/`model`/`modelName` of the candidate the run will use (K-21). The renderer builds C-30 from it (B-171).
- **Acceptance:** UNIT `host::agent::tests::consent_payload_names_provider`.

### B-130 · P2 · agent · Two quick starts cannot leave an unstoppable run
- **Sources:** review-llm-agent P2-2.
- **Files:** `src-tauri/src/agent/run.rs`, `src-tauri/src/host/agent.rs`.
- **Fix:** Check-and-set under one `inner.run` lock before any await; a second start returns `busy`.
- **Acceptance:** UNIT `run::tests::concurrent_starts_yield_one_run`.

### B-131 · P2 · agent · Failover never switches tool dialect under an old system prompt
- **Sources:** review-llm-agent P2-3.
- **Files:** `src-tauri/src/agent/run.rs`.
- **Fix:** Rewrite `messages[0]` when `use_tools` changes on failover (or fail over only within the same dialect).
- **Acceptance:** UNIT `run::tests::dialect_switch_rewrites_system_prompt`.

### B-132 · P2 · agent · Overloads and Retry-After are retried properly; native thinking turns get time
- **Sources:** review-llm-agent P2-4 (agent half).
- **Files:** `src-tauri/src/agent/run.rs`.
- **Fix:** Honour `ApiError.retry_after_ms` (K-17) up to 20 s instead of the fixed 2 s / 6 s; treat 529 as retryable after the first action; native agent turns get a 180 s cap (or stream), reported as a timeout only after it.
- **Acceptance:** UNIT `run::tests::retry_after_is_honoured`; UNIT `run::tests::overload_529_retries`.

### B-133 · P2 · agent · Coordinates are validated against the screenshot they came from
- **Sources:** review-llm-agent P2-5.
- **Files:** `src-tauri/src/agent/run.rs`, `src-tauri/src/agent/tools.rs`.
- **Fix:** Require a finite positive scale; reject non-numeric components and points outside the shot or the person's region; `zoom` updates nothing but its tool text says coordinates stay in screenshot space.
- **Acceptance:** UNIT `run::tests::bad_coordinates_are_rejected`.

### B-134 · P2 · agent · A process name can never panic the run
- **Sources:** review-llm-agent P2-6.
- **Files:** `src-tauri/src/agent/run.rs`.
- **Fix:** Strip a case-insensitive ASCII `.exe` suffix by byte length (`p.len() - 4`) instead of slicing at the lowercased stem's length.
- **Acceptance:** UNIT `run::tests::app_label_kelvin_and_dotted_i`.

### B-135 · P2 · agent · Finding the last JSON object is linear
- **Sources:** review-llm-agent P2-7.
- **Files:** `src-tauri/src/agent/tools.rs`.
- **Fix:** One forward pass with a brace stack (string-aware), or scan only the last 8 KB.
- **Acceptance:** UNIT `tools::tests::last_object_is_linear` (1 MB of `}` returns in < 50 ms).

### B-136 · P2 · agent · `get_page_text` is redacted like every other read
- **Sources:** review-llm-agent P2-8.
- **Files:** `src-tauri/src/agent/run.rs`.
- **Fix:** Pass the text through `redact()`.
- **Acceptance:** UNIT `run::tests::page_text_is_redacted`; SMOKE `passwordRedacted`.

### B-137 · P2 · agent · The helper client never talks to a dying helper and survives odd output
- **Sources:** review-desktop P2-6, P2-17.
- **Files:** `src-tauri/src/desktop/client.rs`.
- **Fix:** At a reply timeout set `ready = false` and take `proc` immediately. Count a start timeout as a death (three-deaths rule). Read stdout with `read_until(b'\n')` and `String::from_utf8_lossy`.
- **Acceptance:** UNIT `client::tests::start_timeout_counts_as_death`; UNIT `client::tests::non_utf8_line_does_not_stop_reader`.

### B-138 · P2 · agent · The helper dies with the app, and system tools are run by absolute path
- **Sources:** review-desktop P2-7.
- **Files:** `src-tauri/src/desktop/client.rs`, `src-tauri/src/host/agent.rs`.
- **Fix:** Put the PowerShell child in a Job object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (feature `Win32_System_JobObjects`, pre-enumerated). Use `%SystemRoot%\System32\taskkill.exe` and `%SystemRoot%\System32\cmd.exe` absolute paths.
- **Acceptance:** UNIT `client::tests::system_tools_use_absolute_paths`; SMOKE `helperNotAtBoot` still passes.

### B-139 · P2 · agent · Install paths with accents, CJK or smart quotes keep the prebuilt helper
- **Sources:** review-desktop P2-8.
- **Files:** `src-tauri/src/desktop/protocol.rs`, `src-tauri/src/desktop/client.rs`.
- **Fix:** Pass `$AevalCache` and `$AevalPrebuilt` base64-encoded with the same `[Text.Encoding]::UTF8.GetString(...)` wrapper the sources use.
- **Acceptance:** UNIT `protocol::tests::bootstrap_paths_are_base64` (path `C:\Users\José’s\…` round-trips).

### B-140 · P2 · agent · Desktop ops look up window handles without main-thread hops
- **Sources:** review-desktop P2-9.
- **Files:** `src-tauri/src/desktop/client.rs`.
- **Fix:** Use `windows::flags::hwnd_for(label)` (cached), falling back to `hwnd()` only for unregistered windows.
- **Acceptance:** UNIT where testable; SMOKE `desktopOwnExcluded`.

### B-141 · P2 · agent · Clicks land on the corrected pixel
- **Sources:** review-desktop P2-10.
- **Files:** `lib/desktop/host.cs`.
- **Fix:** After `MoveTo`, send button down/up as plain `MOUSEINPUT` with only the button flag (no MOVE/ABSOLUTE); same for drag.
- **Acceptance:** SMOKE+ `clickExact` in `selftest/probes/agent.rs` (click a known small target in a webview fixture through `desktop::client`; `elementAt` at the click point names the target). The desktop selfcheck files belong to capture, so the check lives in the agent probe.

### B-142 · P2 · agent · One slow app cannot stall every tree read
- **Sources:** review-desktop P2-11.
- **Files:** `lib/desktop/host.cs`.
- **Fix:** Insert a placeholder generation under `genLock`, register `AddStructureChangedEventHandler` outside it, then publish the handler.
- **Acceptance:** desktop selfcheck `uiaTreeMs` unchanged or better; code note in Progress.

### B-143 · S2 · agent · Task copy says "task" and gives the remedy once
- **Sources:** design-visual F8 (Rust half); design-ux U46 (C-60), C-62.
- **Files:** `src-tauri/src/agent/run.rs`, `src-tauri/src/agent/describe.rs`.
- **Fix:** "The agent stopped" → "The task stopped" everywhere (run.rs:68, :739, :1971); internal error → `The task stopped because of a problem inside AevalBuddy. Nothing else was done.` (C-60); already running → `A task is already running.` (C-62). When a failed step row already shows the reason, the final message carries only the remedy (`Start Outlook normally, then Retry.`).
- **Acceptance:** UNIT `run::tests::final_text_is_remedy_only_after_failed_step`; existing trail text tests updated.

### B-144 · S2 · agent · Each act says which plan item it serves
- **Sources:** design-visual F22 (Rust half).
- **Files:** `src-tauri/src/agent/run.rs`.
- **Fix:** `agent:step` gains `planIndex` (K-20): the index of the plan item the act serves (from the model's tool call when given, else the count of completed acts clamped to the plan length).
- **Acceptance:** UNIT `run::tests::step_payload_has_plan_index`.

## Rules

The coordinator's rules, verbatim: "LF endings; never run the app locally; never weaken or skip a test or gate to get green; commit messages in sentence case describing the effect like the existing log, each ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; push only its own branch".

What each one means here:

- **LF endings** in every file you write. Before each commit run `git diff --cached --check` and `git ls-files --eol <changed files>` (expect `i/lf w/lf`). The repo has no `.gitattributes`, so your editor's endings are what gets committed.
- **Never run the app locally.** No `cargo tauri dev`, no launching a built exe, no `npm start` of the Electron app. This machine has no Rust toolchain and nine lanes share it; CI is the build and the test bed. Local checks are limited to `node --check <file>` for JS and reading code.
- **Never weaken or skip a test or gate to get green.** No `#[ignore]`, no deleted or loosened assertions, no raised thresholds, no `continue-on-error`, no label removed from a label file, no scene removed from ui-shots. An expected value changes only where one of your items specifies the new value (for example B-049's gate tables or B-087's defaults test).
- **Commits** in sentence case describing the effect, like `git log --oneline -15` on `rust-rewrite` (for example "Keep the first-run tour out of the capture and desktop checks"). Item IDs may go in the body. Every message ends with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Push only your own branch**, `rust/agent`: `git push origin HEAD:rust/agent`. Never push `rust-rewrite`, `main`, `ci-logs`, `shots` or another lane's branch; never force-push.
- **Never poll the GitHub API (60 req/hr shared by the whole machine).** No `gh run`, `gh api`, `gh pr`, or `curl` to api.github.com. CI results come only from `tools/ci-wait.mjs`, which reads the `ci-logs` branch with git.
- Edit only the files listed under "Files you own". If you need a change elsewhere, write it under Blockers (file, the change, which item needs it) and carry on with other items. Never create the per-lane files that harness C1 creates (`selftest/probes/<lane>.rs`, `tools/smoke-labels/<lane>.txt`, `tools/ui-shots/lanes/<lane>.mjs`); merge `origin/rust-rewrite` once they exist.
- Do not edit `DESIGN.md`, `PARITY.md`, `ARCHITECTURE.md`, `REQUESTS.md`, `BACKLOG.md` (only B-149 appends to its "Post-merge findings" section), the audits, other lanes' briefs, the Electron reference (`main.js`, `lib/**` except files you own, `package*.json`, `electron-builder.yml`) or `README.md`.
- Never commit a real API key. Tests and probes use fake keys and the fake LLM.

## Verification loop

Every cycle:

1. Make a batch of changes: one or more items, each with its unit tests, probe labels, ui-shots scenes and checks, and its entries.
2. `git diff --cached --check`, `node --check` on changed `.js` and `.mjs` files, re-read the diff against the item specs.
3. Commit, then push once: `git push origin HEAD:rust/agent` (first push: `git push -u origin HEAD:rust/agent`).
4. Wait for CI with the script below. If the push touched `renderer/`, `settings/`, `history/`, `src-tauri/bridge/` or `tools/ui-shots/`, wait on `ui-shots` as well as `rust`.
5. Exit 0: record the items as green in Progress with the sha. Exit 1: read what it printed, fix, and go to step 1. Exit 2: follow the printed sha, or wait again.

The coordinator's CI recipe, verbatim:

- Usage: `node tools/ci-wait.mjs <sha> [workflow=rust|ui-shots] [--timeout-min 30]` (sha can be HEAD/short). It `git fetch`es the `ci-logs` branch every 45 s; no GitHub API.
- Exit 0 = green; 1 = failure (prints summary.md, errors.txt, last 80 lines of the failing gate's log, and the `git show origin/ci-logs:<dir>/fmt.patch | git apply` command when rustfmt fails); 2 = timeout or cancelled (prints the newer sha to wait on if a later push superseded it).
- Full logs: `origin/ci-logs:<ref with / as ->/<sha>/<workflow>/` holds summary.md, meta.json, errors.txt, and prebuild/fmt/clippy/test/build/smoke/capture/desktop .log (ui-shots: bridge/playwright/shots .log). `<ref>/latest/<workflow>/` = newest run.
- ui-shots only runs when renderer/settings/history/bridge/tools/ui-shots paths change, so `ci-wait <sha> ui-shots` exits 2 on other pushes; that is expected.
- Pushing a new commit to the same branch cancels the running CI for the older sha (concurrency cancel-in-progress), so batch changes and push once per cycle.

For this lane the log directory is `origin/ci-logs:rust-agent/<sha>/<workflow>/` and the newest run is `origin/ci-logs:rust-agent/latest/<workflow>/`. Read a file with `git show "origin/ci-logs:rust-agent/latest/rust/smoke.log"` (use PowerShell or set `MSYS_NO_PATHCONV=1` in Git Bash, which otherwise mangles the `ref:path` argument). The rust workflow takes up to an hour on a cold cache; use `--timeout-min 70` when the build is cold. When the same sha is the tip of two branches (right after merging `origin/rust-rewrite` with nothing new on top), add `--ref rust/agent`.

## Starting and staying current

1. Your worktree starts at `rust-rewrite` HEAD. Create the branch with `git switch -c rust/agent` if it does not exist.
2. Read this brief, the cross-lane tables in `.claude/rust-rewrite/BACKLOG.md` (lanes, ownership, hotspots, contracts), `DESIGN.md`, and for each item the audit sections and PARITY rows it cites. The code is the ground truth for current behaviour; `main.js` on `main` is the ground truth for Electron behaviour.
3. Push your C1 (if "Contracts you provide" lists any) before anything else, with a commit message starting `C1 contracts:`. When it is green, write `C1 green at <sha>` in Progress.
4. Each cycle, `git fetch origin`. When a contract you need has landed (`git grep -n "<symbol>" origin/rust-rewrite -- <file>`) or `rust-rewrite` has moved, `git merge origin/rust-rewrite` (a merge commit; never rebase or force-push). A conflict in a file you own: resolve it. A conflict in a file you do not own means you edited it by mistake: take `origin/rust-rewrite`'s side and note it in Progress.
5. If CI fails for a reason entirely outside your files (a broken merge on `rust-rewrite`, a runner outage), record it under Blockers with the log line and keep working on what you can verify.

## Shared documents

Never edit `DESIGN.md`, `PARITY.md` or `ARCHITECTURE.md`. In the same commit as the change, append to `.claude/rust-rewrite/entries/agent.md`:

```
## <DESIGN|PARITY|ARCHITECTURE> · <section or row id> · <D-xx or B-xxx>
Add | Replace "<exact current text>" with:
<exact new text>
```

For a PARITY row whose status or evidence changes: `## PARITY · <row id> · B-xxx` and the line `status: <value>; evidence: <UNIT test or SMOKE label>`. Every decision your lane owns (see "Shared-document entries you write") gets an entry with its reason. The merge agent folds the entries in when your branch merges.

## Stop condition

Stop when every item below and the standing labels task is green on CI at your branch head (rust, and ui-shots when your changes touch its paths), each recorded in Progress with its sha and acceptance; or when every item that is not green has a named entry under Blockers saying what blocks it, why, and what would unblock it. Do not stop with an item that is neither green nor blocked. When you stop, make sure the final state is pushed and the last `ci-wait` result is in Progress.

## Progress

(One line per item as it turns green: `- B-xxx: green at <sha>; <acceptance names>`. Also `C1 green at <sha>` and any notes the next agent needs.)

- C1 green at 89fd79a (K-20 `agent:confirm-request.{value, truncated}` and `agent:step.planIndex`; K-21 consent `provider`/`providerName`/`model`/`modelName`/`free`/`recipients` from today's settings until K-16 and K-18 land).
- B-006: green at 89fd79a; UNIT `agent::run::tests::confirm_payload_carries_full_value`, UNIT `agent::describe::tests::the_card_value_is_the_whole_text` (SMOKE+ `confirmFullText` waits for harness C1).
- B-144: green at 89fd79a; UNIT `run::tests::step_payload_has_plan_index`.
- B-051: in progress; K-21 fields green at 89fd79a (UNIT `host::agent::tests::consent_payload_names_provider`); `recipients` switches to `llm::recipients` when K-18 lands.
- B-008: unit tests green at 3918098 (`gate::tests::space_on_focused_send_button_asks`, `typed_space_on_button_asks`, `space_in_text_field_is_free`); adds the helper op `focused` (host.cs `FocusedCmd`, `Desktop::focused`, adapter `focused`). SMOKE+ `gateKeyActivation` is printed by `agent/smoke.rs`.
- B-049: unit tests green at 3918098 (`gate::tests::unknown_named_button_asks`, `explorer_double_click_asks`, `web_dialog_text_is_read`, `localized_send_asks`); host.cs reports a Pane/Group/Custom whose localized type is "dialog" as role `dialog`. SMOKE `gateBypasses`, `gateFailsClosed` are printed by `agent/smoke.rs`.
- B-048: unit test green at 3918098 (`run::tests::pause_during_dodge_drops_the_action`); SMOKE+ `takeoverStopsStep` is printed by `agent/smoke.rs`; SMOKE `agentTakeover` is in the Notepad batch.
- B-050: unit tests green at 3918098 (`client::tests::key_timeout_scales_with_repeat`, `run::tests::stop_while_typing_cancels_the_helper`, `run::tests::a_timed_out_step_is_an_unknown_outcome`); SMOKE+ `cancelStopsTyping`, `helperTimeoutNoLateInput` and SMOKE `agentStopMs` are in the Notepad batch.

## Blockers

(None yet. Format: `- B-xxx: <what blocks it>; <why>; <what would unblock it>`.)
