#!/usr/bin/env python3
"""Select the original evidence of the round's Codex sessions without changing its bytes."""
import argparse
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
PREFIX = "artifacts/research/regrade-rulings-2026-10-13"
REVIEW_FILES = ("prompt.md", "run.jsonl", "run.err", "last-message.md")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def pin(name):
    return {"path": str((HERE / name).relative_to(HERE.parents[2])), "sha256": digest(HERE / name)}


def capture(assessors, native_root, filing_review=None):
    usage = json.loads((HERE / "usage.v1.json").read_text())
    review = json.loads((HERE / "filing/review.v1.json").read_text())["sessions"] if filing_review else []
    native = {}
    ids = {row["thread_id"] for row in usage["sessions"] + review}
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

    def record(session, log, log_path):
        name = session["session"]
        if digest(log) != session["log_sha256"]:
            raise ValueError(f"saved log hash differs for {name}")
        path = native[session["thread_id"]]
        metadata = json.loads(path.read_text().splitlines()[0])
        if metadata.get("type") != "session_meta" or metadata["payload"]["id"] != session["thread_id"]:
            raise ValueError(f"native transcript identity differs for {name}")
        sessions.append({"session": name, "thread_id": session["thread_id"], "log": select(log, log_path),
                         "native": select(path, f"native/{name}.jsonl")})

    for script in ("batch.sh", "one.sh", "run.sh", "setup.sh"):
        select(assessors / script, f"assessors/{script}")
    for session in usage["sessions"]:
        name = session["session"]
        record(session, assessors / f"logs/{name}.jsonl", f"assessors/logs/{name}.jsonl")
        for suffix in ("err", "last.md"):
            select(assessors / f"logs/{name}.{suffix}", f"assessors/logs/{name}.{suffix}")
        for path in sorted((assessors / name).rglob("*")):
            if path.is_file():
                select(path, f"assessors/{name}/{path.relative_to(assessors / name)}")
    for session in review:
        record(session, filing_review / "run.jsonl", "filing-review/run.jsonl")
        for name in REVIEW_FILES:
            select(filing_review / name, f"filing-review/{name}")
    pins = {"usage": pin("usage.v1.json")}
    if review:
        pins["review"] = pin("filing/review.v1.json")
    return ({"repository": "kamui/code-review-bench", "tag": "evidence-regrade-rulings-2026-10-13-v1",
             "subjects": ["regrade-rulings-2026-10-13"], "files": sorted(files.values(), key=lambda row: row["path"])},
            {"schema_version": 1, **pins, "sessions": sessions})


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assessors", type=Path, required=True)
    parser.add_argument("--native-root", type=Path, required=True)
    parser.add_argument("--filing-review", type=Path, help="working folder of the filing's review session")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    selection, index = capture(args.assessors, args.native_root, args.filing_review)
    args.out.mkdir(parents=True, exist_ok=False)
    for name, value in (("selection.json", selection), ("execution-index.json", index)):
        (args.out / name).write_text(json.dumps(value, indent=2) + "\n")
    print(f"Selected {len(selection['files'])} original files for {len(index['sessions'])} sessions.")
