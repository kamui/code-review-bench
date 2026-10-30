#!/usr/bin/env python3
"""Start the Claude second pass unattended: a 5-batch trial, a check of its measured Claude-week use, then the rest.

    launch.py run --authorization AUTHORIZATION [--not-before 2026-10-31T09:10:00Z]
    launch.py status

Run from the repository root with BENCH_RUNTIME_STORE set, as the systemd units in README.md do. It calls
second_pass.py `run` in chunks and decides between them:

- Trial: the first TRIAL_BATCHES batches. After it, and after each further CHECKPOINT_BATCHES batches until the
  question is settled, it projects the whole pass from what the batches cost against their Opus first grades. It
  stops when the projected cost is over the cap. The weekly meter shows whole points, so a reading of d points
  over the pass's spend s puts the rate between (d - 1) / s and (d + 1) / s. It goes on to the full pass when even
  the highest of those rates keeps the projected pass under WEEK_POINTS_LIMIT points, and stops when even the
  lowest puts it at or over. Between the two it measures CHECKPOINT_BATCHES more batches.
- Throughout, it stops when the pass has used WEEK_POINTS_LIMIT points of the week since it started, or the week
  reaches the authorization's quota stop. It waits for the five-hour window to reset instead of stopping, and
  waits while the stored Claude sign-in expires within 40 minutes, because only a person renews it.
- A failed batch is retried only when its failure is known to pass: a host restart that interrupted the session,
  an overloaded or failing provider, or the plan's usage limit (after the five-hour window resets). It writes the
  attempt's disposition.json with the evidence, at most MAX_RETRIES_PER_BATCH times a batch and MAX_RETRIES in
  all. Any other failure stops it.

A stop writes its reason to launcher.json and launcher.log and ends with exit 0, so systemd does not restart it.
Once stopped or finished, later starts do nothing until an operator removes `stopped` from launcher.json."""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from zoneinfo import ZoneInfo

ROOT = Path.cwd()
TOOL = ROOT / "bench/tools/bench-tools"
DRIVER = Path(os.environ.get("SECOND_PASS_DRIVER", Path(__file__).resolve().parent / "second_pass.py"))
STATE = ROOT / ".local/second-pass-v4"
EASTERN = ZoneInfo("America/New_York")
TRIAL_BATCHES, CHECKPOINT_BATCHES, CHUNK_BATCHES, WORKERS = 5, 5, 10, 3
WEEK_POINTS_LIMIT = 30
RESET_MARGIN_SECONDS = 120
SIGN_IN_MARGIN_SECONDS = 40 * 60
MAX_RETRIES_PER_BATCH, MAX_RETRIES = 2, 8
METER_FAILURES_BEFORE_STOP = 6
SLEEP_SCALE = float(os.environ.get("SECOND_PASS_LAUNCH_SLEEP_SCALE", "1"))
TRANSIENT = {
    "overloaded": re.compile(r"overloaded|API Error: 529", re.I),
    "server-error": re.compile(r"API Error: 5\d\d|Internal server error|Connection error|ECONNRESET", re.I),
    "usage-limit": re.compile(r"usage limit|rate_limit_error|limit reached", re.I),
}


def now():
    return time.time()


