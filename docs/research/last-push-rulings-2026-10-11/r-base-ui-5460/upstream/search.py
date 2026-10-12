#!/usr/bin/env python3
"""Run one GitHub search with gh and save the request and the raw response beside this script.

    python3 search.py issues NAME 'QUERY'
    python3 search.py code NAME 'QUERY'

Writes NAME-request.txt (the command as run) and NAME.json (the response), then prints the hits.
"""
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def main():
    kind, name, query = sys.argv[1], sys.argv[2], sys.argv[3]
    page = sys.argv[4] if len(sys.argv) > 4 else "1"
    command = ["gh", "api", "-X", "GET", f"search/{kind}", "-f", f"q={query}", "-f", "per_page=100", "-f", f"page={page}"]
    (HERE / f"{name}-request.txt").write_text(
        f"gh api -X GET search/{kind} -f q='{query}' -f per_page=100 -f page={page}\n", encoding="utf-8"
    )
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 0:
        (HERE / f"{name}.json").write_text(result.stdout or result.stderr, encoding="utf-8")
        print("FAILED", result.stderr.strip()[:300])
        raise SystemExit(1)
    (HERE / f"{name}.json").write_text(result.stdout, encoding="utf-8")
    data = json.loads(result.stdout)
    print(f"{name}: total_count={data['total_count']} returned={len(data['items'])} incomplete={data.get('incomplete_results')}")
    for item in data["items"]:
        if kind == "issues":
            what = "PR" if "pull_request" in item else "issue"
            print(f"  #{item['number']} {what} {item['created_at']} {item['user']['login']}: {item['title'][:90]}")
        else:
            print(f"  {item['repository']['full_name']} {item['path']}")


if __name__ == "__main__":
    main()
