#!/usr/bin/env python3
"""Prepare and launch the round's blind assessor sessions.

    python3 -I docs/research/rule-v4-credit-rulings-2026-10-26/assessors/run.py prepare
    python3 -I docs/research/rule-v4-credit-rulings-2026-10-26/assessors/run.py launch [SESSION ...]
    python3 -I docs/research/rule-v4-credit-rulings-2026-10-26/assessors/run.py check

`prepare` makes, for each session, a working directory under CACHE/assessors/<session>/ holding only rule.md,
cases/, earlier-rulings.md and prompt.md, and a fresh client home under CACHE/homes/<session>/ whose config is
`bench-tools clean-context codex-config` without its review instructions, with web search and every tool feature
the session does not need turned off. It refuses an existing directory.

`launch` runs at most four sessions at a time with `codex exec`, HOME and CODEX_HOME set to the session's home.
It copies only auth.json into the home before a session and deletes it after. Before each launch it refuses to
start a session that could take the round past the owner's limits (ROUND/blind-limits.v1.json, reruns included), counting every launched session and reserving a ceiling for each one still
running. A session that wrote no valid answer is reported; rerun it once with `launch <folder>-<kind>-rerun-<model>`.

`check` validates every finished session's answers and copies answers.json and notes.md into
ROUND/assessors/<folder>/ as answers-<kind>-<model>.json and notes-<kind>-<model>.md."""
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROUND = HERE.parent
ROOT = HERE.parents[3]
CACHE = Path("/home/jack/.cache/rule-v4-credit-rulings")
SESSIONS, HOMES, LOGS = CACHE / "assessors", CACHE / "homes", CACHE / "assessors/logs"
AUTH = Path.home() / ".codex/auth.json"
CODEX = "/home/jack/.local/bin/codex"
MODELS = {"sol": "gpt-6.1-sol", "astra": "gpt-6-astra"}
EFFORT = "high"
PARALLEL = 2
LIMITS = json.loads((ROUND / "blind-limits.v1.json").read_text(encoding="utf-8")) if (ROUND / "blind-limits.v1.json").exists() else None
SESSION_LIMIT, PRICE_LIMIT = (LIMITS["sessions"], LIMITS["usd"]) if LIMITS else (0, 0.0)
CEILING = {"sol": 0.4, "astra": 0.9}
CANDIDATE_RULE = "bench/rubric/rules.pre-existing.v3.md"

TARGETS = {
    "graphql": "w-graphql-js-3457",
    "base-ui": "r-base-ui-5460",
}

# (folder, kind, cases as (target folder, group), rule, sheet, models)
PLAN = [
    ("graphql", "credit", [("graphql", "C1"), ("graphql", "C2")], "rule-credit.md", "precedents-recovery.v1.md", ("sol", "astra")),
    ("base-ui", "credit", [("base-ui", "C1")], "rule-credit.md", "precedents-recovery.v1.md", ("sol", "astra")),
]

COMMON = {"case", "nearest", "conflict", "rule_gap", "confidence", "short_of_high", "would_settle", "reason"}
FIELDS = {
    "candidates": {"promised", "promised_by", "delivered", "same_fault_as", "outcome", "clauses"},
    "admission": {"promised", "promised_by", "delivered", "intent", "same_fault_as", "outcome", "clauses"},
    "credit": {"facts", "outcome", "identifies_instead", "clauses"},
    "labels": {"outcome", "clauses", "against"},
}
OUTCOMES = {
    "candidates": {"problem", "minor-defect", "suggestion", "relied-on", "duplicate", "refuted", "unproven", "outside-scope", "cannot-tell"},
    "admission": {"admitted", "not-admitted", "cannot-tell"},
    "credit": {"recovers", "does-not-recover", "cannot-tell"},
    "labels": {"serious", "other-material", "unknown"},
}
# Features of client 0.164.0 that a session reading three files and writing two has no use for.
OFF = ["browser_use", "browser_use_external", "in_app_browser", "computer_use", "image_generation", "view_image", "goals",
       "multi_agent", "skill_search", "tool_suggest", "skill_mcp_dependency_install", "apps", "plugins", "remote_plugin",
       "hooks", "memories"]


def sessions():
    for folder, kind, cases, rule, sheet, models in PLAN:
        for model in models:
            yield f"{folder}-{kind}-{model}", folder, kind, cases, rule, sheet, model


