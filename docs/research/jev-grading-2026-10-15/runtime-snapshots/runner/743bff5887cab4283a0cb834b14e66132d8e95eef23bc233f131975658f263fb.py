"""Run frozen Jev requests once, with a shared conservative spend ledger."""

from __future__ import annotations

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
from decimal import Decimal
import fcntl
import hashlib
import json
import math
import os
from pathlib import Path
import re
import sys
import time
from urllib import error, request

EXPERIMENT = Path(__file__).resolve().parents[1]
REPOSITORY = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(REPOSITORY / "bench/tools"))
import clean_context

ENDPOINT = "https://api.typesafe.ai/v1/systemone"
MODEL = "jev-1.13.0"
CAP = Decimal("4.95")
INPUT_PRICE = Decimal("0.042") / Decimal(1_000_000)
RESERVATION_TOKENS = 131_072
RESERVATION = RESERVATION_TOKENS * INPUT_PRICE
OPTIONS = {"yes", "no", "cannot_tell"}


class Refusal(ValueError):
    pass


def now() -> str:
    return datetime.now(timezone.utc).isoformat()


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encode(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, allow_nan=False, sort_keys=True).encode()


def parse(data: bytes) -> object:
    def constant(_: str) -> None:
        raise Refusal("non-finite JSON number")

    def unique(pairs: list) -> dict:
        result = {}
        for key, value in pairs:
            if key in result:
                raise Refusal("duplicate JSON key")
            result[key] = value
        return result

    return json.loads(data.decode("utf-8"), parse_constant=constant, object_pairs_hook=unique)


def write_new(path: Path, data: bytes) -> None:
    with path.open("xb") as handle:
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())


def write_json(path: Path, value: object) -> None:
    write_new(path, encode(value) + b"\n")


def validate_request(raw: bytes) -> dict:
    body = parse(raw)
    if not isinstance(body, dict) or set(body) != {"state", "model", "questions"}:
        raise Refusal("request must contain only state, model, questions")
    if body["model"] != MODEL or not isinstance(body["state"], (str, dict, list)):
        raise Refusal("request model or state is invalid")
    questions = body["questions"]
    if not isinstance(questions, dict) or not questions:
        raise Refusal("questions must be a nonempty map")
    for question in questions.values():
        if not isinstance(question, dict) or set(question) != {"type", "instructions", "criteria"}:
            raise Refusal("question must contain only type, instructions, criteria")
        if question["type"] != "choice" or not isinstance(question["instructions"], (str, dict, list)):
            raise Refusal("only choice questions with explicit instructions are supported")
        criteria = question["criteria"]
        if not isinstance(criteria, dict) or set(criteria) != OPTIONS:
            raise Refusal("choice criteria must be yes, no, cannot_tell")
        if any(value is not None and not isinstance(value, (str, dict, list)) for value in criteria.values()):
            raise Refusal("invalid choice criterion")
        if len(encode({"state": body["state"], "question": question})) > 30_000:
            raise Refusal("state plus question exceeds conservative 30000-byte context limit")
    if len(raw) > 60_000 or len(encode(body)) > 60_000:
        raise Refusal("request exceeds conservative 60000-byte context limit")
    return body


def validate_response(raw: bytes, questions: dict) -> dict:
    body = parse(raw)
    if not isinstance(body, dict) or body.get("model") != MODEL:
        raise Refusal("response model mismatch")
    usage = body.get("usage")
    if not isinstance(usage, dict) or any(type(usage.get(key)) is not int or usage[key] < 0
                                          for key in ("input_tokens", "output_tokens")):
        raise Refusal("response lacks valid token usage")
    if usage["input_tokens"] > RESERVATION_TOKENS:
        raise Refusal("response input tokens exceed reservation")
    answers = body.get("answers")
    if not isinstance(answers, dict) or set(answers) != set(questions):
        raise Refusal("response question IDs mismatch")
    for answer in answers.values():
        if not isinstance(answer, dict) or answer.get("type") != "choice" or answer.get("choice") not in OPTIONS:
            raise Refusal("invalid choice answer")
        probabilities = answer.get("probabilities")
        if not isinstance(probabilities, dict) or set(probabilities) != OPTIONS:
            raise Refusal("invalid choice probabilities")
        values = [*probabilities.values(), answer.get("confidence")]
        if any(type(value) not in (int, float) or not math.isfinite(value) or not 0 <= value <= 1
               for value in values):
            raise Refusal("invalid probability or confidence")
        if abs(sum(probabilities.values()) - 1) > 0.015000001:
            raise Refusal("probabilities do not sum to one")
        if probabilities[answer["choice"]] < max(probabilities.values()):
            raise Refusal("choice is not a highest-probability answer")
    return body


