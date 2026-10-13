#!/usr/bin/env python3
"""Select the original evidence of the round's Codex sessions without changing its bytes.

    python3 docs/research/older-problem-activation-2026-10-13/capture_evidence.py \
        --assessors ~/.t3/review-work/older-activation/assessors --native-root ~/.codex/sessions --out NEW-DIRECTORY

It writes `selection.json`, the input of `evidence_store.py pack`, and `sessions.json`, an index that pins each
session's exec log and native transcript. It selects only the sessions in usage.v1.json, checks each saved log hash
and each native thread identity, and refuses an output directory that exists. Nothing is published by it."""
import argparse
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
PREFIX = "artifacts/research/older-problem-activation-2026-10-13"
SCRIPTS = ("setup.sh", "run.sh", "setup-reconciliation.sh", "run-reconciliation.sh", "setup-more.sh", "run-more.sh", "setup-labels.sh", "setup-labels-2.sh",
           "probe/probe.py", "probe/probe-result.json")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def capture(assessors, native_root):
    usage = json.loads((HERE / "usage.v1.json").read_text(encoding="utf-8"))
    ids, native = {row["thread_id"] for row in usage["sessions"]}, {}
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

    for script in SCRIPTS:
        select(assessors / script, f"assessors/{script}")
    for session in usage["sessions"]:
        name = session["session"]
        log = assessors / f"logs/{name}.jsonl"
        if digest(log) != session["log_sha256"]:
            raise ValueError(f"saved log hash differs for {name}")
        transcript = native[session["thread_id"]]
        metadata = json.loads(transcript.read_text(encoding="utf-8").splitlines()[0])
        if metadata.get("type") != "session_meta" or metadata["payload"]["id"] != session["thread_id"]:
            raise ValueError(f"native transcript identity differs for {name}")
        sessions.append({"session": name, "thread_id": session["thread_id"], "log": select(log, f"assessors/logs/{name}.jsonl"),
                         "native": select(transcript, f"native/{name}.jsonl")})
        for suffix in ("err", "last.md"):
            select(assessors / f"logs/{name}.{suffix}", f"assessors/logs/{name}.{suffix}")
        for path in sorted((assessors / name).rglob("*")):
            if path.is_file():
                select(path, f"assessors/{name}/{path.relative_to(assessors / name)}")
    return ({"repository": "kamui/code-review-bench", "tag": "evidence-older-problem-activation-2026-10-13-v1",
             "subjects": ["older-problem-activation-2026-10-13"], "files": sorted(files.values(), key=lambda row: row["path"])},
            {"schema_version": 1, "usage": {"path": str((HERE / "usage.v1.json").relative_to(HERE.parents[2])), "sha256": digest(HERE / "usage.v1.json")},
             "sessions": sessions})


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assessors", type=Path, required=True)
    parser.add_argument("--native-root", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    selection, index = capture(args.assessors.expanduser(), args.native_root.expanduser())
    args.out.mkdir(parents=True)
    (args.out / "selection.json").write_text(json.dumps(selection, indent=1) + "\n", encoding="utf-8")
    (args.out / "sessions.json").write_text(json.dumps(index, indent=1) + "\n", encoding="utf-8")
    print(len(selection["files"]), "files,", len(index["sessions"]), "sessions")
