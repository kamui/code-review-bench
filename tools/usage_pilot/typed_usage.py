"""Isolated, checked Codex usage model for the migration comparison."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
import json
import math
from pathlib import Path
from typing import NewType, TypeAlias

from bench.tools.codex_usage import RolloutError

U64_MAX = (1 << 64) - 1
COUNT_FIELDS = (
    "input_tokens", "cached_input_tokens", "cache_write_input_tokens",
    "output_tokens", "reasoning_output_tokens",
)
TOOL_TYPES = {"function_call", "custom_tool_call", "local_shell_call"}
JsonValue: TypeAlias = (
    None | bool | int | float | str | list["JsonValue"] | dict[str, "JsonValue"]
)


KnownCount = NewType("KnownCount", int)

@dataclass(frozen=True, slots=True)
class UnknownCount:
    pass


UNKNOWN = UnknownCount()
Count: TypeAlias = KnownCount | UnknownCount


@dataclass(frozen=True, slots=True)
class ResponseId:
    value: str


@dataclass(frozen=True, slots=True)
class OrdinalFallback:
    value: str


@dataclass(frozen=True, slots=True)
class SessionId:
    value: str


ResponseIdentity: TypeAlias = ResponseId | OrdinalFallback


@dataclass(frozen=True, slots=True)
class TokenUsage:
    input_tokens: Count
    cached_input_tokens: Count
    cache_write_input_tokens: Count
    output_tokens: Count
    reasoning_output_tokens: Count


@dataclass(frozen=True, slots=True)
class RequestUsage:
    identity: ResponseIdentity
    tokens: TokenUsage
    tool_calls: KnownCount


@dataclass(frozen=True, slots=True)
class Rollout:
    path: str
    session_id: SessionId | None
    parent_thread_id: SessionId | None
    requests: tuple[RequestUsage, ...]
    models: frozenset[str]
    first: datetime | None
    last: datetime | None


@dataclass(frozen=True, slots=True)
class Summary:
    turns: int
    tool_calls: int
    text_only_turns: int
    input: int
    cache_write: int
    cache_read: int
    output: int
    thinking: int
    models: tuple[str, ...]
    wall_seconds: int | None
    cost: float

    def to_dict(self) -> dict[str, JsonValue]:
        return {
            "turns": self.turns, "tool_calls": self.tool_calls,
            "text_only_turns": self.text_only_turns, "input": self.input,
            "cache_write": self.cache_write, "cache_read": self.cache_read,
            "output": self.output, "thinking": self.thinking,
            "models": list(self.models), "wall_seconds": self.wall_seconds,
            "cost": self.cost,
        }


def parse_count(value: JsonValue, location: str, field: str) -> Count:
    if value is None:
        return UNKNOWN
    if type(value) is not int or not 0 <= value <= U64_MAX:
        raise RolloutError(f"{location}: {field} must be an integer in 0..=u64::MAX")
    return KnownCount(value)


def checked_add(left: int, right: int, location: str, field: str) -> int:
    result = left + right
    if result > U64_MAX:
        raise RolloutError(f"{location}: {field} exceeds u64::MAX")
    return result


def session_id(value: JsonValue, location: str, field: str) -> SessionId | None:
    if value is None:
        return None
    if not isinstance(value, str) or not value:
        raise RolloutError(f"{location}: {field} must be a nonempty string")
    return SessionId(value)


def response_identity(
    payload: dict[str, JsonValue], record: dict[str, JsonValue], location: str,
) -> ResponseIdentity:
    value = payload.get("response_id")
    if value is not None and value != "":
        if not isinstance(value, str):
            raise RolloutError(f"{location}: response_id must be a string")
        return ResponseId(value)
    ordinal = record.get("ordinal")
    if ordinal is not None and type(ordinal) not in (str, int):
        raise RolloutError(f"{location}: ordinal must be a string or integer")
    return OrdinalFallback(f"ordinal-{ordinal}")


def parse_stamp(value: JsonValue) -> datetime | None:
    if not isinstance(value, str) or not value:
        return None
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None


def read_rollout(path: Path) -> Rollout:
    requests: dict[ResponseIdentity, RequestUsage] = {}
    models: set[str] = set()
    current_session = parent = None
    have_meta = False
    pending_tools = 0
    first = last = None
    with open(path, encoding="utf-8") as handle:
        try:
            for number, line in enumerate(handle, 1):
                if not line.strip():
                    continue
                location = f"{path}:{number}"
                try:
                    record: JsonValue = json.loads(line)
                except json.JSONDecodeError as error:
                    raise RolloutError(f"{location}: malformed JSON ({error.msg})") from None
                except ValueError as error:
                    raise RolloutError(f"{location}: malformed JSON ({error})") from None
                if not isinstance(record, dict):
                    raise RolloutError(f"{location}: rollout line is not an object")
                kind = record.get("type")
                if kind not in ("session_meta", "turn_context", "response_item", "token_usage_record"):
                    continue
                payload = record.get("payload")
                if payload is None:
                    payload = {}
                if not isinstance(payload, dict):
                    raise RolloutError(f"{location}: payload is not an object")
                if kind == "session_meta":
                    native_id = session_id(payload.get("id"), location, "id")
                    legacy_id = session_id(payload.get("session_id"), location, "session_id")
                    parent_id = session_id(payload.get("parent_thread_id"), location, "parent_thread_id")
                    if not have_meta and payload:
                        current_session = native_id or legacy_id
                        parent = parent_id
                        have_meta = True
                elif kind == "turn_context":
                    model = payload.get("model")
                    if model is not None and not isinstance(model, str):
                        raise RolloutError(f"{location}: model must be a string")
                    if model:
                        models.add(model)
                elif kind == "response_item" and isinstance(payload.get("type"), str) and payload["type"] in TOOL_TYPES:
                    pending_tools = checked_add(pending_tools, 1, location, "tool_calls")
                elif kind == "token_usage_record":
                    usage = payload.get("usage")
                    if not isinstance(usage, dict):
                        raise RolloutError(f"{location}: token_usage_record without usage object")
                    identity = response_identity(payload, record, location)
                    tokens = TokenUsage(*(parse_count(usage.get(field), location, field) for field in COUNT_FIELDS))
                    previous = requests.get(identity)
                    if previous is None:
                        calls = KnownCount(pending_tools)
                        pending_tools = 0
                    else:
                        calls = previous.tool_calls
                    requests[identity] = RequestUsage(identity, tokens, calls)
                    stamp = parse_stamp(record.get("timestamp"))
                    if stamp is not None:
                        try:
                            first = stamp if first is None or stamp < first else first
                            last = stamp if last is None or stamp > last else last
                        except TypeError:
                            raise RolloutError(f"{location}: incompatible timestamps") from None
        except UnicodeError:
            raise RolloutError(f"{path}: invalid UTF-8") from None
    return Rollout(str(path), current_session, parent, tuple(requests.values()), frozenset(models), first, last)


def require_count(count: Count, location: str, field: str) -> int:
    if isinstance(count, UnknownCount):
        raise RolloutError(f"{location} missing {field}")
    return count


def finite_rate(value: float, name: str) -> float:
    try:
        result = float(value)
    except (TypeError, ValueError, OverflowError):
        raise RolloutError(f"{name} must be finite and non-negative") from None
    if isinstance(value, bool) or not math.isfinite(result) or result < 0:
        raise RolloutError(f"{name} must be finite and non-negative")
    return result


def summarize(
    rollout: Rollout, prices: tuple[float, float], cached_mult: float, write_mult: float,
) -> Summary:
    in_price = finite_rate(prices[0], "input price")
    out_price = finite_rate(prices[1], "output price")
    cached_mult = finite_rate(cached_mult, "cached multiplier")
    write_mult = finite_rate(write_mult, "cache-write multiplier")
    ordinary = cached = write = output = reasoning = calls = text_only = 0
    for index, request in enumerate(rollout.requests, 1):
        location = f"{rollout.path}: request {index}"
        tokens = request.tokens
        inp = require_count(tokens.input_tokens, location, "input_tokens")
        c = require_count(tokens.cached_input_tokens, location, "cached_input_tokens")
        w = require_count(tokens.cache_write_input_tokens, location, "cache_write_input_tokens")
        if c + w > inp:
            raise RolloutError(f"{location} has cached+cache_write ({c}+{w}) above input_tokens ({inp})")
        out = require_count(tokens.output_tokens, location, "output_tokens")
        thinking = require_count(tokens.reasoning_output_tokens, location, "reasoning_output_tokens")
        ordinary = checked_add(ordinary, inp - c - w, rollout.path, "input")
        cached = checked_add(cached, c, rollout.path, "cache_read")
        write = checked_add(write, w, rollout.path, "cache_write")
        output = checked_add(output, out, rollout.path, "output")
        reasoning = checked_add(reasoning, thinking, rollout.path, "thinking")
        calls = checked_add(calls, request.tool_calls, rollout.path, "tool_calls")
        text_only = checked_add(text_only, int(request.tool_calls == 0), rollout.path, "text_only_turns")
    turns = checked_add(0, len(rollout.requests), rollout.path, "turns")
    cost = (ordinary * in_price + cached * in_price * cached_mult
            + write * in_price * write_mult + output * out_price) / 1_000_000
    if not math.isfinite(cost):
        raise RolloutError(f"{rollout.path}: cost must be finite and non-negative")
    wall = None
    if rollout.first is not None and rollout.last is not None:
        wall = int((rollout.last - rollout.first).total_seconds())
    return Summary(turns, calls, text_only, ordinary, write, cached, output, reasoning,
                   tuple(sorted(rollout.models)), wall, cost)
