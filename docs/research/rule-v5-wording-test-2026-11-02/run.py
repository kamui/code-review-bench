#!/usr/bin/env python3
"""Blind wording test of the draft "A result one step away counts" rule (rule version 5, piece 1).

    python3 -I docs/research/rule-v5-wording-test-2026-11-02/run.py launch [SESSION ...]
    python3 -I docs/research/rule-v5-wording-test-2026-11-02/run.py check

VERSION names the directory holding rule.md and the answers. The first wording ran from the cache root, before VERSION existed.

Each session gets one prompt holding the instructions, rule.md and one case from cases/, and no tools. A session
starts in its own empty directory under CACHE with a fresh home, outside every git repository.

Claude sessions run `claude -p --safe-mode --tools ""` with no setting sources or MCP servers. Only
`.credentials.json` is copied into the home, and it is deleted when the session ends. GLM sessions run `pi -p` with
built-in tools, context files, skills, extensions, prompt templates and MCP off, under a fresh PI_CODING_AGENT_DIR whose
`auth.json` holds only the OpenRouter entry, deleted when the session ends.

`launch` refuses a session that could take the Claude or OpenRouter spend past LIMITS, counting each session still
running at its budget. `check` parses each answer and writes answers.json and usage.json in the VERSION directory."""
import json
import os
import re
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
# The rule wording under test; v1 is the first wording, v2 limits the step to what a named thing is for.
VERSION = "v2"
CACHE = Path.home() / ".cache/rule-v5-wording-test-2026-11-02" / VERSION
SESSIONS, LOGS = CACHE / "sessions", CACHE / "logs"
CLAUDE = Path.home() / ".local/bin/claude"
PI = Path.home() / ".bun/bin/pi"
MODELS = {
    "opus": ("claude", "claude-opus-5-5"),
    "sonnet": ("claude", "claude-sonnet-5-5"),
    "glm": ("pi", "openrouter/z-ai/glm-5.3-flash"),
}
EFFORT = "high"
# The fields `bench-tools clean-context claude-account` keeps (rust/bench-tools/src/clean_context.rs).
ACCOUNT_KEYS = ("oauthAccount", "userID", "installMethod", "autoUpdates", "numStartups")
BUDGET = {"opus": 0.6, "sonnet": 0.3, "glm": 0.1}
LIMITS = {"claude": 10.0, "pi": 2.0}
REPEATS = 2
PARALLEL = 6

INSTRUCTIONS = """You are checking one sentence of a code review against one rule of a grading rubric. Use only the rule and the case below. You have no tools.

Question: under the rule, does the sentence the case is about say what goes wrong for the known problem? Answer yes, no or cannot-tell.

Reply with only one JSON object and no other text:

{"case": "<the case id from the case title>", "says_what": "yes" | "no" | "cannot-tell", "basis": "stated" | "shown" | "one-step" | "none", "one_step": {"sentence": "<the fault the claim states>, so <result>", "project_fact": "<the project fact used, such as what a named thing is for, or none>"} | null, "reason": "<two to four sentences>"}

basis: "stated" when the claim says the result in its own words; "shown" when it shows the failure as the rule describes; "one-step" when the rule "A result one step away counts" gives the answer yes; "none" when the answer is no or cannot-tell. Give one_step whenever you considered that rule, and null otherwise.
"""


def cases():
    return sorted(path.stem for path in (HERE / "cases").glob("*.md"))


def sessions():
    for case in cases():
        for model in MODELS:
            for repeat in range(1, REPEATS + 1):
                yield f"{case}-{model}-{repeat}", case, model


def prompt(case):
    rule = (HERE / VERSION / "rule.md").read_text(encoding="utf-8")
    text = (HERE / "cases" / f"{case}.md").read_text(encoding="utf-8")
    return f"{INSTRUCTIONS}\n# The rule\n\n{rule}\n\n# The case\n\n{text}"


def launched():
    return sorted(path.name.removesuffix(".out") for path in LOGS.glob("*.out"))


def cost(name):
    model = name.split("-")[-2]
    client = MODELS[model][0]
    out = LOGS / f"{name}.out"
    if not out.exists():
        return None
    if client == "claude":
        try:
            return json.loads(out.read_text(encoding="utf-8")).get("total_cost_usd")
        except json.JSONDecodeError:
            return None
    total, seen = 0.0, False
    for event in events(out):
        message = event.get("message") or {}
        if event.get("type") == "message_end" and message.get("role") == "assistant":
            total += ((message.get("usage") or {}).get("cost") or {}).get("total") or 0.0
            seen = True
    return total if seen else None


def spent(client, running):
    total = 0.0
    for name in launched():
        model = name.split("-")[-2]
        if MODELS[model][0] != client:
            continue
        paid = None if name in running else cost(name)
        total += BUDGET[model] if paid is None else paid
    return total


def events(path):
    for line in path.read_bytes().split(b"\n"):
        try:
            yield json.loads(line)
        except (json.JSONDecodeError, UnicodeDecodeError):
            continue


