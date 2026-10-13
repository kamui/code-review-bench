#!/usr/bin/env python3
"""Usage: code-hits.py <GROUP> <upstream-dir> <pattern-name> <query>...

Runs each GitHub code search (REST search/code, one page of up to 100 hits), saves the raw
response as <GROUP>-code-q<n>.json, then fetches every distinct source file among the hits and
tests it against the named pattern. For a file that matches, it also looks up the newest commit
of that path before the cut-off and tests the file as it stood then. Writes <GROUP>-code-hits.json.
Read-only. Waits and retries when the search is rate-limited. Hits in repositories of this
benchmark's owner are dropped before anything is saved or printed."""
import base64, json, os, re, subprocess, sys, time, urllib.parse

CUTOFF = "2023-11-09T22:08:09Z"
SKIP_OWNERS = {"kamui"}  # this benchmark's owner: its repositories hold records about this pull request

def gh(args, tries=4):
    for attempt in range(tries):
        r = subprocess.run(["gh", "api", *args], capture_output=True, text=True)
        if r.returncode == 0:
            return json.loads(r.stdout)
        if "rate limit" in (r.stderr + r.stdout).lower() or "403" in r.stderr or "429" in r.stderr:
            time.sleep(65)
            continue
        return {"error": (r.stderr or r.stdout)[:300]}
    return {"error": "rate-limited after retries"}

def input_then_use_union(text):
    """A whole-input union of objects followed by .use(): .input(<expr>) then .use( with the
    expression itself, or a constant it names, built with discriminatedUnion or z.union."""
    found = []
    step = r"(?:(?!\.input\(|\.query\(|\.mutation\(|\.subscription\(|\.use\()[\s\S])"
    for m in re.finditer(r"\.input\(\s*(" + step + r"{0,600}?)\)\s*\.use\(", text):
        expr = m.group(1)
        direct = re.search(r"discriminatedUnion|z\.union\(", expr)
        name = re.fullmatch(r"\s*([A-Za-z_$][\w$]*)\s*,?\s*", expr)
        named = bool(name and re.search(r"\b%s\s*=\s*z\s*\.\s*(discriminatedUnion|union)\(" % re.escape(name.group(1)), text))
        if direct or named:
            found.append(m.group(0)[:200])
    return found

def concat_optional(text):
    """unstable_concat used in a file that also declares an optional or nullish whole object input."""
    if "unstable_concat" not in text:
        return []
    uses = [m.group(0)[:160] for m in re.finditer(r"[^\n]*unstable_concat\([^\n]*", text)]
    optional = re.findall(r"\.input\(\s*z\s*\.object\([\s\S]{0,300}?\)\s*\.(?:optional|nullish)\(\)", text)
    return uses if optional else []

def any_context_builder_middleware(text):
    """initTRPC.context<any>() in a file that also makes a middleware with .middleware( and reads ctx."""
    if not re.search(r"initTRPC\s*\.context<any>\(\)", text):
        return []
    reads = [m.group(0)[:200] for m in re.finditer(r"\.middleware\(\s*(?:async\s*)?\(\s*(?:\{[^)]*\bctx\b[^)]*\}|\w+)\s*\)\s*=>[\s\S]{0,300}?ctx\.\w+", text)]
    return reads

LINES = r"initTRPC|\.middleware\(|\.use\(|unstable_concat|discriminatedUnion|z\.union\(|\.input\("
PATTERNS = {"input-then-use-union": input_then_use_union, "concat-optional": concat_optional,
            "any-context-builder-middleware": any_context_builder_middleware}

def content(repo, path, ref):
    d = gh([f"repos/{repo}/contents/{urllib.parse.quote(path)}?ref={ref}"])
    if "content" not in d:
        return None
    return base64.b64decode(d["content"]).decode("utf-8", "replace")

def main():
    group, out, pattern, *queries = sys.argv[1:]
    test = PATTERNS[pattern]
    files, searches = {}, []
    for n, q in enumerate(queries, 1):
        saved = f"{out}/{group}-code-q{n}.json"
        if os.environ.get("REUSE") == "1" and os.path.exists(saved):
            d = json.load(open(saved))  # the saved response of the same query, not searched again
        else:
            d = gh(["-X", "GET", "search/code", "-f", f"q={q}", "-f", "per_page=100"])
        if "items" in d and "dropped_before_saving" not in d:
            d["dropped_before_saving"] = sum(1 for i in d["items"] if i["repository"]["owner"]["login"] in SKIP_OWNERS)
            d["items"] = [i for i in d["items"] if i["repository"]["owner"]["login"] not in SKIP_OWNERS]
        json.dump(d, open(f"{out}/{group}-code-q{n}.json", "w"), indent=1)
        searches.append({"query": q, "total_count": d.get("total_count"), "returned": len(d.get("items", [])),
                         "dropped_before_saving": d.get("dropped_before_saving"), "error": d.get("error")})
        for i in d.get("items", []):
            files.setdefault((i["repository"]["full_name"], i["path"]), i)
        time.sleep(8)
    rows = []
    for (repo, path), item in sorted(files.items()):
        row = {"repository": repo, "path": path, "fork": item["repository"].get("fork"), "source": bool(re.search(r"\.(ts|tsx|mts|cts|js|jsx)$", path))}
        if row["source"]:
            sha = item["url"].split("ref=")[-1]
            text = content(repo, path, sha)
            row["fetched"] = text is not None
            row["matches_now"] = test(text) if text else []
            row["lines_read"] = [l.strip()[:200] for l in (text or "").splitlines() if re.search(LINES, l)][:40]
            if row["matches_now"]:
                before = gh([f"repos/{repo}/commits?path={urllib.parse.quote(path)}&until={CUTOFF}&per_page=1"])
                if isinstance(before, list) and before:
                    row["newest_commit_before_cutoff"] = {"sha": before[0]["sha"], "date": before[0]["commit"]["committer"]["date"]}
                    old = content(repo, path, before[0]["sha"])
                    row["matches_before_cutoff"] = test(old) if old else None
                else:
                    row["newest_commit_before_cutoff"] = None
                first = gh([f"repos/{repo}/commits?path={urllib.parse.quote(path)}&per_page=1"])
                if isinstance(first, list) and first:
                    row["newest_commit"] = {"sha": first[0]["sha"], "date": first[0]["commit"]["committer"]["date"]}
        rows.append(row)
    json.dump({"cutoff": CUTOFF, "pattern": pattern, "pattern_doc": test.__doc__, "searches": searches, "files": rows},
              open(f"{out}/{group}-code-hits.json", "w"), indent=1)
    src = [r for r in rows if r["source"]]
    print(group, "searches:", searches)
    print("distinct files:", len(rows), "source files:", len(src), "fetched:", sum(1 for r in src if r.get("fetched")),
          "matching now:", sum(1 for r in src if r.get("matches_now")),
          "matching before the cut-off:", sum(1 for r in src if r.get("matches_before_cutoff")))
    for r in src:
        if r.get("matches_now"):
            print("  ", r["repository"], r["path"], "| before:", r.get("newest_commit_before_cutoff"), "| newest:", (r.get("newest_commit") or {}).get("date"))
            for m in r["matches_now"][:2]:
                print("       ", re.sub(r"\s+", " ", m)[:180])

main()
