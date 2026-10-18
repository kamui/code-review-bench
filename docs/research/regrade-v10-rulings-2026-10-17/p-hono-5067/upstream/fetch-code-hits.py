#!/usr/bin/env python3
"""Fetch the first N hits of a saved code search as each file stood at its newest commit at or before the cut-off.
Usage: python3 -I fetch-code-hits.py code-q1.json N   (writes code/<owner>_<repo>__<path>.txt and prints a summary)"""
import base64
import json
import subprocess
import sys
from pathlib import Path

CUT = "2026-07-01T09:39:48Z"
here = Path(__file__).resolve().parent
search, count = json.loads((here / sys.argv[1]).read_text()), int(sys.argv[2])
for item in search["items"][:count]:
    repo, path = item["repository"]["full_name"], item["path"]
    commits = json.loads(subprocess.run(["gh", "api", "-X", "GET", f"repos/{repo}/commits", "-f", f"path={path}",
                                         "-f", f"until={CUT}", "-f", "per_page=1"], capture_output=True, text=True).stdout or "[]")
    first = json.loads(subprocess.run(["gh", "api", "-X", "GET", f"repos/{repo}/commits", "-f", f"path={path}",
                                       "-f", "per_page=100"], capture_output=True, text=True).stdout or "[]")
    created = first[-1]["commit"]["committer"]["date"] if isinstance(first, list) and first else "unknown"
    name = f"{repo.replace('/', '_')}__{path.replace('/', '_')}.txt"
    if not isinstance(commits, list) or not commits:
        print(f"{repo} {path}: no commit at or before the cut-off (file's oldest listed commit {created})")
        continue
    sha, date = commits[0]["sha"], commits[0]["commit"]["committer"]["date"]
    blob = json.loads(subprocess.run(["gh", "api", f"repos/{repo}/contents/{path}?ref={sha}"], capture_output=True, text=True).stdout or "{}")
    text = base64.b64decode(blob.get("content", "")).decode("utf-8", "replace")
    (here / "code" / name).write_text(f"# {repo} {path} at {sha} ({date}); oldest listed commit of the file {created}\n" + text)
    lines = [f"    {i + 1}: {line.strip()[:150]}" for i, line in enumerate(text.splitlines()) if "parseBody" in line or "bodyLimit" in line]
    print(f"{repo} {path} at {sha[:8]} {date} (file since {created})\n" + "\n".join(lines))
