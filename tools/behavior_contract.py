#!/usr/bin/env python3
"""Run language-independent process contracts in disposable directories.

Usage: python3 tools/behavior_contract.py [--corpus corpus.json] [--implementations commands.json]
       [--differential commands.json] [--allow-known-defects]
       python3 tools/behavior_contract.py --verify-decision-history
Inputs: versioned JSON corpus with argv, base64 bytes, filesystem and expectations. Without
--implementations, each program runs its maintained implementation: the Python script where one is
maintained or retained, otherwise the Rust executable rust_runtime.resolve selects.
Exit codes: 0 contracts met (or explicitly allowed baseline defects); 1 differences;
2 invalid inputs or failed execution. Commands are trusted local programs, not sandboxed.
"""
from __future__ import annotations

import argparse
import base64
import difflib
import errno
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import re
import struct
import sys
import tempfile

REPO = Path(__file__).resolve().parents[1]
CORPUS = REPO / "tools/behavior_contracts/usage.v1.json"
PYTHON_PROGRAMS = {"codex": "bench/tools/codex_usage.py", "transcript": "bench/tools/transcript_usage.py",
                   "diff-identity": "bench/tools/diff_identity.py", "normalize-review": "bench/tools/normalize_review.py",
                   "check-manifest": "bench/tools/check_manifest.py"}
# rates.py and upstream.py read the repository that holds them and fetch fixed URLs. These drivers point the
# retained libraries at the case directory, as the Rust commands default to the current directory, and give
# rates.py the `--source PROVIDER=URL` pages that `bench-tools rates check` takes.
PYTHON_DRIVERS = {
    "rates": """import sys
from pathlib import Path
sys.path.insert(0, sys.argv.pop(1))
import rates
rates.BENCH = Path.cwd() / "bench"
rates.CURRENT = rates.BENCH / "rates.current.json"
argv = sys.argv[1:]
while "--source" in argv:
    at = argv.index("--source")
    provider, url = argv[at + 1].split("=", 1)
    rates.SOURCES[provider] = url
    del argv[at:at + 2]
sys.argv[1:] = argv
sys.exit(rates.main())
""",
    "upstream": """import sys
from pathlib import Path
sys.path.insert(0, sys.argv.pop(1))
import claims, upstream
root = Path.cwd()
upstream.ROOT = claims.ROOT = root
upstream.report.__defaults__ = (root, ())
claims.load_registry.__defaults__ = (root / "bench/claims/registry.json", root)
sys.exit(upstream.main())
""",
}
RUST_PROGRAMS = {"roster": ("bench-tools", ["roster"]), "normalize-thermo": ("bench-tools", ["normalize-thermo"]),
                 "derive-packet": ("bench-tools", ["derive-packet"])}
DECISIONS = re.compile(r"(?P<family>.+)-decisions\.v[^/]*\.json")


def decode(value):
    return base64.b64decode(value, validate=True)


def relative(value):
    raw = decode(value)
    if not raw or raw.startswith(b"/") or any(p in (b"", b".", b"..") for p in raw.split(b"/")) or b"\0" in raw:
        raise ValueError("filesystem paths must be nonempty relative byte paths without dot segments")
    return os.fsdecode(raw)


def canonical(value):
    if value is None:
        return b"n"
    if isinstance(value, bool):
        return b"t" if value else b"f"
    if isinstance(value, int):
        return b"i" + str(value).encode("ascii") + b";"
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ValueError("nonfinite contract number")
        return b"d" + struct.pack(">d", value)
    if isinstance(value, str):
        raw = value.encode("utf-8")
        return b"s" + str(len(raw)).encode("ascii") + b":" + raw
    if isinstance(value, list):
        return b"[" + str(len(value)).encode("ascii") + b":" + b"".join(map(canonical, value))
    if isinstance(value, dict):
        keys = sorted(value, key=lambda key: key.encode("utf-8"))
        return b"{" + str(len(keys)).encode("ascii") + b":" + b"".join(
            canonical(key) + canonical(value[key]) for key in keys)
    raise ValueError("unsupported contract value")


def digest(value):
    return hashlib.sha256(canonical(value)).hexdigest()


class UnavailableFixture(Exception):
    pass


def require_capabilities(case, root):
    for capability in case.get("requires", []):
        if capability != "non-utf8-paths":
            raise ValueError(f"unknown fixture capability {capability}")
        probe = root / os.fsdecode(b"probe-\xff")
        try:
            probe.write_bytes(b"")
        except OSError as error:
            if error.errno not in (errno.EILSEQ, errno.EINVAL):
                raise
            raise UnavailableFixture(f"{capability}: {error}") from error
        try:
            if b"probe-\xff" not in os.listdir(os.fsencode(root)):
                raise UnavailableFixture(f"{capability}: filesystem changed the filename bytes")
        finally:
            probe.unlink()


