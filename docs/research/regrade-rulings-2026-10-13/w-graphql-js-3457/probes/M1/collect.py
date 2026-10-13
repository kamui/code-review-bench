#!/usr/bin/env python3
"""Write result-base.txt and result-head.txt from the saved result files.

    python3 collect.py
"""
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
RESULTS = HERE / "results"
COMMITS = (("base", "730d5af8e933235fd5aa312a00be465db0b8acf5"), ("head", "efdbbcfacd92adb78e002f2b8a61f2a6a1504c19"))

for commit, sha in COMMITS:
    out = [
        f"# Results at the {commit}, commit {sha}.",
        "# One line per measurement, copied from the files named. Times are milliseconds unless the field says otherwise.",
        "# `dist` is the package built with the project's resources/build-npm.js; `src` is the source through the project's test loader.",
        "",
    ]
    for path in sorted(RESULTS.glob(f"timing-*-{commit}*.json")):
        data = json.loads(path.read_text())
        out.append(f"## results/{path.name}  (mode {data['mode']}, rules {data['rules']}, node {data['node']}, NODE_ENV {data['node_env']})")
        for row in data["rows"]:
            out.append(json.dumps({k: row[k] for k in ("case", "fields", "pairs", "query_bytes", "errors", "median_ms", "min_ms", "samples_ms")}))
        out.append("")
    for name in (f"count-{commit}.json", f"ordinary-{commit}.json", f"ordinary-fragments-{commit}.json"):
        data = json.loads((RESULTS / name).read_text())
        out.append(f"## results/{name}")
        out += [json.dumps({k: v for k, v in row.items() if k != "label"}) for row in data["rows"]]
        out.append("")
    for path in sorted(list(RESULTS.glob(f"project-benchmark-*-{commit}.json")) + list(RESULTS.glob(f"later-benchmark-*-{commit}.json"))):
        data = json.loads(path.read_text())
        out.append(f"## results/{path.name}")
        out.append(json.dumps({k: v for k, v in data.items() if k not in ("label", "node")}))
        out.append("")
    out.append("## results/gt-w1-case.txt")
    out += [line for line in (RESULTS / "gt-w1-case.txt").read_text().splitlines() if json.loads(line)["label"] == commit]
    out.append("")
    out.append("## results/project-tests.txt")
    out += [line for line in (RESULTS / "project-tests.txt").read_text().splitlines() if line.startswith(commit + " ")]
    (HERE / f"result-{commit}.txt").write_text("\n".join(out) + "\n")
    print(commit, len(out), "lines")
