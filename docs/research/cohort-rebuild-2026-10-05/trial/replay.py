#!/usr/bin/env python3
"""Replay a trial round's saved first-grader verdicts through preparation, validation and mapping.

    python3 docs/research/cohort-rebuild-2026-10-05/trial/replay.py [--round retest-p20]

No model is called and the live records are not touched. The current records are copied to
`.local/switch-replay/current` with no saved grade and a validation policy that pins the next rubric, its rules
and instructions with verdict contract v2. Each batch of `batches.json` is prepared again, which gives it new blind
tokens; the saved verdicts are rewritten to those tokens, checked by `grade.py validate` and, when they pass, saved
by `grade.py map` under a manual assessor record. The trial's graders were not shown the user's rulings on single
comments, so a batch whose saved verdicts differ from one is reported with the validator's lines and not mapped.
The scratch records are checked at the end. Run from the repository root."""
import argparse
import json
import shutil
import subprocess
import sys
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "bench/tools"))
import current_grading  # noqa: E402

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import run as trial  # noqa: E402

WORK = ROOT / ".local/switch-replay"
CURRENT = ".local/switch-replay/current"
NEXT = {"rubric": "bench/rubric/scoring.next.md", "grader": "bench/rubric/grader.next.md", "rules": "bench/rubric/rules.next.md"}


def tool(*args):
    return subprocess.run([sys.executable, str(ROOT / "bench/tools" / args[0]), *map(str, args[1:])], cwd=ROOT,
                          capture_output=True, text=True)


def scratch_records():
    if WORK.exists():
        subprocess.run(["chmod", "-R", "u+w", str(WORK)], check=False)
        shutil.rmtree(WORK)
    (WORK / "current").mkdir(parents=True)
    live = ROOT / current_grading.CURRENT
    for path in live.glob("*.json"):
        shutil.copyfile(path, WORK / "current" / path.name)
    (WORK / "current/grades.json").write_text(json.dumps({"schema_version": 1, "batches": []}, indent=2) + "\n")
    policy = {**json.loads((live / "validation-policy.json").read_text()), "verdicts": "current-verdicts/v2",
              **{field: current_grading.pin_file(ROOT / path, ROOT) for field, path in NEXT.items()}}
    (WORK / "current/validation-policy.json").write_text(json.dumps(policy, indent=2) + "\n")


def retokened(verdicts, tokens, key):
    """The saved verdicts under the blind tokens of the new preparation, with the second fact under its current name."""
    for review in verdicts["reviews"].values():
        for item in review["items"].values():
            for entry in (e for claim in item.get("claims", []) for e in claim["known_problems"]):
                # Verdicts saved before the second fact was renamed call it says_why.
                if "says_why" in entry:
                    entry["identifies_cause"] = entry.pop("says_why")
    new = {review["attempt_id"]: review["token"] for review in key["reviews"]}
    renamed = {old: new[attempt] for old, attempt in tokens.items()}
    return {"reviews": {renamed[token]: review for token, review in verdicts["reviews"].items()},
            "new_candidates": [{**candidate, "items": [{**item, "review": renamed[item["review"]]} for item in candidate["items"]]}
                               for candidate in verdicts["new_candidates"]],
            "link_disputes": [{**dispute, "review": renamed[dispute["review"]]} for dispute in verdicts["link_disputes"]]}


def replay(batch, round_name, assessor):
    saved = HERE / round_name / Path(batch["run"]).name / batch["target"] / "first"
    work, key = WORK / "workspaces" / uuid.uuid4().hex, WORK / "keys" / f"{uuid.uuid4().hex}.json"
    prepare = ["grade.py", "prepare", "--current", CURRENT, "--run", batch["run"], "--target", batch["target"],
               "--work", work, "--key", key]
    if trial.manifest(batch["target"]):
        prepare += ["--cache-replacements", trial.manifest(batch["target"])]
    done = tool(*prepare)
    if done.returncode:
        return "prepare failed", (done.stdout + done.stderr).strip().splitlines()
    verdicts = retokened(json.loads((saved / "verdicts.json").read_text()), json.loads((saved / "tokens.json").read_text()),
                         json.loads(key.read_text()))
    (work / "verdicts.json").write_text(json.dumps(verdicts, indent=2) + "\n")
    done = tool("grade.py", "validate", "--work", work)
    if done.returncode:
        for name in ("clone", "clone-cache"):
            subprocess.run(["chmod", "-R", "u+w", str(work / name)], check=False)
            shutil.rmtree(work / name, ignore_errors=True)
        return "differs from a ruling or the contract", done.stdout.strip().splitlines()
    done = tool("grade.py", "map", "--current", CURRENT, "--work", work, "--key", key, "--assessor", assessor)
    return ("mapped", done.stdout.strip().splitlines()[-1:]) if done.returncode == 0 else (
        "map failed", (done.stdout + done.stderr).strip().splitlines())


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--round", default="retest-p20", help="the directory of saved results to replay")
    args = parser.parse_args()
    scratch_records()
    assessor = WORK / "assessor.json"
    assessor.write_text(json.dumps({"assessor": f"replay of the trial round {args.round}",
                                    "method": "Saved first-grader verdicts under new blind tokens; no model call",
                                    "completed_at": "2026-10-07T00:00:00Z"}) + "\n")
    states = []
    for batch in json.loads((HERE / "batches.json").read_text())["batches"]:
        state, lines = replay(batch, args.round, assessor)
        states.append(state)
        print(f"{batch['run']}/{batch['target']}: {state}", flush=True)
        for line in lines:
            print(f"    {line}", flush=True)
    check = tool("current_grading.py", "check", "--current", CURRENT)
    print(f"{states.count('mapped')} of {len(states)} batches mapped; scratch records: {(check.stdout + check.stderr).strip()}")
    return 1 if check.returncode or {"prepare failed", "map failed"} & set(states) else 0


if __name__ == "__main__":
    sys.exit(main())