class Ledger:
    def __init__(self, directory: Path):
        self.directory = directory
        self.events_path = directory / "events.jsonl"

    @contextmanager
    def locked(self):
        self.directory.mkdir(parents=True, exist_ok=True)
        with (self.directory / "lock").open("a") as handle:
            try:
                fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError:
                raise Refusal("another runner owns the global spend ledger") from None
            try:
                yield
            finally:
                fcntl.flock(handle, fcntl.LOCK_UN)

    def events(self) -> list[dict]:
        if not self.events_path.exists():
            return []
        return [parse(line) for line in self.events_path.read_bytes().splitlines()]

    def append(self, event: dict) -> None:
        with self.events_path.open("ab") as handle:
            handle.write(encode({"timestamp": now(), **event}) + b"\n")
            handle.flush()
            os.fsync(handle.fileno())

    def accounting(self) -> dict:
        obligations = {}
        actual = Decimal(0)
        for event in self.events():
            identity = event["request_sha256"]
            if event["event"] == "reserved":
                if identity in obligations:
                    raise Refusal("duplicate reservation in ledger")
                obligations[identity] = Decimal(event["reserved_usd"])
            elif event["event"] == "completed":
                if identity not in obligations:
                    raise Refusal("completion without reservation")
                cost = Decimal(event["cost_usd"])
                if cost > obligations[identity]:
                    raise Refusal("completion exceeds reservation")
                obligations[identity] = cost
                actual += cost
            elif event["event"] != "unknown_charge":
                raise Refusal("unknown ledger event")
        committed = sum(obligations.values(), Decimal(0))
        return {"cap_usd": str(CAP), "committed_usd": str(committed),
                "actual_usd": str(actual), "unresolved_reserved_usd": str(committed - actual),
                "remaining_usd": str(CAP - committed)}

    def reserve(self, identity: str, metadata: dict) -> None:
        if any(event["request_sha256"] == identity for event in self.events()):
            raise Refusal("request already attempted; automatic retries are forbidden")
        if Decimal(self.accounting()["committed_usd"]) + RESERVATION > CAP:
            raise Refusal("global TypeSafe budget cannot cover a conservative reservation")
        self.append({"event": "reserved", "request_sha256": identity,
                     "reserved_usd": str(RESERVATION), "reservation_tokens": RESERVATION_TOKENS, **metadata})


class NoRedirect(request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def post(body: bytes, credential: str) -> tuple[int, bytes]:
    req = request.Request(ENDPOINT, data=body, method="POST",
                          headers={"Authorization": "Bearer " + credential, "Content-Type": "application/json"})
    opener = request.build_opener(request.ProxyHandler({}), NoRedirect())
    try:
        with opener.open(req, timeout=120) as response:
            return response.status, response.read()
    except error.HTTPError as failure:
        return failure.code, failure.read()


def load_manifest(path: Path, expected_sha: str) -> tuple[bytes, dict, list]:
    raw = path.read_bytes()
    if sha(raw) != expected_sha:
        raise Refusal("manifest hash mismatch")
    manifest = parse(raw)
    if not isinstance(manifest, dict) or manifest.get("schema_version") != 1:
        raise Refusal("manifest schema version must be 1")
    if manifest.get("mode") not in {"pilot", "development", "qualification"}:
        raise Refusal("invalid manifest mode")
    entries = manifest.get("requests")
    if not isinstance(entries, list) or not entries:
        raise Refusal("manifest requests must be a nonempty list")
    loaded = []
    ids = set()
    hashes = set()
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != {"id", "path", "sha256"}:
            raise Refusal("manifest entry must contain only id, path, sha256")
        identity = entry["id"]
        if not isinstance(identity, str) or not re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9._-]{0,119}", identity):
            raise Refusal("invalid request ID")
        if identity in ids or entry["sha256"] in hashes:
            raise Refusal("duplicate request in manifest")
        ids.add(identity)
        hashes.add(entry["sha256"])
        relative = Path(entry["path"])
        if relative.is_absolute() or ".." in relative.parts:
            raise Refusal("request path must be within manifest directory")
        source = (path.parent / relative).resolve()
        if not source.is_relative_to(path.parent.resolve()):
            raise Refusal("request symlink escapes manifest directory")
        request_raw = source.read_bytes()
        if sha(request_raw) != entry["sha256"]:
            raise Refusal("request hash mismatch")
        loaded.append((entry, request_raw, validate_request(request_raw)))
    return raw, manifest, loaded


