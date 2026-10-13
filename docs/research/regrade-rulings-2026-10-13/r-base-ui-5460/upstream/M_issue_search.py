#!/usr/bin/env python3
"""Run one GitHub issue and pull request search and save it: M_issue_search.py OUT.json 'QUERY'"""
import json, subprocess, sys
out, query = sys.argv[1], sys.argv[2]
raw = subprocess.run(["gh", "api", "-X", "GET", "search/issues", "-f", "q=" + query, "-f", "per_page=100"],
                     capture_output=True, text=True)
if raw.returncode:
    sys.exit(raw.stderr)
data = json.loads(raw.stdout)
json.dump(data, open(out, "w"), indent=1)
open(out.replace(".json", "-request.txt"), "w").write(f"gh api -X GET search/issues -f q='{query}' -f per_page=100\n")
print(data.get("total_count"), "hits for", query)
for i in data.get("items", []):
    print(f"  #{i['number']} {i['created_at'][:10]} {'PR' if 'pull_request' in i else 'issue'} {i['user']['login']}: {i['title'][:100]}")
