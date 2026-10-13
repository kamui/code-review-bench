#!/usr/bin/env python3
"""Fetch every hit of C3n-maintainers-search.py that a bot did not open, with its comments (and for
a pull request its review comments and reviews), and write the passages that mention the search
terms into C3n-maintainers-read.txt, each with its author and time and whether it is from before
the last push. Raw responses are saved as C3n-thread-<number>.json. Run from this directory."""
import json
import re
import subprocess

CUTOFF = "2026-08-10T13:33:57Z"
TERMS = re.compile(r"type=.number|number input|leading zero|valueAsNumber|dirty|Number\(|numeric|string value|as a string|always a string|typeof", re.I)


def api(path):
    out = subprocess.run(["gh", "api", "--paginate", "--slurp", path], capture_output=True, text=True)
    if out.returncode != 0:
        return [{"error": out.stderr}]
    pages = json.loads(out.stdout)
    flat = []
    for page in pages:
        flat.extend(page if isinstance(page, list) else [page])
    return flat


hits = json.load(open("C3n-maintainers-hits.json"))
lines = []
read = []
for hit in hits:
    n = hit["number"]
    if hit["user"].endswith("[bot]"):
        lines.append(f"## #{n} {hit['title']} (opened by {hit['user']}, a bot: not read)\n")
        continue
    thread = {"issue": api(f"repos/mui/base-ui/issues/{n}")[0], "comments": api(f"repos/mui/base-ui/issues/{n}/comments")}
    if hit["is_pr"]:
        thread["review_comments"] = api(f"repos/mui/base-ui/pulls/{n}/comments")
        thread["reviews"] = api(f"repos/mui/base-ui/pulls/{n}/reviews")
    json.dump(thread, open(f"C3n-thread-{n}.json", "w"))
    read.append(n)
    lines.append(f"## #{n} {hit['title']} ({'pull request' if hit['is_pr'] else 'issue'}, opened {hit['created_at']} by {hit['user']}; found by {', '.join(hit['queries'])})\n")
    entries = [("body", thread["issue"].get("user", {}).get("login"), thread["issue"].get("created_at"), thread["issue"].get("author_association"), thread["issue"].get("body") or "")]
    for kind in ("comments", "review_comments", "reviews"):
        for c in thread.get(kind, []):
            if "error" in c:
                continue
            entries.append((kind, (c.get("user") or {}).get("login"), c.get("created_at") or c.get("submitted_at"), c.get("author_association"), c.get("body") or ""))
    for kind, user, when, assoc, body in entries:
        if user and user.endswith("[bot]"):
            continue
        for paragraph in re.split(r"\n\s*\n", body):
            if TERMS.search(paragraph):
                timing = "before the last push" if (when or "") < CUTOFF else "AFTER the last push"
                lines.append(f"- [{kind}] {user} ({assoc}) {when} ({timing}):\n  " + paragraph.strip().replace("\n", "\n  ")[:1500] + "\n")
    lines.append("")
open("C3n-maintainers-read.txt", "w").write("\n".join(lines))
print("read", len(read), "threads:", read)
