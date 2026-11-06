#!/usr/bin/env python3
"""Restart the rule-version-6 regrade after a sign-in stop, and never after the quota stop.

    setsid nohup python3 -I docs/research/rule-v6-regrade-2026-11-06/resume.py &

A queue stops launching (exit 4) when the stored Claude sign-in expires within half an hour or a usage window reaches
90%. Only a person renews the sign-in, by using Claude Code on this machine. Every 5 minutes, while start.sh's driver
is not running, this reads the meters (no model call) and the sign-in's expiry. It exits for good once the weekly
window reads 89% or more, or once the driver log says all queues finished, or after a driver exit other than 4; the
owner approved the run up to 90% weekly use only ("Start now and go until you hit 90% weekly usage on Claude.").
Otherwise, when the sign-in expires more than 40 minutes from now, it runs start.sh again, which resumes every queue."""
import json
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
LOG = ROOT / ".local/regrade-v6/resume.log"
DRIVER = ROOT / ".local/regrade-v6/logs/driver.log"
CLIENT = {"claude": {"path": "/home/t3/.cache/code-review-bench/claude-2.1.317/claude", "version": "2.1.317"}}


def log(text):
    with LOG.open("a") as out:
        out.write(f"{datetime.now(timezone.utc):%Y-%m-%dT%H:%M:%SZ} {text}\n")


def driver_running():
    return subprocess.run(["pgrep", "-f", "[r]egrade_queues.sh run .local/regrade-v6"], capture_output=True).returncode == 0


def weekly():
    done = subprocess.run([str(ROOT / "bench/tools/bench-tools"), "subscription-meters", "read", "--clients", json.dumps(CLIENT)],
                          cwd=ROOT, capture_output=True, text=True)
    if done.returncode:
        return None
    return float((json.loads(done.stdout)["claude"]["seven_day"] or {"utilization": 0.0})["utilization"])


def sign_in_minutes():
    oauth = json.loads((Path.home() / ".claude/.credentials.json").read_text())["claudeAiOauth"]
    return (oauth["expiresAt"] / 1000 - time.time()) / 60


def main():
    log("watching")
    while True:
        if not driver_running():
            lines = DRIVER.read_text().splitlines() if DRIVER.exists() else []
            ends = [line for line in lines if " end " in line]
            if any("all queues finished" in line for line in lines):
                return log("all queues finished; exiting")
            if ends and not ends[-1].endswith("exit=4"):
                return log(f"driver stopped: {ends[-1]}; exiting for a person to look")
            week = weekly()
            if week is None:
                log("could not read the meters")
            elif week >= 89:
                return log(f"weekly use {week:.0f}%; the approved run is over; exiting")
            elif sign_in_minutes() > 40:
                log(f"weekly use {week:.0f}%, sign-in good for {sign_in_minutes():.0f} min; restarting")
                subprocess.Popen(["setsid", "sh", str(HERE / "start.sh")], cwd=ROOT, stdin=subprocess.DEVNULL,
                                 stdout=open(Path.home() / ".cache/regrade-v6.out", "a"), stderr=subprocess.STDOUT)
                time.sleep(60)
        time.sleep(300)


if __name__ == "__main__":
    main()