def start(name, case, model):
    client, model_id = MODELS[model]
    directory, home = SESSIONS / name / "work", SESSIONS / name / "home"
    for path in (directory, home):
        if path.exists():
            raise SystemExit(f"refusing existing {path}")
        path.mkdir(parents=True)
    LOGS.mkdir(parents=True, exist_ok=True)
    text = prompt(case)
    env = {key: value for key, value in os.environ.items() if not key.startswith(("CLAUDE", "ANTHROPIC_", "PI_", "OPENROUTER"))}
    env["HOME"] = str(home)
    if client == "claude":
        (home / ".claude").mkdir()
        account = json.loads((Path.home() / ".claude.json").read_text(encoding="utf-8"))
        kept = {key: account[key] for key in ACCOUNT_KEYS if key in account}
        (home / ".claude.json").write_text(json.dumps({**kept, "hasCompletedOnboarding": True}, indent=2), encoding="utf-8")
        secret = home / ".claude/.credentials.json"
        shutil.copyfile(Path.home() / ".claude/.credentials.json", secret)
        os.chmod(secret, 0o600)
        command = [str(CLAUDE), "-p", "--safe-mode", "--model", model_id, "--effort", EFFORT, "--session-id", str(uuid.uuid4()),
                   "--max-budget-usd", str(BUDGET[model]), "--setting-sources", "", "--strict-mcp-config", "--tools", "",
                   "--output-format", "json"]
        stdin = text.encode("utf-8")
    else:
        agent = home / "agent"
        agent.mkdir()
        auth = json.loads((Path.home() / ".pi/agent/auth.json").read_text(encoding="utf-8"))
        secret = agent / "auth.json"
        descriptor = os.open(secret, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump({"openrouter": auth["openrouter"]}, handle)
        shutil.copyfile(Path.home() / ".pi/agent/models-store.json", agent / "models-store.json")
        env.update(PI_CODING_AGENT_DIR=str(agent), PI_SKIP_VERSION_CHECK="1", PI_TELEMETRY="0")
        command = [str(PI), "-p", "--mode", "json", "--model", model_id, "--thinking", EFFORT, "--no-tools", "--no-context-files",
                   "--no-skills", "--no-extensions", "--no-prompt-templates", "--no-themes", "--no-mcp", "--no-session",
                   "--no-approve", text]
        stdin = b""
    shown = [part if part != text else "<prompt>" for part in command]
    (LOGS / f"{name}.command.json").write_text(json.dumps({"command": shown, "HOME": env["HOME"], "cwd": str(directory)}, indent=1) + "\n")
    (LOGS / f"{name}.prompt.md").write_text(text, encoding="utf-8")
    process = subprocess.Popen(command, cwd=directory, env=env, stdin=subprocess.PIPE,
                               stdout=open(LOGS / f"{name}.out", "wb"), stderr=open(LOGS / f"{name}.stderr", "wb"))
    process.stdin.write(stdin)
    process.stdin.close()
    return process, secret


def launch(names):
    queue = list(sessions())
    if names:
        queue = [entry for entry in queue if entry[0] in names]
    queue = [entry for entry in queue if entry[0] not in launched()]
    running = {}
    while queue or running:
        for name, (process, secret) in list(running.items()):
            if process.poll() is not None:
                secret.unlink(missing_ok=True)
                print(f"{name}: exit {process.returncode}", flush=True)
                del running[name]
        while queue and len(running) < PARALLEL:
            name, case, model = queue[0]
            client = MODELS[model][0]
            if spent(client, running) + BUDGET[model] > LIMITS[client]:
                print(f"stop: {name} could pass the {client} limit of ${LIMITS[client]}", flush=True)
                queue = [entry for entry in queue if MODELS[entry[2]][0] != client]
                continue
            queue.pop(0)
            running[name] = start(name, case, model)
            print(f"{name}: started", flush=True)
        time.sleep(2)
    for client in LIMITS:
        print(f"{client}: ${spent(client, {}):.4f} at list price")


def final_text(name, client):
    out = LOGS / f"{name}.out"
    if client == "claude":
        return json.loads(out.read_text(encoding="utf-8")).get("result", "")
    text = ""
    for event in events(out):
        message = event.get("message") or {}
        if event.get("type") == "message_end" and message.get("role") == "assistant":
            text = "".join(block.get("text", "") for block in message.get("content", []) if block.get("type") == "text")
    return text


def observed(name, client):
    out = LOGS / f"{name}.out"
    if client == "claude":
        return sorted(json.loads(out.read_text(encoding="utf-8")).get("modelUsage", {}))
    return sorted({(event.get("message") or {}).get("model") for event in events(out)
                   if event.get("type") == "message_end" and (event.get("message") or {}).get("role") == "assistant"} - {None})


def check():
    answers, usage = [], []
    for name, case, model in sessions():
        if name not in launched():
            continue
        client = MODELS[model][0]
        entry = {"session": name, "case": case, "model": MODELS[model][1], "repeat": int(name.split("-")[-1]),
                 "models_observed": observed(name, client), "usd": cost(name)}
        usage.append(entry)
        try:
            text = final_text(name, client)
            match = re.search(r"\{.*\}", text, re.S)
            answer = json.loads(match.group(0))
            if answer.get("says_what") not in {"yes", "no", "cannot-tell"}:
                raise ValueError(f"says_what {answer.get('says_what')!r}")
            answers.append({**entry, "answer": answer})
        except Exception as error:
            answers.append({**entry, "answer": None, "problem": str(error)[:300]})
    (HERE / VERSION / "answers.json").write_text(json.dumps(answers, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    (HERE / VERSION / "usage.json").write_text(json.dumps(usage, indent=1) + "\n", encoding="utf-8")
    for client in LIMITS:
        print(f"{client}: ${spent(client, {}):.4f} at list price")
    print(f"{sum(a['answer'] is not None for a in answers)} of {len(answers)} answers parsed")


if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1] not in {"launch", "check"}:
        raise SystemExit(__doc__)
    launch(sys.argv[2:]) if sys.argv[1] == "launch" else check()
