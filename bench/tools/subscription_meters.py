"""Read the pinned clients' subscription meters and say when a reading stops a queue. No model is called."""

from __future__ import annotations

from datetime import datetime, timezone
import json
import math
from pathlib import Path
import select
import subprocess
import time
import urllib.request

SIGN_IN_MARGIN_SECONDS = 1800


class Unreadable(Exception):
    """No reading says how much of each pinned client's quota is used."""


class SignInUnreadable(Unreadable):
    """The stored Claude sign-in is missing or is not JSON."""


def stored_sign_in() -> dict:
    path = Path.home() / ".claude/.credentials.json"
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise SignInUnreadable(f"cannot read {path}: {error}") from error


def json_object(value, what: str) -> dict:
    """``value`` as the JSON object a provider should have sent; one it left out or sent empty is empty."""
    if value and not isinstance(value, dict):
        raise ValueError(f"{what} is not an object")
    return value or {}


def meters(clients: dict, env: dict) -> dict:
    """Each pinned client's subscription meters, read from its provider; no model is called."""
    usage = {"at": datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")}
    if "claude" in clients:
        credentials = stored_sign_in()
        request = urllib.request.Request("https://api.anthropic.com/api/oauth/usage", headers={
            "Authorization": "Bearer " + credentials["claudeAiOauth"]["accessToken"], "anthropic-beta": "oauth-2025-04-20",
            "User-Agent": "claude-code/" + clients["claude"]["version"].split()[0]})
        with urllib.request.urlopen(request, timeout=30) as response:
            reading = json_object(json.load(response), "the Claude usage response")
        usage["claude"] = {key: reading.get(key) for key in ("five_hour", "seven_day")}
        usage["claude_sign_in_expires_at"] = credentials["claudeAiOauth"].get("expiresAt")
    if "codex" in clients:
        server = subprocess.Popen([clients["codex"]["path"], "app-server"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, text=True, bufsize=1, env=env)
        try:
            def send(row: dict) -> None:
                server.stdin.write(json.dumps(row) + "\n")
                server.stdin.flush()

            def receive(identifier: int) -> dict:
                deadline = time.monotonic() + 30
                while time.monotonic() < deadline:
                    if not select.select([server.stdout], [], [], 1)[0]:
                        continue
                    line = server.stdout.readline()
                    if not line:
                        raise RuntimeError("app-server closed before returning usage")
                    row = json_object(json.loads(line), "an app-server message")
                    if row.get("id") == identifier:
                        if "error" in row:
                            raise RuntimeError("app-server could not return usage")
                        return row["result"]
                raise TimeoutError("app-server usage timeout")

            send({"id": 1, "method": "initialize", "params": {"clientInfo": {"name": "bench-meter", "version": "1.0"}}})
            receive(1)
            send({"method": "initialized", "params": {}})
            send({"id": 2, "method": "account/rateLimits/read", "params": {}})
            result = json_object(receive(2), "the app-server usage result")
            usage["codex"] = {key: result.get(key) for key in ("ordinaryUsageAllowed", "rateLimits")}
        finally:
            server.terminate()
            server.wait(timeout=10)
    return usage


def used(usage: dict) -> dict:
    """The percentages each client's usage windows report; a window that reports no finite number is left out."""
    claude = json_object(usage.get("claude"), "the Claude reading")
    limits = json_object(json_object(usage.get("codex"), "the ChatGPT reading").get("rateLimits"), "the ChatGPT rate limits")
    windows = {"claude": [json_object(window, f"the Claude usage window {name}").get("utilization")
                          for name, window in claude.items()],
               "codex": [json_object(limits.get(key), f"the ChatGPT usage window {key}").get("usedPercent")
                         for key in ("primary", "secondary")]}
    return {client: [percent for percent in percents if type(percent) in (int, float) and math.isfinite(percent)]
            for client, percents in windows.items()}


def read_meters(clients: dict, env: dict) -> dict:
    """A reading that says how much of each pinned client's quota is used, or ``Unreadable``."""
    try:
        usage = meters(clients, env)
        silent = [client for client in clients if not used(usage)[client]]
        if silent:
            raise ValueError(f"no usage window was reported for {', '.join(silent)}")
        return usage
    except (OSError, ValueError, KeyError, TypeError, RuntimeError, subprocess.SubprocessError) as error:
        raise Unreadable(str(error)) from error


def sign_in_expiring(usage: dict, moment: float) -> bool:
    expires = usage.get("claude_sign_in_expires_at")
    return type(expires) in (int, float) and expires / 1000 - moment < SIGN_IN_MARGIN_SECONDS


def quota_stop(usage: dict, threshold: float, moment: float | None = None, work: str = "review") -> str | None:
    """The reason the reading stops the queue; ``work`` names what must not be running when the sign-in is renewed."""
    percents = used(usage)
    if any(percent >= threshold for percent in percents["claude"]):
        return "Claude usage reached the quota stop"
    if sign_in_expiring(usage, time.time() if moment is None else moment):
        return f"the stored Claude sign-in expires within half an hour; renew it while no {work} runs"
    codex = usage.get("codex")
    if codex is not None and not json_object(codex, "the ChatGPT reading").get("ordinaryUsageAllowed"):
        return "ChatGPT ordinary usage is unavailable"
    if any(percent >= threshold for percent in percents["codex"]):
        return "ChatGPT usage reached the quota stop"
    return None
