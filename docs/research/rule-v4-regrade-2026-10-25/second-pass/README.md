# Second pass of evaluator audit round evaluator-audit-v2-2026-10-29

The owner chose Claude Opus 5.5 at high effort as the second grader of the 170 sampled batches of [`sample.json`](../../../../bench/grading/current/audit/evaluator-audit-v2-2026-10-29/sample.json) (691 units). The pass may spend at most $275 at list-price equivalent, reruns included, and $6 a session. Launches stop at 90% of the Claude week ([audit decision 10](../../../../bench/grading/rulings/cohort-rebuild-audit.v6.md), [`approval-second-pass.v1.json`](../approval-second-pass.v1.json)). [`authorization.second-pass.v1.json`](../authorization.second-pass.v1.json) pins these limits, the plan, the runtime and the files below. The no-charge preflight is [`preflight-second-pass.v1.json`](../preflight-second-pass.v1.json).

## Files

- [`second_pass.py`](second_pass.py) is the last round's [driver](../../rule-v3-regrade-2026-10-23/second-pass/README.md) with a Claude grader. Its `plan`, `preflight`, `run` and `status` commands, exit codes, budget rule and restart rules are the ones that README states, with these changes:
  - The grader is `claude`, `claude-opus-5-5`, `high`. The queue copies the installed `claude` into `client-bin/` and refuses a version other than `grader.cliVersion`.
  - `grade dispatch` gets `--max-budget-usd 6`. Each running session counts at that limit until its priced receipt settles it, so the cap check uses a true upper bound.
  - The preflight runs `grading probe-claude`, and the quota check reads the Claude meters.
  - `run` reads `grades.json` nowhere. It relies on `evaluator-audit claims` and `evaluator-audit second`, which check the sample's pinned grades, so batches may join `grades.json` while the round runs.
  - It never runs `grade map`.
- [`plan.v1.json`](plan.v1.json) lists the 170 batches in sample order. Each batch has the cache replacement manifest its regrade queue used (109 rebuilt, 39 selected, 22 with none) and the list price of its latest Opus first grade, which totals $214.25.
- [`launch.py`](launch.py) runs the pass unattended, in chunks:
  1. It records the week's meter as a baseline and runs a trial of 5 batches.
  2. It projects the whole pass. The projected cost is the trial's spend scaled by the batches' Opus first-grade prices. The meter shows whole points, so d points over a spend of s put the rate between (d − 1)/s and (d + 1)/s.
  3. It runs the full pass when even the highest of those rates keeps the projection under 30 points of the week and the projected cost is inside the cap. It stops when the lowest rate reaches 30 points or the cost is over the cap. Otherwise it measures 5 more batches and decides again.
  4. Throughout, it stops when the week has moved 30 points since the baseline or reaches 90%.
  5. When the five-hour window reaches 90%, it waits for the window to reset.
  6. When the stored Claude sign-in expires within 40 minutes, it waits, checking every 5 minutes, because only a person renews it.
  7. It retries a failed batch only for a host restart, an overloaded or failing provider, or the plan's usage limit. Each retry writes the attempt's `disposition.json` with the evidence. A batch gets at most 2 retries and the pass at most 8. Any other failure stops it.
  8. A stop writes its reason to `launcher.json` and `launcher.log`.
- `test_second_pass.py` and `test_launch.py` exercise both scripts against fakes in a temporary directory, with no model calls: `python3 -I <file>`.

## The units

The pass runs from a shell, never from an assistant session. Two systemd user units in `~/.config/systemd/user` start it, in the checkout at `/home/jack/.cache/regrade-v4`:

- `second-pass-v4.timer` fires once, at Sat 2026-10-31 5:10 AM ET, with `Persistent=true`. If the host is off then, it fires at the next boot.
- `second-pass-v4.service` runs `launch.py run`, and it is enabled for `default.target`, so a reboot during the pass resumes it. Before 5:10 AM, or once the pass has finished or stopped, a start does nothing and exits 0. The host keeps the user manager running without a login (`Linger=yes`).

Status, without a model call:

```sh
cd ~/.cache/regrade-v4
python3 -I docs/research/rule-v4-regrade-2026-10-25/second-pass/launch.py status
systemctl --user list-timers second-pass-v4.timer
systemctl --user status second-pass-v4.service
tail -f .local/second-pass-v4/launcher.log      # one plain line per decision, in ET
tail -f .local/second-pass-v4/driver.log        # second_pass.py output
```

To resume after a stop, read the reason, fix its cause, delete the `"phase": "stopped"` entry from `.local/second-pass-v4/launcher.json` (set it to `"full"`), and run `systemctl --user start second-pass-v4.service`. To cancel before it starts, run `systemctl --user disable --now second-pass-v4.timer second-pass-v4.service`.

Keep the checkout's pinned files unchanged until the pass is done. A changed file makes `second_pass.py` refuse (exit 2), and the launcher then stops and says so.
