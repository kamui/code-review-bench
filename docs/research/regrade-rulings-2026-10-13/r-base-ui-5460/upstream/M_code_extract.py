#!/usr/bin/env python3
"""For every hit of a saved code search, fetch the file as it is now and as it stood at the last
commit before the cut-off, and extract the lines that matter.

    python3 M_code_extract.py SEARCH.json OUT.json 'REGEX' ['REGEX' ...]

Each regular expression selects lines to keep (with the import lines of the file). Dates are the
committer dates of the file's newest commit before the cut-off and of its oldest commit."""
import base64, json, re, subprocess, sys

CUTOFF = "2026-08-10T13:33:57Z"
search, out, patterns = sys.argv[1], sys.argv[2], [re.compile(p) for p in sys.argv[3:]]

def api(path, *fields):
    cmd = ["gh", "api", "-X", "GET", path]
    for f in fields:
        cmd += ["-f", f]
    r = subprocess.run(cmd, capture_output=True, text=True)
    return json.loads(r.stdout) if r.returncode == 0 and r.stdout.strip() else None

def text_at(repo, path, ref):
    d = api(f"repos/{repo}/contents/{path}", f"ref={ref}")
    if not d or "content" not in d:
        return None
    return base64.b64decode(d["content"]).decode("utf-8", "replace")

def keep(text):
    rows = []
    for n, line in enumerate(text.splitlines(), 1):
        if re.match(r"\s*import\b", line) or any(p.search(line) for p in patterns):
            rows.append(f"{n}: {line.rstrip()[:220]}")
    return rows

result = []
for item in json.load(open(search))["items"]:
    repo, path = item["repository"]["full_name"], item["path"]
    entry = {"repository": repo, "path": path}
    before = api(f"repos/{repo}/commits", f"path={path}", f"until={CUTOFF}", "per_page=1")
    newest = api(f"repos/{repo}/commits", f"path={path}", "per_page=1")
    entry["newest_commit"] = newest[0]["commit"]["committer"]["date"] if newest else None
    if before:
        sha = before[0]["sha"]
        entry["last_commit_before_cutoff"] = {"sha": sha, "date": before[0]["commit"]["committer"]["date"]}
        then = text_at(repo, path, sha)
        entry["lines_before_cutoff"] = keep(then) if then is not None else None
    else:
        entry["last_commit_before_cutoff"] = None
    now = text_at(repo, path, item.get("url", "").split("ref=")[-1] if "ref=" in item.get("url", "") else "HEAD")
    entry["lines_now"] = keep(now) if now is not None else None
    result.append(entry)
    print(repo, path, "| before cut-off:", bool(before), "| newest:", entry["newest_commit"])
json.dump(result, open(out, "w"), indent=1)
