"""Select one CI job's coverage from the complete PR diff, defaulting to full coverage."""
import argparse
import json
from pathlib import Path, PurePosixPath
import re
import subprocess


ROOT = Path(__file__).resolve().parents[1]
JOBS = frozenset({"build", "mapping-replay", "grading-tests", "client"})
UI_PREFIXES = ("src/components/", "src/routes/")
ASSET_SUFFIXES = {".css", ".svg", ".png", ".jpg", ".jpeg", ".webp", ".ico", ".woff", ".woff2"}
UNRELATED_FILES = {"src/styles.css", ".github/workflows/rust-runtime.yml"}
BENCH_TEST_DRIVERS = {"tools/run_bench_tests.py", "tools/fetch_freeze_commits.py"}
RUNTIME = "bench/tools/rust_runtime.py"


def unrelated(path: str) -> bool:
    return (path.startswith(UI_PREFIXES) and Path(path).suffix in {".tsx", ".css"}
            or path in UNRELATED_FILES
            or path.startswith("public/") and Path(path).suffix in ASSET_SUFFIXES)


def is_doc(path: str) -> bool:
    return path.startswith("docs/") or "/" not in path and path.endswith(".md")


def jobs_reading(path: str) -> frozenset[str]:
    """The jobs whose result a change to this path can affect: every job for an unknown path."""
    parent, name = str(PurePosixPath(path).parent), PurePosixPath(path).name
    if unrelated(path):
        return frozenset()
    if path in BENCH_TEST_DRIVERS:
        return frozenset({"build", "grading-tests"})
    if parent == "tools" and name.startswith("test_") and name.endswith(".py"):
        return frozenset({"build"})
    if parent == "bench/tools" and name.endswith(".py") and path != RUNTIME:
        return frozenset({"grading-tests"})
    return JOBS


def names(doc: str) -> set[str]:
    """A reader can reach a doc through its own path or any directory that holds it, except `docs` itself."""
    parts = PurePosixPath(doc).parts
    return {"/".join(parts[:end]) for end in range(2 if parts[0] == "docs" else 1, len(parts) + 1)}


def read_docs(docs: list[str], root: Path) -> set[str]:
    """The docs another tracked file names, counting every file except Markdown prose as a possible reader."""
    wanted = set().union(*map(names, docs))
    found = subprocess.run(
        ["git", "grep", "-l", "-z", "-I", "-F", *(arg for name in wanted for arg in ("-e", name)),
         "--", ".", ":(exclude,glob)**/*.md"],
        cwd=root, capture_output=True)
    if found.returncode == 1:
        return set()
    found.check_returncode()
    mention = re.compile(r"(?<![A-Za-z0-9_.-])("
                         + "|".join(map(re.escape, sorted(wanted, key=len, reverse=True)))
                         + r")/?(?![A-Za-z0-9_./-])")
    read = set()
    for reader in found.stdout.decode("utf-8", "surrogateescape").split("\0"):
        if reader:
            named = set(mention.findall((root / reader).read_text(encoding="utf-8", errors="replace")))
            read.update(doc for doc in docs if doc != reader and names(doc) & named)
    return read


def selection(event_name: str, event: dict, root: Path = ROOT) -> tuple[frozenset[str], str]:
    if event_name != "pull_request":
        return JOBS, "Main and manual runs retain full coverage"
    pr = event["pull_request"]
    base, head = pr["base"]["sha"], pr["head"]["sha"]
    paths = subprocess.check_output(
        ["git", "diff", "--name-only", "--no-renames", "-z", f"{base}...{head}", "--"],
        cwd=root,
    ).decode("utf-8").split("\0")
    paths = [path for path in paths if path]
    if not paths:
        return JOBS, "Empty change detection retains full coverage"
    docs = [path for path in paths if is_doc(path)]
    jobs = frozenset().union(*(jobs_reading(path) for path in paths if not is_doc(path)))
    read = read_docs(docs, root) if docs and jobs != JOBS else set()
    if read:
        jobs = JOBS
    return jobs, (f"{len(paths)} changed paths, {len(docs)} of them docs and {len(read)} of those read, "
                  f"select {', '.join(sorted(jobs)) or 'no gated job'}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--event-name", required=True)
    parser.add_argument("--event-path", required=True, type=Path)
    parser.add_argument("--job", required=True, choices=sorted(JOBS))
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        jobs, reason = selection(args.event_name, json.loads(args.event_path.read_text()))
        run = args.job in jobs
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        run, reason = True, f"Change detection unavailable: {error}"
    print(reason)
    with args.output.open("a") as output:
        output.write(f"run={'true' if run else 'false'}\n")


if __name__ == "__main__":
    main()