def snapshot(root):
    result = []
    for directory, dirs, files in os.walk(root):
        for name in sorted(dirs + files):
            path = Path(directory) / name
            relative_path = os.fsencode(str(path.relative_to(root)))
            entry = {"path_b64": base64.b64encode(relative_path).decode("ascii")}
            if path.is_symlink():
                entry.update(kind="symlink", target_b64=base64.b64encode(os.fsencode(os.readlink(path))).decode("ascii"))
            elif path.is_dir():
                entry.update(kind="directory")
            else:
                raw = path.read_bytes()
                entry.update(kind="file", bytes_b64=base64.b64encode(raw).decode("ascii"),
                             sha256=hashlib.sha256(raw).hexdigest())
            result.append(entry)
    return sorted(result, key=lambda item: item["path_b64"])


def execute(case, commands):
    with tempfile.TemporaryDirectory(prefix="behavior-contract-") as temporary:
        root = Path(temporary)
        require_capabilities(case, root)
        for entry in case["initial"]:
            path = root / relative(entry["path_b64"])
            path.parent.mkdir(parents=True, exist_ok=True)
            if entry["kind"] == "directory":
                path.mkdir(exist_ok=True)
            elif entry["kind"] == "file":
                path.write_bytes(decode(entry["bytes_b64"]))
            else:
                raise ValueError("initial entries support file or directory")
        home = root / ".home"
        home.mkdir()
        environment = {"PATH": os.defpath, "HOME": str(home), "LC_ALL": "C.UTF-8",
                       "TZ": "UTC", "PYTHONDONTWRITEBYTECODE": "1", "PYTHONUTF8": "1"}
        argv = [os.fsdecode(decode(arg["bytes_b64"])) if isinstance(arg, dict) else arg
                for arg in case["argv"]]
        result = subprocess.run([*commands[case["program"]], *argv], cwd=root,
                                input=decode(case["stdin_b64"]), capture_output=True,
                                env=environment, timeout=10)
        return {"exit": result.returncode, "stdout": result.stdout, "stderr": result.stderr,
                "filesystem": snapshot(root)}


def json_value(raw):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key {key!r}")
            result[key] = value
        return result
    def invalid(value):
        raise ValueError(f"nonfinite JSON number {value}")
    return json.loads(raw, object_pairs_hook=unique, parse_constant=invalid)


def pointer(value, path):
    for part in path.split("/")[1:]:
        key = part.replace("~1", "/").replace("~0", "~")
        value = value[int(key)] if isinstance(value, list) else value[key]
    return value


def compare(expected, actual):
    differences = []
    if expected["exit"] != actual["exit"]:
        differences.append(f"exit: expected {expected['exit']}, actual {actual['exit']}")
    for stream in ("stdout", "stderr"):
        spec = expected[stream]
        raw = actual[stream]
        try:
            if spec["mode"] == "bytes":
                wanted = decode(spec["bytes_b64"])
                if wanted != raw:
                    differences.append(f"{stream} bytes: expected {wanted!r}, actual {raw!r}")
            elif spec["mode"] == "prefix":
                wanted = decode(spec["bytes_b64"])
                if not raw.startswith(wanted):
                    differences.append(f"{stream} prefix: expected {wanted!r}, actual {raw!r}")
            elif spec["mode"] in ("json", "json-pointers"):
                value = json_value(raw)
                if spec["mode"] == "json-pointers":
                    value = {path: pointer(value, path) for path in spec["value"]}
                if digest(value) != digest(spec["value"]):
                    diff = "".join(difflib.unified_diff(
                        json.dumps(spec["value"], indent=2, sort_keys=True).splitlines(True),
                        json.dumps(value, indent=2, sort_keys=True).splitlines(True),
                        fromfile="expected", tofile="actual"))
                    differences.append(f"{stream} semantic:\n{diff}")
            else:
                raise ValueError(f"unknown comparison mode {spec['mode']}")
        except (ValueError, KeyError, IndexError, TypeError, UnicodeError) as error:
            differences.append(f"{stream} semantic: {error}; actual bytes {raw!r}")
    if expected["filesystem"] != actual["filesystem"]:
        differences.append("filesystem effects:\n" + "".join(difflib.unified_diff(
            json.dumps(expected["filesystem"], indent=2, sort_keys=True).splitlines(True),
            json.dumps(actual["filesystem"], indent=2, sort_keys=True).splitlines(True),
            fromfile="expected", tofile="actual")))
    return differences


