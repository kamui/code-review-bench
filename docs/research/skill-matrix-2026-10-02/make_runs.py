#!/usr/bin/env python3
"""Create the unfrozen run definitions and new arms of the 2026-10-02 skill matrix.

Usage::

    python3 docs/research/skill-matrix-2026-10-02/make_runs.py [RUN_ID ...]

Without arguments it creates the new arms and the fifteen runs of the first freeze. With run ids it
creates only those runs, for a later freeze. Each run copies the frozen inputs of the run that already benchmarked the same review method, so a
setup keeps one skill snapshot, invocation, client and execution policy across task cohorts. It
refuses to overwrite an existing arm or run. ``freeze.py`` pins the freeze commit afterwards.
"""

import hashlib
import json
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
BENCH = ROOT / "bench"
NOW = datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")
DATE = "2026-10-02"
SELECTED = ["u-grpc-go-6919", "v-django-17914", "w-graphql-js-3457", "x-kubernetes-141463", "y-django-16631"]
REPLICATES = 3
AUTHORIZATION = ("The user authorized these reviews against the remaining usage of the Claude and ChatGPT plans, with no "
                 "dollar cap; the caps below bound list-price-equivalent accounting only.")
REBUILT = ("Frozen dependency archives were deleted and rebuilt from the unchanged recipes; each attempt's notes name "
           "the replacement manifest that selected its archive.")


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def rate(model):
    return max((row for row in read(BENCH / "rates.current.json")["rates"] if row["model"] == model),
               key=lambda row: row["as_of"])


def published_registers():
    versions = {}
    for suite in read(BENCH / "scoreboard.current.json")["suites"]:
        results = read(BENCH / suite["cohort_run"] / suite["cohort_results"])
        versions.update({row["target"]: row["register_version"] for row in results["inputs"]})
    return versions


def cohort(source_run, targets, registers):
    entries = {entry["target"]: entry for entry in read(BENCH / "runs" / source_run / "manifest.json")["cohort"]}
    return [{**entries[target], "register_version": registers[target]} for target in targets]


def new_arm(source, arm_id, model, label, budget):
    path = BENCH / "arms" / f"{arm_id}.json"
    if path.exists():
        raise SystemExit(f"{path} exists")
    arm = read(BENCH / "arms" / f"{source}.json")
    arm.update(id=arm_id, model=model, label=label, budget_usd_per_attempt=budget)
    if arm["requested_workers"]["model"]:
        arm["requested_workers"]["model"] = model
    return path, arm


ARMS = [
    ("claude-builtin-opus-gaps-high", "claude-builtin-sonnet-5-5-net-high", "claude-sonnet-5-5",
     "Claude built-in / claude-sonnet-5-5 / High", 2.5),
    ("codex-ce-luna-high", "codex-ce-sol61-high", "gpt-6.1-sol",
     "Codex CLI exec, GPT-6.1 Sol at high reasoning effort, frozen ce-code-review skill, clean per-cell home", 6.0),
    ("codex-ce-luna-high", "codex-ce-astra-high", "gpt-6-astra",
     "Codex CLI exec, GPT-6 Astra at high reasoning effort, frozen ce-code-review skill, clean per-cell home", 25.0),
    ("codex-thermo-high", "codex-thermo-sol61-high", "gpt-6.1-sol",
     "Codex CLI exec, GPT-6.1 Sol at high reasoning effort, frozen thermo-nuclear-code-quality-review skill, clean per-cell home", 1.0),
    ("codex-thermo-high", "codex-thermo-astra-high", "gpt-6-astra",
     "Codex CLI exec, GPT-6 Astra at high reasoning effort, frozen thermo-nuclear-code-quality-review skill, clean per-cell home", 4.0),
]

