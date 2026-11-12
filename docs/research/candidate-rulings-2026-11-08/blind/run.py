#!/usr/bin/env python3
"""Launch and check the round's blind assessor sessions on Claude, under bench/grading/rulings/same-family-blind.v6.md.

    python3 -I docs/research/candidate-rulings-2026-11-08/blind/run.py launch [SESSION ...]
    python3 -I docs/research/candidate-rulings-2026-11-08/blind/run.py check

This is docs/research/candidate-rulings-2026-11-07/blind/run.py for this round's two case files, both on
i-requests-6667. `launch` refuses until the owner's receipt and blind-limits.v1.json, which copies its limits, are
saved. A session's working directory under CACHE/sessions/<target>-<case>-<model>/ holds only `rule.md` (the rule the records
pin), `earlier-rulings.md` (the precedent sheet the records pin) and `cases/<case>.md`, outside every git repository.
The prompt is brief-candidates.md on stdin. The session's home under CACHE/homes/ is fresh; its `.claude.json` holds
only the account fields (`bench-tools clean-context claude-account`). `launch` runs `claude -p --safe-mode` with no
setting sources, no MCP servers and only Read, Write, Glob and Grep, with `CLAUDE_CODE_DISABLE_FAST_MODE=1` and the
setting `fastMode: false`, copies `.credentials.json` into the home for the session and deletes it after, and refuses
a session that could take the round past blind-limits.v1.json. Rerun a session that wrote no valid answer once with
`launch <target>-<case>-rerun-<model>`.

`check` validates each finished session's answers.json, refuses one whose result reports a speed other than
`standard`, and writes the answers, with the party's model, family, effort, brief and precedent pins added, to
blind/answers/<target>/<case>/<model>/answers.json beside its notes.md, and writes usage.v1.json."""
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
import uuid
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROUND = HERE.parent
ROOT = HERE.parents[3]
CACHE = Path.home() / ".cache/candidate-rulings-2026-11-08-blind"
SESSIONS, HOMES, LOGS, PROMPTS = CACHE / "sessions", CACHE / "homes", CACHE / "logs", CACHE / "prompts"
CREDENTIALS = Path.home() / ".claude/.credentials.json"
ACCOUNT = Path.home() / ".claude.json"
CLAUDE = Path.home() / ".local/share/mise/installs/claude/2.1.318/claude"
MODELS = {"opus": "claude-opus-5-5", "sonnet": "claude-sonnet-5-5"}
EFFORT = "high"
BUDGET = {"opus": 1.0, "sonnet": 0.5}
NO_FAST = json.dumps({"fastMode": False})
BRIEF = HERE / "brief-candidates.md"
RULE = ROOT / "bench/rubric/rules.pre-existing.v4.md"
SHEET = ROUND / "assessors/precedents-candidate.v1.md"
RECEIPT = ROOT / "bench/grading/rulings/same-family-blind.v6.md"
LIMITS = ROUND / "blind-limits.v1.json"
CASES = {f"{target}-{Path(name).stem}": ROUND / target / "cases" / name for target, name in (
    ("i-requests-6667", "N1.md"), ("i-requests-6667", "N2.md"))}
OUTCOMES = {"problem", "minor-defect", "suggestion", "relied-on", "duplicate", "refuted", "unproven", "outside-scope", "cannot-tell"}
FIELDS = {"case", "promised", "promised_by", "delivered", "same_fault_as", "outcome", "clauses",
          "nearest", "conflict", "rule_gap", "confidence", "short_of_high", "would_settle", "reason"}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return str(path.resolve().relative_to(ROOT))


def sessions():
    for case in CASES:
        for model in MODELS:
            yield f"{case}-{model}", case, model


def find(name):
    base = name.replace("-rerun-", "-")
    for session, case, model in sessions():
        if session == base:
            return name, case, model
    raise SystemExit(f"unknown session {name}")