def utc(moment):
    return datetime.fromtimestamp(moment, timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def eastern(moment):
    return datetime.fromtimestamp(moment, EASTERN).strftime("%a %b %-d, %-I:%M %p ET")


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def log(message):
    line = f"{eastern(now())}  {message}"
    with (STATE / "launcher.log").open("a", encoding="utf-8") as handle:
        handle.write(line + "\n")
    print(line, flush=True)


def sleep_until(moment, why):
    seconds = max(0.0, moment - now())
    log(f"Waiting until {eastern(moment)} ({seconds / 60:.0f} min): {why}")
    time.sleep(seconds * SLEEP_SCALE)


class Launcher:
    def __init__(self, authorization):
        self.authorization_path = authorization
        self.authorization = read(ROOT / authorization)
        self.plan = read(ROOT / self.authorization["plan"]["path"])
        self.cap = float(self.authorization["budgetCapUsd"])
        self.quota = float(self.authorization["quotaStopPercent"])
        self.queue = ROOT / self.plan["queueDirectory"]
        self.state_path = STATE / "launcher.json"
        self.state = read(self.state_path) if self.state_path.exists() else {
            "phase": "trial", "baseline": None, "retries": {}, "decisions": []}

    def save(self):
        staged = self.state_path.with_suffix(".tmp")
        staged.write_text(json.dumps(self.state, indent=2) + "\n", encoding="utf-8")
        staged.replace(self.state_path)

    def stop(self, reason):
        self.state.update({"phase": "stopped", "stopped": {"at": utc(now()), "reason": reason}})
        self.save()
        log(f"STOPPED. {reason}")
        return 0

    # what the queue holds

    def saved(self, batch):
        folder = ROOT / self.plan["recordRoot"] / "second" / Path(batch["run"]).name / batch["target"]
        return any(folder.glob("assessment-*/receipt.json"))

    def saved_batches(self):
        return [batch for batch in self.plan["order"] if self.saved(batch)]

    def spent(self):
        path = self.queue / "status.json"
        return float(read(path).get("spentUpperUsd") or 0.0) if path.exists() else 0.0

    # meters and sign-in

    def client(self):
        pinned = self.queue / "client-bin/claude"
        path = pinned if pinned.exists() else Path(shutil.which("claude")).resolve()
        return {"claude": {"path": str(path), "version": self.authorization["grader"]["cliVersion"]}}

    def meters(self):
        """The five-hour and weekly windows, as dicts with utilization and resets_at, or None. Calls no model."""
        done = subprocess.run([str(TOOL), "subscription-meters", "read", "--clients", json.dumps(self.client())],
                              cwd=ROOT, capture_output=True, text=True)
        if done.returncode:
            log(f"Could not read the Claude meters: {(done.stderr or done.stdout).strip()[:300]}")
            return None
        reading = json.loads(done.stdout)["claude"]
        windows = []
        for name in ("five_hour", "seven_day"):
            window = reading[name]
            reset = window.get("resets_at")
            windows.append({"utilization": float(window["utilization"]),
                            "resets_at": datetime.fromisoformat(reset).timestamp() if reset else None})
        self.state["lastReading"] = {"at": utc(now()), "fiveHour": windows[0]["utilization"],
                                     "week": windows[1]["utilization"]}
        return windows

    @staticmethod
    def sign_in_expires():
        credentials = Path(os.environ["HOME"]) / ".claude/.credentials.json"
        try:
            return read(credentials)["claudeAiOauth"]["expiresAt"] / 1000
        except (OSError, KeyError, ValueError, TypeError):
            return None

    def wait_for_sign_in(self):
        expires = self.sign_in_expires()
        if expires is not None and expires - now() > SIGN_IN_MARGIN_SECONDS:
            return
        log("The stored Claude sign-in expires "
            + (f"at {eastern(expires)}" if expires else "at an unknown time")
            + ". Only a person renews it: use Claude Code on this machine once. Checking every 5 minutes.")
        waited = 0
        while True:
            time.sleep(300 * SLEEP_SCALE)
            waited += 300
            expires = self.sign_in_expires()
            if expires is not None and expires - now() > SIGN_IN_MARGIN_SECONDS:
                log(f"The sign-in was renewed; it now expires at {eastern(expires)}.")
                return
            if waited % 3600 == 0:
                log(f"Still waiting for a renewed Claude sign-in ({waited // 3600} h so far).")

    # the trial's measurement

    def projection(self):
        """The projected cost of the pass and the range of week points it would use, from what it has used so far."""
        baseline = self.state["baseline"]
        spent = self.spent() - baseline["spentUsd"]
        saved = self.saved_batches()
        first_saved = sum(batch["firstGradeUsd"] for batch in saved)
        first_all = sum(batch["firstGradeUsd"] for batch in self.plan["order"])
        if spent <= 0 or first_saved <= 0:
            return None
        cost = spent / first_saved * first_all
        points = self.state["lastReading"]["week"] - baseline["week"]
        return {"batches": len(saved), "spentUsd": round(spent, 2), "weekPoints": points,
                "projectedCostUsd": round(cost, 2),
                "projectedPointsLow": round(max(0.0, points - 1) / spent * cost, 1),
                "projectedPointsHigh": round((points + 1) / spent * cost, 1)}

    def decide(self):
        found = self.projection()
        if found is None:
            log("Nothing has been spent yet to measure; measuring more batches.")
            self.state["phase"] = "checkpoint"
            return None
        self.state["decisions"].append({"at": utc(now()), **found})
        summary = (f"{found['batches']} batches cost ${found['spentUsd']:.2f} and moved the week "
                   f"{found['weekPoints']:g} points. Projected pass: ${found['projectedCostUsd']:.2f}, "
                   f"{found['projectedPointsLow']:g} to {found['projectedPointsHigh']:g} points of the week.")
        if found["projectedCostUsd"] > self.cap:
            return self.stop(f"{summary} That is over the ${self.cap:.0f} cap.")
        if found["projectedPointsLow"] >= WEEK_POINTS_LIMIT:
            return self.stop(f"{summary} Even the lowest rate the meter allows reaches {WEEK_POINTS_LIMIT} points.")
        if found["projectedPointsHigh"] < WEEK_POINTS_LIMIT:
            log(f"{summary} Under {WEEK_POINTS_LIMIT} points and the cap: running the full pass.")
            self.state["phase"] = "full"
        else:
            log(f"{summary} The meter's whole points cannot settle it yet; measuring {CHECKPOINT_BATCHES} more batches.")
            self.state["phase"] = "checkpoint"
        self.state["checkpointFrom"] = found["batches"]
        self.save()
        return None

    # failures

    def failed_attempts(self):
        found = []
        for batch in self.plan["order"]:
            if self.saved(batch):
                continue
            folder = self.queue / "attempts" / Path(batch["run"]).name / batch["target"]
            attempts = sorted(folder.glob("attempt-*"), key=lambda p: int(p.name.split("-")[1])) if folder.is_dir() else []
            if attempts and not (attempts[-1] / "disposition.json").exists():
                found.append((batch, attempts[-1]))
        return found

    @staticmethod
    def booted_at():
        for line in Path("/proc/stat").read_text().splitlines():
            if line.startswith("btime "):
                return float(line.split()[1])
        return 0.0

    def classify(self, attempt):
        """The kind of a failure known to pass and its evidence, or (None, evidence)."""
        work = attempt / "work"
        reserved = read(attempt / "reservation.json")["at"] if (attempt / "reservation.json").exists() else None
        started = datetime.fromisoformat(reserved.replace("Z", "+00:00")).timestamp() if reserved else now()
        if not (work / "dispatch.json").exists() and started < self.booted_at():
            return "host-restart", f"started {reserved}, before this boot at {utc(self.booted_at())}"
        text = ""
        for name in ("stdout.txt", "stderr.txt", "dispatch.json"):
            if (work / name).is_file():
                text += (work / name).read_text(encoding="utf-8", errors="replace")[-20000:]
        for kind, pattern in TRANSIENT.items():
            match = pattern.search(text)
            if match:
                start = max(0, match.start() - 120)
                return kind, text[start:match.end() + 120].replace("\n", " ")
        tail = (attempt / "audit.log").read_text(errors="replace")[-400:] if (attempt / "audit.log").exists() else ""
        return None, tail.replace("\n", " ") or "no error text found"

    def handle_failure(self):
        failed = self.failed_attempts()
        if not failed:
            return self.stop("second_pass.py exited 1 and no failed attempt was found; read driver.log.")
        wait_for_window = False
        for batch, attempt in failed:
            name = f"{batch['run']}/{batch['target']}"
            kind, evidence = self.classify(attempt)
            if kind is None:
                return self.stop(f"{name} {attempt.name} failed in a way not known to pass: {evidence[:400]}")
            retries = self.state["retries"]
            if retries.get(name, 0) >= MAX_RETRIES_PER_BATCH or sum(retries.values()) >= MAX_RETRIES:
                return self.stop(f"{name} {attempt.name} failed ({kind}) and the retries are spent.")
            (attempt / "disposition.json").write_text(json.dumps({
                "decision": "retry", "kind": kind, "evidence": evidence[:600], "at": utc(now()), "by": "launch.py",
                "note": "The failure is one known to pass; the batch gets its next attempt and this attempt's charge "
                        "or reservation stays in the total."}, indent=2) + "\n")
            retries[name] = retries.get(name, 0) + 1
            log(f"Retrying {name} after {attempt.name} failed ({kind}): {evidence[:200]}")
            wait_for_window = wait_for_window or kind == "usage-limit"
        self.save()
        if wait_for_window:
            windows = self.meters()
            if windows and windows[0]["resets_at"]:
                sleep_until(windows[0]["resets_at"] + RESET_MARGIN_SECONDS, "a session hit the plan's usage limit")
        return None

    # the loop

    def driver(self, limit):
        command = [sys.executable, "-I", str(DRIVER), "run", "--authorization", self.authorization_path,
                   "--workers", str(WORKERS), "--limit", str(limit)]
        with (STATE / "driver.log").open("a", encoding="utf-8") as handle:
            handle.write(f"\n== {utc(now())} {' '.join(command[2:])}\n")
            handle.flush()
            return subprocess.call(command, cwd=ROOT, stdout=handle, stderr=subprocess.STDOUT)

    def run(self):
        if self.state["phase"] in ("stopped", "finished"):
            print(f"Nothing to do: the pass is {self.state['phase']}"
                  + (f" ({self.state['stopped']['reason']})" if self.state["phase"] == "stopped" else "") + ".")
            return 0
        log(f"Starting in phase '{self.state['phase']}': {len(self.saved_batches())} of "
            f"{len(self.plan['order'])} batches saved, ${self.spent():.2f} of ${self.cap:.0f} spent.")
        meter_failures = quota_exits = 0
        while True:
            self.wait_for_sign_in()
            windows = self.meters()
            if windows is None:
                meter_failures += 1
                if meter_failures >= METER_FAILURES_BEFORE_STOP:
                    return self.stop("The Claude meters could not be read for an hour.")
                time.sleep(600 * SLEEP_SCALE)
                continue
            meter_failures = 0
            five_hour, week = windows
            if self.state["baseline"] is None:
                self.state["baseline"] = {"at": utc(now()), "week": week["utilization"], "weekResetsAt": week["resets_at"],
                                          "spentUsd": self.spent()}
                log(f"Baseline: the week is at {week['utilization']:g}%, the five-hour window at "
                    f"{five_hour['utilization']:g}%.")
            self.save()
            baseline = self.state["baseline"]
            if week["resets_at"] and baseline["weekResetsAt"] and abs(week["resets_at"] - baseline["weekResetsAt"]) > 3600:
                return self.stop("The Claude week reset during the pass, so its use can no longer be measured from the "
                                 "baseline. Start it again to take a new baseline.")
            if week["utilization"] >= self.quota:
                return self.stop(f"The week is at {week['utilization']:g}%, at or over the {self.quota:g}% stop.")
            if week["utilization"] - baseline["week"] >= WEEK_POINTS_LIMIT:
                return self.stop(f"The week moved {week['utilization'] - baseline['week']:g} points since the pass "
                                 f"started, the {WEEK_POINTS_LIMIT}-point limit.")
            if five_hour["utilization"] >= self.quota:
                sleep_until((five_hour["resets_at"] or now() + 3600) + RESET_MARGIN_SECONDS,
                            f"the five-hour window is at {five_hour['utilization']:g}%")
                continue
            saved = len(self.saved_batches())
            if self.state["phase"] == "trial":
                limit = TRIAL_BATCHES - saved
            elif self.state["phase"] == "checkpoint":
                limit = self.state.get("checkpointFrom", saved) + CHECKPOINT_BATCHES - saved
            else:
                limit = CHUNK_BATCHES
            if limit <= 0:
                if self.decide() is not None:
                    return 0
                continue
            log(f"Running up to {limit} batches ({self.state['phase']}); week {week['utilization']:g}%, five-hour "
                f"{five_hour['utilization']:g}%, ${self.spent():.2f} spent.")
            code = self.driver(limit)
            status = read(self.queue / "status.json") if (self.queue / "status.json").exists() else {}
            log(f"second_pass.py exited {code} ({status.get('state')}): {status.get('saved')} of "
                f"{status.get('planned')} saved, ${status.get('spentUpperUsd', 0):.2f} spent"
                + (f"; {status['reason']}" if status.get("reason") else "") + ".")
            if code == 0 and status.get("state") == "complete":
                self.state["phase"] = "finished"
                self.save()
                log("FINISHED: every sampled batch has a second assessment.")
                return 0
            quota_exits = quota_exits + 1 if code == 4 else 0
            if quota_exits >= 2:
                sleep_until(now() + 600, "second_pass.py stopped at the quota twice; reading the meters again")
            if code in (0, 4):
                continue
            if code == 1:
                if self.handle_failure() is not None:
                    return 0
                continue
            if code == 3:
                return self.stop(f"The ${self.cap:.0f} cap cannot fund another batch.")
            return self.stop(f"second_pass.py exited {code}; read driver.log.")


def status():
    state = read(STATE / "launcher.json") if (STATE / "launcher.json").exists() else {}
    queue = read(STATE / "queue/status.json") if (STATE / "queue/status.json").exists() else {}
    print(f"Phase: {state.get('phase', 'not started')}"
          + (f": {state['stopped']['reason']}" if state.get("phase") == "stopped" else ""))
    print(f"Batches saved: {queue.get('saved', 0)} of {queue.get('planned', 170)}; driver state: {queue.get('state')}")
    print(f"Spent: ${queue.get('spentUpperUsd', 0):.2f} settled, ${queue.get('reservedUsd', 0):.2f} reserved, "
          f"cap ${queue.get('budgetCapUsd', 275):.0f}")
    if state.get("baseline"):
        last = state.get("lastReading", {})
        print(f"Week: {state['baseline']['week']:g}% at the start, {last.get('week', '?')}% at {last.get('at', '?')}; "
              f"five-hour window {last.get('fiveHour', '?')}%")
    for decision in state.get("decisions", []):
        print(f"Measured at {decision['at']}: {decision['batches']} batches, ${decision['spentUsd']:.2f}, "
              f"{decision['weekPoints']:g} points; projected ${decision['projectedCostUsd']:.2f}, "
              f"{decision['projectedPointsLow']:g} to {decision['projectedPointsHigh']:g} points")
    print(f"Free disk: {shutil.disk_usage(ROOT).free / 2**30:.0f} GiB")
    if (STATE / "launcher.log").exists():
        print("\nLast log lines:")
        print("".join((STATE / "launcher.log").read_text().splitlines(keepends=True)[-12:]), end="")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    run_parser = commands.add_parser("run")
    run_parser.add_argument("--authorization", required=True)
    run_parser.add_argument("--not-before", help="an ISO time; an earlier start does nothing")
    commands.add_parser("status")
    args = parser.parse_args()
    if not (ROOT / "bench/grading/current/audits.json").is_file():
        print("run from the repository root", file=sys.stderr)
        return 2
    STATE.mkdir(parents=True, exist_ok=True)
    if args.command == "status":
        return status()
    if not os.environ.get("BENCH_RUNTIME_STORE"):
        print("set BENCH_RUNTIME_STORE to the runtime store the authorization pins", file=sys.stderr)
        return 2
    not_before = datetime.fromisoformat(args.not_before.replace("Z", "+00:00")).timestamp() if args.not_before else None
    if not_before and now() < not_before:
        print(f"Started before {eastern(not_before)}; the pass starts then.")
        return 0
    return Launcher(args.authorization).run()


if __name__ == "__main__":
    sys.exit(main())
