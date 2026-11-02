#!/usr/bin/env python3
"""Run the blind GLM assessors of round rule-v5-rulings-2026-11-02 for one question.

    python3 -I docs/research/rule-v5-rulings-2026-11-02/assessors/run.py QUESTION [BRIEF]

QUESTION names the case file `cases/QUESTION.md`. BRIEF, by default `brief-credit.md`, must name it in its file list, with every pin current. Two
sessions, `assessor-1` and `assessor-2`, each run `pi -p` with `openrouter/z-ai/glm-5.3-flash` at thinking high,
with built-in tools, context files, skills, extensions, prompt templates and MCP off, in an empty directory with a
fresh home and PI_CODING_AGENT_DIR whose `auth.json` holds only the OpenRouter entry, deleted when the session ends.
The prompt is the brief's assessor text followed by the files under the names the brief gives. Each session's raw
output goes to CACHE; its parsed answer goes to `answers/QUESTION/<assessor>/answers.json` with `notes.md`."""
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
CACHE = Path.home() / ".cache/rule-v5-rulings-2026-11-02"
PI = Path.home() / ".bun/bin/pi"
MODEL, FAMILY, EFFORT = "openrouter/z-ai/glm-5.3-flash", "glm", "high"
FILE_LINE = re.compile(r"^\s*- `([^`]+)` ← \[`[^`]+`\]\(([^)]+)\) \(sha256 `([0-9a-f]{64})`\)$")
MARKER = "## Text for the assessor\n\n---\n\n"


def brief(question, name):
    text = (HERE / name).read_text(encoding="utf-8")
    files = []
    for line in text.splitlines():
        match = FILE_LINE.match(line)
        if match:
            target, source, sha = match.groups()
            path = (HERE / source).resolve()
            if hashlib.sha256(path.read_bytes()).hexdigest() != sha:
                raise SystemExit(f"{source} differs from the brief's pin")
            files.append((target, path))
    if f"cases/{question}.md" not in {target for target, _ in files}:
        raise SystemExit(f"the brief names no cases/{question}.md")
    return text.split(MARKER, 1)[1], files


def run(question, assessor, name):
    instructions, files = brief(question, name)
    prompt = instructions + "".join(f"\n\n# File `{target}`\n\n{path.read_text(encoding='utf-8')}" for target, path in files)
    base = CACHE / f"{question}-{assessor}"
    work, agent = base / "work", base / "home/agent"
    if base.exists():
        raise SystemExit(f"refusing existing {base}")
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
               "--no-approve", prompt]
    try:
        done = subprocess.run(command, cwd=work, env=env, capture_output=True, timeout=1200)
    finally:
        secret.unlink(missing_ok=True)
    (base / "out.jsonl").write_bytes(done.stdout)
    (base / "stderr.txt").write_bytes(done.stderr)
    text, models, usd = "", set(), 0.0
    for line in done.stdout.split(b"\n"):
        try:
            event = json.loads(line)
        except (json.JSONDecodeError, UnicodeDecodeError):
            continue
        message = event.get("message") or {}
        if event.get("type") == "message_end" and message.get("role") == "assistant":
            text = "".join(block.get("text", "") for block in message.get("content", []) if block.get("type") == "text")
            models.add(f"{message.get('provider')}/{message.get('responseModel') or message.get('model')}")
            usd += ((message.get("usage") or {}).get("cost") or {}).get("total") or 0.0
    reply = json.loads(re.search(r"\{.*\}", text, re.S).group(0))
    pins = {"path": str((HERE / name).relative_to(HERE.parents[2])),
            "sha256": hashlib.sha256((HERE / name).read_bytes()).hexdigest()}
    answers = [{**answer, "model": MODEL, "family": FAMILY, "effort": EFFORT, "brief": pins} for answer in reply["answers"]]
    out = HERE / "answers" / question / assessor
    out.mkdir(parents=True, exist_ok=False)
    (out / "answers.json").write_text(json.dumps(answers, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    (out / "notes.md").write_text(str(reply.get("notes", "")).strip() + "\n", encoding="utf-8")
    (out / "session.json").write_text(json.dumps({"exit": done.returncode, "models_observed": sorted(models), "usd": usd}, indent=1) + "\n")
    print(f"{assessor}: exit {done.returncode}, models {sorted(models)}, ${usd:.4f}, {[a['outcome'] for a in answers]}")


if __name__ == "__main__":
    if len(sys.argv) not in (2, 3):
        raise SystemExit(__doc__)
    for assessor in ("assessor-1", "assessor-2"):
        run(sys.argv[1], assessor, sys.argv[2] if len(sys.argv) == 3 else "brief-credit.md")
