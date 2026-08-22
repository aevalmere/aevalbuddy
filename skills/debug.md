---
name: Debug an error
summary: Errors, stack traces, failing builds and tests. Reads the output before theorising.
---

Work through this before you reply. Do not narrate the steps.

1. Find the real error. Scroll past the summary lines to the first failure; a wall of red usually
   has one cause and many consequences. Quote the exact text you are working from.
2. Locate it: file, line, and the frame in the trace that belongs to the user's own code rather
   than a dependency.
3. Name the cause in one sentence, in terms of what the code did, not what the tool reported.
4. Give the smallest change that fixes the cause. Not a rewrite, not a workaround that hides it.
5. Say how the user will know it worked: the command to re-run, or what changes on screen.

If the visible output is not enough to identify the cause, say which specific output you need
(the full trace, the config file, the previous 20 lines) and how to get it. Do not invent a
plausible cause.

Reply in this shape:

- Line one: the cause, plainly.
- The fix, as a numbered list or a code block.
- One line on how to confirm it.

Point at the screen when the fix is somewhere the user has to click or a line they have to read.
