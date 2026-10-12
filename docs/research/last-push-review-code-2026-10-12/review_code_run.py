#!/usr/bin/env python3
"""Define, check and freeze the review-code replacement run on the last-push packets."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

RECORD = Path(__file__).resolve().parent
RECUT = RECORD.parent / "last-push-recut-2026-10-07"
sys.path.insert(0, str(RECUT))
import skill_replacement_runs as recut
from skill_replacement_runs import BENCH, ROOT, now, read, sha, write
import check_manifest
import compare
import run_cell
import skill_provenance

RUN = BENCH / "runs/2026-10-12-last-push-claude-review-code-sonnet"
REPLACED = BENCH / "runs/2026-09-28-sonnet-5-5-rebench"
ARM = "review-code-sonnet-5-5-high-enforced"
REPLACEMENTS = (RECUT / "packet-replacements.v1.json").relative_to(ROOT)
# The checkout of https://github.com/kamui/skills. This repository was split from it and holds none of its objects.
SKILLS = Path.home() / "development/skills"
SKILL = RUN / "inputs/skill"
QUEUE = RECORD / "review-code-queue.claude.v1.json"
REVIEWS = 30
CAP = 60
# The owner's Claude ceiling of 2026-10-08 less the accounted usage of the built-in and the CE and thermo replacements.
CEILING = 900 - 41.090368 - 338.978025


def saved_arm():
    return next(arm for arm in read(REPLACED / "manifest.json")["arms"] if arm["id"] == ARM)


def review_code_block(script):
    lines = script.split("\n")
    start = lines.index("  review-code)")
    return lines[start:lines.index("    ;;", start)]


def check_adapter(source_freeze):
    """The invocation and the sandbox profile are the ones the saved reviews ran under."""
    def saved(name):
        return skill_provenance.git(SKILLS, "show", f"{source_freeze}:bench/tools/{name}").decode()
    if review_code_block(saved("dispatch.sh")) != review_code_block((BENCH / "tools/dispatch.sh").read_text()):
        raise SystemExit(f"dispatch.sh: the review-code block differs from {source_freeze}")
    if saved("review_isolation.py") != (BENCH / "tools/review_isolation.py").read_text():
        raise SystemExit(f"review_isolation.py differs from {source_freeze}")


def vendored_tree():
    """The git tree of inputs/skill, written to the object store so that run_cell.py can archive it."""
    relative = SKILL.relative_to(ROOT).as_posix()
    with tempfile.TemporaryDirectory() as scratch:
        env = {**os.environ, "GIT_INDEX_FILE": str(Path(scratch) / "index")}
        subprocess.run(["git", "-C", str(ROOT), "add", "--", relative], check=True, env=env)
        return subprocess.check_output(["git", "-C", str(ROOT), "write-tree", f"--prefix={relative}/"], env=env, text=True).strip()


def define():
    if RUN.exists():
        raise SystemExit(f"run already defined: {RUN}")
    pin = recut.client_pins()["claude"]
    template = read(REPLACED / "manifest.json")
    tree = saved_arm()["resolved_skill_tree"]
    arm_path = BENCH / "arms" / (ARM + ".json")
    arm = read(arm_path)
    problems = check_manifest.validate(read(BENCH / "schema/arm.schema.json"), arm)
    if problems:
        raise SystemExit(f"{ARM}: {problems}")
    if sha(arm_path) != saved_arm()["arm_file_sha256"]:
        raise SystemExit(f"{ARM}: the arm file changed since {REPLACED.name} froze it")
    check_adapter(template["freeze_commit"])
    recovered = read(BENCH / "skill-provenance.v2.json")["skills"][tree]
    SKILL.mkdir(parents=True)
    archive = skill_provenance.git(SKILLS, "archive", "--format=tar", tree)
    subprocess.run(["tar", "-x", "-C", str(SKILL)], input=archive, check=True)
    if vendored_tree() != tree:
        raise SystemExit(f"{SKILL}: the files do not hash to tree {tree}")
    subprocess.run([sys.executable, str(BENCH / "tools/skill_provenance.py"), "--repository", str(SKILLS),
                    "--revision", recovered["source_commit"], "--skill-path", recovered["skill_path"],
                    "--frozen", str(SKILL), "--benchmark-tree", tree, "--out", str(RUN / "inputs/skill-provenance.json")], check=True)
    provenance = read(RUN / "inputs/skill-provenance.json")
    differing = [key for key in provenance if key in recovered and provenance[key] != recovered[key]]
    if differing:
        raise SystemExit(f"skill provenance differs from bench/skill-provenance.v2.json: {differing}")
    replacements = {row["target"]: row["replacement"] for row in read(ROOT / REPLACEMENTS)["targets"]}
    targets = sorted(target for target in replacements if target[0] in "iklmnopqrs")
    assert len(targets) == 10
    stamp = now()
    cohort = []
    for target_id in targets:
        target_dir = BENCH / "targets" / target_id
        target = read(target_dir / "target.json")
        versions = [int(path.name.split(".v")[1].split(".")[0]) for path in target_dir.glob("register.v*.json")]
        cohort.append({"target": target_id, "register_version": max(versions),
                       "packet_sha256": replacements[target_id]["sha256"],
                       "diff_manifest_sha256": target["diff_manifest_sha256"],
                       "provisioning_sha256": compare.provisioning_hash(target),
                       "cohort_group": "regression" if target_id[0] in "iklmn" else "fresh",
                       "execution_available": True})
    cells = [{"target": target, "arm": ARM, "replicate": replicate} for replicate in range(1, 4) for target in targets]
    rates = [max((row for row in read(BENCH / "rates.current.json")["rates"] if row["model"] == arm["model"]),
                 key=lambda row: row["as_of"])]
    entry = {"id": ARM, "arm_file_sha256": sha(arm_path), "resolved_skill_tree": tree,
             "skill_provenance": {"path": "inputs/skill-provenance.json", "sha256": sha(RUN / "inputs/skill-provenance.json")},
             "expected_cli_version": "claude-code " + pin["version"].split()[0],
             "expected_prompt_hashes": [], "billing_mode": "subscription"}
    manifest = {"schema_version": 1, "run_id": RUN.name, "created_at": stamp,
                "method_revision": template["method_revision"], "rubric_version": 2,
                "metric_code_revision": "pending freeze", "arms": [entry],
                "packet_replacements": {"path": REPLACEMENTS.as_posix(), "sha256": sha(ROOT / REPLACEMENTS)},
                "cohort": cohort, "exclusions": [], "planned_cells": cells,
                "caps": {"max_attempts": len(cells) + 10, "replacements": 10, "spend_usd": CAP,
                         "closeout_reserve_usd": 0, "max_in_flight": 1, "attempt_usd": arm["budget_usd_per_attempt"]},
                "sealed_order": [run_cell.cell_key(cell) for cell in cells], "rates": rates,
                "execution_policy": template["execution_policy"],
                "deviations": [
                    {"at": stamp, "what": f"replacement-inputs-v1: Replaces the saved reviews of {ARM} from {REPLACED.name} on the owner's ten rerun tasks, using packet.v2.md through the frozen packet replacement manifest. Keeps that source's arm file, skill tree, execution policy and per-attempt bound. inputs/skill holds the files of the pinned tree, taken from the skills repository, because this repository held no object of that tree for run_cell.py to archive; the directory hashes to the pinned git tree. The review-code block of dispatch.sh and review_isolation.py are byte-identical to those at the source's freeze commit {template['freeze_commit']} in the skills repository. Uses the client copied before the earlier replacement runs, with subscription billing and the current runner, clean-context receipt, audit, rate-check and valid-only cleanup. The first scored review of the run is its setup probe. Saved reviews of j are retained separately.", "invalidates": []},
                    {"at": stamp, "what": "review-code-context-v1: The review-code adapter predates bench/policies/empty-harness-v1.md and does not send its text, which the skill runners add. It gives the reviewer a fresh home whose only skill is the pinned tree, user-level settings only, no MCP servers and the claude-strict-v2 sandbox. The saved reviews of this arm ran the same way, so the configuration is unchanged.", "invalidates": []}]}
    problems = check_manifest.validate(read(BENCH / "schema/run-manifest.schema.json"), manifest)
    if problems:
        raise SystemExit(f"{RUN.name}: {problems}")
    write(RUN / "manifest.json", manifest)
    write(RUN / "clients.frozen.json", {"claude": pin})
    (RUN / "clean-context-policy.md").write_bytes((ROOT / "docs/clean-context.md").read_bytes())
    (RUN / "README.md").write_text(f"# {RUN.name}\n\nReplaces {len(cells)} saved reviews of `{ARM}` from `{REPLACED.name}` on the last-push packets.\n\nThe manifest pins the source's arm file, skill tree, execution policy and per-attempt bound, the client version, rates, packet replacements, order and a ${CAP} list-price-equivalent cap. `inputs/skill` holds the files of the pinned tree and `inputs/skill-provenance.json` records their source. `clients.frozen.json` pins the executable copy. The run's first scored review is its setup probe. The definition and authorization are in [the record](../../../docs/research/last-push-review-code-2026-10-12/README.md).\n")
    check()


def check():
    manifest = read(RUN / "manifest.json")
    entry, cells = manifest["arms"][0], manifest["planned_cells"]
    problems = check_manifest.validate(read(BENCH / "schema/run-manifest.schema.json"), manifest)
    assert not problems, problems
    assert len(cells) == REVIEWS and len({cell["target"] for cell in cells}) == 10, len(cells)
    assert len(set(manifest["sealed_order"])) == len(cells)
    assert set(manifest["sealed_order"]) == {run_cell.cell_key(cell) for cell in cells}
    assert entry["billing_mode"] == "subscription"
    assert manifest["caps"]["max_in_flight"] == 1
    assert manifest["caps"]["spend_usd"] <= CEILING, manifest["caps"]
    tree = entry["resolved_skill_tree"]
    assert {key: entry[key] for key in ("arm_file_sha256", "resolved_skill_tree")} == {
        key: saved_arm()[key] for key in ("arm_file_sha256", "resolved_skill_tree")}
    assert sha(BENCH / "arms" / (ARM + ".json")) == entry["arm_file_sha256"]
    skill_provenance.verify_pin(RUN, entry)
    assert vendored_tree() == tree, "inputs/skill does not hash to the pinned tree"
    assert "SKILL.md" in skill_provenance.git(ROOT, "ls-tree", "--name-only", tree).decode().split()
    hooks = {read(path)["hook_sha256"] for path in (REPLACED / "attempts").glob("att-*/isolation.json")}
    assert hooks == {sha(BENCH / "tools/review_isolation.py")}, "review_isolation.py is not the hook the saved reviews ran"
    client = read(RUN / "clients.frozen.json")["claude"]
    assert entry["expected_cli_version"] == "claude-code " + client["version"].split()[0]
    print(json.dumps({"reviews": len(cells), "cap": manifest["caps"]["spend_usd"], "ceiling_left": round(CEILING, 2),
                      "skill_tree": tree, "client": client["version"]}), flush=True)


def write_queue():
    write(QUEUE, {
        "queue_id": "2026-10-12-last-push-review-code-claude",
        "runs": [RUN.relative_to(ROOT).as_posix()],
        "clients": {"claude": recut.client_pins()["claude"]},
        "ceilings_usd": {"claude": round(CEILING, 2)},
        "environment": {
            "BENCH_RATES": "{repo}/bench/rates.current.json",
            "BENCH_ARCHIVE_ROOT": "{repo}/artifacts/transcripts",
            "BENCH_CACHE_ROOT": str(Path.home() / ".t3/bench-cache"),
            "BENCH_CACHE_REPLACEMENTS": ":".join("{repo}/docs/research/" + name + "/cache-replacements.v1.json"
                for name in ("selected-cache-rebuild-2026-10-02", "original-cache-rebuild-2026-10-02"))}})


def freeze(commit):
    check()
    commit = subprocess.check_output(["git", "rev-parse", commit + "^{commit}"], cwd=ROOT, text=True).strip()
    manifest = read(RUN / "manifest.json")
    if manifest.get("frozen_at"):
        raise SystemExit(f"already frozen: {RUN}")
    paths = [ROOT / "docs/clean-context.md", Path(__file__), Path(recut.__file__), ROOT / REPLACEMENTS,
             BENCH / "arms" / (ARM + ".json"), *(path for path in RUN.rglob("*") if path.is_file())]
    for path in paths:
        relative = path.relative_to(ROOT).as_posix()
        committed = subprocess.check_output(["git", "show", f"{commit}:{relative}"], cwd=ROOT)
        if committed != path.read_bytes():
            raise SystemExit(f"not committed at {commit}: {relative}")
    manifest.update(frozen_at=now(), freeze_commit=commit, metric_code_revision=commit)
    write(RUN / "manifest.json", manifest)
    print("frozen", RUN.name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["define", "check", "queue", "freeze"])
    parser.add_argument("--commit")
    args = parser.parse_args()
    if args.action == "define":
        define()
    elif args.action == "check":
        check()
    elif args.action == "queue":
        write_queue()
    elif not args.commit:
        parser.error("freeze requires --commit")
    else:
        freeze(args.commit)


if __name__ == "__main__":
    main()
