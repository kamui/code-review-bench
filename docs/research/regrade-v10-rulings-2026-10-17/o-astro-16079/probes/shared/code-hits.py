#!/usr/bin/env python3
"""Reads every hit of the public-code searches: fetches the file, cuts out the isr { ... } block of the
Vercel adapter call, and dates the file by its newest commit on the default branch.

usage: python3 -I code-hits.py <upstream directory>
writes <upstream>/public-code-read.json"""
import base64
import json
import re
import subprocess
import sys
from pathlib import Path

up = Path(sys.argv[1])
SKIP_OWNER = {"kamui"}  # the benchmark owner's repositories: dropped unread


def gh(path):
    out = subprocess.run(["gh", "api", path], capture_output=True, text=True)
    return json.loads(out.stdout) if out.returncode == 0 else {"error": out.stderr.strip()}


def isr_block(text):
    m = re.search(r"\bisr\s*:", text)
    if not m:
        return None
    start = m.start()
    depth, i, opened = 0, m.end(), False
    while i < len(text):
        c = text[i]
        if c in "{[":
            depth += 1
            opened = True
        elif c in "}]":
            depth -= 1
            if opened and depth == 0:
                return text[start:i + 1]
        elif c in ",\n" and not opened and text[m.end():i].strip():
            return text[start:i]
        i += 1
    return text[start:start + 400]


rows, seen = [], set()
for name in ("public-code-q1.json", "public-code-q2.json"):
    for item in json.loads((up / name).read_text())["items"]:
        repo, path = item["repository"]["full_name"], item["path"]
        if (repo, path) in seen:
            continue
        seen.add((repo, path))
        row = {"query": name, "repo": repo, "path": path}
        if repo.split("/")[0] in SKIP_OWNER:
            row["skipped"] = "repository of this benchmark's owner"
            rows.append(row)
            continue
        content = gh(f"repos/{repo}/contents/{path}")
        if "content" not in content:
            row["error"] = content.get("error") or content.get("message")
            rows.append(row)
            continue
        text = base64.b64decode(content["content"]).decode("utf-8", "replace")
        commits = gh(f"repos/{repo}/commits?path={path}&per_page=1")
        row["file_date"] = commits[0]["commit"]["committer"]["date"] if isinstance(commits, list) and commits else None
        row["fork"] = item["repository"].get("fork")
        block = isr_block(text)
        row["isr"] = block
        row["exclude"] = re.findall(r"exclude\s*:\s*\[([^\]]*)\]", block or "", re.S)
        rows.append(row)
        print(repo, path, row["file_date"], (row["exclude"] or [""])[0].replace("\n", " ")[:120], flush=True)
(up / "public-code-read.json").write_text(json.dumps(rows, indent=1))
