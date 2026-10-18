"""Census tracked Python and interpreter references without executing reviewed code.

Usage: python3 tools/python_inventory.py [--ref COMMIT] [--output FILE] [--check]
Inputs: tracked Git blobs, or explicit read-only host roots with --observe-runtime.
Exit codes: 0 inventory written or verified; nonzero invalid input or incomplete census.
"""
from __future__ import annotations

import argparse
import ast
from collections import Counter, defaultdict
from datetime import datetime, timezone
import hashlib
import io
import gzip
import json
from pathlib import Path
import posixpath
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = "docs/python-migration/inventory.v2.json.gz"
SHELL_DIRECTORY = r'\$\(dirname\s+(?:"\$0"|\$0)\)'
PYTHON_PATH = re.compile(
    rf"(?:{SHELL_DIRECTORY}|\$\{{[A-Za-z_]\w*\}}|\$[A-Za-z_]\w*)?[\w./-]+\.py\b"
)
REFERENCE = re.compile(
    r"\b(?:python(?:[23](?:\.\d+)?)?|pypy\d*|PYTHON\w*)\b|sys\.executable|"
    + PYTHON_PATH.pattern + r"|importlib|__import__|\b(?:exec|eval|compile)\("
)
GROUPS = {
    "behavioral-contracts": (151, "behavior_contract"),
    "usage": (152, "codex_usage transcript_usage"),
    "runtime": (153, "seal skill_provenance fetch_freeze_commits"),
    "processing": (154, "build_packet derive_packet diff_identity normalize_review normalize_thermo "
                   "compare score scoreboard_svg rates roster upstream export_explorer collect_run methodology"),
    "storage": (155, "evidence_inventory evidence_store reconcile_evidence recover_evidence "
                "local_evidence_resolution native_artifacts import_benchmark"),
    "grading": (156, "attempt_audit audit_log calibration check_manifest claim_grading claims "
                "current_grading delegation evaluator_audit grade grading_policy grading_profile "
                "grading_validation packet_selection pre_existing_grading regrade ruling_dossier "
                "ruling_record ruling_summary"),
    "supervision": (157, "claude_container_controller_v4 claude_container_probe claude_container_probe_v4 "
                    "claude_container_resources claude_fixture_git claude_fixture_git_attack claude_skill_runner "
                    "clean_context codex_container_probe codex_fixture_git codex_grade_dispatch codex_grading "
                    "codex_skill_runner file_attempt grading_client_probe provision review_isolation "
                    "review_queue run_cell subscription_meters"),
    "contributor": (158, "contributor contributor_adapter_probe contributor_adapters contributor_common "
                   "contributor_openrouter contributor_secrets contributor_submit"),
    "cleanup": (159, "prune_workspace retire_workspace"),
    "retirement": (160, "run_bench_tests select_grading_tests python_inventory"),
}
DYNAMIC_ROOTS = {
    "bench/tools/test_packet_replacements.py": ["docs/research/last-push-recut-2026-10-07"],
    "bench/tools/test_review_code_selection.py": ["docs/research/last-push-review-code-2026-10-12/selection"],
    "bench/tools/test_last_push_move.py": ["docs/research/last-push-rulings-2026-10-11/filing",
                                           "docs/research/regrade-rulings-2026-10-13/filing/record.py",
                                           "docs/research/older-problem-activation-2026-10-13/filing/record.py"],
}
RESEARCH_GROUPS = {
    152: ["last-push-review-code-2026-10-12/selection/usage.py", "last-push-rulings-2026-10-11/usage.py"],
    153: ["portable-evidence-2026-10-10/freeze_commits.py"],
    154: ["builtin-review-benchmark-2026-09-24/answers.py", "last-push-recut-2026-10-07/plan.py",
          "last-push-recut-2026-10-07/recut.py", "last-push-recut-2026-10-07/stage_edition.py",
          "last-push-recut-2026-10-07/verify.py"],
    155: ["last-push-recut-2026-10-07/relocate_transcripts.py"],
    156: ["cohort-rebuild-2026-10-05/regrade/plan.py",
          "last-push-review-code-2026-10-12/selection/collect.py", "last-push-review-code-2026-10-12/selection/packets.py",
          "last-push-review-code-2026-10-12/selection/selection.py", "last-push-rulings-2026-10-11/filing/collect.py",
          "last-push-rulings-2026-10-11/filing/move.py", "last-push-rulings-2026-10-11/filing/packets.py",
          "last-push-rulings-2026-10-11/filing/record.py", "older-problem-activation-2026-10-13/filing/admit.py",
          "older-problem-activation-2026-10-13/filing/receipts.py", "older-problem-activation-2026-10-13/filing/record.py",
          "older-problem-activation-2026-10-13/filing/wordings.py", "reference-calibration-2026-10-03/blind_items.py",
          "regrade-rulings-2026-10-13/filing/record.py"],
    157: ["last-push-recut-2026-10-07/dispatch_replacements.py", "last-push-recut-2026-10-07/replacement_runs.py",
          "last-push-recut-2026-10-07/skill_replacement_runs.py", "skill-matrix-2026-10-02/dispatch_serial.py"],
}


