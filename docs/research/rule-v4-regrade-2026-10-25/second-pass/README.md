# Second pass of evaluator audit round evaluator-audit-v2-2026-10-29

The owner chose Claude Opus 5.5 at high effort as the second grader of the 170 sampled batches of [`sample.json`](../../../../bench/grading/current/audit/evaluator-audit-v2-2026-10-29/sample.json) (691 units). The pass may spend at most $275 at list-price equivalent, reruns included, and $6 a session. Launches stop at 90% of the Claude week ([audit decision 10](../../../../bench/grading/rulings/cohort-rebuild-audit.v6.md), [`approval-second-pass.v1.json`](../approval-second-pass.v1.json)). [`authorization.second-pass.v1.json`](../authorization.second-pass.v1.json) pins these limits, the plan, the runtime and the files below. The no-charge preflight is [`preflight-second-pass.v1.json`](../preflight-second-pass.v1.json).

## Files

- [`second_pass.py`](second_pass.py) is the last round's [driver](../../rule-v3-regrade-2026-10-23/second-pass/README.md) with a Claude grader. Its `plan`, `preflight`, `run` and `status` commands, exit codes, budget rule and restart rules are the ones that README states, with these changes:
  - The grader is `claude`, `claude-opus-5-5`, `high`. The queue copies the installed `claude` into `client-bin/` and refuses a version other than `grader.cliVersion`.
  - `grade dispatch` gets `--max-budget-usd 6`. Each running session counts at that limit until its priced receipt settles it, so the cap check uses a true upper bound.
  - The preflight runs `grading probe-claude`, and the quota check reads the Claude meters.
  - `run` reads `grades.json` nowhere and leaves that check to `evaluator-audit claims` and `evaluator-audit second`. The bench-tools this authorization pins still refuses those steps once `grades.json` differs from the file the sample pins. So the pass runs in its own checkout, `/home/jack/.cache/regrade-v4`, whose `grades.json` stays the pinned file until the pass ends. New batches join `grades.json` in other checkouts. Only a later authorization that pins a bench-tools which reads the round's own pinned copy lets this checkout's `grades.json` change mid-pass. `test_second_pass.py`'s `runs_after_grades_change` checks only the driver, because its fake tool does not refuse.
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

The pass runs from a shell, never from an assistant session. [`start.sh`](start.sh) sets the environment and runs `launch.py run` under a lock, so only one launcher runs however it is started. Before 5:10 AM, or once the pass has finished or stopped, a start does nothing and exits 0. Two systemd user units in `~/.config/systemd/user` start it, in the checkout at `/home/jack/.cache/regrade-v4`:

- `second-pass-v4.timer` fires once, at Sat 2026-10-31 5:10 AM ET, with `Persistent=true`. If the host is off then, it fires at the next boot.
- `second-pass-v4.service` runs `start.sh`, and it is wanted by `default.target`, so a reboot during the pass resumes it. The host keeps the user manager running without a login (`Linger=yes`).

On 2026-10-30 the running user manager could not be reached: its private socket file was replaced at 4:27 PM ET, so `systemctl --user` fails with "Failed to connect to bus" and the manager cannot load new units until it restarts. Both units are enabled by their symlinks in `timers.target.wants/` and `default.target.wants/`, which the manager reads at the next boot. Until then, this cron line starts the pass, and `@reboot` covers a restart:

```cron
*/10 * * * * /home/jack/.cache/regrade-v4/docs/research/rule-v4-regrade-2026-10-25/second-pass/start.sh >> /home/jack/.cache/second-pass-v4-logs/cron.out 2>&1
@reboot sleep 120 && /home/jack/.cache/regrade-v4/docs/research/rule-v4-regrade-2026-10-25/second-pass/start.sh >> /home/jack/.cache/second-pass-v4-logs/cron.out 2>&1
```

Remove the cron lines once `launch.py status` says the pass finished or stopped.

Status, without a model call:

```sh
cd ~/.cache/regrade-v4
python3 -I docs/research/rule-v4-regrade-2026-10-25/second-pass/launch.py status
systemctl --user list-timers second-pass-v4.timer   # works once the user manager is reachable
systemctl --user status second-pass-v4.service
tail -f .local/second-pass-v4/launcher.log      # one plain line per decision, in ET
tail -f .local/second-pass-v4/driver.log        # second_pass.py output
```

To resume after a stop, read the reason, fix its cause, set `"phase"` in `.local/second-pass-v4/launcher.json` to `"full"`, and run `start.sh`. After a stop because the Claude week reset, also set `"baseline"` to `null`, so the launcher takes a new baseline. Without that it stops again, even though its stop message says a restart takes a new baseline. To cancel before it starts, remove the cron lines and the two symlinks.

Keep the checkout's pinned files unchanged until the pass is done. A changed file makes `second_pass.py` refuse (exit 2), and the launcher then stops and says so.