def find(name):
    base = name.replace("-rerun-", "-")
    for entry in sessions():
        if entry[0] == base:
            return (name,) + entry[1:]
    raise SystemExit(f"unknown session {name}")


def case_files(kind, cases):
    for target, group in cases:
        directory = ROUND / TARGETS[target] / ("cards" if kind == "labels" else "cases")
        name = f"{TARGETS[target]}-{group}.md" if kind == "labels" else f"{group}.md"
        yield directory / f"{group}.md", name


def config(home, directory):
    path = subprocess.run(
        ["mise", "exec", "--", "bench/tools/bench-tools", "clean-context", "codex-config", "--home", str(home), "--clone", str(directory)],
        cwd=ROOT, check=True, capture_output=True, text=True,
        env={**os.environ, "BENCH_RUNTIME_STORE": "/home/jack/.cache/regrade-v4-store"},
    ).stdout.strip()
    lines = Path(path).read_text(encoding="utf-8").splitlines()
    kept = [line for line in lines if not line.startswith("developer_instructions")]
    if len(kept) != len(lines) - 1:
        raise SystemExit(f"{path}: expected one developer_instructions line")
    first_table = next(i for i, line in enumerate(kept) if line.startswith("["))
    extra = ['web_search = "disabled"'] + [f"features.{name} = false" for name in OFF]
    extra = [line for line in extra if line.split(" = ")[0] not in {k.split(" = ")[0] for k in kept}]
    Path(path).write_text("\n".join(kept[:first_table] + extra + kept[first_table:]) + "\n", encoding="utf-8")


def prepare(name, folder, kind, cases, rule, sheet, model):
    directory, home = SESSIONS / name, HOMES / name
    for path in (directory, home):
        if path.exists() or path.is_symlink():
            raise SystemExit(f"refusing existing {path}")
    (directory / "cases").mkdir(parents=True)
    home.mkdir(parents=True)
    rule_path = ROOT / rule if rule == CANDIDATE_RULE else HERE / rule
    shutil.copyfile(rule_path, directory / "rule.md")
    shutil.copyfile(HERE / sheet, directory / "earlier-rulings.md")
    for source, target in case_files(kind, cases):
        shutil.copyfile(source, directory / "cases" / target)
    shutil.copyfile(HERE / f"brief-{kind}.md", directory / "prompt.md")
    config(home, directory)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def usage(log):
    found = None
    if log.exists():
        for line in log.read_text(encoding="utf-8").splitlines():
            try:
                found = json.loads(line).get("usage", found)
            except json.JSONDecodeError:
                continue
    return found


def price(model, used):
    rows = json.loads((ROOT / "bench/rates.current.json").read_text(encoding="utf-8"))["rates"]
    rate = max((row for row in rows if row["model"] == MODELS[model]), key=lambda row: row["as_of"])
    uncached = used["input_tokens"] - used["cached_input_tokens"]
    return (uncached * rate["input"] + used["cached_input_tokens"] * rate["cache_read"] + used["output_tokens"] * rate["output"]) / 1e6


def launched():
    return sorted(log.name.removesuffix(".jsonl") for log in LOGS.glob("*.jsonl"))


def spent(running):
    total = 0.0
    for name in launched():
        model = name.rsplit("-", 1)[1]
        used = usage(LOGS / f"{name}.jsonl")
        total += CEILING[model] if name in running or used is None else price(model, used)
    return total


def start(name, model):
    directory, home = SESSIONS / name, HOMES / name
    shutil.copyfile(AUTH, home / ".codex/auth.json")
    os.chmod(home / ".codex/auth.json", 0o600)
    env = {key: value for key, value in os.environ.items() if not key.startswith(("CODEX_", "OPENAI_"))}
    env.update(HOME=str(home), CODEX_HOME=str(home / ".codex"))
    command = [CODEX, "exec", "-m", MODELS[model], "-c", f'model_reasoning_effort="{EFFORT}"', "-s", "workspace-write",
               "-C", str(directory), "--skip-git-repo-check", "--ignore-rules", "--json", "-o", str(directory / "last-message.md"), "-"]
    (LOGS / f"{name}.command.json").write_text(json.dumps({"command": command, "HOME": env["HOME"], "CODEX_HOME": env["CODEX_HOME"]}, indent=1) + "\n")
    return subprocess.Popen(command, cwd=directory, env=env, stdin=open(directory / "prompt.md", "rb"),
                            stdout=open(LOGS / f"{name}.jsonl", "wb"), stderr=open(LOGS / f"{name}.stderr", "wb"))


