#!/usr/bin/env python3
"""Local CLI adapters; production discovery and rendering remain unchanged.

Transcript coverage is shared Python validation plus the production parser, not
a Rust port. Prevalidation deliberately rereads the file before that parser.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
import subprocess
import sys
from typing import TYPE_CHECKING, TypedDict

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from bench.tools import codex_usage

if TYPE_CHECKING:
    from tools.usage_pilot import typed_usage

DEFAULT_BINARY = ROOT / "rust/usage-pilot/target/release/usage-pilot"
TRANSCRIPT_COUNTS = (
    "input_tokens", "output_tokens", "cache_creation_input_tokens", "cache_read_input_tokens",
)
_ADD = codex_usage.add


def add_checked(total: dict, part: dict) -> dict:
    for field in ("turns", "tool_calls", "text_only_turns", "input", "cache_write",
                  "cache_read", "output", "thinking"):
        if total.get(field, 0) + part[field] > (1 << 64) - 1:
            raise codex_usage.RolloutError(f"TOTAL: {field} exceeds u64::MAX")
    if not math.isfinite(total.get("cost", 0.0) + part["cost"]):
        raise codex_usage.RolloutError("TOTAL: cost must be finite and non-negative")
    return _ADD(total, part)


class TypedRollout(TypedDict):
    path: str
    rollout: typed_usage.Rollout


class RustRollout(TypedDict):
    path: str


def read_typed_rollout(path: Path) -> TypedRollout:
    from tools.usage_pilot import typed_usage

    return {"path": str(path), "rollout": typed_usage.read_rollout(path)}


def summarize_typed(
    rollout: TypedRollout, prices: tuple[float, float], cached_mult: float, write_mult: float,
) -> dict[str, typed_usage.JsonValue]:
    from tools.usage_pilot import typed_usage

    return typed_usage.summarize(rollout["rollout"], prices, cached_mult, write_mult).to_dict()


def read_rust_rollout(path: Path) -> RustRollout:
    return {"path": str(path)}


def summarize_rust(
    rollout: RustRollout, prices: tuple[float, float], cached_mult: float, write_mult: float,
    *, binary: Path = DEFAULT_BINARY,
) -> dict[str, typed_usage.JsonValue]:
    try:
        result = subprocess.run(
            [str(binary), rollout["path"], str(prices[0]), str(prices[1]),
             str(cached_mult), str(write_mult)],
            capture_output=True,
        )
    except OSError as error:
        raise codex_usage.RolloutError(str(error)) from None
    if result.returncode:
        try:
            error = json.loads(result.stderr)
        except (ValueError, UnicodeError):
            raise codex_usage.RolloutError(
                f"usage-pilot exited {result.returncode} without a structured error"
            ) from None
        if not isinstance(error, dict) or not isinstance(error.get("detail"), str):
            raise codex_usage.RolloutError("usage-pilot returned an invalid error")
        raise codex_usage.RolloutError(error["detail"])
    try:
        summary = json.loads(result.stdout)
    except (ValueError, UnicodeError):
        raise codex_usage.RolloutError("usage-pilot returned an invalid summary") from None
    if not isinstance(summary, dict):
        raise codex_usage.RolloutError("usage-pilot returned an invalid summary")
    return summary


def validate_transcript(path: str) -> None:
    from tools.usage_pilot import typed_usage

    requests: dict[str | tuple[str, int], int] = {}
    anonymous = 0
    try:
        with open(path, encoding="utf-8") as handle:
            for number, line in enumerate(handle, 1):
                if not line.strip():
                    continue
                try:
                    record = json.loads(line)
                except ValueError:
                    raise SystemExit(f"transcript_usage: {path}:{number}: malformed JSON") from None
                if not isinstance(record, dict) or record.get("type") != "assistant":
                    continue
                message = record.get("message")
                if not isinstance(message, dict):
                    continue
                usage = message.get("usage")
                if not isinstance(usage, dict):
                    continue
                if record.get("isApiErrorMessage") or message.get("model") == "<synthetic>":
                    continue
                key = record.get("requestId")
                if not isinstance(key, str) or not key:
                    anonymous += 1
                    key = ("line", anonymous)
                index = requests.setdefault(key, len(requests) + 1)
                location = f"{path}: request {index}"
                for field in TRANSCRIPT_COUNTS:
                    count = typed_usage.parse_count(usage.get(field), location, field)
                    typed_usage.require_count(count, location, field)
    except OSError as error:
        raise SystemExit(f"transcript_usage: cannot read {path}: {error.strerror}") from None
    except UnicodeError:
        raise SystemExit(f"transcript_usage: {path}: invalid UTF-8") from None
    except codex_usage.RolloutError as error:
        raise SystemExit(f"transcript_usage: {error}") from None


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("program", choices=("codex", "transcript"))
    parser.add_argument("--engine", choices=("typed", "rust"))
    parser.add_argument("--binary", type=Path)
    options, remaining = parser.parse_known_args(args)
    if options.program == "transcript":
        from bench.tools import transcript_usage

        if options.engine or options.binary:
            parser.error("transcript uses the shared Python adapter; no engine or binary applies")
        original = transcript_usage.read_transcript
        def read_validated(path: str):
            validate_transcript(path)
            return original(path)

        transcript_usage.read_transcript = read_validated
        try:
            return transcript_usage.main(remaining)
        finally:
            transcript_usage.read_transcript = original
    if options.engine is None:
        parser.error("codex requires --engine typed|rust")
    if options.binary and (options.engine != "rust" or not options.binary.is_absolute()):
        parser.error("--binary requires --engine rust and an absolute path")

    def rust_summary(
        rollout: RustRollout, prices: tuple[float, float], cached_mult: float, write_mult: float,
    ) -> dict[str, typed_usage.JsonValue]:
        return summarize_rust(rollout, prices, cached_mult, write_mult,
                              binary=options.binary or DEFAULT_BINARY)

    original_read, original_summary = codex_usage.read_rollout, codex_usage.summarize
    original_add = codex_usage.add
    original_argv = sys.argv
    codex_usage.read_rollout = read_typed_rollout if options.engine == "typed" else read_rust_rollout
    codex_usage.summarize = summarize_typed if options.engine == "typed" else rust_summary
    codex_usage.add = add_checked
    sys.argv = ["codex_usage.py", *remaining]
    try:
        return codex_usage.main()
    finally:
        codex_usage.read_rollout, codex_usage.summarize = original_read, original_summary
        codex_usage.add = original_add
        sys.argv = original_argv


if __name__ == "__main__":
    sys.exit(main())