CE_SANDBOX = {"sandbox": "bwrap-v1", "network_allowed": True}
RUNS = [
    {"id": "claude-builtin-selected", "template": "2026-09-29-claude-opus-gaps-high", "cohort": "selected",
     "arms": ["claude-builtin-opus-gaps-high", "claude-builtin-sonnet-5-5-net-high"], "attempt_usd": None,
     "what": "Claude Code built-in /code-review on the five selected PR tasks with the pinned 2.1.284 client, under "
             "the execution policy of the published selected-task Codex cohort. The Sonnet 5.5 arm is the Opus gap "
             "arm with its model changed, so both built-in arms state the network allowance that policy grants."},
    {"id": "claude-ce-opus-5-5-high-selected", "template": "2026-09-29-claude-ce-opus-5-5-high", "cohort": "selected",
     "arms": ["claude-ce-opus-5-5-high"], "attempt_usd": 20.0},
    {"id": "claude-ce-sonnet-5-5-high-selected", "template": "2026-09-29-claude-ce-sonnet-5-5-high", "cohort": "selected",
     "arms": ["claude-ce-sonnet-5-5-high"], "attempt_usd": 12.0, "runner": CE_SANDBOX,
     "what": "runner.json freezes the bwrap-v1 sandbox and network allowance from the start; the source run adopted "
             "both as deviations sandbox-rerun.v1 and network-allowed.v1, and all of its valid trials ran under them."},
    {"id": "claude-thermo-opus-5-5-high-selected", "template": "2026-09-29-claude-thermo-opus-5-5-high", "cohort": "selected",
     "arms": ["claude-thermo-opus-5-5-high"], "attempt_usd": 6.0},
    {"id": "claude-thermo-sonnet-5-5-high-selected", "template": "2026-09-29-claude-thermo-sonnet-5-5-high", "cohort": "selected",
     "arms": ["claude-thermo-sonnet-5-5-high"], "attempt_usd": 6.0},
    {"id": "codex-ce-luna-high-selected", "template": "2026-09-29-codex-ce-luna-high", "cohort": "selected",
     "arms": ["codex-ce-luna-high"], "attempt_usd": 0.5},
    {"id": "codex-thermo-luna-high-selected", "template": "2026-09-29-codex-thermo-high", "cohort": "selected",
     "arms": ["codex-thermo-high"], "attempt_usd": 0.2},
]
LATER = [
    {"id": "claude-builtin-fable-selected", "template": "2026-09-29-claude-fable-high", "cohort": "selected",
     "arms": ["claude-builtin-fable-high"], "attempt_usd": None,
     "what": "Added on the user's instruction to run the Fable 5.1 built-in after every other bench, if Claude plan "
             "usage remains. On 2026-10-03 the user asked for it to run, reporting \"8% on my claude usage\"."},
]
for skill, template in (("ce", "2026-09-29-codex-ce-luna-high"), ("thermo", "2026-09-29-codex-thermo-high")):
    for name, model in (("sol61", "gpt-6.1-sol"), ("astra", "gpt-6-astra")):
        arm = f"codex-{skill}-{name}-high"
        for suffix, cohort_name in (("-selected", "selected"), ("", "original")):
            RUNS.append({"id": arm + suffix, "template": template, "cohort": cohort_name, "arms": [arm],
                         "attempt_usd": next(row[4] for row in ARMS if row[1] == arm),
                         "invocation": ("`gpt-6-luna`", f"`{model}`"),
                         "what": f"Every model call uses {model} at high effort in place of gpt-6-luna; the skill, "
                                 "invocation, client and runner are otherwise the source run's."})