def prepare(name, case):
    directory, home, prompt_file = SESSIONS / name, HOMES / name, PROMPTS / f"{name}.md"
    for path in (directory, home, prompt_file):
        if path.exists() or path.is_symlink():
            raise SystemExit(f"refusing existing {path}")
    (directory / "cases").mkdir(parents=True)
    (home / ".claude").mkdir(parents=True)
    PROMPTS.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(RULE, directory / "rule.md")
    shutil.copyfile(SHEET, directory / "earlier-rulings.md")
    shutil.copyfile(CASES[case], directory / "cases" / CASES[case].name)
    prompt_file.write_text(BRIEF.read_text(encoding="utf-8"), encoding="utf-8")
    subprocess.run(["bench/tools/bench-tools", "clean-context", "claude-account", "--source", str(ACCOUNT), "--home", str(home)],
                   cwd=ROOT, check=True, capture_output=True)


def result(log):
    found = None
    if log.exists():
        for line in log.read_text(encoding="utf-8").splitlines():
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if event.get("type") == "result":
                found = event
    return found


def launched():
    return sorted(log.name.removesuffix(".jsonl") for log in LOGS.glob("*.jsonl"))


def spent(running):
    total = 0.0
    for name in launched():
        model = name.rsplit("-", 1)[1]
        done = result(LOGS / f"{name}.jsonl")
        total += BUDGET[model] if name in running or done is None else done.get("total_cost_usd", BUDGET[model])
    return total


def start(name, model):
    directory, home = SESSIONS / name, HOMES / name
    shutil.copyfile(CREDENTIALS, home / ".claude/.credentials.json")
    os.chmod(home / ".claude/.credentials.json", 0o600)
    env = {key: value for key, value in os.environ.items() if not key.startswith(("CLAUDE", "ANTHROPIC_"))}
    env.update(HOME=str(home), CLAUDE_CODE_DISABLE_FAST_MODE="1")
    command = [str(CLAUDE), "-p", "--safe-mode", "--model", MODELS[model], "--effort", EFFORT, "--session-id", str(uuid.uuid4()),
               "--max-budget-usd", str(BUDGET[model]), "--setting-sources", "", "--settings", NO_FAST, "--strict-mcp-config",
               "--permission-mode", "acceptEdits",
               "--allowedTools", "Read,Write,Glob,Grep", "--disallowedTools", "Bash,Edit,WebFetch,WebSearch,Agent",
               "--output-format", "stream-json", "--verbose"]
    (LOGS / f"{name}.command.json").write_text(json.dumps({"command": command, "HOME": env["HOME"], "cwd": str(directory),
                                                           "CLAUDE_CODE_DISABLE_FAST_MODE": env["CLAUDE_CODE_DISABLE_FAST_MODE"]}, indent=1) + "\n")
    return subprocess.Popen(command, cwd=directory, env=env, stdin=open(PROMPTS / f"{name}.md", "rb"),
                            stdout=open(LOGS / f"{name}.jsonl", "wb"), stderr=open(LOGS / f"{name}.stderr", "wb"))


def launch(names):
    for needed in (RECEIPT, LIMITS):
        if not needed.is_file():
            raise SystemExit(f"refusing: {relative(needed) if needed.is_relative_to(ROOT) else needed} is not saved; the owner has not allowed same-family blind answers for this round")
    limits = json.loads(LIMITS.read_text(encoding="utf-8"))
    LOGS.mkdir(parents=True, exist_ok=True)
    print("limits:", limits["text"])
    queue = [find(name) for name in names] if names else list(sessions())
    queue = [entry for entry in queue if entry[0] not in launched()]
    running = {}
    while queue or running:
        for name, process in list(running.items()):
            if process.poll() is not None:
                (HOMES / name / ".claude/.credentials.json").unlink(missing_ok=True)
                print(f"{name}: exit {process.returncode}", flush=True)
                del running[name]
        while queue and len(running) < 3:
            name, case, model = queue[0]
            if len(launched()) + 1 > limits["sessions"] or spent(running) + BUDGET[model] > limits["usd"]:
                print(f"stop: {name} could cross the limits ({len(launched())} sessions, ${spent(running):.2f} with budgets)")
                queue = []
                break
            queue.pop(0)
            if not (SESSIONS / name).exists():
                prepare(name, case)
            running[name] = start(name, model)
            print(f"{name}: started", flush=True)
        time.sleep(5)
    print(f"{len(launched())} sessions, ${spent({}):.2f} at list price")


