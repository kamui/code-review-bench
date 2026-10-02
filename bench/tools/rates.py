#!/usr/bin/env python3
"""Check or refresh dated standard, short-context prices from official provider Markdown.

Usage: python3 bench/tools/rates.py check|refresh [--model ID ...] [--rates FILE] [--json]
The catalog uses bench/schema/rates.schema.json. Refresh appends changed prices only;
historical catalogs and frozen runs cannot be refresh destinations. No model calls are made.
Exit codes: 0 verified/refreshed, 1 changed or unsupported prices, 2 unreadable input/source.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import math
from pathlib import Path
import re
import sys
import tempfile
import urllib.request

import check_manifest

BENCH = Path(__file__).resolve().parents[1]
CURRENT = BENCH / "rates.current.json"
FIELDS = ("input", "output", "cache_read", "cache_write_5m", "cache_write_1h")
SOURCES = {
    "openai": "https://developers.openai.com/api/docs/pricing.md",
    "anthropic": "https://docs.anthropic.com/en/docs/about-claude/pricing.md",
}


class RateError(ValueError):
    pass


def read_catalog(path: Path) -> dict:
    catalog = json.loads(path.read_text(encoding="utf-8"))
    schema = json.loads((BENCH / "schema/rates.schema.json").read_text(encoding="utf-8"))
    problems = check_manifest.validate(schema, catalog)
    if problems:
        raise RateError("; ".join(problems))
    seen = set()
    for row in catalog.get("rates", []):
        key = (row["model"], row["as_of"])
        if key in seen:
            problems.append(f"duplicate dated rate: {key}")
        seen.add(key)
        for field in FIELDS:
            if not math.isfinite(row[field]):
                problems.append(f"non-finite {field} for {row['model']}")
        datetime.strptime(row["as_of"], "%Y-%m-%d")
    if problems:
        raise RateError("; ".join(problems))
    return catalog


def latest(catalog: dict, models: list[str] | None = None) -> dict:
    rows = {}
    for row in sorted(catalog["rates"], key=lambda row: row["as_of"]):
        rows[row["model"]] = row
    selected = models or sorted(rows)
    missing = sorted(set(selected) - rows.keys())
    if missing:
        raise RateError(f"no catalog entry for {', '.join(missing)}; add a model with its billing policy first")
    if not selected:
        raise RateError("the rate catalog is empty")
    return {model: rows[model] for model in selected}


def table_rows(markdown: str, heading: str, header: list[str]) -> list[list[str]]:
    sections = re.split(r"(?m)^#{1,6} ", markdown)
    section = next((part for part in sections if part and part.splitlines()[0].strip() == heading), None)
    if section is None:
        raise RateError(f"provider pricing section is missing: {heading}")
    rows = [[cell.strip() for cell in line.strip().strip("|").split("|")]
            for line in section.splitlines() if line.strip().startswith("|")]
    if not rows or rows[0] != header:
        raise RateError(f"provider pricing columns changed: {heading}")
    return rows[2:]


def price(cell: str) -> float:
    match = re.fullmatch(r"\$([0-9]+(?:\.[0-9]+)?)(?: / MTok)?(?:<sup>\d+</sup>)?", cell)
    if not match:
        raise RateError(f"unsupported provider price: {cell!r}")
    return float(match[1])


def parse_prices(provider: str, markdown: str, models: list[str]) -> dict:
    if provider == "openai":
        header = ["Model", "Short context input", "Short context cached input", "Short context cache writes",
                  "Short context output", "Long context input", "Long context cached input",
                  "Long context cache writes", "Long context output"]
        rows = table_rows(markdown, "Standard pricing data", header)
        positions = (1, 4, 2, 3, 3)
    else:
        header = ["Model", "Base input tokens", "5m cache writes", "1h cache writes",
                  "Cache hits and refreshes", "Output tokens"]
        rows = table_rows(markdown, "Model pricing", header)
        positions = (1, 5, 4, 2, 3)
    result = {}
    for row in rows:
        name = row[0]
        if provider == "anthropic":
            name = re.sub(r"\s+\(.*", "", name).lower().replace(".", "-").replace(" ", "-")
        if name not in models:
            continue
        if len(row) != len(header) or name in result:
            raise RateError(f"ambiguous provider pricing row for {name}")
        result[name] = {field: price(row[index]) for field, index in zip(FIELDS, positions)}
    missing = sorted(set(models) - result.keys())
    if missing:
        raise RateError(f"provider has no supported standard short-context prices for {', '.join(missing)}")
    return result


def fetch(url: str) -> str:
    request = urllib.request.Request(url, headers={"User-Agent": "code-review-bench-rates/1"})
    with urllib.request.urlopen(request, timeout=20) as response:
        return response.read().decode("utf-8")


def inspect(catalog: dict, models: list[str] | None = None) -> dict:
    saved = latest(catalog, models)
    grouped = {provider: [] for provider in SOURCES}
    for model in saved:
        provider = "openai" if model.startswith("gpt-") else "anthropic" if model.startswith("claude-") else None
        if provider is None:
            raise RateError(f"unsupported rate provider for {model}")
        grouped[provider].append(model)
    observed, sources = {}, {}
    for provider, selected in grouped.items():
        if not selected:
            continue
        markdown = fetch(SOURCES[provider])
        sources[provider] = {"url": SOURCES[provider], "sha256": hashlib.sha256(markdown.encode()).hexdigest()}
        observed.update(parse_prices(provider, markdown, selected))
    changes = []
    for model, row in saved.items():
        fields = {field: {"saved": row[field], "current": observed[model][field]}
                  for field in FIELDS if row[field] != observed[model][field]}
        if fields:
            changes.append({"model": model, "fields": fields})
    return {"checked_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
            "sources": sources, "models": list(saved), "prices": observed, "changes": changes}


def refresh(path: Path, models: list[str] | None = None) -> dict:
    path = path.resolve()
    if path == BENCH / "rates.json" or path.is_relative_to(BENCH / "runs") or path.is_relative_to(BENCH.parent / "artifacts"):
        raise RateError("historical rate evidence is immutable; refresh bench/rates.current.json")
    with path.with_name("." + path.name + ".lock").open("a", encoding="utf-8") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        catalog = read_catalog(path)
        report = inspect(catalog, models)
        today = report["checked_at"][:10]
        saved = latest(catalog, models)
        for change in report["changes"]:
            model = change["model"]
            if saved[model]["as_of"] >= today:
                raise RateError(f"cannot append {model} prices on {today}; preserve the existing same-day or future entry")
            provider = "openai" if model.startswith("gpt-") else "anthropic"
            source = report["sources"][provider]
            catalog["rates"].append({**saved[model], **report["prices"][model], "as_of": today,
                                     "source": f"{source['url']} checked {today}; sha256 {source['sha256']}; standard short-context prices"})
        if report["changes"]:
            temporary = None
            try:
                with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent, delete=False) as stream:
                    temporary = Path(stream.name)
                    stream.write(json.dumps(catalog, indent=2) + "\n")
                temporary.chmod(path.stat().st_mode & 0o777)
                temporary.replace(path)
            finally:
                if temporary is not None:
                    temporary.unlink(missing_ok=True)
        report["refreshed"] = bool(report["changes"])
        return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("command", choices=("check", "refresh"))
    parser.add_argument("--rates", type=Path, default=CURRENT)
    parser.add_argument("--model", action="append")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    try:
        report = refresh(args.rates, args.model) if args.command == "refresh" else inspect(read_catalog(args.rates), args.model)
    except RateError as error:
        print(f"rates.py: {error}")
        return 1
    except (OSError, ValueError) as error:
        print(f"rates.py: {error}", file=sys.stderr)
        return 2
    if args.json:
        print(json.dumps(report, indent=2))
    else:
        for change in report["changes"]:
            for field, values in change["fields"].items():
                print(f"{change['model']} {field}: {values['saved']} -> {values['current']} USD per million tokens")
        print(f"{len(report['models'])} models checked; {len(report['changes'])} changed"
              + ("; catalog refreshed" if report.get("refreshed") else ""))
    return 1 if args.command == "check" and report["changes"] else 0


if __name__ == "__main__":
    sys.exit(main())