def differential(left, right, case):
    expected = {"exit": left["exit"], "filesystem": left["filesystem"]}
    errors = []
    for stream in ("stdout", "stderr"):
        if case["expected"][stream]["mode"] in ("json", "json-pointers") and left["exit"] == 0:
            try:
                expected[stream] = {"mode": "json", "value": json_value(left[stream])}
            except (ValueError, UnicodeError) as error:
                errors.append(f"{stream} primary semantic: {error}; actual bytes {left[stream]!r}")
                expected[stream] = {"mode": "bytes", "bytes_b64": base64.b64encode(left[stream]).decode("ascii")}
        else:
            expected[stream] = {"mode": "bytes", "bytes_b64": base64.b64encode(left[stream]).decode("ascii")}
    return errors + compare(expected, right)


def verify_decision_history(repo):
    def git(*args):
        return subprocess.check_output(["git", "-C", str(repo), *args], stderr=subprocess.PIPE)
    def first_added(path):
        commits = git("log", "--reverse", "--format=%H", "--diff-filter=A", "HEAD", "--", path).decode().split()
        return commits[0] if commits else None
    if git("rev-parse", "--is-shallow-repository").strip() != b"false":
        raise ValueError("decision history requires a full git checkout (fetch-depth: 0)")
    directory = "tools/behavior_contracts/"
    names = {Path(path).name for path in git("log", "--format=", "--name-only", "--diff-filter=A", "HEAD", "--",
                                             directory).decode().splitlines() if path.endswith(".json")}
    names |= {path.name for path in (repo / directory).glob("*.json")}
    corpora = {}
    for name in sorted(names - {"digest-vectors.json"}):
        if not DECISIONS.fullmatch(name) and (repo / directory / name).is_file():
            corpus = json_value((repo / directory / name).read_bytes())
            if isinstance(corpus, dict) and "decisions" in corpus:
                corpora[name] = corpus["decisions"]
    families = sorted({match["family"] for name in names if (match := DECISIONS.fullmatch(name))})
    for family in families:
        verify_family(repo, directory, family, git, first_added,
                      [name for name, decisions in corpora.items() if DECISIONS.fullmatch(decisions)
                       and DECISIONS.fullmatch(decisions)["family"] == family])
    unknown = sorted(name for name, decisions in corpora.items()
                     if not (DECISIONS.fullmatch(decisions) and DECISIONS.fullmatch(decisions)["family"] in families))
    if unknown:
        raise ValueError(f"{', '.join(unknown)}: corpus must use the latest published decision version")


def verify_family(repo, directory, family, git, first_added, corpora):
    """Each published version of one decision family is unchanged, and the chain from its root is unbroken.

    A family whose v1 is the legacy decisions/1 map anchors every version at the commit that adopted v2;
    every other version, including a decisions/2 root, is anchored at its own first addition."""
    pattern = f"{family}-decisions.v*.json"
    root = directory + f"{family}-decisions.v1.json"
    root_commit = first_added(root)
    if root_commit is None:
        raise ValueError(f"{root}: decision history has no committed first version")
    legacy = "records" not in json_value(git("show", f"{root_commit}:{root}"))
    anchor = first_added(directory + f"{family}-decisions.v2.json") if legacy else root_commit
    if anchor is None:
        raise ValueError("decision history has no committed v2 adoption")
    releases = {path: anchor for path in git("ls-tree", "-r", "--name-only", anchor, "--", directory).decode().splitlines()
                if Path(path).match(pattern)}
    for commit in git("log", "--reverse", "--format=%H", "--diff-filter=A", anchor + "..HEAD", "--",
                      directory + pattern).decode().splitlines():
        for path in git("diff-tree", "--root", "--no-commit-id", "--name-only", "-r", "--diff-filter=A",
                        commit, "--", directory + pattern).decode().splitlines():
            releases.setdefault(path, commit)
    for path, commit in releases.items():
        current = repo / path
        if not current.is_file() or current.read_bytes() != git("show", commit + ":" + path):
            raise ValueError(f"{path}: published decision changed; add a new version superseding it")
    versions = [directory + f"{family}-decisions.v{number}.json" for number in range(1, len(releases) + 1)]
    if set(versions) != set(releases):
        raise ValueError("decision history versions must be contiguous")
    if not corpora:
        raise ValueError(f"{family}: no corpus uses this decision history")
    for corpus in corpora:
        if json_value((repo / directory / corpus).read_bytes())["decisions"] != Path(versions[-1]).name:
            raise ValueError(f"{corpus}: corpus must use the latest published decision version")
    if not legacy and "previous" in json_value((repo / versions[0]).read_bytes()):
        raise ValueError(f"{versions[0]}: a first decision version has no predecessor")
    for number, path in enumerate(versions[1:], 2):
        previous = json_value((repo / path).read_bytes()).get("previous", {})
        expected_format = "decisions/1" if legacy and number == 2 else "decisions/2"
        if previous.get("path") != Path(versions[number - 2]).name or previous.get("format") != expected_format:
            raise ValueError(f"{path}: decision history must retain its immediate predecessor")
    load_decisions(repo / versions[-1])


