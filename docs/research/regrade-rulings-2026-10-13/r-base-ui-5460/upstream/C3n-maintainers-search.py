#!/usr/bin/env python3
"""Search mui/base-ui issues and pull requests created before the last push to #5460 for what
maintainers said about a number-typed Field.Control or Input, a numeric controlled value, and
edits that leave the stored value the same. Saves each request line and raw response beside
this script. Run from this directory: python3 C3n-maintainers-search.py"""
import json
import subprocess
import time

CUTOFF = "2026-08-10T13:33:57Z"
QUERIES = {
    "number-type-field-control": 'repo:mui/base-ui "Field.Control" "type=\\"number\\""',
    "number-input-dirty": "repo:mui/base-ui Field number input dirty value",
    "leading-zero": 'repo:mui/base-ui "leading zero"',
    "leading-zeros": 'repo:mui/base-ui "leading zeros"',
    "valueasnumber": "repo:mui/base-ui valueAsNumber",
    "input-type-number": 'repo:mui/base-ui Input "type=\\"number\\"" onValueChange',
    "numeric-controlled-value": "repo:mui/base-ui Field.Control controlled numeric value dirty",
    "native-value-differs": "repo:mui/base-ui Field.Control controlled value differs from input DOM value",
    "value-prop-unchanged": 'repo:mui/base-ui Field.Control "value prop" unchanged validation change event',
}
seen = {}
for name, query in QUERIES.items():
    q = f"{query} created:<{CUTOFF}"
    request = f"gh api -X GET search/issues -f q='{q}' -f per_page=100"
    open(f"C3n-maintainers-{name}-request.txt", "w").write(request + "\n")
    out = subprocess.run(["gh", "api", "-X", "GET", "search/issues", "-f", f"q={q}", "-f", "per_page=100"], capture_output=True, text=True)
    open(f"C3n-maintainers-{name}.json", "w").write(out.stdout if out.returncode == 0 else json.dumps({"error": out.stderr}))
    data = json.loads(out.stdout) if out.returncode == 0 else {"items": [], "total_count": None, "error": out.stderr}
    print(name, data.get("total_count"), data.get("error", ""))
    for item in data.get("items", []):
        seen.setdefault(item["number"], {"number": item["number"], "title": item["title"], "created_at": item["created_at"], "user": item["user"]["login"], "is_pr": "pull_request" in item, "queries": []})["queries"].append(name)
    time.sleep(3)
json.dump(sorted(seen.values(), key=lambda x: x["number"]), open("C3n-maintainers-hits.json", "w"), indent=1)
print(len(seen), "distinct hits")
