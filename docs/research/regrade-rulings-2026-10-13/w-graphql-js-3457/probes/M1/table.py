#!/usr/bin/env python3
"""Summarise the saved timing files of this probe.

    python3 table.py RESULTS_DIRECTORY

Prints, for each run set, the median of the per-file medians and the lowest sample at each commit or variant,
the head-to-base ratio, and how the time grows when the number of fields grows.
"""
import json
import math
import statistics
import sys
from collections import defaultdict
from pathlib import Path

CASES = ["no-arguments", "one-scalar", "two-scalars", "nested-input-object"]


def load(paths):
    """label -> (case, fields) -> {"medians": [...], "samples": [...]}"""
    table = defaultdict(lambda: defaultdict(lambda: {"medians": [], "samples": []}))
    for path in paths:
        data = json.loads(Path(path).read_text())
        for row in data["rows"]:
            cell = table[row["label"]][(row["case"], row["fields"])]
            cell["medians"].append(row["median_ms"])
            cell["samples"].extend(row["samples_ms"])
    return table


def figure(cell):
    return statistics.median(cell["medians"]), min(cell["samples"]), len(cell["samples"])


def compare(title, paths, labels=("base", "head")):
    paths = sorted(paths)
    if not paths:
        return
    table = load(paths)
    print(f"## {title}")
    print("files: " + ", ".join(Path(p).name for p in paths))
    first, second = labels
    print(f"| case | fields | {first} median ms | {first} min ms | {second} median ms | {second} min ms | ratio of medians | ratio of minima | samples each |")
    print("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    keys = sorted(table[first], key=lambda k: (CASES.index(k[0]) if k[0] in CASES else 9, k[1]))
    for key in keys:
        if key not in table[second]:
            continue
        a_med, a_min, a_n = figure(table[first][key])
        b_med, b_min, b_n = figure(table[second][key])
        print(f"| {key[0]} | {key[1]} | {a_med:.1f} | {a_min:.1f} | {b_med:.1f} | {b_min:.1f} | {b_med / a_med:.2f} | {b_min / a_min:.2f} | {a_n}, {b_n} |")
    print()
    print("Growth: exponent k in time ~ fields^k, from the lowest samples at the smallest and largest sizes run.")
    print("| case | " + " | ".join(labels) + " |")
    print("| --- | " + " | ".join("---" for _ in labels) + " |")
    for case in CASES:
        cells = []
        for label in labels:
            sizes = sorted(n for (c, n) in table[label] if c == case)
            if len(sizes) < 2:
                cells.append("n/a")
                continue
            low, high = sizes[0], sizes[-1]
            t_low = min(table[label][(case, low)]["samples"])
            t_high = min(table[label][(case, high)]["samples"])
            cells.append(f"{math.log(t_high / t_low) / math.log(high / low):.2f} ({low} to {high})")
        if any(c != "n/a" for c in cells):
            print(f"| {case} | " + " | ".join(cells) + " |")
    print()


def variants(title, directory, names):
    rows = {}
    for name in names:
        paths = sorted(directory.glob(f"timing-dist-{name}-fix*.json")) if name not in ("base", "head") else sorted(directory.glob(f"timing-dist-{name}-variants*.json"))
        if not paths:
            continue
        table = load(paths)
        label = next(iter(table))
        rows[name] = table[label]
    if not rows:
        return
    print(f"## {title}")
    names = [n for n in names if n in rows]
    keys = sorted(rows[names[0]], key=lambda k: (CASES.index(k[0]) if k[0] in CASES else 9, k[1]))
    print("Median ms (lowest sample ms).")
    print("| case | fields | " + " | ".join(names) + " |")
    print("| --- | --- | " + " | ".join("---" for _ in names) + " |")
    for key in keys:
        cells = []
        for name in names:
            cell = rows[name].get(key)
            cells.append("n/a" if cell is None else f"{statistics.median(cell['medians']):.1f} ({min(cell['samples']):.1f})")
        print(f"| {key[0]} | {key[1]} | " + " | ".join(cells) + " |")
    print()


def per_pair(title, paths):
    paths = sorted(paths)
    if not paths:
        return
    table = load(paths)
    print(f"## {title}")
    print("Microseconds for one pair of selections: the median time divided by fields * (fields - 1) / 2.")
    print("| case | fields | base | head | added at head |")
    print("| --- | --- | --- | --- | --- |")
    for key in sorted(table["base"], key=lambda k: (CASES.index(k[0]), k[1])):
        pairs = key[1] * (key[1] - 1) / 2
        base = statistics.median(table["base"][key]["medians"]) * 1000 / pairs
        head = statistics.median(table["head"][key]["medians"]) * 1000 / pairs
        print(f"| {key[0]} | {key[1]} | {base:.2f} | {head:.2f} | {head - base:.2f} |")
    print()


def small(title, directory, pattern, labels):
    rows = {}
    for label in labels:
        path = directory / pattern.format(label)
        if path.exists():
            rows[label] = json.loads(path.read_text())["rows"]
    if not rows:
        return
    labels = [label for label in labels if label in rows]
    print(f"## {title}")
    print("Microseconds for one validation: overlap rule alone / all default rules.")
    print("| workload | query bytes | " + " | ".join(labels) + " |")
    print("| --- | --- | " + " | ".join("---" for _ in labels) + " |")
    for index, row in enumerate(rows[labels[0]]):
        cells = [f"{rows[label][index]['overlap_rule_alone_us']:.0f} / {rows[label][index]['all_default_rules_us']:.0f}" for label in labels]
        print(f"| {row['workload']} | {row['query_bytes']} | " + " | ".join(cells) + " |")
    print()


def benchmarks(title, directory, pattern, labels):
    print(f"## {title}")
    print("| benchmark | " + " | ".join(labels) + " |")
    print("| --- | " + " | ".join("---" for _ in labels) + " |")
    for stem in sorted({p.name.rsplit("-" + label + ".json", 1)[0] for label in labels for p in directory.glob(pattern.format(label))}):
        cells, name = [], stem
        for label in labels:
            path = directory / f"{stem}-{label}.json"
            if path.exists():
                data = json.loads(path.read_text())
                name = data["benchmark"]
                cells.append(f"{data['median_ms_per_measure']:.3f}")
            else:
                cells.append("n/a")
        print(f"| {name} ({stem}), ms for one measure() | " + " | ".join(cells) + " |")
    print()


def counts(title, directory, labels):
    print(f"## {title}")
    print("Calls to print() for one pair / syntax nodes handed to print() for one pair, 100 fields.")
    rows = {label: json.loads((directory / f"count-{label}.json").read_text())["rows"] for label in labels if (directory / f"count-{label}.json").exists()}
    labels = [label for label in labels if label in rows]
    print("| case | " + " | ".join(labels) + " |")
    print("| --- | " + " | ".join("---" for _ in labels) + " |")
    for index, row in enumerate(rows[labels[0]]):
        cells = [f"{rows[label][index]['print_calls_per_pair']:.2f} / {rows[label][index]['nodes_handed_to_print_per_pair']:.2f}" for label in labels]
        print(f"| {row['case']} | " + " | ".join(cells) + " |")
    print()


if __name__ == "__main__":
    directory = Path(sys.argv[1])
    compare("Built package, overlap rule alone, 100 to 1000 fields, three alternating rounds", directory.glob("timing-dist-*-round*.json"))
    compare("Built package, overlap rule alone, 2000 fields", directory.glob("timing-dist-*-2000.json"))
    compare("Built package, all default rules", directory.glob("timing-dist-*-allrules.json"))
    compare("Built package, overlap rule alone, NODE_ENV=production", directory.glob("timing-dist-*-production.json"))
    compare("Source through the project's test loader, overlap rule alone", list(directory.glob("timing-src-base.json")) + list(directory.glob("timing-src-head.json")))
    compare("Source through the project's test loader, no arguments, 2000 fields", directory.glob("timing-src-*-noargs-2000.json"))
    variants("Fix variants, built package, overlap rule alone", directory, ["base", "head", "fix-fastpath", "fix-with-arguments-only", "fix-both", "fix-maintainers-pr-3967", "fix-memo"])
    per_pair("Time for one pair, built package, overlap rule alone, three rounds", directory.glob("timing-dist-*-round*.json"))
    everything = ["base", "head", "fix-fastpath", "fix-with-arguments-only", "fix-both", "fix-maintainers-pr-3967", "fix-memo"]
    counts("Work asked of the printer", directory, everything)
    small("Small queries, built package", directory, "ordinary-{}.json", ["base", "head"])
    small("Fragment workloads, built package", directory, "ordinary-fragments-{}.json", ["base", "head", "fix-fastpath", "fix-with-arguments-only"])
    benchmarks("The project's own benchmarks at the head, run against each built package", directory, "project-benchmark-*-{}.json", ["base", "head"])
    benchmarks("The benchmark the maintainers added later (pull request 3967), run against each built package", directory, "later-benchmark-*-{}.json", everything)
