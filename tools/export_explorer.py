"""Export validated per-review facts for the explorer and scorecard from current evidence.

Python loads the current facts; `bench-tools export-explorer` restores missing evidence, writes, checks and
publishes the export. This front goes once #156 ports `load_current`.
"""

import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "bench" / "tools"))
import current_grading
import rust_runtime


class Failed(Exception):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code


def tool(*arguments):
    try:
        executable = str(rust_runtime.resolve("bench-tools"))
    except (rust_runtime.Refused, rust_runtime.InputError) as error:
        raise Failed(2, f"export_explorer.py: {error}") from None
    done = subprocess.run([executable, "export-explorer", *arguments], capture_output=True)
    if done.returncode:
        raise Failed(done.returncode, os.fsdecode(done.stderr).rstrip("\n"))
    return done.stdout


def current(root):
    selected, documents = current_grading.load_current(root)
    return selected, documents, current_grading.coverage_status(selected, documents)


def build(root=ROOT, public=None):
    root = Path(root).resolve()
    public = root / "public" if public is None else Path(public)
    stage = root / ".cache" / "explorer-export"
    selected, documents, status = current(root)
    with tempfile.TemporaryDirectory(prefix="explorer-facts-") as scratch:
        facts = Path(scratch) / "facts.json"
        facts.write_text(json.dumps({"selected": selected, "documents": documents, "status": status}, allow_nan=False),
                         encoding="utf-8")
        common = ["--root", str(root), "--facts", str(facts), "--base-path", os.environ.get("BASE_PATH", "/")]
        tool("evidence", "--restore", *common)
        try:
            summary = tool("stage", *common, "--stage", str(stage)).decode("utf-8")
            if current(root)[2]["dataset_hash"] != status["dataset_hash"]:
                raise current_grading.Inconsistent("current evidence changed during export")
            interrupt = signal.signal(signal.SIGINT, signal.SIG_IGN)
            try:
                tool("publish", "--stage", str(stage), "--public", str(public))
            finally:
                signal.signal(signal.SIGINT, interrupt)
        finally:
            previous = stage / "previous"
            if not previous.exists() or not any(previous.iterdir()):
                shutil.rmtree(stage, ignore_errors=True)
    print(summary, end="")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT, help="repository whose current facts are exported")
    parser.add_argument("--public", type=Path, help="directory that receives data/ and evidence/ (ROOT/public)")
    args = parser.parse_args(argv)
    try:
        build(args.root, args.public)
    except Failed as failure:
        print(failure, file=sys.stderr)
        return failure.code
    return 0


if __name__ == "__main__":
    sys.exit(main())
