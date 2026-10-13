#!/usr/bin/env python3
"""Check every quotation in a case, dossier or card against its sources.

    python3 check-quotes.py FILE.md REPOSITORY_ROOT CHECKOUT_AT_HEAD

A quotation is text between straight double quotes, at least 12 characters long. Each must appear, after
whitespace is collapsed and comment markers are dropped, in one of: the saved upstream responses, the answer
key and its records, the task packet, or the checkout at the head. The script prints each quotation it
cannot find.
"""
import json
import re
import sys
from pathlib import Path

text_path, root, checkout = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
here = Path(__file__).resolve().parents[2]


def squash(text):
    text = re.sub(r"(?m)^\s*(//|\*|/\*\*?)\s?", " ", text)
    return re.sub(r"\s+", " ", text)


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for item in value.values():
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)


corpus = []
for path in sorted((here / "upstream").iterdir()):
    raw = path.read_text(encoding="utf-8", errors="replace")
    if path.suffix == ".json":
        corpus.extend(strings(json.loads(raw)))
    else:
        corpus.append(raw)
corpus.extend(strings(json.loads((here / "packet.json").read_text())))
for extra in (
    "bench/targets/w-graphql-js-3457/register.v1.json",
    "docs/research/cohort-rebuild-2026-10-05/second-pass/answer-key-causes/causes.v1.json",
):
    corpus.extend(strings(json.loads((root / extra).read_text())))
for extra in (
    "bench/targets/w-graphql-js-3457/packet.md",
    "bench/grading/rulings/reference-calibration.v2.md",
    "docs/research/codex-calibration-2026-10-02/control-graphql-audit.v1.md",
    "docs/research/cohort-rebuild-2026-10-05/second-pass/terms/two-questions.v8.md",
    "docs/research/impact-boundary-2026-10-04/impact-boundary.v4.md",
):
    corpus.append((root / extra).read_text())
for extra in (
    "README.md",
    "docs/APIReference-Validation.md",
    "src/validation/validate.ts",
    "src/validation/rules/OverlappingFieldsCanBeMergedRule.ts",
    "src/validation/__tests__/OverlappingFieldsCanBeMergedRule-test.ts",
    "benchmark/validateGQL-benchmark.js",
    "benchmark/validateInvalidGQL-benchmark.js",
):
    corpus.append((checkout / extra).read_text())
haystack = squash("\n".join(corpus))

text = text_path.read_text()
text = re.sub(r"```.*?```", "", text, flags=re.S)
found, missing = 0, []
for line in text.splitlines():
    # A quotation may itself hold quoted words, so try the widest span on the line first.
    spans = [m.span() for m in re.finditer(r'"', line)]
    positions = [s[0] for s in spans]
    used = -1
    i = 0
    while i < len(positions) - 1:
        matched = False
        for j in range(len(positions) - 1, i, -1):
            candidate = line[positions[i] + 1 : positions[j]]
            if len(candidate) >= 12 and squash(candidate).strip() in haystack:
                found += 1
                i = j + 1
                matched = True
                break
        if not matched:
            candidate = line[positions[i] + 1 : positions[i + 1]]
            if len(candidate) >= 12:
                missing.append(candidate)
            i += 2
print(f"{text_path.name}: {found} quotations found in their sources, {len(missing)} not found")
for item in missing:
    print("  NOT FOUND:", item[:160])
