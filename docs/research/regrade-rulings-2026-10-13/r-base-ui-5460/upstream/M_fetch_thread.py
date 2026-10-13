#!/usr/bin/env python3
"""Fetch one issue or pull request of mui/base-ui with its comments (and review comments for a
pull request) and save them: M_fetch_thread.py PREFIX NUMBER [NUMBER ...]"""
import json, subprocess, sys
prefix = sys.argv[1]
def api(path):
    r = subprocess.run(["gh", "api", "--paginate", path], capture_output=True, text=True)
    return json.loads(r.stdout) if r.returncode == 0 and r.stdout.strip() else None
for number in sys.argv[2:]:
    issue = api(f"repos/mui/base-ui/issues/{number}")
    comments = api(f"repos/mui/base-ui/issues/{number}/comments") or []
    data = {"issue": issue, "comments": comments}
    if issue and "pull_request" in issue:
        data["review_comments"] = api(f"repos/mui/base-ui/pulls/{number}/comments") or []
        data["reviews"] = api(f"repos/mui/base-ui/pulls/{number}/reviews") or []
    json.dump(data, open(f"{prefix}-{number}.json", "w"), indent=1)
    print(number, issue and issue["title"], "| comments", len(comments))