def git(*args, cwd=ROOT):
    return subprocess.check_output(["git", "-C", str(cwd), *args])


class Snapshot:
    def __init__(self, ref, root=ROOT):
        self.root = root
        self.revision = git("rev-parse", f"{ref}^{{commit}}", cwd=root).decode().strip()
        self.paths = git("ls-tree", "-r", "--name-only", "-z", self.revision, cwd=root).decode().split("\0")[:-1]
        self.blobs = {}

    def preload(self, paths):
        ordered = sorted(paths)
        result = subprocess.run(["git", "-C", str(self.root), "cat-file", "--batch"],
                                input="".join(f"{self.revision}:{p}\n" for p in ordered).encode(),
                                capture_output=True, check=True)
        stream = io.BytesIO(result.stdout)
        for path in ordered:
            header = stream.readline().split()
            if len(header) != 3 or header[1] != b"blob":
                raise ValueError(f"Not a recorded blob: {path}")
            self.blobs[path] = stream.read(int(header[2]))
            if stream.read(1) != b"\n":
                raise ValueError(f"Invalid object framing: {path}")

    def read(self, path):
        if path in self.blobs:
            return self.blobs[path]
        return git("show", f"{self.revision}:{path}", cwd=self.root)


def category(path):
    if path.startswith("artifacts/import-source/"):
        return "imported-original"
    if ("/inputs/skill/" in path or "/skills/" in path or path.startswith("bench/targets/")
            or path == "bench/policies/grading-commands.v1.json"):
        return "target-or-skill-dependency"
    if path.startswith(("bench/runs/", "bench/fixtures/", "bench/claims/evidence/")):
        return "frozen-evidence"
    if path.startswith(("bench/tools/", "tools/")):
        return "maintained"
    if any(part in Path(path).parts for part in ("probes", "upstream", "evidence", "reproductions")):
        return "target-or-skill-dependency"
    if (path.startswith(("src/", ".github/", "bin/", "bench/harness/", "bench/schema/", "bench/policies/", "bench/arms/"))
            or path in ("package.json", "README.md", "AGENTS.md", "bench/README.md", "bench/contributor-invocation.md")
            or (path.startswith("docs/") and not path.startswith("docs/research/"))):
        return "active-outside-tooling"
    return "historical-replay"


def references(snapshot):
    pattern = (r"python([23]([.][0-9]+)?)?|pypy|PYTHON|sys[.]executable|"
               r"[[:alnum:]_./-]+[.]py|importlib|__import__|\b(exec|eval|compile)\(")
    result = subprocess.run(["git", "-C", str(snapshot.root), "grep", "-n", "-I", "-E", pattern,
                             snapshot.revision, "--", "."],
                            capture_output=True)
    if result.returncode not in (0, 1):
        raise RuntimeError(result.stderr.decode())
    rows = []
    for line in result.stdout.decode(errors="replace").splitlines():
        _, path, number, text = line.split(":", 3)
        matches = sorted({m.group() for m in REFERENCE.finditer(text)})
        if matches:
            rows.append({"path": path, "line": int(number), "tokens": matches})
    return rows


def sibling_variables(text):
    assignments = defaultdict(list)
    directory = rf'(?:{SHELL_DIRECTORY}|"{SHELL_DIRECTORY}"|\$\(cd\s+"{SHELL_DIRECTORY}"\s*&&\s*pwd\))'
    for match in re.finditer(r"(?m)^\s*(?:export\s+)?([A-Za-z_]\w*)=(.*)$", text):
        assignments[match[1]].append(re.fullmatch(directory, match[2].strip()) is not None)
    return {name for name, values in assignments.items() if all(values)}


