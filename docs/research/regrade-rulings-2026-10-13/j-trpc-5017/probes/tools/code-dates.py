#!/usr/bin/env python3
"""Usage: code-dates.py <GROUP> <upstream-dir> <line-regex> [<query-for-page-2>]

Follow-up to code-hits.py. Optionally fetches page 2 of one code search (saved as
<GROUP>-code-q1-page2.json, hits in repositories of this benchmark's owner dropped first), then for
every source file of <GROUP>-code-hits.json and of that page with a line matching <line-regex>,
looks up the newest commit of its path before the cut-off and its newest commit, and whether the
line was there before the cut-off. Writes <GROUP>-code-dates.json. Read-only."""
import base64, json, re, subprocess, sys, time, urllib.parse
CUTOFF = "2023-11-09T22:08:09Z"
SKIP_OWNERS = {"kamui"}
def gh(args):
    for _ in range(4):
        r = subprocess.run(["gh", "api", *args], capture_output=True, text=True)
        if r.returncode == 0: return json.loads(r.stdout)
        if "rate limit" in (r.stderr + r.stdout).lower(): time.sleep(65); continue
        return {"error": (r.stderr or r.stdout)[:300]}
    return {"error": "rate-limited after retries"}
def content(repo, path, ref):
    d = gh([f"repos/{repo}/contents/{urllib.parse.quote(path)}?ref={ref}"])
    return base64.b64decode(d["content"]).decode("utf-8", "replace") if "content" in d else None
group, out, regex, *rest = sys.argv[1:]
files = {}
hits = json.load(open(f"{out}/{group}-code-hits.json"))
for r in hits["files"]:
    if r["source"] and any(re.search(regex, l) for l in r.get("lines_read", [])):
        files[(r["repository"], r["path"])] = None
page2 = None
if rest:
    d = gh(["-X", "GET", "search/code", "-f", f"q={rest[0]}", "-f", "per_page=100", "-f", "page=2"])
    if "items" in d:
        d["dropped_before_saving"] = sum(1 for i in d["items"] if i["repository"]["owner"]["login"] in SKIP_OWNERS)
        d["items"] = [i for i in d["items"] if i["repository"]["owner"]["login"] not in SKIP_OWNERS]
    json.dump(d, open(f"{out}/{group}-code-q1-page2.json", "w"), indent=1)
    page2 = {"query": rest[0], "page": 2, "total_count": d.get("total_count"), "returned": len(d.get("items", [])), "dropped_before_saving": d.get("dropped_before_saving"), "error": d.get("error")}
    for i in d.get("items", []):
        if re.search(r"\.(ts|tsx|mts|cts|js|jsx)$", i["path"]):
            files.setdefault((i["repository"]["full_name"], i["path"]), i["url"].split("ref=")[-1])
rows = []
for (repo, path), ref in sorted(files.items()):
    row = {"repository": repo, "path": path}
    if ref:
        text = content(repo, path, ref) or ""
        row["lines_read"] = [l.strip()[:200] for l in text.splitlines() if re.search(r"initTRPC|\.middleware\(|\.use\(|unstable_concat|discriminatedUnion|z\.union\(|\.input\(", l)][:40]
        if not any(re.search(regex, l) for l in row["lines_read"]):
            row["has_line_now"] = False; rows.append(row); continue
    newest = gh([f"repos/{repo}/commits?path={urllib.parse.quote(path)}&per_page=1"])
    before = gh([f"repos/{repo}/commits?path={urllib.parse.quote(path)}&until={CUTOFF}&per_page=1"])
    row["newest_commit"] = newest[0]["commit"]["committer"]["date"] if isinstance(newest, list) and newest else None
    if isinstance(before, list) and before:
        old = content(repo, path, before[0]["sha"]) or ""
        row["before_cutoff"] = {"sha": before[0]["sha"], "date": before[0]["commit"]["committer"]["date"],
                                "lines": [l.strip()[:200] for l in old.splitlines() if re.search(regex, l)]}
    else:
        row["before_cutoff"] = None
    repo_info = gh([f"repos/{repo}"])
    row["repository_created"] = repo_info.get("created_at")
    rows.append(row)
json.dump({"cutoff": CUTOFF, "line_regex": regex, "page2": page2, "files": rows}, open(f"{out}/{group}-code-dates.json", "w"), indent=1)
print(page2)
for r in rows:
    print(r["repository"], r["path"], "| repo created", r.get("repository_created"), "| newest", r.get("newest_commit"), "| before cut-off:", r.get("before_cutoff"), "| has line now:", r.get("has_line_now", True))
    for l in r.get("lines_read", [])[:6]: print("      ", l[:150])
