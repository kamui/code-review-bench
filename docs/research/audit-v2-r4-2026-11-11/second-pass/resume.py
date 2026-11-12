#!/usr/bin/env python3
"""Restart round 3's second pass after a sign-in stop, and never after any other stop.

    setsid nohup python3 -I docs/research/audit-v2-r4-2026-11-11/second-pass/resume.py &

The launcher stops launching when the stored Claude sign-in expires within half an hour, and only a person renews it
by using Claude Code on this machine. Every 5 minutes, while start.sh's launcher is not running, this reads the queue
state and the sign-in's expiry. It exits for good once the queue is complete, or when the queue stopped for a reason
other than the sign-in. Otherwise, when the sign-in expires more than 40 minutes from now, it runs start.sh again,
which resumes the pass. The launcher keeps its own $130 cap and 90% weekly stop."""
import json
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
LOCAL = ROOT / ".local/second-pass-r4"
LOG = LOCAL / "resume.log"
STATUS = LOCAL / "queue/status.json"
LAUNCHER = LOCAL / "launcher.log"


def log(text):
    with LOG.open("a") as out:
        out.write(f"{datetime.now(timezone.utc):%Y-%m-%dT%H:%M:%SZ} {text}\n")


def running():
    return subprocess.run(["pgrep", "-f", "[l]aunch.py run --authorization docs/research/audit-v2-r4"], capture_output=True).returncode == 0


def sign_in_minutes():
    oauth = json.loads((Path.home() / ".claude/.credentials.json").read_text())["claudeAiOauth"]
    return (oauth["expiresAt"] / 1000 - time.time()) / 60


def main():
    log("watching")
    while True:
        if not running():
            state = json.loads(STATUS.read_text())
            if state["saved"] >= state["planned"]:
                return log(f"queue complete: {state['saved']} of {state['planned']}; exiting")
            last = LAUNCHER.read_text().splitlines()[-3:] if LAUNCHER.exists() else []
            if state["state"] != "quota-stopped" or not any("sign-in" in line for line in last):
                return log(f"queue {state['state']}, not a sign-in stop; exiting for a person to look")
            minutes = sign_in_minutes()
            if minutes > 40:
                log(f"sign-in good for {minutes:.0f} min; restarting at {state['saved']} of {state['planned']}")
                subprocess.Popen(["setsid", "sh", str(HERE / "start.sh")], cwd=ROOT, stdin=subprocess.DEVNULL,
                                 stdout=open(LOCAL / "start.out", "a"), stderr=subprocess.STDOUT)
                time.sleep(120)
        time.sleep(300)


if __name__ == "__main__":
    main()