def file_references(source, token, python_paths, siblings=()):
    prefix, slash, remainder = token.partition("/")
    if slash and (re.fullmatch(SHELL_DIRECTORY, prefix)
                  or prefix.startswith("$") and prefix[1:].strip("{}") in siblings):
        relative = posixpath.normpath(str(Path(source).parent / remainder))
        return [(relative, "path-reference")] if relative in python_paths else []
    candidate = token.removeprefix("./")
    relative = posixpath.normpath(str(Path(source).parent / candidate))
    matches = {path for path in (candidate, relative) if path in python_paths}
    if matches:
        return [(path, "path-reference") for path in sorted(matches)]
    return []


def import_edges(path, imports, python_paths):
    edges = set()
    for name, line in imports:
        module = name.replace(".", "/")
        candidates = [str(Path(path).parent / f"{module}.py"), f"{module}.py",
                      f"bench/tools/{module}.py", f"tools/{module}.py"]
        edges.update((candidate, line, "import") for candidate in candidates if candidate in python_paths)
    return edges


def embedded_imports(text):
    imports = []
    for number, line in enumerate(text.splitlines(), 1):
        froms = list(re.finditer(r"\bfrom\s+([\w.]+)\s+import\s+", line))
        imports.extend((match[1], number) for match in froms)
        for match in re.finditer(r"\bimport\s+([A-Za-z_][\w.]*(?:\s+as\s+\w+)?"
                                 r"(?:\s*,\s*[A-Za-z_][\w.]*(?:\s+as\s+\w+)?)*)", line):
            if not any(source.start() <= match.start() < source.end() for source in froms):
                imports.extend((name.strip().split()[0], number) for name in match[1].split(","))
    return imports


def dependencies(path, text, python_paths, names):
    edges = set()
    imports = []
    parse_error = None
    try:
        tree = ast.parse(text)
    except SyntaxError as error:
        tree = None
        parse_error = f"{error.msg} at line {error.lineno}"
    if tree:
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                imports.extend((alias.name, node.lineno) for alias in node.names)
            elif isinstance(node, ast.ImportFrom) and node.module:
                imports.append((node.module, node.lineno))
    edges.update(import_edges(path, imports, python_paths))
    siblings = sibling_variables(text)
    for number, line in enumerate(text.splitlines(), 1):
        for token in PYTHON_PATH.findall(line):
            resolved = file_references(path, token, python_paths, siblings)
            if not resolved:
                resolved = [(target, "possible-name-reference") for target in names[Path(token).name]]
            edges.update((target, number, kind) for target, kind in resolved)
    return sorted(edges), imports, parse_error


def responsibility(path):
    stem = Path(path).stem
    if stem.startswith("test_"):
        return {"owner": "kamui", "issue": 151, "group": "behavioral-contracts"}
    for group, (issue, names) in GROUPS.items():
        if stem in names.split():
            return {"owner": "kamui", "issue": issue, "group": group}
    raise ValueError(f"Maintained responsibility has no migration owner: {path}")


def research_responsibility(path):
    relative = path.removeprefix("docs/research/")
    for issue, assigned in RESEARCH_GROUPS.items():
        if relative in assigned:
            return {"owner": "kamui", "issue": issue, "group": "research-callers"}
    raise ValueError(f"Active research responsibility has no migration owner: {path}")


