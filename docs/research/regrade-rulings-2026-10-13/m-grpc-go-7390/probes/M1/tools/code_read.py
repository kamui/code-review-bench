#!/usr/bin/env python3
"""Read every public code-search hit that is not a copy of a grpc-go file.

    python3 code_read.py UPSTREAM_DIR

It takes the distinct files in UPSTREAM_DIR/code-q*.json, sets aside the ones whose path or repository shows a
copy of grpc-go's own source, fetches each of the others into UPSTREAM_DIR/code-files/ and writes its lines
that mention UpdateAddresses to UPSTREAM_DIR/code-read.json."""
import base64
import glob
import json
import os
import re
import subprocess
import sys

COPY = re.compile(r"(gracefulswitch|balancer_v1_wrapper|balancer/base/balancer_test|clientconn_test|google\.golang\.org|grpc-go|grpc@"
                  r"|/grpc/(internal|balancer|xds|interop|test)/|^(internal|balancer|xds|interop|test)/|pickfirst|grpclb|orcalb"
                  r"|cdsbalancer|clusterimpl|outlierdetection|ringhash|testutils/balancer\.go|^clientconn|/grpc/clientconn)")


def main(upstream):
    files = {}
    for name in sorted(glob.glob(f"{upstream}/code-q*.json")):
        for item in json.load(open(name)).get("items", []):
            files[(item["repository"]["full_name"], item["path"])] = item
    os.makedirs(f"{upstream}/code-files", exist_ok=True)
    read = []
    for repo, path in sorted(key for key in files if not COPY.search(key[1]) and not COPY.search(key[0])):
        saved = "code-files/" + repo.replace("/", "__") + "__" + path.replace("/", "__") + ".txt"
        if not os.path.exists(f"{upstream}/{saved}"):
            done = subprocess.run(["gh", "api", f"repos/{repo}/contents/{path}"], capture_output=True, text=True)
            if done.returncode:
                read.append({"repo": repo, "path": path, "error": done.stderr.strip()[:120]})
                continue
            open(f"{upstream}/{saved}", "w").write(base64.b64decode(json.loads(done.stdout)["content"]).decode("utf-8", "replace"))
        text = open(f"{upstream}/{saved}").read()
        read.append({"repo": repo, "path": path, "saved": saved,
                     "lines": [line.strip() for line in text.splitlines() if "UpdateAddresses" in line]})
    json.dump(read, open(f"{upstream}/code-read.json", "w"), indent=1)
    print(len(files), "distinct files;", len(read), "not copies of grpc-go files")


if __name__ == "__main__":
    main(sys.argv[1])
