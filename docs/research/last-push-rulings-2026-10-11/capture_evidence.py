#!/usr/bin/env python3
"""Select original intake and assessor evidence without changing its bytes."""
import argparse
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
PREFIX = "artifacts/research/last-push-rulings-2026-10-11"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def capture(intake, assessors, native_root):
    usage = json.loads((HERE / "usage.v1.json").read_text())
    native = {}
    ids = {row["thread_id"] for row in usage["sessions"]}
    for path in native_root.rglob("*.jsonl"):
        for thread in ids:
            if path.name.endswith(f"{thread}.jsonl"):
                if thread in native:
                    raise ValueError(f"multiple native transcripts for {thread}")
                native[thread] = path
    files, sessions = {}, []

    def select(source, relative):
        path = f"{PREFIX}/{relative}"
        if source.is_symlink() or not source.is_file():
            raise ValueError(f"missing or linked original evidence: {source}")
        files[path] = {"source": str(source.resolve()), "path": path, "kind": "evidence"}
        return {"path": path, "sha256": digest(source)}

    for path in sorted(intake.rglob("*")):
        if path.is_file():
            select(path, f"intake/{path.relative_to(intake)}")
    select(assessors / "run.sh", "assessors/run.sh")
    for session in usage["sessions"]:
        name = session["session"]
        matching = name in ("r-base-ui-5460", "s-seaweedfs-10735")
        log = intake / f"logs/{name}.run.jsonl" if matching else assessors / f"logs/{name}.jsonl"
        if digest(log) != session["log_sha256"]:
            raise ValueError(f"usage log hash differs for {name}")
        log_pin = select(log, f"intake/logs/{log.name}" if matching else f"assessors/logs/{log.name}")
        if not matching:
            for suffix in ("err", "last.md"):
                path = assessors / f"logs/{name}.{suffix}"
                select(path, f"assessors/logs/{path.name}")
            for path in sorted((assessors / name).rglob("*")):
                if path.is_file():
                    select(path, f"assessors/{name}/{path.relative_to(assessors / name)}")
        path = native[session["thread_id"]]
        metadata = json.loads(path.read_text().splitlines()[0])
        if metadata.get("type") != "session_meta" or metadata["payload"]["id"] != session["thread_id"]:
            raise ValueError(f"native transcript identity differs for {name}")
        sessions.append({"session": name, "thread_id": session["thread_id"], "log": log_pin,
                         "native": select(path, f"native/{name}.jsonl")})
    return ({"repository": "kamui/code-review-bench", "tag": "evidence-last-push-rulings-2026-10-11-v1",
             "subjects": ["last-push-rulings-2026-10-11"], "files": sorted(files.values(), key=lambda row: row["path"])},
            {"schema_version": 1, "usage": {"path": str(HERE.relative_to(HERE.parents[2]) / "usage.v1.json"),
                                            "sha256": digest(HERE / "usage.v1.json")}, "sessions": sessions})


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--intake", type=Path, required=True)
    parser.add_argument("--assessors", type=Path, required=True)
    parser.add_argument("--native-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    selection, index = capture(args.intake, args.assessors, args.native_root)
    args.out.mkdir(parents=True, exist_ok=False)
    for name, value in (("selection.json", selection), ("execution-index.json", index)):
        (args.out / name).write_text(json.dumps(value, indent=2) + "\n")
    print(f"Selected {len(selection['files'])} original files for {len(index['sessions'])} sessions.")