def census(snapshot):
    paths = set(snapshot.paths)
    py = {p for p in paths if p.endswith(".py")}
    names = defaultdict(list)
    for path in sorted(py):
        if category(path) in ("maintained", "historical-replay"):
            names[Path(path).name].append(path)
    refs = references(snapshot)
    sources = py | {r["path"] for r in refs}
    live = {p for p in sources if category(p) in ("maintained", "active-outside-tooling")}
    snapshot.preload(py | live | {p for p in paths if p.startswith("bench/runs/")
                               and p.endswith(("/manifest.json", "/attempt.json"))}
                     | {"bench/import-manifest.json"})
    incoming = defaultdict(list)
    graph = defaultdict(set)
    content = {}
    parsed = {}
    for path in sorted(py):
        data = snapshot.read(path)
        text = data.decode(errors="replace")
        content[path] = data
        edges, imports, error = dependencies(path, text, py, names)
        parsed[path] = (imports, error)
        for target, line, kind in edges:
            incoming[target].append({"path": path, "line": line, "kind": kind})
            if kind != "possible-name-reference":
                graph[path].add(target)
    for path in sorted(live - py):
        for target, line, _ in import_edges(path, embedded_imports(snapshot.read(path).decode(errors="replace")), py):
            incoming[target].append({"path": path, "line": line, "kind": "embedded-import-reference"})
            graph[path].add(target)
    siblings = {path: sibling_variables(snapshot.read(path).decode(errors="replace")) for path in live - py}
    for row in refs:
        path = row["path"]
        if path in py:
            continue
        for token in row["tokens"]:
            if not token.endswith(".py"):
                continue
            resolved = file_references(path, token, py, siblings.get(path, ()))
            if not resolved and path in live:
                resolved = [(target, "possible-name-reference") for target in names[Path(token).name]]
            for target, kind in resolved:
                incoming[target].append({"path": path, "line": row["line"], "kind": kind})
                if kind != "possible-name-reference":
                    graph[path].add(target)
    for caller, locations in DYNAMIC_ROOTS.items():
        if caller not in paths:
            continue
        for target in sorted(py):
            if target in locations or str(Path(target).parent) in locations:
                graph[caller].add(target)
                kind = "audited-dynamic-file" if target in locations else "audited-dynamic-directory"
                incoming[target].append({"path": caller, "line": 1, "kind": kind})
    pending = list(live)
    while pending:
        for dependency in graph[pending.pop()] - live:
            live.add(dependency)
            pending.append(dependency)
    files = []
    for path in sorted(py):
        data = content[path]
        classification = category(path)
        if path in live and classification == "historical-replay":
            classification = "active-outside-tooling"
        imports, error = parsed[path]
        row = {"path": path, "class": classification, "sha256": hashlib.sha256(data).hexdigest(),
               "physical_lines": len(data.splitlines()), "callers": incoming[path],
               "imports": [{"module": name, "line": line} for name, line in imports],
               "parse_error": error,
               "code_resolution": f"git show {snapshot.revision}:{path}",
               "interpreter": "unknown exact version; inspect invocation references and runtime records"}
        if classification in ("maintained", "active-outside-tooling"):
            tree = ast.parse(data)
            row["responsibility"] = (ast.get_docstring(tree) or Path(path).stem).splitlines()[0]
            row["public_symbols"] = [node.name for node in tree.body
                                     if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef))
                                     and not node.name.startswith("_")]
            row["current_tests"] = sorted({r["path"] for r in incoming[path] if Path(r["path"]).name.startswith("test_")}
                                          | {p for p in py if Path(p).name == "test_" + Path(path).name})
            row["contract_record"] = "docs/python-migration/README.md#responsibilities-and-contracts"
        if classification == "maintained":
            row["migration"] = responsibility(path)
        elif classification == "active-outside-tooling":
            row["migration"] = research_responsibility(path)
        files.append(row)
    for row in refs:
        row["class"] = category(row["path"])
        if row["path"] in live and row["class"] == "historical-replay":
            row["class"] = "active-outside-tooling"
    manifests = []
    for path in sorted(p for p in paths if re.fullmatch(r"bench/runs/[^/]+/manifest\.json", p)):
        manifest = json.loads(snapshot.read(path))
        prefix = path.rsplit("/", 1)[0] + "/"
        attempts = [json.loads(snapshot.read(p)) for p in sorted(paths)
                    if p.startswith(prefix + "attempts/") and p.endswith("/attempt.json")]
        dispositions = Counter(a.get("disposition", "unknown") for a in attempts)
        completed = {json.dumps(a.get("cell"), sort_keys=True) for a in attempts
                     if a.get("disposition") == "valid completed"}
        planned = manifest.get("planned_cells")
        unfilled = ([cell for cell in planned if json.dumps(cell, sort_keys=True) not in completed]
                    if isinstance(planned, list) else None)
        manifests.append({"path": path, "freeze_commit": manifest.get("freeze_commit"),
                          "planned_cells": planned, "cells_without_saved_valid_attempt": unfilled,
                          "saved_attempts": len(attempts), "dispositions": dict(sorted(dispositions.items())),
                          "liveness": "unknown; saved evidence does not establish controller termination or completion",
                          "runtime_resolution": "freeze_commit plus pinned inputs; see runtime policy in README.md",
                          "runtime_records": sorted(p for p in paths if p.startswith(prefix + "inputs/")
                                                    and Path(p).name in ("runner.json", "skill-pin.json", "skill-provenance.json",
                                                                         "client.json", "rates.json")),
                          "arms": manifest.get("arms"), "execution_policy": manifest.get("execution_policy"),
                          "deviations": manifest.get("deviations")})
    imported = json.loads(snapshot.read("bench/import-manifest.json"))
    unpreserved = [row for row in imported["files"] if "preserved_path" not in row]
    return {"schema": "python-migration-inventory/1", "source_commit": snapshot.revision,
            "tracked_paths": len(paths), "classes": dict(sorted(Counter(r["class"] for r in files).items())),
            "python_files": files, "references": refs, "runs": manifests,
            "imported_without_preserved_path": unpreserved,
            "import_source": {k: imported[k] for k in ("repository", "revision")},
            "runtime_paths": ["bench/tools", "bench/schema", "bench/policies", "docs/clean-context.md"]}


