#!/usr/bin/env python3
"""Screen saved no-credit rulings under the draft of rule version 5 with two blind GLM answers each.

    python3 -I docs/research/rule-v5-rulings-2026-11-02/screen/run.py [KEY ...]

manifest.json lists each ruling's case and precedent sheet, the rule and the assessor text, all pinned; a changed pin
stops the run. Each session runs `pi -p` with `openrouter/z-ai/glm-5.3-flash` at thinking high, with built-in tools,
context files, skills, extensions, prompt templates and MCP off, an empty input, an empty directory, and a fresh home
and PI_CODING_AGENT_DIR whose `auth.json` holds only the OpenRouter entry, deleted when the session ends. The prompt is
the assessor text of `assessors/brief-credit.md` followed by `rule.md`, `earlier-rulings.md` and `cases/KEY.md`.
Answers go to `answers/KEY/<assessor>/`; a key whose answers exist is skipped."""
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
CACHE = Path.home() / ".cache/rule-v5-rulings-2026-11-02/screen"
PI = Path.home() / ".bun/bin/pi"
MODEL, FAMILY, EFFORT = "openrouter/z-ai/glm-5.3-flash", "glm", "high"
ASSESSORS = ("assessor-1", "assessor-2")
PARALLEL = 6
TIMEOUT = 1200
MARKER = "## Text for the assessor\n\n---\n\n"


def pinned(entry):
    path = ROOT / entry["path"]
    if hashlib.sha256(path.read_bytes()).hexdigest() != entry["sha256"]:
        raise SystemExit(f"{entry['path']} differs from the manifest's pin")
    return path.read_text(encoding="utf-8")


def prompt(manifest, entry):
    text = pinned(manifest["assessor_text"]).split(MARKER, 1)[1]
    files = [("rule.md", pinned(manifest["rule"])), ("earlier-rulings.md", pinned(entry["precedents"])),
             (f"cases/{entry['key']}.md", pinned(entry["case"]))]
    return text + "".join(f"\n\n# File `{name}`\n\n{body}" for name, body in files)


def start(manifest, entry, assessor):
    base = CACHE / f"{entry['key']}-{assessor}"
    if base.exists():
        raise SystemExit(f"refusing existing {base}")
    work, agent = base / "work", base / "home/agent"
    work.mkdir(parents=True)
    agent.mkdir(parents=True)
    auth = json.loads((Path.home() / ".pi/agent/auth.json").read_text(encoding="utf-8"))
    secret = agent / "auth.json"
    descriptor = os.open(secret, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
        json.dump({"openrouter": auth["openrouter"]}, handle)
    shutil.copyfile(Path.home() / ".pi/agent/models-store.json", agent / "models-store.json")
    env = {key: value for key, value in os.environ.items() if not key.startswith(("CLAUDE", "ANTHROPIC_", "PI_", "OPENROUTER"))}
    env.update(HOME=str(base / "home"), PI_CODING_AGENT_DIR=str(agent), PI_SKIP_VERSION_CHECK="1", PI_TELEMETRY="0")
    command = [str(PI), "-p", "--mode", "json", "--model", MODEL, "--thinking", EFFORT, "--no-tools", "--no-context-files",
               "--no-skills", "--no-extensions", "--no-prompt-templates", "--no-themes", "--no-mcp", "--no-session",
               "--no-approve", prompt(manifest, entry)]
    process = subprocess.Popen(command, cwd=work, env=env, stdin=subprocess.DEVNULL,
                               stdout=open(base / "out.jsonl", "wb"), stderr=open(base / "stderr.txt", "wb"))
    return process, secret, base, time.monotonic()


def finish(manifest, entry, assessor, base, code):
    text, models, usd = "", set(), 0.0
    for line in (base / "out.jsonl").read_bytes().split(b"\n"):
        try:
            event = json.loads(line)
        except (json.JSONDecodeError, UnicodeDecodeError):
            continue
        message = event.get("message") or {}
        if event.get("type") == "message_end" and message.get("role") == "assistant":
            text = "".join(block.get("text", "") for block in message.get("content", []) if block.get("type") == "text")
            models.add(f"{message.get('provider')}/{message.get('responseModel') or message.get('model')}")
            usd += ((message.get("usage") or {}).get("cost") or {}).get("total") or 0.0
    out = HERE / "answers" / entry["key"] / assessor
    out.mkdir(parents=True, exist_ok=False)
    session = {"exit": code, "models_observed": sorted(models), "usd": usd}
    try:
        reply = json.loads(re.search(r"\{.*\}", text, re.S).group(0))
        answers = [{**answer, "model": MODEL, "family": FAMILY, "effort": EFFORT, "brief": manifest["assessor_text"],
                    "precedents": entry["precedents"]} for answer in reply["answers"]]
        (out / "answers.json").write_text(json.dumps(answers, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
        (out / "notes.md").write_text(str(reply.get("notes", "")).strip() + "\n", encoding="utf-8")
        session["outcome"] = answers[0].get("outcome")
    except Exception as error:
        session["problem"] = str(error)[:300]
    (out / "session.json").write_text(json.dumps(session, indent=1) + "\n")
    print(f"{entry['key']} {assessor}: {session}", flush=True)


def main(keys):
    manifest = json.loads((HERE / "manifest.json").read_text(encoding="utf-8"))
    pinned(manifest["rule"])
    queue = [(entry, assessor) for entry in manifest["entries"] for assessor in ASSESSORS
             if (not keys or entry["key"] in keys) and not (HERE / "answers" / entry["key"] / assessor).exists()]
    running = []
    while queue or running:
        for item in list(running):
            entry, assessor, (process, secret, base, began) = item
            code = process.poll()
            if code is None and time.monotonic() - began > TIMEOUT:
                process.kill()
                code = process.wait()
            if code is not None:
                secret.unlink(missing_ok=True)
                finish(manifest, entry, assessor, base, code)
                running.remove(item)
        while queue and len(running) < PARALLEL:
            entry, assessor = queue.pop(0)
            running.append((entry, assessor, start(manifest, entry, assessor)))
        time.sleep(2)


if __name__ == "__main__":
    main(set(sys.argv[1:]))