def problems(name, case, done):
    faults = []
    speed = (done.get("usage") or {}).get("speed")
    if speed != "standard":
        faults.append(f"speed {speed!r}, not 'standard'")
    path = SESSIONS / name / "answers.json"
    if not path.exists():
        return faults + ["no answers.json"]
    try:
        answers = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        return faults + [f"answers.json does not parse: {error}"]
    if not isinstance(answers, list) or len(answers) != 1 or not isinstance(answers[0], dict):
        return faults + ["answers.json is not a list of one object"]
    answer = answers[0]
    if answer.get("case") != CASES[case].stem:
        faults.append(f"case {answer.get('case')} != {CASES[case].stem}")
    if set(answer) != FIELDS:
        faults.append(f"fields differ: missing {sorted(FIELDS - set(answer))}, extra {sorted(set(answer) - FIELDS)}")
    if answer.get("outcome") not in OUTCOMES:
        faults.append("outcome outside the brief's list")
    level, short = answer.get("confidence"), answer.get("short_of_high")
    if level not in {"high", "medium", "low"} or not isinstance(short, list):
        faults.append("confidence fields malformed")
    elif (level == "high") != (not short) or (level == "medium") != (len(short) == 1) or answer.get("would_settle") != (level == "high"):
        faults.append("confidence, short_of_high and would_settle disagree")
    if not (SESSIONS / name / "notes.md").exists():
        faults.append("no notes.md")
    return faults


def party(model):
    return {"by": f"assessor-{model}", "model": MODELS[model], "family": "claude", "effort": EFFORT, "blind": True, "exposure": "none",
            "brief": {"path": relative(BRIEF), "sha256": digest(BRIEF)}, "precedents": {"path": relative(SHEET), "sha256": digest(SHEET)},
            "written": "before-question"}


def check():
    usage, failed = [], 0
    for name in launched():
        _, case, model = find(name)
        done = result(LOGS / f"{name}.jsonl") or {}
        faults = problems(name, case, done)
        raw = SESSIONS / name / "answers.json"
        usage.append({"session": name, "case": case, "model": MODELS[model], "effort": EFFORT, "budget_usd": BUDGET[model],
                      "fast_mode": "off (CLAUDE_CODE_DISABLE_FAST_MODE=1, fastMode false)", "speed": (done.get("usage") or {}).get("speed"),
                      "subtype": done.get("subtype"), "turns": done.get("num_turns"), "usd_list": done.get("total_cost_usd"),
                      "answers_sha256": digest(raw) if raw.exists() else None, "usage": done.get("usage")})
        if not faults:
            recorded = [{**party(model), **answer} for answer in json.loads(raw.read_text(encoding="utf-8"))]
            out = HERE / "answers" / CASES[case].parent.parent.name / CASES[case].stem / model
            out.mkdir(parents=True, exist_ok=True)
            outputs = {"answers.json": (json.dumps(recorded, indent=1, ensure_ascii=False) + "\n").encode("utf-8"),
                       "notes.md": (SESSIONS / name / "notes.md").read_bytes()}
            for target, data in outputs.items():
                destination = out / target
                if destination.exists() and destination.read_bytes() != data:
                    faults.append(f"{destination} exists with other bytes; not replaced")
                    continue
                destination.write_bytes(data)
        failed += bool(faults)
        print(name, "FAILED" if faults else "ok", "; ".join(faults))
    total = sum(row["usd_list"] or 0 for row in usage)
    (ROUND / "usage.v1.json").write_text(json.dumps({"sessions": usage, "count": len(usage), "usd_list": round(total, 4)}, indent=1) + "\n", encoding="utf-8")
    print(f"{len(usage)} sessions, {failed} failed, ${total:.2f} at list price")


if __name__ == "__main__":
    step = sys.argv[1] if len(sys.argv) > 1 else ""
    if step == "launch":
        launch(sys.argv[2:])
    elif step == "check":
        check()
    else:
        raise SystemExit(__doc__)