def launch(names):
    limits = json.loads((ROUND / "blind-limits.v1.json").read_text(encoding="utf-8"))
    print("limits:", limits["text"])
    queue = [find(name) for name in names] if names else list(sessions())
    queue = [entry for entry in queue if entry[0] not in launched()]
    running = {}
    while queue or running:
        for name, process in list(running.items()):
            if process.poll() is not None:
                (HOMES / name / ".codex/auth.json").unlink(missing_ok=True)
                print(f"{name}: exit {process.returncode}", flush=True)
                del running[name]
        while queue and len(running) < PARALLEL:
            name, *_, model = queue[0]
            if len(launched()) + 1 > SESSION_LIMIT or spent(running) + CEILING[model] > PRICE_LIMIT:
                print(f"stop: {name} could cross the limits ({len(launched())} sessions, ${spent(running):.2f} with ceilings)")
                queue = []
                break
            queue.pop(0)
            if not (SESSIONS / name).exists():
                prepare(*find(name))
            running[name] = start(name, model)
            print(f"{name}: started", flush=True)
        time.sleep(5)
    print(f"{len(launched())} sessions, ${spent({}):.2f} at list price")


def problems(name, kind, cases):
    directory = SESSIONS / name
    path = directory / "answers.json"
    if not path.exists():
        return ["no answers.json"]
    try:
        answers = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        return [f"answers.json does not parse: {error}"]
    if not isinstance(answers, list) or not all(isinstance(a, dict) for a in answers):
        return ["answers.json is not a list of objects"]
    wanted = sorted(target.removesuffix(".md") for _, target in case_files(kind, cases))
    found = sorted(str(a.get("case")) for a in answers)
    faults = [] if found == wanted else [f"cases {found} != {wanted}"]
    for answer in answers:
        missing = (COMMON | FIELDS[kind]) - set(answer)
        if missing:
            faults.append(f"{answer.get('case')}: missing {sorted(missing)}")
    if not (directory / "notes.md").exists():
        faults.append("no notes.md")
    return faults


def remarks(kind, answers):
    notes = []
    for answer in answers:
        extra = set(answer) - COMMON - FIELDS[kind]
        if extra:
            notes.append(f"{answer['case']}: extra fields {sorted(extra)}")
        if answer.get("outcome") not in OUTCOMES[kind]:
            notes.append(f"{answer['case']}: outcome outside the brief's list")
        level, short = answer.get("confidence"), answer.get("short_of_high")
        if level not in {"high", "medium", "low"} or not isinstance(short, list):
            notes.append(f"{answer['case']}: confidence fields malformed")
        elif (level == "high") != (len(short) == 0) or (level == "medium") != (len(short) == 1) or answer.get("would_settle") != (level == "high"):
            notes.append(f"{answer['case']}: confidence, short_of_high and would_settle disagree")
    return notes


def check():
    report = {}
    for name in launched():
        _, folder, kind, cases, *_, model = find(name)
        faults = problems(name, kind, cases)
        entry = {"ok": not faults, "problems": faults}
        if not faults:
            answers = json.loads((SESSIONS / name / "answers.json").read_text(encoding="utf-8"))
            entry["remarks"] = remarks(kind, answers)
            out = HERE / folder
            out.mkdir(exist_ok=True)
            for source, target in (("answers.json", f"answers-{kind}-{model}.json"), ("notes.md", f"notes-{kind}-{model}.md")):
                destination = out / target
                if destination.exists() and digest(destination) != digest(SESSIONS / name / source):
                    entry["problems"].append(f"{destination.name} exists with other bytes; not replaced")
                    continue
                shutil.copyfile(SESSIONS / name / source, destination)
        report[name] = entry
    for name, entry in report.items():
        print(name, "ok" if entry["ok"] else "FAILED", "; ".join(entry["problems"] + entry.get("remarks", [])))


if __name__ == "__main__":
    step = sys.argv[1] if len(sys.argv) > 1 else ""
    if step == "prepare":
        for entry in sessions():
            prepare(*entry)
            print("prepared", entry[0])
    elif step == "launch":
        LOGS.mkdir(parents=True, exist_ok=True)
        launch(sys.argv[2:])
    elif step == "check":
        check()
    else:
        raise SystemExit(__doc__)
