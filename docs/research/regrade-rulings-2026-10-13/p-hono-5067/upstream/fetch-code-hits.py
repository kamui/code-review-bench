#!/usr/bin/env python3
"""Fetch each named public file as it stood at its newest commit at or before the cut-off, save it under code/, and
print the lines that match a pattern with context.

    python3 fetch-code-hits.py PATTERN CONTEXT repo:path [repo:path ...]
"""
import json
import re
import subprocess
import sys
from pathlib import Path

CUTOFF = "2026-07-01T09:39:48Z"
OUT = Path(__file__).parent / "code"


def gh(*args):
    done = subprocess.run(["gh", "api", *args], capture_output=True, text=True)
    return done.stdout if done.returncode == 0 else None


pattern, context = re.compile(sys.argv[1]), int(sys.argv[2])
for spec in sys.argv[3:]:
    repo, path = spec.split(":", 1)
    commits = gh("-X", "GET", f"repos/{repo}/commits", "-f", f"path={path}", "-f", f"until={CUTOFF}", "-f", "per_page=1")
    commits = json.loads(commits) if commits else []
    if not commits:
        print(f"\n##### {repo}:{path}: no commit at or before {CUTOFF} (the file is later than the cut-off, or was not found)")
        continue
    sha, date = commits[0]["sha"], commits[0]["commit"]["committer"]["date"]
    text = gh("-H", "Accept: application/vnd.github.raw", f"repos/{repo}/contents/{path}?ref={sha}")
    if text is None:
        print(f"\n##### {repo}:{path}: content not fetched at {sha}")
        continue
    name = (repo + "__" + path).replace("/", "_")
    (OUT / f"{name}.txt").write_text(f"# {repo}:{path} at {sha}, committed {date}\n{text}", encoding="utf-8")
    lines = text.splitlines()
    shown = set()
    for index, line in enumerate(lines):
        if pattern.search(line):
            shown.update(range(max(0, index - context), min(len(lines), index + context + 1)))
    print(f"\n##### {repo}:{path} at {sha[:9]}, committed {date}, {len(lines)} lines -> code/{name}.txt")
    last = None
    for index in sorted(shown):
        if last is not None and index != last + 1:
            print("   ...")
        print(f"{index + 1:5}: {lines[index][:200]}")
        last = index
