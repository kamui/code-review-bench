#!/usr/bin/env python3
"""Check every quotation of 20 characters or more in the given Markdown files against the saved sources.

    python3 probes/check-quotes.py SOURCE_CHECKOUT FILE.md [FILE.md ...]

A quotation is text between straight double quotes on one line. The sources are packet.json, everything under
upstream/ and probes/, the task packet, and the Hono source of SOURCE_CHECKOUT. White space is collapsed before
comparing. Prints each quotation that no source contains."""
import json
import re
import sys
from pathlib import Path

here = Path(__file__).resolve().parent.parent
checkout = Path(sys.argv[1])


def strings(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for item in value.values():
            yield from strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from strings(item)


def squash(text):
    return re.sub(r"\s+", " ", text)


corpus = []
for path in [here / "packet.json", *sorted((here / "upstream").rglob("*")), *sorted((here / "probes").rglob("*"))]:
    if not path.is_file() or path.name == "check-quotes.py":
        continue
    text = path.read_text(encoding="utf-8", errors="replace")
    if path.suffix == ".json":
        try:
            text = "\n".join(strings(json.loads(text)))
        except json.JSONDecodeError:
            pass
    corpus.append(text)
corpus.append((here.parents[3] / "bench/targets/p-hono-5067/packet.v2.md").read_text(encoding="utf-8"))
for path in sorted((checkout / "src").rglob("*.ts")):
    corpus.append(path.read_text(encoding="utf-8"))
whole = squash("\n".join(corpus))

missing = 0
for name in sys.argv[2:]:
    text = Path(name).read_text(encoding="utf-8")
    quotes = set()
    fenced = False
    for line in text.splitlines():
        if line.startswith("```"):
            fenced = not fenced
            continue
        if fenced:
            continue
        line = line.replace('\\"', "\u0001")
        for quote in line.split('"')[1::2]:
            if len(quote) >= 20:
                quotes.add(quote.replace("\u0001", '"'))
    found = 0
    for quote in sorted(quotes):
        if squash(quote) in whole:
            found += 1
        else:
            missing += 1
            print(f"{name}: NOT FOUND: {quote[:160]}")
    print(f"{name}: {found} of {len(quotes)} quotations found in the sources")
raise SystemExit(1 if missing else 0)
