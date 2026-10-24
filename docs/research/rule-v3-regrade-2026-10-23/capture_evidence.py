#!/usr/bin/env python3
"""Select the stale-batch regrade archives and the blind-session files of the two ruling rounds, without changing their bytes.

    python3 -I docs/research/rule-v3-regrade-2026-10-23/capture_evidence.py --out NEW_DIR

Each archive keeps its repository path, the one its committed evidence.json pins, and must match that hash. Each
round's sessions are those of its usage.v1.json: the working directory, the files under assessors/logs/ named for the
session and the client home, under artifacts/research/<round>/ with the cache layout. A session's exec log must match
its saved hash and its native transcript must name its thread. Credential files are left out by name, and each client
home's model-catalog cache and installation ID by the owner's narrowing (evidence-authorization.v3.json); every
exclusion is listed with its reason."""
import argparse
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
TAG = "evidence-rule-v3-regrade-2026-10-23-rulings-v1"
QUEUES = ("rule-v3-regrade-2026-10-23-rebuilt-rulings", "rule-v3-regrade-2026-10-23-selected-rulings")
ROUNDS = {
    "regrade-v3-rulings-2026-10-23": Path("/home/jack/.cache/regrade-v3-rulings"),
    "regrade-v3b-rulings-2026-10-23": Path("/home/jack/.cache/regrade-v3b-rulings"),
}
LOG_SUFFIXES = (".jsonl", ".stderr", ".command.json")
CREDENTIALS = {"auth.json", ".credentials.json"}
NARROWED = {".codex/models_cache.json", ".codex/installation_id"}
NARROWING = "left out by the owner's narrowing, evidence-authorization.v3.json"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def regular(path):
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"missing or linked file: {path}")
    return path


def archives(files):
    subjects = []
    for queue in QUEUES:
        for record in sorted((ROOT / "bench/regrading" / queue).glob("*/*/attempt-*/evidence.json")):
            archive = json.loads(record.read_text(encoding="utf-8"))["archive"]
            source = regular(ROOT / archive["path"])
            if source.parent != record.parent or digest(source) != archive["sha256"]:
                raise ValueError(f"archive differs from its evidence.json: {record}")
            files.append({"source": str(source), "path": archive["path"], "kind": "evidence"})
            subjects.append(str(record.parent.relative_to(ROOT)))
    return subjects


def sessions(files, excluded):
    pins = {}
    for name, cache in ROUNDS.items():
        usage_path = ROOT / "docs/research" / name / "usage.v1.json"
        usage = json.loads(usage_path.read_text(encoding="utf-8"))["sessions"]
        expected = {row["session"] for row in usage}
        found = {path.name for path in (cache / "assessors").iterdir() if path.name != "logs"}
        logs = {path.name for path in (cache / "assessors/logs").iterdir()}
        if found != expected or {path.name for path in (cache / "homes").iterdir()} != expected:
            raise ValueError(f"{name}: session folders differ from usage.v1.json")
        if logs != {session + suffix for session in expected for suffix in LOG_SUFFIXES}:
            raise ValueError(f"{name}: assessors/logs/ differs from usage.v1.json")
        prefix = f"artifacts/research/{name}"

        def select(source):
            files.append({"source": str(regular(source)), "path": f"{prefix}/{source.relative_to(cache)}", "kind": "evidence"})

        for row in usage:
            session = row["session"]
            log = cache / "assessors/logs" / f"{session}.jsonl"
            if digest(log) != row["log_sha256"]:
                raise ValueError(f"{name}/{session}: exec log differs from usage.v1.json")
            rollouts = list((cache / "homes" / session / ".codex/sessions").rglob(f"rollout-*-{row['thread_id']}.jsonl"))
            meta = json.loads(rollouts[0].read_text(encoding="utf-8").splitlines()[0]) if len(rollouts) == 1 else {}
            if meta.get("type") != "session_meta" or meta["payload"]["id"] != row["thread_id"]:
                raise ValueError(f"{name}/{session}: native transcript does not name thread {row['thread_id']}")
            for suffix in LOG_SUFFIXES:
                select(cache / "assessors/logs" / f"{session}{suffix}")
            home = cache / "homes" / session
            for tree in (cache / "assessors" / session, home):
                for path in sorted(tree.rglob("*")):
                    if path.is_dir() and not path.is_symlink():
                        continue
                    if path.name in CREDENTIALS:
                        excluded.append({"path": str(path), "reason": "credential"})
                    elif tree == home and str(path.relative_to(home)) in NARROWED:
                        excluded.append({"path": str(path), "reason": NARROWING})
                    else:
                        select(path)
        pins[name] = {"path": str(usage_path.relative_to(ROOT)), "sha256": digest(usage_path), "sessions": len(usage)}
    return pins


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    files, excluded = [], []
    subjects = archives(files)
    pins = sessions(files, excluded)
    subjects += [f"artifacts/research/{name}" for name in ROUNDS]
    selection = {"repository": "kamui/code-review-bench", "tag": TAG, "subjects": subjects,
                 "files": sorted(files, key=lambda row: row["path"])}
    args.out.mkdir(parents=True, exist_ok=False)
    (args.out / "selection.json").write_text(json.dumps(selection, indent=2) + "\n", encoding="utf-8")
    (args.out / "capture.json").write_text(json.dumps({"usage": pins, "excluded": excluded}, indent=2) + "\n", encoding="utf-8")
    reasons = {}
    for row in excluded:
        reasons[row["reason"]] = reasons.get(row["reason"], 0) + 1
    print(f"Selected {len(files)} files for {len(subjects)} subjects; excluded {reasons}.")


if __name__ == "__main__":
    main()
