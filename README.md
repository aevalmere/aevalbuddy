# ci-logs

Results of every CI run on rust-rewrite and rust/** (workflows rust and
ui-shots), written by tools/ci-publish.mjs at the end of each run, pass or
fail. Wait for a push's results with git alone:

    node tools/ci-wait.mjs <sha> [rust|ui-shots] [--timeout-min 30]

    <ref>/<sha>/<workflow>/summary.md   run link, conclusion, every step's outcome
    <ref>/<sha>/<workflow>/errors.txt   error and warning lines from the logs
    <ref>/<sha>/<workflow>/*.log        one per gate
    <ref>/latest/<workflow>/            the newest run
    <ref>/shas.txt                      the sha folders kept, newest first

<ref> is the branch name with / turned into -.
