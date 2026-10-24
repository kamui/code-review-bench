"""For each fetched file: does code run a database query after teardown_databases() in the same function?
Prints the teardown call, keepdb handling, and up to 12 following lines of the same function."""
import json, re, sys
idx = json.load(open(sys.argv[1]))
QUERY = re.compile(r"cursor\(|\.objects\.|connection\.|connections\[|call_command|\.save\(|execute\(")
rows = []
for h in idx:
    if not h["file"]:
        rows.append({**h, "verdict": "unreadable"}); continue
    lines = open(h["file"]).read().splitlines()
    found = []
    for i, l in enumerate(lines):
        if re.search(r"teardown_databases\(", l) and not re.search(r"def teardown_databases", l):
            indent = len(l) - len(l.lstrip())
            after = []
            for j in range(i + 1, min(i + 40, len(lines))):
                lj = lines[j]
                if lj.strip() and (len(lj) - len(lj.lstrip())) < indent and not lj.lstrip().startswith((")", "]", "}")):
                    # left the block; stop at the end of the enclosing function
                    if re.match(r"\s*(def |class |@)", lj) or (len(lj) - len(lj.lstrip())) == 0:
                        break
                after.append((j + 1, lj))
                if re.match(r"\s*(def |class )", lj):
                    break
            q = [a for a in after if QUERY.search(a[1]) and "teardown" not in a[1]]
            found.append({"line": i + 1, "call": l.strip(), "query_after": q[:4]})
    keepdb = any("keepdb" in l for l in lines)
    pool = any(re.search(r"[\"']pool[\"']\s*:", l) for l in lines)
    rows.append({**h, "calls": found, "mentions_keepdb": keepdb, "sets_pool_option": pool})
json.dump(rows, open(sys.argv[2], "w"), indent=1)
for r in rows:
    flag = any(c["query_after"] for c in r.get("calls", []))
    print(("QUERY-AFTER " if flag else "            ") + f"{r['query']} {r['repo']}/{r['path']} keepdb={r.get('mentions_keepdb')} pool={r.get('sets_pool_option')}")
    for c in r.get("calls", []):
        for ln, t in c["query_after"]:
            print(f"      L{ln}: {t.strip()[:140]}")