def main():
    registers = published_registers()
    provenance = read(BENCH / "skill-provenance.v2.json")["skills"]
    wanted = sys.argv[1:]
    if not wanted:
        for source, arm_id, model, label, budget in ARMS:
            path, arm = new_arm(source, arm_id, model, label, budget)
            write(path, arm)
    original = [entry["target"] for entry in read(BENCH / "runs/2026-09-29-codex-ce-luna-high/manifest.json")["cohort"]]
    for spec in ([row for row in RUNS + LATER if row["id"] in wanted] if wanted else RUNS):
        run = BENCH / "runs" / f"{DATE}-{spec['id']}"
        source = BENCH / "runs" / spec["template"]
        run.mkdir()
        template = read(source / "manifest.json")
        entries = []
        for arm_id in spec["arms"]:
            arm = read(BENCH / "arms" / f"{arm_id}.json")
            pinned = next((entry for entry in template["arms"] if entry["id"] == arm_id), template["arms"][0])
            entry = {"id": arm_id, "billing_mode": "subscription",
                     "arm_file_sha256": sha(BENCH / "arms" / f"{arm_id}.json"),
                     "resolved_skill_tree": pinned["resolved_skill_tree"],
                     "expected_cli_version": pinned["expected_cli_version"],
                     "expected_prompt_hashes": arm["adapter"].get("expected_prompt_variants", [])}
            entries.append(entry)
        if (source / "inputs").is_dir():
            inputs = run / "inputs"
            shutil.copytree(source / "inputs", inputs)
            if spec.get("runner"):
                write(inputs / "runner.json", {**read(inputs / "runner.json"), **spec["runner"]})
            if spec.get("invocation"):
                old, new = spec["invocation"]
                text = (inputs / "invocation.md").read_text(encoding="utf-8")
                assert old in text, (spec["id"], old)
                (inputs / "invocation.md").write_text(text.replace(old, new), encoding="utf-8")
            pin = read(inputs / "input-pin.json")
            pin["pinned_at_utc"] = NOW
            for row in pin["files"]:
                data = (inputs / row["path"]).read_bytes()
                row.update(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
            write(inputs / "input-pin.json", pin)
            write(inputs / "skill-provenance.json", provenance[entries[0]["resolved_skill_tree"]])
            entries[0]["skill_provenance"] = {"path": "inputs/skill-provenance.json",
                                              "sha256": sha(inputs / "skill-provenance.json")}
        targets = SELECTED if spec["cohort"] == "selected" else original
        cohort_source = "2026-09-30-selected-prs-review-only" if spec["cohort"] == "selected" else "2026-09-29-codex-ce-luna-high"
        cells = [{"target": target, "arm": arm_id, "replicate": replicate}
                 for replicate in range(1, REPLICATES + 1) for target in targets for arm_id in spec["arms"]]
        replacements = max(4, len(cells) // 3)
        bound = spec["attempt_usd"] or max(read(BENCH / "arms" / f"{a}.json")["budget_usd_per_attempt"] for a in spec["arms"])
        reserve = round(2.5 * len(targets), 2)
        caps = {"max_attempts": len(cells) + replacements, "replacements": replacements,
                "spend_usd": round((len(cells) + replacements) * bound + reserve, 2),
                "closeout_reserve_usd": reserve, "max_in_flight": 4}
        if spec["attempt_usd"]:
            caps["attempt_usd"] = spec["attempt_usd"]
        policy = read(BENCH / "runs" / (cohort_source if spec["id"] == "claude-builtin-selected" else spec["template"])
                      / "manifest.json")["execution_policy"]
        what = (f"Copies the frozen inputs of run {spec['template']} for the "
                f"{'five selected' if spec['cohort'] == 'selected' else 'twelve original'} PR tasks. "
                + (spec.get("what", "") + " " if spec.get("what") else "") + REBUILT + " " + AUTHORIZATION)
        write(run / "manifest.json", {
            "schema_version": 1, "run_id": run.name, "created_at": NOW, "method_revision": template["method_revision"],
            "rubric_version": 2, "metric_code_revision": "pending freeze", "arms": entries, "exclusions": [],
            "cohort": cohort(cohort_source, targets, registers), "planned_cells": cells, "caps": caps,
            "sealed_order": [f"{cell['target']}/{cell['arm']}/{cell['replicate']}" for cell in cells],
            "rates": [rate(read(BENCH / "arms" / f"{arm_id}.json")["model"]) for arm_id in spec["arms"]],
            "execution_policy": policy,
            "deviations": [{"at": "pre-dispatch setup", "what": what, "invalidates": []}]})
        print(run.name, len(cells), "cells", caps)


if __name__ == "__main__":
    main()