def default_commands(corpus):
    """The maintained implementation of each program the corpus runs."""
    commands = {}
    for program in sorted({case["program"] for case in corpus["cases"]}):
        if program in PYTHON_PROGRAMS:
            commands[program] = [sys.executable, str(REPO / PYTHON_PROGRAMS[program])]
        elif program in PYTHON_DRIVERS:
            commands[program] = [sys.executable, "-c", PYTHON_DRIVERS[program], str(REPO / "bench/tools")]
        elif program in RUST_PROGRAMS:
            if str(REPO / "bench/tools") not in sys.path:
                sys.path.insert(0, str(REPO / "bench/tools"))
            import rust_runtime
            name, arguments = RUST_PROGRAMS[program]
            try:
                commands[program] = [str(rust_runtime.resolve(name)), *arguments]
            except (rust_runtime.Refused, rust_runtime.InputError) as error:
                raise ValueError(f"{program}: {error}") from None
    return commands


def load_decisions(path):
    decisions = json_value(path.read_bytes())
    previous = {}
    if "previous" in decisions:
        source = decisions["previous"]
        prior_path = path.parent / source["path"]
        if hashlib.sha256(prior_path.read_bytes()).hexdigest() != source["sha256"]:
            raise ValueError("previous decisions changed")
        previous = load_decisions(prior_path) if source["format"] == "decisions/2" else json_value(prior_path.read_bytes())
    records = decisions["records"]
    if set(previous) - set(records):
        raise ValueError("decision history cannot remove cases")
    for name, record in records.items():
        payload = {key: value for key, value in record.items() if key != "decision"}
        if not record["reason"].strip() or record["decision"] != "decision-" + digest(payload):
            raise ValueError(f"{name}: changed contract needs an explicit decision record")
        old = previous.get(name)
        if old and record != old:
            if (record.get("supersedes") != old["contract_sha256"]
                    or record["decision"] == old["decision"] or record["reason"] == old["reason"]):
                raise ValueError(f"{name}: new decision must supersede the previous digest with a new reason")
    return records


def load_corpus(path):
    corpus = json_value(path.read_bytes())
    if corpus["format"] != "behavior-contract/1":
        raise ValueError("unsupported corpus format")
    decisions = load_decisions(path.parent / corpus["decisions"])
    ids = set()
    for case in corpus["cases"]:
        if case["id"] in ids:
            raise ValueError("duplicate case id")
        ids.add(case["id"])
        decision = decisions[case["id"]]
        if not decision["reason"] or decision["contract_sha256"] != digest(case):
            raise ValueError(f"{case['id']}: changed contract needs an explicit decision")
    if ids != set(decisions):
        raise ValueError("corpus case ids differ from decisions; removal needs an explicit decision")
    return corpus


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=CORPUS)
    parser.add_argument("--implementations", type=Path)
    parser.add_argument("--differential", type=Path)
    parser.add_argument("--allow-known-defects", action="store_true")
    parser.add_argument("--verify-decision-history", action="store_true")
    args = parser.parse_args()
    try:
        if args.verify_decision_history:
            verify_decision_history(REPO)
            print("Published decision history verified")
            return 0
        corpus = load_corpus(args.corpus)
        commands = (json.loads(args.implementations.read_text(encoding="utf-8")) if args.implementations
                    else default_commands(corpus))
        other = json.loads(args.differential.read_text(encoding="utf-8")) if args.differential else None
        failed = 0
        for case in corpus["cases"]:
            try:
                actual = execute(case, commands)
            except UnavailableFixture as error:
                print(f"SKIP UNAVAILABLE {case['id']}: {error}")
                continue
            differences = compare(case["expected"], actual)
            known = case.get("baseline_defect")
            if differences and args.allow_known_defects and known and not compare(known["observed"], actual):
                print(f"KNOWN DEFECT {case['id']}: {known['resolution']}\n" + "\n".join(differences))
            elif differences:
                failed += 1
                print(f"FAIL {case['id']}\n" + "\n".join(differences))
            else:
                print(f"PASS {case['id']}")
            if other:
                candidate = execute(case, other)
                differences = differential(actual, candidate, case)
                if differences:
                    approved_fix = (args.allow_known_defects and known
                                    and not compare(known["observed"], actual)
                                    and not compare(case["expected"], candidate))
                    if approved_fix:
                        print(f"APPROVED FIX {case['id']}: {known['resolution']}\n" + "\n".join(differences))
                    else:
                        failed += 1
                        print(f"DIFFERENTIAL {case['id']}\n" + "\n".join(differences))
        return 1 if failed else 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(f"behavior_contract: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
