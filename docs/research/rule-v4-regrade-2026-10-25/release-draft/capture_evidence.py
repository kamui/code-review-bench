#!/usr/bin/env python3
"""Select the rule-version-4 regrade's grading archives without changing their bytes.

    python3 -I docs/research/rule-v4-regrade-2026-10-25/release-draft/capture_evidence.py --out NEW_DIR

Selects every bench/regrading/rule-v4-regrade-2026-10-25*/<run>/<target>/attempt-<N>/evidence.json whose archive no
manifest in bench/evidence/manifests/ lists yet, failed attempts included. Each archive keeps its repository path, the
one its committed evidence.json pins, and must match that hash. Writes NEW_DIR/selection.json for
`bench-tools evidence pack`; uploads nothing."""
import argparse
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
TAG = "evidence-rule-v4-regrade-2026-10-25-v1"
QUEUE_GLOB = "rule-v4-regrade-2026-10-25*"


def published_paths():
    paths = set()
    for manifest in sorted((ROOT / "bench/evidence/manifests").glob("*.json")):
        for package in json.loads(manifest.read_text(encoding="utf-8")).get("packages", []):
            paths.update(member["path"] for member in package["members"])
    return paths


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    published = published_paths()
    files, subjects, queues = [], [], {}
    for record in sorted((ROOT / "bench/regrading").glob(f"{QUEUE_GLOB}/*/*/attempt-*/evidence.json")):
        archive = json.loads(record.read_text(encoding="utf-8"))["archive"]
        if archive["path"] in published:
            continue
        source = ROOT / archive["path"]
        if source.is_symlink() or not source.is_file():
            raise ValueError(f"missing or linked archive: {source}")
        if source.parent != record.parent or hashlib.sha256(source.read_bytes()).hexdigest() != archive["sha256"]:
            raise ValueError(f"archive differs from its evidence.json: {record}")
        files.append({"source": str(source), "path": archive["path"], "kind": "evidence"})
        subjects.append(str(record.parent.relative_to(ROOT)))
        queue = record.relative_to(ROOT).parts[2]
        queues[queue] = queues.get(queue, 0) + 1
    selection = {"repository": "kamui/code-review-bench", "tag": TAG, "subjects": subjects,
                 "files": sorted(files, key=lambda row: row["path"])}
    args.out.mkdir(parents=True, exist_ok=False)
    (args.out / "selection.json").write_text(json.dumps(selection, indent=2) + "\n", encoding="utf-8")
    print(f"Selected {len(files)} archives for {len(subjects)} subjects: {dict(sorted(queues.items()))}.")


if __name__ == "__main__":
    main()