def run(manifest_path: Path, manifest_sha: str, output: Path, *,
        ledger_directory: Path = EXPERIMENT / "spend-ledger", credential_env: str = "TYPESAFE_API_KEY",
        transport=post) -> dict:
    raw, manifest, loaded = load_manifest(manifest_path, manifest_sha)
    credential = os.environ.get(credential_env)
    if not credential:
        raise Refusal("required credential environment variable is missing")
    credential_bytes = credential.encode()
    if credential_bytes in raw or any(credential_bytes in item[1] for item in loaded):
        raise Refusal("credential appears in frozen input")
    ledger = Ledger(ledger_directory)
    with ledger.locked():
        for entry, _, _ in loaded:
            if any(event["request_sha256"] == entry["sha256"] for event in ledger.events()):
                raise Refusal("request already attempted; automatic retries are forbidden")
        output.mkdir(parents=True, exist_ok=False)
        write_new(output / "manifest.json", raw)
        results = []
        for entry, request_raw, body in loaded:
            attempt = output / entry["id"]
            attempt.mkdir()
            context = clean_context.prepare(attempt)
            write_new(attempt / "request.json", request_raw)
            metadata = {"request_id": entry["id"], "manifest_sha256": manifest_sha,
                        "mode": manifest["mode"], "attempt": str(attempt.resolve()),
                        "context_id": context["context_id"], "requested_model": MODEL,
                        "runner_sha256": sha(Path(__file__).read_bytes()),
                        "clean_context_sha256": sha(Path(clean_context.__file__).read_bytes())}
            ledger.reserve(entry["sha256"], metadata)
            started = time.monotonic()
            result = {**metadata, "request_sha256": entry["sha256"], "started_at": now(),
                      "status": "unknown_charge", "reserved_usd": str(RESERVATION)}
            try:
                status, response_raw = transport(request_raw, credential)
                result["http_status"] = status
                result["response_sha256"] = sha(response_raw)
                if credential_bytes in response_raw:
                    write_new(attempt / "response.redacted", response_raw.replace(credential_bytes, b"[REDACTED]"))
                    raise Refusal("credential appeared in response; raw bytes were not persisted")
                write_new(attempt / "response.json", response_raw)
                if status != 200:
                    raise Refusal("HTTP response was not successful")
                response = validate_response(response_raw, body["questions"])
                cost = response["usage"]["input_tokens"] * INPUT_PRICE
                result.update(status="completed", resolved_model=response["model"],
                              usage=response["usage"], cost_usd=str(cost))
            except Exception as failure:
                result["failure_type"] = type(failure).__name__
            result.update(ended_at=now(), elapsed_seconds=round(time.monotonic() - started, 6))
            write_json(attempt / "result.json", result)
            ledger.append({"event": result["status"], **result})
            results.append(result)
            if result["status"] != "completed":
                break
        summary = {"schema_version": 1, "manifest_sha256": manifest_sha, "mode": manifest["mode"],
                   "results": results, "accounting": ledger.accounting(),
                   "unattempted": len(loaded) - len(results)}
        write_json(output / "summary.json", summary)
        return summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--manifest-sha256", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--credential-env", default="TYPESAFE_API_KEY")
    args = parser.parse_args()
    try:
        summary = run(args.manifest, args.manifest_sha256, args.output, credential_env=args.credential_env)
    except Exception as failure:
        print("runner refused: " + (str(failure) if isinstance(failure, Refusal) else type(failure).__name__), file=sys.stderr)
        return 2
    print(json.dumps({"summary": str(args.output / "summary.json"), "accounting": summary["accounting"]}))
    return 0 if summary["unattempted"] == 0 and all(item["status"] == "completed" for item in summary["results"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
