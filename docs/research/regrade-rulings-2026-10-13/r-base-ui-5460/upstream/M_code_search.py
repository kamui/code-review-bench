#!/usr/bin/env python3
"""Run one GitHub code search, drop hits from this benchmark's own repositories unread, and save.

    python3 M_code_search.py OUT.json 'QUERY'

The query gets `-repo:kamui/code-review-bench` added, and any hit from a repository of the benchmark's
owner (`kamui/...`, which hold this benchmark's own records about the pull request) is removed before
saving, with the number removed recorded as `dropped_own_repository`.
The request as sent is written beside the response as OUT-request.txt."""
import json, subprocess, sys

out, query = sys.argv[1], sys.argv[2]
sent = query + " -repo:kamui/code-review-bench"
raw = subprocess.run(["gh", "api", "-X", "GET", "search/code", "-f", "q=" + sent, "-f", "per_page=100"],
                     capture_output=True, text=True)
if raw.returncode:
    sys.exit(raw.stderr)
data = json.loads(raw.stdout)
kept = [i for i in data.get("items", []) if not i["repository"]["full_name"].startswith("kamui/")]
data["dropped_own_repository"] = len(data.get("items", [])) - len(kept)
data["items"] = kept
json.dump(data, open(out, "w"), indent=1)
open(out.replace(".json", "-request.txt"), "w").write(f"gh api -X GET search/code -f q='{sent}' -f per_page=100\n")
print(data.get("total_count"), "total;", len(kept), "kept;", data["dropped_own_repository"], "dropped")
for i in kept:
    print(" ", i["repository"]["full_name"], i["path"])
