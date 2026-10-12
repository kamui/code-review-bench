#!/usr/bin/env python3
"""Fetch one issue or pull request of mui/base-ui with its comments and review comments, save the raw
responses beside this script, and print every passage that matches the given words, with author and time.

    python3 fetch_thread.py NUMBER [WORD ...]
"""
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
CUT_OFF = "2026-08-10T13:33:57Z"


def get(path, name, paginate=False):
    command = ["gh", "api", path] + (["--paginate", "--slurp"] if paginate else [])
    result = subprocess.run(command, capture_output=True, text=True)
    target = HERE / name
    if result.returncode != 0:
        target.write_text(result.stderr, encoding="utf-8")
        return None
    target.write_text(result.stdout, encoding="utf-8")
    data = json.loads(result.stdout)
    if paginate:
        data = [item for page in data for item in page]
    return data


def main():
    number = sys.argv[1]
    words = sys.argv[2:]
    issue = get(f"repos/mui/base-ui/issues/{number}", f"issue-{number}.json")
    records = [("body", issue["user"]["login"], issue["created_at"], issue.get("body") or "")]
    kind = "PR" if "pull_request" in issue else "issue"
    extra = ""
    if kind == "PR":
        pull = get(f"repos/mui/base-ui/pulls/{number}", f"pr-{number}.json")
        extra = f" merged_at={pull.get('merged_at')}"
        for comment in get(f"repos/mui/base-ui/pulls/{number}/comments", f"pr-{number}-review-comments.json", True) or []:
            records.append(("review-comment", comment["user"]["login"], comment["created_at"], comment.get("body") or ""))
        for review in get(f"repos/mui/base-ui/pulls/{number}/reviews", f"pr-{number}-reviews.json", True) or []:
            records.append(("review", review["user"]["login"], review.get("submitted_at") or "", review.get("body") or ""))
    for comment in get(f"repos/mui/base-ui/issues/{number}/comments", f"issue-{number}-comments.json", True) or []:
        records.append(("comment", comment["user"]["login"], comment["created_at"], comment.get("body") or ""))
    print(f"#{number} {kind} by {issue['user']['login']} created {issue['created_at']} state={issue['state']}{extra}: {issue['title']}")
    print(f"  records: {len(records)}; after the last push ({CUT_OFF}): {sum(1 for r in records if r[2] > CUT_OFF)}")
    if not words:
        return
    pattern = re.compile("|".join(re.escape(word) for word in words), re.IGNORECASE)
    for where, who, when, text in records:
        for line in text.splitlines():
            if pattern.search(line):
                marker = "LATER" if when > CUT_OFF else "by-cut-off"
                print(f"  [{where} {who} {when} {marker}] {line.strip()[:600]}")


if __name__ == "__main__":
    main()
