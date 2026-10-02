#!/usr/bin/env python3
"""List the benchmarks the default models have and the ones still missing.

Usage::

    python3 bench/tools/models.py [--root DIR]

``bench/models.md`` holds two hand-edited Markdown tables: under ``## Models`` one row per model
with its client and effort, and under ``## Methods`` one row per review method with the clients it
runs on, comma-separated. Columns are found by header name, so an extra column such as notes is
ignored, and backticks around a cell are dropped.

A method and a model combine only when the method lists the model's client. For every suite in
``bench/scoreboard.current.json`` and every such combination, one tab-separated line is printed:
suite, method, client, model, effort, then the id of the scoreboard entry whose source arm
requests that model at that effort, or ``missing`` when the suite has none. A combination the
tables rule out is not printed. Nothing is written and no reviewer or grader is started.

Exit codes: 0 the lines were printed; 1 the list cannot be used, one line per violation on stdout
(a row without its columns, a repeated row, a model with no ``bench/rates.current.json`` entry, a
client with no ``bench/harness/<client>.json``, an empty table); 2 an input cannot be read, named
on stderr.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys

REPO = Path(__file__).resolve().parents[2]
LIST = "bench/models.md"


class InputError(Exception):
    """An input cannot be read; exit code 2."""


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        raise InputError(f"cannot read {path}: {error}") from error


def read_json(path: Path):
    try:
        return json.loads(read_text(path))
    except ValueError as error:
        raise InputError(f"cannot read {path}: {error}") from error


def table(text: str, section: str) -> list:
    """Return (line number, {header: cell}) for each body row of the table under the section's heading."""
    rows, header, inside = [], None, False
    for number, line in enumerate(text.splitlines(), 1):
        line = line.strip()
        if line.startswith("#"):
            inside, header = line.lstrip("#").strip().lower() == section, None
        elif inside and line.startswith("|"):
            cells = [cell.strip().strip("`").strip() for cell in line.strip("|").split("|")]
            if header is None:
                header = [cell.lower() for cell in cells]
            elif set(line) - set("|-: "):
                rows.append((number, dict(zip(header, cells))))
    return rows


def read_list(root: Path) -> tuple:
    """Return the listed models, the clients of each listed method, and the list's violations."""
    text = read_text(root / LIST)
    priced = {rate["model"] for rate in read_json(root / "bench/rates.current.json")["rates"]}
    models, methods, problems = [], {}, []

    def check_client(number: int, client: str) -> None:
        if not (root / "bench/harness" / f"{client}.json").is_file():
            problems.append(f"{LIST}:{number}: client {client} has no bench/harness/{client}.json")

    for number, row in table(text, "models"):
        model = {key: row.get(key) for key in ("model", "client", "effort")}
        if not all(model.values()):
            problems.append(f"{LIST}:{number}: a model row needs Model, Client and Effort")
            continue
        if any((listed["model"], listed["effort"]) == (model["model"], model["effort"]) for listed in models):
            problems.append(f"{LIST}:{number}: repeats {model['model']} at {model['effort']}")
        if model["model"] not in priced:
            problems.append(f"{LIST}:{number}: model {model['model']} has no entry in bench/rates.current.json")
        check_client(number, model["client"])
        models.append(model)
    for number, row in table(text, "methods"):
        method = row.get("method")
        clients = [client.strip() for client in row.get("clients", "").split(",") if client.strip()]
        if not method or not clients:
            problems.append(f"{LIST}:{number}: a method row needs Method and Clients")
            continue
        if method in methods:
            problems.append(f"{LIST}:{number}: repeats {method}")
        for client in clients:
            check_client(number, client)
        methods[method] = clients
    for name, found in (("Models", models), ("Methods", methods)):
        if not found:
            problems.append(f"{LIST}: no rows under ## {name}")
    return models, methods, problems


def status(root: Path, models: list, methods: dict) -> list:
    lines = []
    for suite in read_json(root / "bench/scoreboard.current.json")["suites"]:
        benchmarked = {}
        for entry in suite["entries"]:
            for source in entry["sources"]:
                arm = read_json(root / "bench/arms" / f"{source['arm']}.json")
                benchmarked.setdefault((entry.get("method"), arm.get("model"), arm.get("effort")), entry["id"])
        for method, clients in methods.items():
            for model in models:
                if model["client"] in clients:
                    entry_id = benchmarked.get((method, model["model"], model["effort"]), "missing")
                    lines.append("\t".join([suite["id"], method, model["client"], model["model"], model["effort"], entry_id]))
    return lines


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=REPO, help="repository to read (default: this one)")
    args = parser.parse_args()
    try:
        models, methods, problems = read_list(args.root)
        lines = problems or status(args.root, models, methods)
    except InputError as error:
        print(f"models.py: {error}", file=sys.stderr)
        return 2
    for line in lines:
        print(line)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
