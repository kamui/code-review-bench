#!/usr/bin/env python3
"""List the benchmarks the roster has and the ones still missing.

Usage::

    python3 bench/tools/roster.py [--root DIR]

``bench/roster.json`` is the hand-edited roster: the clients, models and efforts that benchmarks
run against by default. It is an object keyed by client, and each client maps the models it runs
to the list of efforts to benchmark them at.

Review methods are not listed. A method counts as running on a client once any entry in
``bench/scoreboard.current.json`` has benchmarked it there, the client being that of the source
arm's kind. For every suite, every such method and every listed model on one of the method's
clients, one tab-separated line is printed per effort: suite, method, client, model, effort, then
the id of the entry whose source arm runs that model at that effort on that client, or ``missing``
when the suite has none. A method that has never run on a model's client is not printed for that model. Nothing is
written and no reviewer or grader is started.

Exit codes: 0 the lines were printed; 1 the roster cannot be used, one line per violation on stdout
(a client with no ``bench/harness/<client>.json``, a model with no ``bench/rates.current.json``
entry, a model without a list of distinct efforts, an empty roster); 2 an input cannot be read,
named on stderr, which includes a key written twice in the roster.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

REPO = Path(__file__).resolve().parents[2]
ROSTER = "bench/roster.json"
CLIENT_OF_KIND = {"claude-builtin": "claude-code", "claude-skill": "claude-code", "review-code": "claude-code",
                  "codex": "codex", "codex-skill": "codex"}


class InputError(Exception):
    """An input cannot be read; exit code 2."""


def unique_keys(pairs: list) -> dict:
    keys = [key for key, _ in pairs]
    repeated = sorted({key for key in keys if keys.count(key) > 1})
    if repeated:
        raise ValueError(f"{', '.join(repeated)} is written more than once")
    return dict(pairs)


def read_json(path: Path, object_pairs_hook=None):
    try:
        return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=object_pairs_hook)
    except (OSError, ValueError) as error:
        raise InputError(f"cannot read {path}: {error}") from error


def read_roster(root: Path) -> tuple:
    """Return the listed models, one per effort, and the roster's violations."""
    listed = read_json(root / ROSTER, unique_keys)
    if not isinstance(listed, dict):
        return [], [f"{ROSTER}: needs an object keyed by client"]
    priced = {rate["model"] for rate in read_json(root / "bench/rates.current.json")["rates"]}
    models, problems = [], []
    for client, efforts_of in listed.items():
        if not (root / "bench/harness" / f"{client}.json").is_file():
            problems.append(f"{ROSTER}: client {client} has no bench/harness/{client}.json")
        if not isinstance(efforts_of, dict):
            problems.append(f"{ROSTER}: {client} needs an object of models")
            continue
        for model, efforts in efforts_of.items():
            if model not in priced:
                problems.append(f"{ROSTER}: model {model} has no entry in bench/rates.current.json")
            named = isinstance(efforts, list) and efforts and all(effort and isinstance(effort, str) for effort in efforts)
            if not named or len(set(efforts)) != len(efforts):
                problems.append(f"{ROSTER}: {client} {model} needs a list of distinct efforts")
                continue
            models.extend({"client": client, "model": model, "effort": effort} for effort in efforts)
    if not models and not problems:
        problems.append(f"{ROSTER}: lists no models")
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
                client = CLIENT_OF_KIND[arm["kind"]]
                clients.setdefault(method, set()).add(client)
                benchmarked.setdefault((suite["id"], method, client, arm.get("model"), arm.get("effort")), entry["id"])
    lines = []
    for suite in suites:
        for method, runs_on in clients.items():
            for model in models:
                if model["client"] in runs_on:
                    combination = (method, model["client"], model["model"], model["effort"])
                    lines.append("\t".join([suite["id"], *combination, benchmarked.get((suite["id"], *combination), "missing")]))
    return lines


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=REPO, help="repository to read (default: this one)")
    args = parser.parse_args()
    try:
        models, problems = read_roster(args.root)
        lines = problems or status(args.root, models)
    except InputError as error:
        print(f"roster.py: {error}", file=sys.stderr)
        return 2
    for line in lines:
        print(line)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
