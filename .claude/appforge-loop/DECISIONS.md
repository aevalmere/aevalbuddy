# Decisions

What was chosen, what was rejected, and why. Only the ones where the rejected
option was genuinely reasonable.

## The display is latched, not read live

Rejected: `workArea()` returning the cursor's display on every call. It is the
one-line version and it is wrong. Every positioning function runs during a
glide, so the panel would walk to another monitor because the user reached for
their mouse mid-answer. Latched at the moment the overlay is summoned, which is
where intent is known, and re-latched on any display change.

## Failover consults the catalogue only when it is already in hand

Rejected: fetching the model list inside `nextFreeModel`. It would put a
network round trip inside a retry that is already the slow path, on an app
whose default models are queued. A cold cache returns "no opinion" and the
curated order still applies, because an empty cache must never mean "nothing to
try" -- that would be a worse failure than the one being fixed.

Accepted as the cost: the catalogue is warmed once at boot, one unauthenticated
GET, failure silent.

## history.trim estimates sizes rather than measuring them

Rejected: keeping `JSON.stringify` but guarding it with the count check the
comment already claimed existed. That would have fixed the common case and left
the pathological one -- a history at the cap still serialises on every answer,
which is exactly when the user has most to lose.

The estimator reads deliberately high. An estimate that read low would let the
file exceed its cap, which is worse than the cost it replaced. The self-check
measures the real serialised length against the budget rather than trusting the
estimate that produced it: 4185557 against 4194304, so it is on the safe side
and not throwing away history for nothing.

## The multi-line field keeps the 32px rhythm rather than introducing a new one

Rejected: a comfortable multi-line box with its own padding. DESIGN.md v0.4
lists "paddings run 14/16/12 on the panel, 8/10 in the field, 30px buttons" as a
tell, against a rule of "one rhythm: 12px gutter, 32px control height,
everywhere". One row is 21px of line plus 11px of padding, which is exactly the
control height, so a single-line question is visually identical to what shipped
and each further line adds exactly one line of panel.

Rejected: seven rows as a magic number in JS. It is a CSS `max-height` so the
field and the panel cannot disagree about it.

## D8 was demoted after a probe rather than shipped as written

The first write-up called it a BLOCKER: `motion.stop` reads `win.id` from
inside its own 60Hz interval on the destroyed branch, which would be an
unkillable error loop. A twenty-line Electron probe showed `.id` survives
destroy. The defect is the null case alone. The dramatic version was the more
publishable one, which is the reason to check.

## The gate fails on dead listen channels

Rejected: leaving it as a printed note. A note is what `modelDefaultOffered=false`
was, and that went past unread for a whole release. Making it fail first
required teaching the check that main.js sends through a local `send` helper,
because matching only `webContents.send` reported four live channels as dead --
a check that cries wolf gets switched off, which is the same outcome as not
having one.

## Not done, deliberately

- **No test framework.** The repo promises no build step and one runtime
  dependency. `AEVAL_SMOKE=1` already boots the real app and asserts on real
  windows, which is a better gate for this program than a mocked unit suite,
  and adding a second way to run tests is the "two tools for one concern"
  defect.
- **Reasoning-token streaming.** The default reader and brain are both
  reasoning models, and OpenRouter can stream `delta.reasoning`. Showing it
  would fill the wait with something to read. It is a real feature and it is
  not a defect, so it stayed out of a defect-driven run.
