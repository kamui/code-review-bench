"""Usage: python3 -I code_hits.py <out.json> <pattern-name> <search.json>...
Drops hits in the benchmark owner's own repositories from each saved search response (rewriting it with a count),
fetches every other hit's file through the GitHub contents API, tests it with the named pattern, and records the
date of the newest commit that touched the file. Writes one record per distinct file to <out.json>."""
import base64, json, re, subprocess, sys, time

PATTERNS = {
    # a whole input that is a Date, Map or Set, followed (anywhere after it in the same chain) by .use(
    "date-map-set-then-use": r"\.input\(\s*z\.(date|map|set)\([^;]*?\)\s*\)[\s\S]{0,400}?\.use\(",
    # an input of z.never()
    "never-input": r"\.input\(\s*z\.never\(\)\s*\)",
    # a middleware typed against the default ProcedureParams that reads ctx
    "middleware-function-procedure-params": r"MiddlewareFunction<\s*ProcedureParams\b|MiddlewareFunction<[^>]*ProcedureParams",
    # a whole input that is an array, followed by .use(
    "array-then-use": r"\.input\(\s*z\.array\([^;]*?\)\s*\)\s*\.use\(",
}

def gh(*args):
    for attempt in range(3):
        r = subprocess.run(["gh", "api", *args], capture_output=True, text=True)
        if r.returncode == 0:
            return json.loads(r.stdout)
        time.sleep(5)
    return {"error": r.stderr.strip()[:300]}

out, pattern_name, searches = sys.argv[1], sys.argv[2], sys.argv[3:]
pattern = re.compile(PATTERNS[pattern_name])
seen, records = set(), []
for path in searches:
    data = json.load(open(path))
    items = data.get("items", [])
    kept = [i for i in items if i["repository"]["owner"]["login"].lower() != "kamui"]
    dropped = len(items) - len(kept)
    data["items"] = kept
    data["note_dropped_unread"] = f"{dropped} hits in repositories of the benchmark's owner were removed before saving, unread"
    json.dump(data, open(path, "w"), indent=1)
    for item in kept:
        key = (item["repository"]["full_name"], item["path"])
        if key in seen:
            continue
        seen.add(key)
        content = gh(item["url"].replace("https://api.github.com/", ""))
        text = base64.b64decode(content["content"]).decode("utf-8", "replace") if "content" in content else ""
        commits = gh(f"repos/{key[0]}/commits?path={item['path']}&per_page=1")
        last = commits[0]["commit"]["committer"]["date"] if isinstance(commits, list) and commits else None
        matches = [m.group(0)[:300] for m in pattern.finditer(text)]
        records.append({"repository": key[0], "path": item["path"], "html_url": item["html_url"],
                        "newest_commit_touching_file": last, "pattern": pattern_name,
                        "matches": matches, "fetched": bool(text)})
json.dump(records, open(out, "w"), indent=1)
print(out, len(records), "files;", sum(1 for r in records if r["matches"]), "match")
