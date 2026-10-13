#!/usr/bin/env python3
"""Read every issue and pull request the maintainer searches returned.

    python3 maintainers_read.py UPSTREAM_DIR

For each distinct hit in UPSTREAM_DIR/maintainers-q*.json it fetches the body, every comment and, for a pull
request, every review comment, and writes the lines that mention UpdateAddresses or an empty address list to
UPSTREAM_DIR/maintainers-read.json with their author and date. The fetched threads themselves are not kept."""
import glob
import json
import re
import subprocess
import sys

PATTERN = re.compile(r"UpdateAddress|empty address|empty list|no addresses|zero addresses|empty addr", re.I)


def api(path):
    done = subprocess.run(["gh", "api", "--paginate", "--slurp", path], capture_output=True, text=True)
    return [item for page in json.loads(done.stdout) for item in page] if done.returncode == 0 else []


def main(upstream):
    hits = {}
    for name in sorted(glob.glob(f"{upstream}/maintainers-q*.json")):
        for item in json.load(open(name))["items"]:
            hits[item["number"]] = item
    read = {}
    for number, item in sorted(hits.items()):
        pull = "pull_request" in item
        texts = [("body", item["user"]["login"], item["created_at"], item.get("body") or "")]
        for comment in api(f"repos/grpc/grpc-go/issues/{number}/comments"):
            texts.append(("comment", comment["user"]["login"], comment["created_at"], comment["body"] or ""))
        if pull:
            for comment in api(f"repos/grpc/grpc-go/pulls/{number}/comments"):
                texts.append(("review:" + comment.get("path", ""), comment["user"]["login"], comment["created_at"], comment["body"] or ""))
        lines = [f"{where} | {who} | {when[:10]} | {line.strip()[:500]}"
                 for where, who, when, text in texts for line in text.splitlines() if PATTERN.search(line)]
        read[number] = {"title": item["title"], "created": item["created_at"], "kind": "pr" if pull else "issue",
                        "texts": len(texts), "matching_lines": lines}
    json.dump(read, open(f"{upstream}/maintainers-read.json", "w"), indent=1)


if __name__ == "__main__":
    main(sys.argv[1])