def observe_runtime(work_root, queue_root):
    workspaces = []
    for run in sorted(work_root.iterdir()) if work_root.is_dir() else []:
        if not run.is_dir() or run.is_symlink():
            continue
        attempts = []
        for attempt in sorted(run.glob("att-*")):
            if not attempt.is_dir() or attempt.is_symlink():
                continue
            attempts.append({"attempt": attempt.name, "present": [name for name in
                             ("prepare.json", "cell.json", "timing.json", "stop.json", "workspace-pruned.json",
                              "home", "clone", "clone-cache", "clone-work") if (attempt / name).exists()]})
        workspaces.append({"path": str(run), "attempts": attempts,
                           "controller": "unknown; no process or lock probe performed"})
    queues = []
    for pin in sorted(queue_root.glob("*/queue.pin.json")):
        data = json.loads(pin.read_text())
        log = pin.parent / "controller.jsonl"
        launches, ended = {}, set()
        last = None
        for line in log.read_text().splitlines() if log.is_file() else []:
            event = json.loads(line)
            last = event.get("at")
            if "launched" in event:
                launches[event["launched"]] = event.get("run")
            if "ended" in event:
                ended.add(event["ended"])
        queues.append({"path": str(pin.parent), "pin_sha256": hashlib.sha256(pin.read_bytes()).hexdigest(),
                       "runs": data.get("runs"), "last_event_at": last,
                       "launches_without_saved_end": {k: v for k, v in launches.items() if k not in ended},
                       "controller": "unknown; a saved end does not prove process termination"})
    return {"schema": "python-runtime-observation/1", "observed_at": datetime.now(timezone.utc).isoformat(),
            "work_root": str(work_root), "queue_root": str(queue_root), "workspaces": workspaces, "queues": queues,
            "checkout_state_roots": {name: (ROOT / name).exists() for name in (".local", ".bench")},
            "scope": "Directory names, artifact presence, queue pins and controller event identities only. "
                     "No home, credentials, environment, model, process, lock or controller operation was accessed."}


def main():
    census_path = ROOT / DESTINATION
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ref", help="Commit to inventory; defaults to HEAD, or recorded commit for --check")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--observe-runtime", action="store_true", help="Read host state without invoking controllers")
    parser.add_argument("--work-root", type=Path, default=Path.home() / ".t3/bench-runs")
    parser.add_argument("--queue-root", type=Path, default=Path.home() / ".t3/bench-queues")
    args = parser.parse_args()
    if args.observe_runtime:
        if args.check or args.ref or args.output is not None:
            parser.error("--observe-runtime emits JSON to stdout and cannot use --output, --check or --ref")
        print(json.dumps(observe_runtime(args.work_root, args.queue_root), indent=2, sort_keys=True))
        return
    args.output = args.output or census_path
    previous = args.output.read_bytes() if args.check else None
    if previous is not None and args.output.suffix == ".gz":
        previous = gzip.decompress(previous)
    ref = args.ref or (json.loads(previous)["source_commit"] if args.check else "HEAD")
    result = census(Snapshot(ref))
    rendered = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.check:
        if previous.decode() != rendered:
            raise SystemExit("Inventory differs from its recorded source; regenerate to a new version")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        if args.output.suffix == ".gz":
            with args.output.open("wb") as output:
                with gzip.GzipFile(filename="", mode="wb", fileobj=output, mtime=0) as compressed:
                    compressed.write(rendered.encode())
        else:
            args.output.write_text(rendered)
    print(f"{'Verified' if args.check else 'Wrote'} {args.output}: {len(result['python_files'])} Python files, "
          f"{len(result['references'])} reference lines at {result['source_commit']}")


if __name__ == "__main__":
    main()
