#!/usr/bin/env python3
"""List the benchmarks the default models have and the ones still missing.

Usage::

    python3 bench/tools/models.py [--root DIR]

``bench/models.json`` holds the hand-edited default list: ``models`` is an array with one object
per model, naming the ``model``, the ``client`` that runs it and its ``effort``. Other keys are
ignored.

Review methods are not listed. A method counts as running on a client once any entry in
``bench/scoreboard.current.json`` has benchmarked it there, the client being that of the source
arm's kind. For every suite, every such method and every listed model on one of the method's
clients, one tab-separated line is printed: suite, method, client, model, effort, then the id of
the entry whose source arm requests that model at that effort, or ``missing`` when the suite has
none. A method that has never run on a model's client is not printed for that model. Nothing is
written and no reviewer or grader is started.

Exit codes: 0 the lines were printed; 1 the list cannot be used, one line per violation on stdout
(an entry without its three keys, a repeated entry, a model with no ``bench/rates.current.json``
entry, a client with no ``bench/harness/<client>.json``, an empty list); 2 an input cannot be
read, named on stderr.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

REPO = Path(__file__).resolve().parents[2]
LIST = "bench/models.json"
KEYS = ("model", "client", "effort")
CLIENT_OF_KIND = {"claude-builtin": "claude-code", "claude-skill": "claude-code", "review-code": "claude-code",
                  "codex": "codex", "codex-skill": "codex"}


class InputError(Exception):
    """An input cannot be read; exit code 2."""


def read_json(path: Path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise InputError(f"cannot read {path}: {error}") from error


def read_list(root: Path) -> tuple:
    """Return the listed models and the list's violations."""
    listed = read_json(root / LIST)
    rows = listed.get("models") if isinstance(listed, dict) else None
    if not isinstance(rows, list) or not rows:
        return [], [f"{LIST}: models must be a non-empty array"]
    priced = {rate["model"] for rate in read_json(root / "bench/rates.current.json")["rates"]}
    models, problems = [], []
    for index, row in enumerate(rows):
        where = f"{LIST}: models[{index}]"
        if not isinstance(row, dict) or not all(row.get(key) and isinstance(row[key], str) for key in KEYS):
            problems.append(f"{where}: needs model, client and effort")
            continue
        model = {key: row[key] for key in KEYS}
        if any((seen["model"], seen["effort"]) == (model["model"], model["effort"]) for seen in models):
            problems.append(f"{where}: repeats {model['model']} at {model['effort']}")
        if model["model"] not in priced:
            problems.append(f"{where}: model {model['model']} has no entry in bench/rates.current.json")
        if not (root / "bench/harness" / f"{model['client']}.json").is_file():
            problems.append(f"{where}: client {model['client']} has no bench/harness/{model['client']}.json")
        models.append(model)
    return models, problems


def status(root: Path, models: list) -> list:
    suites = read_json(root / "bench/scoreboard.current.json")["suites"]
    clients, benchmarked = {}, {}
    for suite in suites:
        for entry in suite["entries"]:
            method = entry.get("method")
            if not method:
                continue
            for source in entry["sources"]:
                path = root / "bench/arms" / f"{source['arm']}.json"
                arm = read_json(path)
                if arm.get("kind") not in CLIENT_OF_KIND:
                    raise InputError(f"{path}: kind {arm.get('kind')!r} has no client in CLIENT_OF_KIND")
                clients.setdefault(method, set()).add(CLIENT_OF_KIND[arm["kind"]])
                benchmarked.setdefault((suite["id"], method, arm.get("model"), arm.get("effort")), entry["id"])
    lines = []
    for suite in suites:
        for method, runs_on in clients.items():
            for model in models:
                if model["client"] in runs_on:
                    entry_id = benchmarked.get((suite["id"], method, model["model"], model["effort"]), "missing")
                    lines.append("\t".join([suite["id"], method, model["client"], model["model"], model["effort"], entry_id]))
    return lines


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=REPO, help="repository to read (default: this one)")
    args = parser.parse_args()
    try:
        models, problems = read_list(args.root)
        lines = problems or status(args.root, models)
    except InputError as error:
        print(f"models.py: {error}", file=sys.stderr)
        return 2
    for line in lines:
        print(line)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
