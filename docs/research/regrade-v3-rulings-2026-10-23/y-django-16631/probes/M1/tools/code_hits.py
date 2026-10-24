"""Fetch each code-search hit and test it. Run with python3 -I.

Usage: code_hits.py <search json> <pattern> <download dir> <out json>
  pattern: rotation-save-every | catches-session-interrupted
Hits in repositories of the benchmark's owner (kamui) are dropped unread.
"""
import base64
import json
import re
import subprocess
import sys
from pathlib import Path

search, pattern, download, out = sys.argv[1], sys.argv[2], Path(sys.argv[3]), sys.argv[4]
download.mkdir(parents=True, exist_ok=True)
items = json.load(open(search))["items"]

TESTS = {
    "rotation-save-every": lambda t: bool(
        re.search(r"^\s*SECRET_KEY_FALLBACKS\s*=\s*(?!\[\s*\])\S", t, re.M)
        and re.search(r"^\s*SESSION_SAVE_EVERY_REQUEST\s*=\s*True\b", t, re.M)
    ),
    "catches-session-interrupted": lambda t: bool(
        re.search(r"except\s*\(?[^\n]*SessionInterrupted", t)
        and re.search(r"django\.contrib\.sessions", t)
    ),
}


def gh(*args):
    return subprocess.run(["gh", "api", *args], capture_output=True, text=True, check=True).stdout


def last_commit_date(repo, path):
    data = json.loads(gh("-X", "GET", f"repos/{repo}/commits", "-f", f"path={path}", "-f", "per_page=1"))
    return data[0]["commit"]["committer"]["date"] if data else None


rows = []
seen = set()
for item in items:
    repo, path = item["repository"]["full_name"], item["path"]
    if repo.split("/")[0].lower() == "kamui":
        rows.append({"repo": repo, "path": path, "state": "dropped unread (benchmark owner)"})
        continue
    if (repo, path) in seen:
        continue
    seen.add((repo, path))
    try:
        blob = json.loads(gh(f"repos/{repo}/contents/{path}", "-H", "Accept: application/vnd.github+json"))
        text = base64.b64decode(blob["content"]).decode("utf-8", "replace")
    except Exception as exc:  # noqa: BLE001 - recorded, not hidden
        rows.append({"repo": repo, "path": path, "state": f"fetch failed: {exc.__class__.__name__}"})
        continue
    (download / f"{repo.replace('/', '__')}__{path.replace('/', '__')}").write_text(text)
    match = TESTS[pattern](text)
    row = {"repo": repo, "path": path, "state": "read", "match": match}
    if match:
        row["last_commit"] = last_commit_date(repo, path)
        lines = [ln.strip() for ln in text.splitlines() if re.search(r"SECRET_KEY_FALLBACKS|SESSION_SAVE_EVERY_REQUEST|SessionInterrupted", ln)]
        row["lines"] = lines[:8]
    rows.append(row)

json.dump({"search": search, "pattern": pattern, "rows": rows}, open(out, "w"), indent=1)
read = [r for r in rows if r["state"] == "read"]
print(f"hits {len(items)}, read {len(read)}, matched {sum(r['match'] for r in read)}")
for r in read:
    if r["match"]:
        print(r["repo"], r["path"], r["last_commit"], r["lines"])
