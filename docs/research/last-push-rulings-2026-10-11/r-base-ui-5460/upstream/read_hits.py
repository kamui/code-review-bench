#!/usr/bin/env python3
"""List the distinct hits of several saved issue searches, fetch each thread that a person opened, and
print every passage that matches the given words, marked as written by the cut-off or later.

    python3 read_hits.py OUTPUT.txt 'word1|word2' SEARCH.json [SEARCH.json ...]

Writes OUTPUT.txt (what was printed) and OUTPUT.json (the hits, which were read and why not)."""
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def main():
    output, words, names = sys.argv[1], sys.argv[2].split("|"), sys.argv[3:]
    hits = {}
    for name in names:
        data = json.loads((HERE / name).read_text(encoding="utf-8"))
        for item in data["items"]:
            entry = hits.setdefault(item["number"], {"number": item["number"], "title": item["title"], "author": item["user"]["login"],
                                                     "created_at": item["created_at"], "queries": []})
            entry["queries"].append(name)
    lines = []
    for number in sorted(hits):
        entry = hits[number]
        if entry["author"].endswith("[bot]"):
            entry["read"] = False
            entry["unread"] = "a dependency update opened by a bot"
            continue
        result = subprocess.run([sys.executable, str(HERE / "fetch_thread.py"), str(number), *words], capture_output=True, text=True)
        entry["read"] = True
        entry["matching_passages"] = max(0, len(result.stdout.splitlines()) - 2)
        lines.append(result.stdout)
    (HERE / output).write_text("\n".join(lines), encoding="utf-8")
    summary = {"searches": names, "words_looked_for": words, "distinct_hits": len(hits),
               "read": sum(1 for entry in hits.values() if entry["read"]), "hits": [hits[number] for number in sorted(hits)]}
    (HERE / output.replace(".txt", ".json")).write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    print(f"{output}: {summary['distinct_hits']} distinct hits, {summary['read']} read")


if __name__ == "__main__":
    main()
