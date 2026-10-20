#!/usr/bin/env python3
"""Run language-independent process contracts in disposable directories.

Usage: python3 tools/behavior_contract.py [--corpus corpus.json] [--implementations commands.json]
       [--differential commands.json] [--allow-known-defects]
       python3 tools/behavior_contract.py --real-record [list.json] [--root ROOT] [--implementations commands.json]
       --differential commands.json
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
import fnmatch
import gzip
import hashlib
import io
import json
from json.decoder import WHITESPACE, scanstring
from json.scanner import NUMBER_RE
import math
import os
from pathlib import Path
import posixpath
import shlex
import shutil
import stat
import subprocess
import re
import struct
import sys
import tarfile
import tempfile
import zipfile

REPO = Path(__file__).resolve().parents[1]
CORPUS = REPO / "tools/behavior_contracts/usage.v1.json"
PYTHON_PROGRAMS = {"codex": "bench/tools/codex_usage.py", "transcript": "bench/tools/transcript_usage.py",
                   "diff-identity": "bench/tools/diff_identity.py", "normalize-review": "bench/tools/normalize_review.py",
                   "evidence-store": "bench/tools/evidence_store.py",
                   "python-check-manifest": "bench/tools/check_manifest.py",
                   "evidence-inventory": "bench/tools/evidence_inventory.py"}
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
upstream.capture.__defaults__ = (root,)
claims.load_registry.__defaults__ = (root / "bench/claims/registry.json", root)
sys.exit(upstream.main())
""",
    # The retained native_artifacts.py, which bench-tools native-artifacts replaced. file_artifacts has no command
    # line; `file` runs it and unindexed with the module's own refusals and exit status, and prints both as
    # bench-tools native-artifacts file does.
    "python-native-artifacts": """import sys
from pathlib import Path
sys.path.insert(0, sys.argv.pop(1))
sys.argv[0] = "native_artifacts.py"
import native_artifacts
if sys.argv[1:2] != ["file"]:
    native_artifacts.main()
    sys.exit(0)
import argparse, json, zipfile
parser = argparse.ArgumentParser(prog="native_artifacts.py file")
parser.add_argument("root", type=Path)
parser.add_argument("index", type=Path)
parser.add_argument("out", type=Path)
parser.add_argument("--native-relative")
args = parser.parse_args(sys.argv[2:])
try:
    pin = native_artifacts.file_artifacts(args.root, args.index, args.out, args.native_relative)
    left_out = native_artifacts.unindexed(args.root, args.index)
except (native_artifacts.ArtifactError, OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
    parser.exit(1, f"native_artifacts.py: {error}\\n")
print(json.dumps({"pin": pin, "unindexed": left_out}))
""",
    # file_attempt.archive_transcripts and recorded_archive_path as they stood before bench-tools evidence
    # archive-transcripts replaced them, from a command line with that command's arguments.
    "python-archive-transcripts": """import argparse, hashlib, json, os, sys, tarfile, tempfile
from pathlib import Path
sys.argv.pop(1)
def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()
def sha256_file(path):
    with open(path, "rb") as handle:
        return sha256_bytes(handle.read())
def recorded_archive_path(dest, out):
    parts = Path(out).parts
    for index in range(len(parts) - 2, 0, -1):
        if parts[index:index + 2] == ("bench", "runs") and Path(dest).is_relative_to(Path(*parts[:index])):
            return Path(dest).relative_to(Path(*parts[:index])).as_posix()
    home = os.path.expanduser("~")
    return dest.replace(home, "~", 1) if dest.startswith(home) else dest
def archive_transcripts(paths, attempt_dir, dest, out):
    dest = os.path.abspath(dest)
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    with tarfile.open(dest, "w:gz") as tar:
        for path in sorted(set(paths)):
            tar.add(path, arcname=os.path.relpath(path, attempt_dir))
    ok = True
    with tempfile.TemporaryDirectory() as scratch:
        with tarfile.open(dest, "r:gz") as tar:
            members = tar.getmembers()
            for member in members:
                if not member.isfile() or member.name.startswith(("/", "..")):
                    ok = False
                    continue
                data = tar.extractfile(member).read()
                if sha256_bytes(data) != sha256_file(os.path.join(attempt_dir, member.name)):
                    ok = False
            if len(members) != len(set(paths)):
                ok = False
    return {"path": recorded_archive_path(dest, out), "sha256": sha256_file(dest),
            "restoration_check": "passed" if ok else "failed"}
parser = argparse.ArgumentParser(prog="archive-transcripts")
parser.add_argument("--attempt-dir", required=True)
parser.add_argument("--dest", required=True)
parser.add_argument("--out", required=True)
parser.add_argument("paths", nargs="*")
args = parser.parse_args()
print(json.dumps(archive_transcripts(args.paths, args.attempt_dir, args.dest, args.out)))
""",
    # prune_workspace resolves an archive through the retained evidence_store.resolve; this makes that one call, so
    # a step case can show Python resolving what Rust restored and the reverse.
    "python-resolve": """import sys
from pathlib import Path
sys.path.insert(0, sys.argv.pop(1))
import evidence_store
try:
    print(evidence_store.resolve(Path.cwd(), sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else None))
except evidence_store.EvidenceError as error:
    print(f"evidence: {error}")
    sys.exit(1)
""",
    # `ruling {check,route,summary,precedents} [--root DIR]` and `ruling dossier DIR`, as bench-tools spells them, run
    # the retained ruling_record.py and ruling_dossier.py. ruling_record.py reads the checkout that holds it, and its
    # functions bind that root as a default argument, so the driver rebinds them to the case directory and runs the
    # command-line block with `__file__` placed there. `check` is ruling_record.py without a command word.
    "ruling": """import runpy, sys, types
from pathlib import Path
tools = Path(sys.argv.pop(1))
sys.path.insert(0, str(tools))
argv, root = sys.argv[1:], Path.cwd()
for at, argument in enumerate(argv):
    if argument == "--":
        break
    if argument == "--root" and at + 1 < len(argv):
        root = Path(argv[at + 1]).resolve()
        del argv[at:at + 2]
        break
    if argument.startswith("--root="):
        root = Path(argument.removeprefix("--root=")).resolve()
        del argv[at]
        break
if argv[:1] == ["dossier"]:
    sys.argv = [str(tools / "ruling_dossier.py"), *argv[1:]]
    runpy.run_path(sys.argv[0], run_name="__main__")
    sys.exit(0)
import delegation, ruling_record, ruling_summary
for module in (ruling_record, ruling_summary, delegation):
    for value in vars(module).values():
        if isinstance(value, types.FunctionType) and value.__defaults__:
            value.__defaults__ = tuple(root if default is ruling_record.ROOT else default for default in value.__defaults__)
ruling_record.ROOT = root
script = tools / "ruling_record.py"
sys.argv = [str(script), *(argv[1:] if argv[:1] == ["check"] else argv)]
code = compile(script.read_text(encoding="utf-8"), str(script), "exec")
exec(code, {"__name__": "__main__", "__file__": str(root / "bench/tools/ruling_record.py"), "__builtins__": __builtins__})
""",
    # `claims [--root DIR] {check,inventory,dossier,evidence}` runs the retained claims.py. It and the modules it
    # imports read the checkout that holds them and bind that root as a default argument, so the driver rebinds them to
    # the case directory, or the `--root` argument before the command word, as bench-tools reads the current directory.
    "claims": """import sys, types
from pathlib import Path
tools = Path(sys.argv.pop(1))
sys.path.insert(0, str(tools))
argv, root = sys.argv[1:], Path.cwd()
for at, argument in enumerate(argv):
    if argument in ("--", "check", "inventory", "dossier", "evidence"):
        break
    if argument == "--root" and at + 1 < len(argv):
        root = Path(argv[at + 1]).resolve()
        del argv[at:at + 2]
        break
    if argument.startswith("--root="):
        root = Path(argument.removeprefix("--root=")).resolve()
        del argv[at]
        break
import claims, current_grading, upstream
for module in (claims, current_grading, upstream):
    for value in vars(module).values():
        if isinstance(value, types.FunctionType) and value.__defaults__:
            value.__defaults__ = tuple(root if default == module.ROOT else default for default in value.__defaults__)
claims.ROOT = upstream.ROOT = current_grading.ROOT = root
claims.DEFAULT_REGISTRY = root / "bench/claims/registry.json"
claims.DEFAULT_EXTRACTS = root / "bench/claims/evidence-extracts.v1.json"
sys.argv = [str(tools / "claims.py"), *argv]
sys.exit(claims.main())
""",
    # grading_validation.py reads inputs.json one directory above its own file, where grade.py copies it into a
    # workspace; `bench-tools grading validate` names that file with --inputs. Pointing the module's __file__ beside
    # the named file makes the retained script read it in place, so the case tree holds no copied tools.
    "grading": """import argparse, sys
from pathlib import Path
sys.path.insert(0, sys.argv.pop(1))
import grading_validation
if sys.argv[1:2] != ["validate"]:
    sys.exit("grading: the retained Python implements only validate")
parser = argparse.ArgumentParser(prog="grading_validation.py", add_help=False)
parser.add_argument("--inputs", required=True)
known, rest = parser.parse_known_args(sys.argv[2:])
if Path(known.inputs).name != "inputs.json":
    parser.error("grading_validation.py reads only a file named inputs.json")
grading_validation.__file__ = str(Path(known.inputs).parent / "tools" / "grading_validation.py")
sys.argv = ["grading_validation.py", *rest]
sys.exit(grading_validation.main())
""",
    # archive_attempt has no command line; regrade.py calls it in process and stops on the failures it catches.
    # `ATTEMPT ARCHIVE_DIR` takes both from the case directory, the repository the receipt's paths are relative to,
    # as `bench-tools regrade archive-attempt` does.
    "regrade-archive-attempt": """import json, sys
from pathlib import Path
sys.path.insert(0, sys.argv.pop(1))
import regrade
regrade.ROOT = Path.cwd()
attempt, archives = (Path.cwd() / argument for argument in sys.argv[1:])
try:
    print(json.dumps(regrade.archive_attempt(attempt, archives)))
except regrade.FAILURES as error:
    print(f"regrade.py: {error}", file=sys.stderr)
    sys.exit(2)
""",
    # `current-grading {inventory,check,status} [--root DIR]` runs the retained current_grading.py, whose `--root`
    # defaults to the case directory here, as bench-tools reads the current directory. `fingerprints` has no Python
    # command; it prints what regrade.py reads in process: each selected batch with batch_state's fingerprint and
    # state, read without checking saved grades.
    # methodology.py reads the repository that holds it; the driver reads the case directory, as bench-tools
    # methodology reads the current directory.
    "methodology": """import sys
from pathlib import Path
sys.path.insert(0, sys.argv.pop(1))
sys.argv[0] = "methodology.py"
import methodology
methodology.ROOT = Path.cwd()
methodology.main()
""",
    # calibration.py's `--root` defaults to current_grading.ROOT and evaluator_audit.py's to the repository holding
    # it; both drivers read the case directory, as bench-tools reads the current directory.
    "calibration": """import sys
from pathlib import Path
sys.path.insert(0, sys.argv.pop(1))
sys.argv[0] = "calibration.py"
import current_grading
current_grading.ROOT = Path.cwd()
import calibration
sys.exit(calibration.main())
""",
    "evaluator-audit": """import sys
from pathlib import Path
sys.path.insert(0, sys.argv.pop(1))
sys.argv[0] = "evaluator_audit.py"
import evaluator_audit
evaluator_audit.ROOT = Path.cwd()
sys.exit(evaluator_audit.main())
""",
    "current-grading": """import argparse, json, sys
from pathlib import Path
sys.path.insert(0, sys.argv.pop(1))
sys.argv[0] = "current_grading.py"
import current_grading
current_grading.ROOT = Path.cwd()
parser = argparse.ArgumentParser(description=current_grading.__doc__.splitlines()[0])
parser.add_argument("operation", choices=("inventory", "check", "status", "fingerprints"))
parser.add_argument("--root", type=Path, default=current_grading.ROOT)
parser.add_argument("--current", default=str(current_grading.CURRENT))
parser.add_argument("--out", type=Path)
args = parser.parse_args()
if args.operation != "fingerprints":
    sys.exit(current_grading.main())
try:
    selected, documents = current_grading.load_current(args.root, args.current, grades=False)
    rows = []
    for batch in selected["batches"]:
        fingerprint, state = current_grading.batch_state(batch, selected, documents, args.root)
        rows.append({**batch, "input_fingerprint": fingerprint, "state": state})
    print(json.dumps(rows, indent=2))
except current_grading.Inconsistent as error:
    print(str(error))
    sys.exit(1)
except (current_grading.InputError, OSError) as error:
    print(f"current_grading.py: {error}", file=sys.stderr)
    sys.exit(2)
""",
}
DRIVER_ARGUMENTS = {}
RUST_PROGRAMS = {"roster": ("bench-tools", ["roster"]), "normalize-thermo": ("bench-tools", ["normalize-thermo"]),
                 "derive-packet": ("bench-tools", ["derive-packet"]), "build-packet": ("bench-tools", ["build-packet"]),
                 "export-explorer": ("bench-tools", ["export-explorer"]), "archive": ("bench-tools", ["archive"]),
                 "evidence": ("bench-tools", ["evidence"]), "recover-evidence": ("bench-tools", ["evidence", "recover"]),
                 "restore-local": ("bench-tools", ["evidence", "restore-local"]),
                 "collect-run": ("bench-tools", ["evidence", "collect"]),
                 "reconcile-evidence": ("bench-tools", ["evidence", "reconcile"]),
                 "native-artifacts": ("bench-tools", ["native-artifacts"]),
                 "archive-transcripts": ("bench-tools", ["evidence", "archive-transcripts"]),
                 "import-benchmark": ("bench-tools", ["import-benchmark"]),
                 "regrade-status": ("bench-tools", ["regrade", "status", "--"]),
                 "audit-log": ("bench-tools", ["audit-log"]),
                 "check-manifest": ("bench-tools", ["check-manifest"])}
# The retained Python each moved program replaced, which a differential runs as the primary.
PYTHON_COUNTERPARTS = {"native-artifacts": "python-native-artifacts", "archive-transcripts": "python-archive-transcripts",
                       "check-manifest": "python-check-manifest"}
# Python programs that draw blind tokens or shuffles run under ENTROPY_DRIVER, so a recorded expectation is the
# same on every recording. The grading slices of #156 add their programs here, each by its script or its driver.
PYTHON_ENTROPY = {"claims": "claims"}
ENTROPY_DRIVER = """import hashlib, random, runpy, secrets, sys
from pathlib import Path
drawn = 0
def token_hex(nbytes=None):
    global drawn
    count, raw = 32 if nbytes is None else nbytes, b""
    while len(raw) < count:
        raw += hashlib.sha256(b"behavior-contract-entropy:%d" % drawn).digest()
        drawn += 1
    return raw[:count].hex()
secrets.token_hex = token_hex
fixed = random.Random(20261019)
random.SystemRandom = lambda *args, **kwargs: fixed
script = sys.argv.pop(1)
if script == "-c":
    source = sys.argv.pop(1)
    sys.argv[0] = "-c"
    exec(compile(source, "<driver>", "exec"), {"__name__": "__main__", "__builtins__": __builtins__})
    sys.exit(0)
sys.path.insert(0, str(Path(script).parent))
sys.argv[0] = script
runpy.run_path(script, run_name="__main__")
"""
# Setup programs that are part of a case, not of an implementation, so every command map runs the same one.
# `fixture-git [--directory DIR] [FILE ...]` commits DIR, by default the case root, with a fixed identity, clock and
# no compression, removes the config and message, whose bytes depend on the host, rewrites the index from the commit
# without file timestamps, and writes the commit into each named file in place of FREEZE_PLACEHOLDER. The resulting
# objects, refs and index are the same bytes on every host until a later command refreshes the index.
FREEZE_PLACEHOLDER = "<freeze commit>"
SETUP_PROGRAMS = {"fixture-git": """import os, subprocess, sys
from pathlib import Path
fixed = {"GIT_AUTHOR_NAME": "Fixture", "GIT_AUTHOR_EMAIL": "fixture@example.invalid", "GIT_CONFIG_NOSYSTEM": "1",
         "GIT_COMMITTER_NAME": "Fixture", "GIT_COMMITTER_EMAIL": "fixture@example.invalid",
         "GIT_AUTHOR_DATE": "2026-01-01T00:00:00Z", "GIT_COMMITTER_DATE": "2026-01-01T00:00:00Z"}
environment = {**os.environ, **fixed}
names, directory = sys.argv[1:], Path(".")
if names[:1] == ["--directory"]:
    directory, names = Path(names[1]), names[2:]
def git(*args):
    return subprocess.run(["git", "-C", str(directory), "-c", "core.compression=0", "-c",
                           "core.logAllRefUpdates=false", "-c", "commit.gpgsign=false", *args], env=environment,
                          check=True, capture_output=True).stdout
git("init", "-q", "--template=", "-b", "main")
git("add", "-A")
git("commit", "-q", "-m", "Freeze the fixture run")
commit = git("rev-parse", "HEAD").strip()
for name in ("index", "COMMIT_EDITMSG", "config"):
    (directory / ".git" / name).unlink(missing_ok=True)
git("read-tree", "HEAD")
for name in names:
    path = Path(name)
    path.write_bytes(path.read_bytes().replace(%r, commit))
""" % FREEZE_PLACEHOLDER.encode("ascii")}
GRADING_OUTCOMES = ("family-outcome", "credit-fact", "unresolved-state", "candidate-state")
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


def snapshot(root, modes=False):
    """Every entry below root. A special file is recorded by kind and mode without being opened."""
    result = []
    for directory, dirs, files in os.walk(root):
        for name in sorted(dirs + files):
            path = Path(directory) / name
            relative_path = os.fsencode(str(path.relative_to(root)))
            entry = {"path_b64": base64.b64encode(relative_path).decode("ascii")}
            info = path.lstat()
            if stat.S_ISLNK(info.st_mode):
                entry.update(kind="symlink", target_b64=base64.b64encode(os.fsencode(os.readlink(path))).decode("ascii"))
            elif stat.S_ISDIR(info.st_mode):
                entry.update(kind="directory")
            elif stat.S_ISREG(info.st_mode):
                raw = path.read_bytes()
                entry.update(kind="file", bytes_b64=base64.b64encode(raw).decode("ascii"),
                             sha256=hashlib.sha256(raw).hexdigest())
            else:
                entry.update(kind="special", mode=stat.S_IMODE(info.st_mode))
            if modes and entry["kind"] in ("file", "directory"):
                entry["mode"] = stat.S_IMODE(info.st_mode)
            result.append(entry)
    return sorted(result, key=lambda item: item["path_b64"])


def populate(root, entries):
    modes = []
    for entry in entries:
        path = root / relative(entry["path_b64"])
        path.parent.mkdir(parents=True, exist_ok=True)
        if path.is_symlink() or path.exists() and not path.is_dir():
            path.unlink()
        if entry["kind"] == "directory":
            path.mkdir(exist_ok=True)
        elif entry["kind"] == "file":
            path.write_bytes(decode(entry["bytes_b64"]))
        elif entry["kind"] == "symlink":
            os.symlink(os.fsdecode(decode(entry["target_b64"])), path)
        elif entry["kind"] == "special":
            os.mkfifo(path)
        else:
            raise ValueError("initial entries support file, directory, symlink or special")
        if "mode" in entry:
            if entry["kind"] == "symlink":
                raise ValueError("a symlink has no mode of its own")
            modes.append((path, entry["mode"]))
    for path, mode in sorted(modes, key=lambda item: -len(item[0].parts)):
        path.chmod(mode)


def remove(root, paths):
    """Remove each path that exists. A path an implementation failed to create is left to the next snapshot."""
    for value in paths:
        path = root / relative(value)
        if path.is_symlink() or path.exists() and not path.is_dir():
            path.unlink()
        elif path.is_dir():
            shutil.rmtree(path)


# Stands in for `gh` in cases whose corpus names a state directory. It answers only the release reads, upload and
# asset download that evidence storage runs, and the paginated GET reads of `upstream collect`. It records every
# argument list in calls.jsonl, and takes its release, assets, upstream responses and injected faults from that
# directory, so no case reaches the network. `upstream collect` reads four endpoints at a time, so the stub keeps
# calls.jsonl sorted when it records one of those reads, and the file is the same whatever order they ran in.
GH_STUB = r"""import fcntl, json, os, sys
from pathlib import Path
from urllib.parse import quote
state = Path(os.environ["CONTRACT_GH_STATE"])
args = sys.argv[1:]
state.mkdir(parents=True, exist_ok=True)
upstream_read = len(args) == 6 and args[:5] == ["api", "--method", "GET", "--paginate", "--slurp"]
with (state / "calls.jsonl").open("a+", encoding="utf-8") as log:
    fcntl.flock(log, fcntl.LOCK_EX)
    if upstream_read:
        log.seek(0)
        lines = sorted([*log.read().splitlines(), json.dumps(args)])
        log.seek(0)
        log.truncate()
        log.write("".join(line + "\n" for line in lines))
    else:
        log.write(json.dumps(args) + "\n")
def fail(message, code=1):
    sys.stderr.write(message + "\n")
    sys.exit(code)
if upstream_read:
    # upstream.json maps each endpoint to the response its read gets: an exit status, stdout given as JSON pages,
    # as text or as base64 bytes, and stderr. An endpoint it does not list answers 404.
    try:
        responses = json.loads((state / "upstream.json").read_text(encoding="utf-8"))
    except FileNotFoundError:
        responses = {}
    response = responses.get(args[5])
    if response is None:
        fail("gh: Not Found (HTTP 404)")
    if "pages" in response:
        sys.stdout.write(json.dumps(response["pages"]))
    elif "stdout_b64" in response:
        import base64
        sys.stdout.buffer.write(base64.b64decode(response["stdout_b64"]))
    else:
        sys.stdout.write(response.get("stdout", ""))
    sys.stdout.flush()
    sys.stderr.write(response.get("stderr", ""))
    sys.exit(response.get("exit", 0))
try:
    release = json.loads((state / "release.json").read_text(encoding="utf-8"))
except FileNotFoundError:
    fail("HTTP 404: Not Found")
faults = set(release.get("faults", []))
routes = f"repos/{release['repository']}/releases/"
stored = state / "assets"
def assets():
    found = [{"id": n, "name": f"filler-{n}.bin", "size": 1} for n in range(1, release.get("filler_assets", 0) + 1)]
    if stored.is_dir():
        for directory in sorted(stored.iterdir(), key=lambda path: int(path.name)):
            for file in sorted(directory.iterdir()):
                found.append({"id": int(directory.name), "name": file.name, "size": file.stat().st_size})
    return found
if args == ["api", routes + "tags/" + quote(release["tag"], safe="")]:
    print(json.dumps({"id": release["id"], "tag_name": release["tag"], "draft": release.get("draft", False)}))
elif args == ["api", "--paginate", "--slurp", routes + f"{release['id']}/assets?per_page=100"]:
    listed = assets()
    print(json.dumps([listed[at:at + 100] for at in range(0, len(listed), 100)] or [[]]))
elif args[:2] == ["release", "upload"] and len(args) == 6 and args[2] == release["tag"] and args[4:] == ["--repo", release["repository"]]:
    data = Path(args[3]).read_bytes()
    if any(asset["name"] == Path(args[3]).name for asset in assets()):
        fail("asset under the same name already exists")
    if "rejected-upload" in faults:
        fail("HTTP 422: upload rejected")
    target = stored / str(max([100] + [asset["id"] for asset in assets()]) + 1) / Path(args[3]).name
    target.parent.mkdir(parents=True)
    target.write_bytes(data[:len(data) // 2] if "partial-upload" in faults else data)
    if faults & {"partial-upload", "lost-upload-response"}:
        fail("connection reset by peer")
elif len(args) == 4 and args[0] == "api" and args[1].startswith(routes + "assets/") and args[2:] == ["-H", "Accept: application/octet-stream"]:
    matches = [stored / str(asset["id"]) / asset["name"] for asset in assets()
               if routes + f"assets/{asset['id']}" == args[1] and asset["id"] > release.get("filler_assets", 0)]
    if not matches:
        fail("HTTP 404: Not Found")
    data = matches[0].read_bytes()
    if "truncated-download" in faults:
        data = data[:len(data) // 2]
    if "oversized-download" in faults:
        data += b"\0" * 64
    sys.stdout.buffer.write(data)
else:
    fail("contract gh stub: unsupported arguments", 2)
"""


def command_for(commands, program, argv):
    """A program's command prefix. A command map may route a program by its first argument, as
    {"fetch": [...], "pack": [...]}, while only some of its subcommands have moved to another implementation."""
    command = commands[program]
    if isinstance(command, dict):
        if not argv or argv[0] not in command:
            raise ValueError(f"{program}: the command map routes no subcommand {argv[:1]}")
        return command[argv[0]]
    return command


def execute(case, commands, options=None):
    """Run a case's command, or each of its steps on one shared root, and observe the results."""
    options = options or {}
    commands = with_setup(commands)
    modes = options.get("snapshot_modes", False)
    with tempfile.TemporaryDirectory(prefix="behavior-contract-") as temporary, \
            tempfile.TemporaryDirectory(prefix="behavior-contract-bin-") as tools:
        root = Path(temporary)
        require_capabilities(case, root)
        previous_umask = os.umask(0o022) if modes else None
        try:
            populate(root, case["initial"])
            home = root / ".home"
            home.mkdir()
        finally:
            if previous_umask is not None:
                os.umask(previous_umask)
        environment = {"PATH": os.defpath, "HOME": str(home), "LC_ALL": "C.UTF-8",
                       "TZ": "UTC", "PYTHONDONTWRITEBYTECODE": "1", "PYTHONUTF8": "1"}
        if "gh_stub_b64" in options:
            stub = Path(tools) / "gh"
            (Path(tools) / "gh.py").write_text(GH_STUB, encoding="utf-8")
            stub.write_text(f'#!/bin/sh\nexec {shlex.quote(sys.executable)} {shlex.quote(str(Path(tools) / "gh.py"))} "$@"\n')
            stub.chmod(0o755)
            environment.update(PATH=tools + os.pathsep + os.defpath,
                               CONTRACT_GH_STATE=str(root / relative(options["gh_stub_b64"])))
        results = []
        for step in case.get("steps", [case]):
            if "remove" in step:
                remove(root, step["remove"])
                continue
            if "shell" in step:
                subprocess.run(["/bin/sh", "-c", step["shell"]], cwd=root, env=environment, check=True,
                               capture_output=True, timeout=10)
                continue
            if "write" in step:
                previous_umask = os.umask(0o022) if modes else None
                try:
                    populate(root, step["write"])
                finally:
                    if previous_umask is not None:
                        os.umask(previous_umask)
                continue
            argv = [os.fsdecode(decode(arg["bytes_b64"])) if isinstance(arg, dict) else arg
                    for arg in step["argv"]]
            result = subprocess.run([*command_for(commands, step.get("program", case.get("program")), argv), *argv], cwd=root,
                                    input=decode(step["stdin_b64"]), capture_output=True,
                                    env=environment, timeout=10, **({"umask": 0o022} if modes else {}))
            results.append(normalize(case, root, {"exit": result.returncode, "stdout": result.stdout,
                                                  "stderr": result.stderr, "filesystem": snapshot(root, modes)}))
        return {"steps": results} if "steps" in case else results[0]


def json_spans(text, pointers, keys=()):
    """Character spans of the values at the given RFC 6901 pointers in valid JSON text, and of each key, as
    (pointer, key), of the objects at the pointers in keys."""
    spans = {}

    def skip(position):
        return WHITESPACE.match(text, position).end()

    def value(position, path):
        position = skip(position)
        start, char = position, text[position]
        if char in "{[":
            close, index = "}" if char == "{" else "]", 0
            position = skip(position + 1)
            if text[position] == close:
                position += 1
            else:
                while True:
                    if char == "{":
                        start_key = skip(position)
                        key, position = scanstring(text, start_key + 1)
                        if path in keys:
                            spans[(path, key)] = (start_key, position)
                        child = path + "/" + key.replace("~", "~0").replace("/", "~1")
                        position = skip(position) + 1
                    else:
                        child, index = f"{path}/{index}", index + 1
                    position = skip(value(position, child)) + 1
                    if text[position - 1] == close:
                        break
        elif char == '"':
            position = scanstring(text, position + 1)[1]
        else:
            number = NUMBER_RE.match(text, position)
            position = number.end() if number else position + next(
                len(word) for word in ("true", "false", "null") if text.startswith(word, position))
        if path in pointers:
            spans[path] = (start, position)
        return position

    value(0, "")
    return spans


def archive_members(raw, form):
    members = []
    if form == "tar.gz":
        with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as bundle:
            for item in bundle:
                raw_name = os.fsencode(item.name)
                try:
                    name = {"name": raw_name.decode("utf-8")}
                except UnicodeDecodeError:
                    name = {"name_b64": base64.b64encode(raw_name).decode("ascii")}
                member = {**name, "type": item.type.decode("latin-1"), "size": item.size, "mode": item.mode}
                if item.isfile():
                    member["sha256"] = hashlib.sha256(bundle.extractfile(item).read()).hexdigest()
                members.append(member)
    elif form == "gzip":
        data = gzip.decompress(raw)
        members.append({"size": len(data), "sha256": hashlib.sha256(data).hexdigest(),
                        "bytes_b64": base64.b64encode(data).decode("ascii")})
    elif form == "zip":
        with zipfile.ZipFile(io.BytesIO(raw)) as bundle:
            for item in bundle.infolist():
                members.append({"name": item.filename, "size": item.file_size, "mode": item.external_attr >> 16,
                                "sha256": hashlib.sha256(bundle.read(item)).hexdigest()})
    else:
        raise ValueError(f"unknown archive format {form}")
    return members


def normalize(case, root, actual):
    """Apply the case's declared archive, digest, manifest, reference, volatile and root comparisons.

    A declared archive is compared by its members, and its SHA-256 and that of each declared digest file are
    replaced everywhere by a label, so an implementation that compresses differently still matches. Restored and
    pre-existing files match no declaration and keep exact byte comparison. Each named evidence manifest must name
    the archive bytes actually beside it; the size of a declared archive is masked only after that check. Each
    declared storage reference must name, by path and SHA-256, a file written beside its record, checked before
    any digest becomes a label."""
    if "structured" in case:
        return structured(case, root, actual)
    declared = ("archives", "digests", "manifests", "references", "volatile", "mask_root", "mask_patterns")
    if not any(key in case for key in declared):
        return actual
    entries = {os.fsdecode(decode(entry["path_b64"])): dict(entry) for entry in actual["filesystem"]}
    raw = {name: decode(entry["bytes_b64"]) for name, entry in entries.items() if entry["kind"] == "file"}
    issues = []
    archives = []
    for rule in case.get("archives", []):
        for name in sorted(raw):
            if fnmatch.fnmatchcase(name, rule["glob"]):
                try:
                    members = archive_members(raw[name], rule["format"])
                except (tarfile.TarError, zipfile.BadZipFile, gzip.BadGzipFile, OSError, EOFError, ValueError) as error:
                    issues.append(f"{name}: unreadable {rule['format']} archive: {error}")
                    members = None
                archives.append((json.dumps(members, sort_keys=True), name, rule["format"], members))
    labels = {}
    for number, (_, name, form, members) in enumerate(sorted(archives), 1):
        labels[hashlib.sha256(raw[name]).hexdigest()] = f"<archive {number}>"
        entries[name].update(kind="archive", format=form, members=members)
    def substitute(data):
        for sha, label in labels.items():
            data = data.replace(sha.encode("ascii"), label.encode("ascii"))
        return data
    for pattern in case.get("digests", []):
        for name in sorted(raw):
            if fnmatch.fnmatchcase(name, pattern):
                labels.setdefault(hashlib.sha256(raw[name]).hexdigest(),
                                  f"<sha256 of {os.fsdecode(substitute(os.fsencode(name)))}>")
    masks = {}
    for name in case.get("manifests", []):
        if name not in raw:
            continue
        try:
            packages = json_value(raw[name])["packages"]
        except (KeyError, ValueError, UnicodeError) as error:
            issues.append(f"{name}: unreadable evidence manifest: {error}")
            continue
        for index, package in enumerate(packages):
            asset = str(Path(name).parent / str(package.get("asset")))
            if asset not in raw or package.get("sha256") != hashlib.sha256(raw[asset]).hexdigest() \
                    or package.get("bytes") != len(raw[asset]):
                issues.append(f"{name}: package {index} names {package.get('asset')!r}, "
                              f"{package.get('sha256')!r} and {package.get('bytes')!r} bytes, "
                              "which differ from the archive written beside it")
            elif entries[asset]["kind"] == "archive":
                masks.setdefault(name, {})[f"/packages/{index}/bytes"] = "<archive size>"
    for rule in case.get("references", []):
        name = rule["path"]
        if name not in raw:
            continue
        try:
            record = json_value(raw[name])
            references = {field: record[field] for field in rule["fields"]}
        except (KeyError, TypeError, ValueError, UnicodeError) as error:
            issues.append(f"{name}: unreadable storage reference: {error}")
            continue
        for field, ref in references.items():
            target = str(Path(name).parent / str(ref.get("path") if isinstance(ref, dict) else None))
            if not isinstance(ref, dict) or target not in raw \
                    or ref.get("sha256") != hashlib.sha256(raw[target]).hexdigest():
                issues.append(f"{name}: {field} names {ref!r}, which differs from the file written beside it")
    for rule in case.get("volatile", []):
        for pointer in rule["pointers"]:
            masks.setdefault(rule["path"], {})[pointer] = "<volatile>"
    for name, pointers in masks.items():
        if name not in raw:
            continue
        try:
            json_value(raw[name])
            text = raw[name].decode("utf-8")
            spans = json_spans(text, pointers)
        except (KeyError, ValueError, UnicodeError) as error:
            issues.append(f"{name}: volatile fields unreadable: {error}")
            continue
        if set(spans) != set(pointers):
            issues.append(f"{name}: missing volatile fields {sorted(set(pointers) - set(spans))}")
        for pointer, (start, end) in sorted(spans.items(), key=lambda item: -item[1][0]):
            text = text[:start] + json.dumps(pointers[pointer]) + text[end:]
        raw[name] = text.encode("utf-8")
    roots = sorted({os.fsencode(str(root)), os.fsencode(os.path.realpath(root))}, key=len, reverse=True)
    def rewrite(data):
        if case.get("mask_root"):
            for prefix in roots:
                data = data.replace(prefix, b"<root>")
        return substitute(data)
    filesystem = []
    for name, entry in entries.items():
        entry["path_b64"] = base64.b64encode(substitute(os.fsencode(name))).decode("ascii")
        if entry["kind"] == "archive":
            del entry["bytes_b64"], entry["sha256"]
        elif entry["kind"] == "file":
            data = rewrite(raw[name])
            entry.update(bytes_b64=base64.b64encode(data).decode("ascii"), sha256=hashlib.sha256(data).hexdigest())
        filesystem.append(entry)
    def diagnostic(data):
        for rule in case.get("mask_patterns", []):
            data = re.sub(rule["pattern"].encode("utf-8"), rule["label"].encode("utf-8"), data)
        return rewrite(data)
    result = {**actual, "stdout": diagnostic(actual["stdout"]), "stderr": diagnostic(actual["stderr"]),
              "filesystem": sorted(filesystem, key=lambda item: item["path_b64"])}
    if issues:
        result["inconsistencies"] = issues
    return result


STRUCTURED = ("pins", "tokens", "clocks", "identity", "sections", "members")
UNSTRUCTURED = ("archives", "digests", "manifests", "references", "volatile")


def check_structured(case):
    spec = case["structured"]
    if set(spec) - set(STRUCTURED) or set(UNSTRUCTURED) & set(case):
        raise ValueError(f"{case['id']}: a structured comparison declares only {', '.join(STRUCTURED)}")
    for rule in spec.get("pins", []):
        if set(rule) - {"glob", "base", "map", "pointer", "path"} or ("pointer" in rule) != ("path" in rule) \
                or "map" in rule and "pointer" in rule:
            raise ValueError(f"{case['id']}: a pin rule names a glob and at most one of map, or pointer with path")
    for rule in spec.get("identity", []):
        if not rule.get("label") or not rule.get("pointers"):
            raise ValueError(f"{case['id']}: a runner identity rule names its pointers and its decided label")
    for rule in spec.get("sections", []):
        if not re.fullmatch(r"#+ ", rule.get("heading", "")):
            raise ValueError(f"{case['id']}: a section rule names a Markdown heading prefix such as '### '")
    for rule in spec.get("members", []):
        if set(rule) != {"glob", "pointers"} or not rule["pointers"]:
            raise ValueError(f"{case['id']}: a members rule names a glob and the pointers of the objects to order")
    for rule in spec.get("tokens", []):
        if not isinstance(rule, dict) or set(rule) != {"prefix", "sites", "references"} \
                or not re.fullmatch(r"[a-z]+", str(rule["prefix"])) or not rule["sites"] \
                or any(set(site) - {"keys"} != {"glob", "pointers"} or not site["pointers"]
                       or site.get("keys", True) is not True for site in rule["sites"]) \
                or not isinstance(rule["references"], list) or not all(map(token_reference, rule["references"])):
            raise ValueError(f"{case['id']}: a token rule names a lowercase prefix, such as blind, its sites, each "
                             "a glob with the JSON pointers that hold generated tokens (and keys: true when the keys "
                             "of the objects there are the tokens), and its references, each a glob with pointers "
                             "(and keys: true for object keys), a glob with line_starts, a glob with names: true, a "
                             "stream, or a glob alone for whole files")


def token_reference(reference):
    fields = set(reference) if isinstance(reference, dict) else None
    return (fields == {"stream"} and reference["stream"] in ("stdout", "stderr")
            or fields == {"glob", "names"} and reference["names"] is True
            or fields in ({"glob", "pointers"}, {"glob", "pointers", "keys"}) and bool(reference["pointers"])
            and reference.get("keys", True) is True
            or fields == {"glob", "line_starts"} and bool(reference["line_starts"])
            and all(isinstance(start, str) and start for start in reference["line_starts"])
            or fields == {"glob"})


def pin_base(rule, name):
    base = rule.get("base", "")
    return posixpath.dirname(name) if base == "record" else base


def pin_targets(rule, name, raw):
    """Each (pointer, pinned SHA-256, named path) a JSON record states, by the rule that matched it."""
    value = json_value(raw)
    found = []
    if "pointer" in rule:
        found.append((rule["pointer"], pointer(value, rule["pointer"]), rule["path"]))
    elif "map" in rule:
        members = pointer(value, rule["map"])
        if not isinstance(members, dict):
            raise ValueError(f"{rule['map']} is not an object of pins")
        found += [(rule["map"] + "/" + key.replace("~", "~0").replace("/", "~1"), sha, key)
                  for key, sha in members.items()]
    else:
        def walk(item, path):
            if isinstance(item, dict):
                if isinstance(item.get("path"), str) and isinstance(item.get("sha256"), str):
                    found.append((path + "/sha256", item["sha256"], item["path"]))
                for key, child in item.items():
                    walk(child, path + "/" + key.replace("~", "~0").replace("/", "~1"))
            elif isinstance(item, list):
                for index, child in enumerate(item):
                    walk(child, f"{path}/{index}")
        walk(value, "")
    base = pin_base(rule, name)
    result = []
    for at, sha, path in found:
        joined = posixpath.normpath(posixpath.join(base, path)) if isinstance(path, str) else None
        if joined is None or path.startswith("/") or joined == ".." or joined.startswith("../"):
            raise ValueError(f"{at} names {path!r}, which is outside the case root")
        result.append((at, sha, joined))
    return result


def pins_of(spec, files):
    """Every pin the declared rules find, as (record, pointer, SHA-256, target), and the problems reading them."""
    pins, issues = [], []
    for rule in spec.get("pins", []):
        for name in sorted(files):
            if fnmatch.fnmatchcase(name, rule["glob"]):
                try:
                    pins += [(name, *pin) for pin in pin_targets(rule, name, files[name])]
                except (KeyError, IndexError, TypeError, ValueError, UnicodeError) as error:
                    issues.append(f"{name}: unreadable pins: {error}")
    return pins, issues


def replace_pointers(name, data, values):
    text = data.decode("utf-8")
    spans = json_spans(text, values)
    if set(spans) != set(values):
        raise ValueError(f"{name}: missing fields {sorted(set(values) - set(spans))}")
    for at, (start, end) in sorted(spans.items(), key=lambda item: -item[1][0]):
        text = text[:start] + json.dumps(values[at]) + text[end:]
    return text.encode("utf-8")


def relabel_json(data, values, keys, relabel):
    """Relabel within the raw JSON strings at the value pointers and within the keys of the objects at the key
    pointers, leaving every other byte as written."""
    json_value(data)
    text = data.decode("utf-8")
    spans = json_spans(text, {*values, *keys}, set(keys))
    missing = sorted({*values, *keys} - set(spans))
    if missing:
        raise ValueError(f"missing fields {missing}")
    for at in values:
        if text[spans[at][0]] != '"':
            raise ValueError(f"{at} is not a string")
    for at in keys:
        if text[spans[at][0]] != "{":
            raise ValueError(f"{at} is not an object")
    regions = sorted({span for at, span in spans.items() if at in values or isinstance(at, tuple)})
    relabeled = [(start, end, relabel(text[start:end].encode("utf-8")).decode("utf-8")) for start, end in regions]
    for start, end, replacement in reversed(relabeled):
        text = text[:start] + replacement + text[end:]
    return text.encode("utf-8")


def case_inputs(case):
    """Every input the case gives its commands, raw and as each string a JSON input decodes to: names and bytes it
    writes, arguments and stdin."""
    blobs = []
    for step in [case, *case.get("steps", [])]:
        for entry in [*step.get("initial", []), *step.get("write", [])]:
            blobs += [decode(entry[key]) for key in ("path_b64", "bytes_b64", "target_b64") if key in entry]
        blobs += [decode(arg["bytes_b64"]) if isinstance(arg, dict) else os.fsencode(arg)
                  for arg in step.get("argv", [])]
        blobs.append(decode(step.get("stdin_b64", "")))
    strings = []
    def walk(value):
        if isinstance(value, str):
            strings.append(value.encode("utf-8", "surrogatepass"))
        elif isinstance(value, dict):
            for key, child in value.items():
                walk(key)
                walk(child)
        elif isinstance(value, list):
            for child in value:
                walk(child)
    for blob in blobs:
        try:
            walk(json.loads(blob))
        except (ValueError, RecursionError):
            continue
    return blobs + strings


def order_sections(data, heading):
    """Sort each run of adjacent sections under one heading level by their bytes with tokens masked."""
    lines = data.splitlines(keepends=True)
    level = len(heading) - 1
    result, run, current = [], [], None
    def flush():
        result.extend(line for section in sorted(run, key=lambda s: (TOKEN_SHAPE.sub(b"<token>", b"".join(s)),
                                                                     b"".join(s))) for line in section)
        run.clear()
    for line in lines:
        depth = len(line) - len(line.lstrip(b"#"))
        if line.startswith(heading.encode("utf-8")):
            current = [line]
            run.append(current)
        elif current is not None and 0 < depth <= level and line[depth:depth + 1] == b" ":
            current = None
            flush()
            result.append(line)
        elif current is not None:
            current.append(line)
        else:
            result.append(line)
    flush()
    return b"".join(result)


TOKEN_SHAPE = re.compile(rb"(?<![0-9A-Za-z_-])[a-z]+-[0-9a-f]{6,8}(?![0-9A-Za-z_])")


def relabel_line_starts(data, starts, shape, label):
    """Relabel only a token written right after one of the line starts, such as the `### ` of a dossier heading,
    leaving the rest of each line, which may quote source text, exact."""
    lines = data.splitlines(keepends=True)
    for number, line in enumerate(lines):
        for start in starts:
            prefix = start.encode("utf-8")
            if line.startswith(prefix):
                match = shape.match(line, len(prefix))
                if match:
                    lines[number] = line[:match.start()] + shape.sub(label, match.group(0)) + line[match.end():]
                break
    return b"".join(lines)


def order_members(data, pointers):
    """Order the members of the objects at the pointers by key, in a file written as `json.dumps(value, indent=2)`
    and a newline, so an object built in a shuffled order compares by its members. Any other form is refused."""
    value = json_value(data)
    if data != (json.dumps(value, indent=2) + "\n").encode("utf-8"):
        raise ValueError("not written as json.dumps(value, indent=2) and a newline")
    for at in pointers:
        parent, key = (None, None) if at == "" else (pointer(value, at.rsplit("/", 1)[0]),
                                                    at.rsplit("/", 1)[1].replace("~1", "/").replace("~0", "~"))
        members = value if at == "" else pointer(value, at)
        if not isinstance(members, dict):
            raise ValueError(f"{at} is not an object")
        ordered = dict(sorted(members.items()))
        if at == "":
            value = ordered
        else:
            parent[int(key) if isinstance(parent, list) else key] = ordered
    return (json.dumps(value, indent=2) + "\n").encode("utf-8")


def structured(case, root, actual):
    """Compare outputs that carry blind tokens, shuffles, clocks or runner identity.

    Every pin the case declares is checked against the bytes it names before anything is normalized. Then only
    the declared normalizations apply: clock fields and runner identity fields take fixed labels, each run of
    dossier sections is ordered by content, and each token the run wrote at a declared site becomes its first-seen
    ordinal at that site and at the rule's declared references, scanning stdout, stderr, then entries in path
    order, names before contents. A reference is a JSON string value or object's keys at a pointer, entry names,
    a stream, or a whole file; a whole-file reference must never name a file that carries quotes, source text or
    evidence, so those files declare pointer references. Every other byte stays exact, occurrences of site tokens
    included. A generated token that the case's inputs already hold, raw or as a decoded JSON string, and text
    already in a label's form are inconsistencies, since either could make a changed literal look like a reference.
    Last, every pin is recomputed over the normalized bytes it names, so the trees compare byte for byte."""
    spec = case["structured"]
    entries = {os.fsdecode(decode(entry["path_b64"])): dict(entry) for entry in actual["filesystem"]}
    files = {name: decode(entry["bytes_b64"]) for name, entry in entries.items() if entry["kind"] == "file"}
    pins, issues = pins_of(spec, files)
    for name, at, sha, target in pins:
        if target not in files:
            issues.append(f"{name}: {at} pins {target}, which was not written")
        elif sha != hashlib.sha256(files[target]).hexdigest():
            issues.append(f"{name}: {at} pins {sha} for {target}, whose bytes hash to "
                          f"{hashlib.sha256(files[target]).hexdigest()}")
    labels = [(rule, {at: "<volatile>" for at in rule["pointers"]}) for rule in spec.get("clocks", [])]
    labels += [(rule, dict.fromkeys(rule["pointers"], rule["label"])) for rule in spec.get("identity", [])]
    for rule, values in labels:
        for name in sorted(files):
            if fnmatch.fnmatchcase(name, rule["glob"]):
                try:
                    json_value(files[name])
                    files[name] = replace_pointers(name, files[name], values)
                except (KeyError, ValueError, UnicodeError) as error:
                    issues.append(f"{name}: declared fields unreadable: {error}")
    for rule in spec.get("sections", []):
        for name in sorted(files):
            if fnmatch.fnmatchcase(name, rule["glob"]):
                files[name] = order_sections(files[name], rule["heading"])
    streams = {"stdout": actual["stdout"], "stderr": actual["stderr"]}
    ordinals = {}
    if spec.get("tokens"):
        prefixes = b"|".join(re.escape(rule["prefix"].encode()) for rule in spec["tokens"])
        shape = re.compile(rb"(?<![0-9A-Za-z_-])(" + prefixes + rb")-[0-9a-f]{6,8}(?![0-9A-Za-z_])")
        generated, found, claimed, unclaimed = set(), set(), {}, {}
        for rule in spec["tokens"]:
            prefix = rule["prefix"]
            for site in rule["sites"]:
                names = [name for name in sorted(files) if fnmatch.fnmatchcase(name, site["glob"])]
                if not names:
                    issues.append(f"{site['glob']}: no file holds the declared {prefix} tokens")
                for name in names:
                    for at in site["pointers"]:
                        try:
                            value = pointer(json_value(files[name]), at)
                        except (KeyError, IndexError, TypeError, ValueError) as error:
                            issues.append(f"{name}: {at} should hold a {prefix} token: {error!r}")
                            continue
                        if site.get("keys"):
                            if not isinstance(value, dict):
                                issues.append(f"{name}: {at} holds {json.dumps(value)}, not an object of {prefix} tokens")
                                continue
                            keyed = [key for key in value if key.startswith(prefix + "-")]
                            claimed.setdefault((name, at), set()).update(keyed)
                            unclaimed.setdefault((name, at), set()).update(value)
                            for key in keyed:
                                if re.fullmatch(re.escape(prefix) + r"-[0-9a-f]{6,8}", key):
                                    generated.add(key.encode("ascii"))
                                    found.add((prefix, name, ("keys", at)))
                                else:
                                    issues.append(f"{name}: {at} has the key {json.dumps(key)}, not a {prefix} token")
                        elif isinstance(value, str) and re.fullmatch(re.escape(prefix) + r"-[0-9a-f]{6,8}", value):
                            generated.add(value.encode("ascii"))
                            found.add((prefix, name, at))
                        else:
                            issues.append(f"{name}: {at} holds {json.dumps(value)}, not a {prefix} token")
        for (name, at), keys in sorted(unclaimed.items()):
            for key in sorted(keys - claimed.get((name, at), set())):
                issues.append(f"{name}: {at} has the key {json.dumps(key)}, which no token rule declares")
        inputs = case_inputs(case)
        for token in sorted(generated):
            if any(token in blob for blob in inputs):
                issues.append(f"{token.decode('ascii')}: the run generated a token the case's inputs already hold, "
                              "so its references cannot be told from literal text")
        taken = re.compile(rb"<(?:" + prefixes + rb") [0-9]+>")
        for where, data in [*streams.items(), *((f"path {name}", os.fsencode(name)) for name in entries),
                            *files.items()]:
            held = sorted({text.decode("ascii") for text in taken.findall(data)})
            if held:
                issues.append(f"{where}: already holds {', '.join(held)}, the form of a token label")
        def masked(name):
            return shape.sub(b"<token>", os.fsencode(name)), os.fsencode(name)
        def label(match):
            prefix = match.group(1).decode("ascii")
            key = match.group(0)
            if key not in generated:
                return key
            if key not in ordinals:
                ordinals[key] = f"<{prefix} {1 + sum(o.startswith(f'<{prefix} ') for o in ordinals.values())}>"
            return ordinals[key].encode("ascii")
        renamed = {name: name for name in entries}
        for rule in spec["tokens"]:
            own = re.compile(rb"(?<![0-9A-Za-z_-])(" + re.escape(rule["prefix"].encode()) +
                             rb")-[0-9a-f]{6,8}(?![0-9A-Za-z_])")
            def relabel(data, own=own):
                return own.sub(label, data)
            references = rule["references"]
            for stream in ("stdout", "stderr"):
                if {"stream": stream} in references:
                    streams[stream] = relabel(streams[stream])
            def declared(name, *fields):
                return [reference for reference in references
                        if set(reference) - {"keys"} == {"glob", *fields} and fnmatch.fnmatchcase(name, reference["glob"])]
            for name in sorted(entries, key=masked):
                if declared(name, "names"):
                    renamed[name] = os.fsdecode(relabel(os.fsencode(renamed[name])))
                if name not in files:
                    continue
                if declared(name):
                    files[name] = relabel(files[name])
                    continue
                for reference in declared(name, "line_starts"):
                    files[name] = relabel_line_starts(files[name], reference["line_starts"], own, label)
                at_pointers = declared(name, "pointers")
                sites = [at for prefix, site, at in found if (prefix, site) == (rule["prefix"], name)]
                values = [at for at in sites if not isinstance(at, tuple)]
                values += [at for reference in at_pointers if not reference.get("keys") for at in reference["pointers"]]
                keys = [at[1] for at in sites if isinstance(at, tuple)]
                keys += [at for reference in at_pointers if reference.get("keys") for at in reference["pointers"]]
                if values or keys:
                    try:
                        files[name] = relabel_json(files[name], values, keys, relabel)
                    except (ValueError, UnicodeError) as error:
                        issues.append(f"{name}: declared {rule['prefix']} references unreadable: {error}")
        entries = {renamed[name]: entry for name, entry in entries.items()}
        files = {renamed[name]: data for name, data in files.items()}
    for rule in spec.get("members", []):
        for name in sorted(files):
            if fnmatch.fnmatchcase(name, rule["glob"]):
                try:
                    files[name] = order_members(files[name], rule["pointers"])
                except (KeyError, IndexError, TypeError, ValueError, UnicodeError) as error:
                    issues.append(f"{name}: declared members unreadable: {error}")
    final = {}
    repins, problems = pins_of(spec, files)
    issues += problems
    def settle(name, open_names):
        if name not in final:
            if name in open_names:
                issues.append(f"{name}: pins form a cycle")
                return files[name]
            values = {at: hashlib.sha256(settle(target, open_names | {name})).hexdigest()
                      for record, at, _sha, target in repins if record == name and target in files}
            try:
                final[name] = replace_pointers(name, files[name], values) if values else files[name]
            except (ValueError, UnicodeError) as error:
                issues.append(f"{name}: pins unreadable after normalization: {error}")
                final[name] = files[name]
        return final[name]
    roots = sorted({os.fsencode(str(root)), os.fsencode(os.path.realpath(root))}, key=len, reverse=True)
    def diagnostic(data):
        if case.get("mask_root"):
            for prefix in roots:
                data = data.replace(prefix, b"<root>")
        for rule in case.get("mask_patterns", []):
            data = re.sub(rule["pattern"].encode("utf-8"), rule["label"].encode("utf-8"), data)
        return data
    filesystem = []
    for name, entry in entries.items():
        entry["path_b64"] = base64.b64encode(os.fsencode(name)).decode("ascii")
        if entry["kind"] == "file":
            data = settle(name, frozenset())
            entry.update(bytes_b64=base64.b64encode(data).decode("ascii"), sha256=hashlib.sha256(data).hexdigest())
        filesystem.append(entry)
    result = {**actual, "stdout": diagnostic(streams["stdout"]), "stderr": diagnostic(streams["stderr"]),
              "filesystem": sorted(filesystem, key=lambda item: item["path_b64"])}
    if issues:
        result["inconsistencies"] = issues
    return result


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
    differences = [f"consistency: {issue}" for issue in actual.get("inconsistencies", [])]
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
    differences += outcome_differences(expected.get("outcomes", []), actual)
    if expected["filesystem"] != actual["filesystem"]:
        differences.append("filesystem effects:\n" + "".join(difflib.unified_diff(
            json.dumps(expected["filesystem"], indent=2, sort_keys=True).splitlines(True),
            json.dumps(actual["filesystem"], indent=2, sort_keys=True).splitlines(True),
            fromfile="expected", tofile="actual")))
    return differences


def outcome_differences(outcomes, actual):
    """Each hand-stated grading outcome must hold in the output, a written file or the JSON a stream printed,
    whichever implementation wrote it."""
    files = {os.fsdecode(decode(entry["path_b64"])): entry for entry in actual["filesystem"] if entry["kind"] == "file"}
    differences = []
    for outcome in outcomes:
        where = f"{outcome['kind']} {outcome.get('path') or outcome.get('stream')}{outcome['pointer']}"
        try:
            raw = actual[outcome["stream"]] if "stream" in outcome else decode(files[outcome["path"]]["bytes_b64"])
            value = pointer(json_value(raw), outcome["pointer"])
        except (KeyError, IndexError, TypeError, ValueError, UnicodeError) as error:
            differences.append(f"outcome {where}: unreadable: {error!r}")
            continue
        if digest(value) != digest(outcome["value"]):
            differences.append(f"outcome {where}: expected {json.dumps(outcome['value'])}, actual {json.dumps(value)}")
    return differences


def compare_case(case, actual):
    if "steps" not in case:
        return compare(case["expected"], actual)
    expected = [step["expected"] for step in case["steps"] if "argv" in step]
    return [f"step {number}: {difference}" for number, (wanted, observed) in enumerate(zip(expected, actual["steps"]), 1)
            for difference in compare(wanted, observed)]


def differential_case(case, left, right):
    if "steps" not in case:
        return differential(left, right, case)
    commands = [step for step in case["steps"] if "argv" in step]
    return [f"step {number}: {difference}" for number, (step, primary, candidate)
            in enumerate(zip(commands, left["steps"], right["steps"]), 1)
            for difference in differential(primary, candidate, step)]


def commands_of(case, actual):
    """Each command a case runs with what it observed: the case itself, or each command step."""
    if "steps" not in case:
        return [(case, actual)]
    return list(zip([step for step in case["steps"] if "argv" in step], actual["steps"]))


def explained(case, actual):
    """The resolutions of the differing commands when each matches its own baseline_defect observation."""
    resolutions = []
    for command, observed in commands_of(case, actual):
        if compare(command["expected"], observed):
            known = command.get("baseline_defect")
            if not known or compare(known["observed"], observed):
                return None
            resolutions.append(known["resolution"])
    return ", ".join(resolutions) or None


def approved(case, actual, candidate):
    """The resolutions when each command where the implementations differ is a recorded baseline defect that the
    primary still shows and the candidate fixes."""
    resolutions = []
    for (command, left), (_, right) in zip(commands_of(case, actual), commands_of(case, candidate)):
        if differential(left, right, command):
            known = command.get("baseline_defect")
            if not known or compare(known["observed"], left) or compare(command["expected"], right):
                return None
            resolutions.append(known["resolution"])
    return ", ".join(resolutions) or None


def differential(left, right, case):
    expected = {"exit": left["exit"], "filesystem": left["filesystem"],
                "outcomes": case.get("expected", {}).get("outcomes", [])}
    errors = [f"primary consistency: {issue}" for issue in left.get("inconsistencies", [])]
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
    programs = {step["program"] for case in corpus["cases"] for step in [case, *case.get("steps", [])] if "program" in step}
    for program in sorted(programs):
        if program in SETUP_PROGRAMS:
            continue
        if program in PYTHON_ENTROPY:
            commands[program] = entropy_command(PYTHON_ENTROPY[program])
        elif program in PYTHON_PROGRAMS:
            commands[program] = [sys.executable, str(REPO / PYTHON_PROGRAMS[program])]
        elif program in PYTHON_DRIVERS:
            commands[program] = [sys.executable, "-c", PYTHON_DRIVERS[program],
                                 *(str(REPO / path) for path in DRIVER_ARGUMENTS.get(program, ["bench/tools"]))]
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


def python_commands(corpus):
    """The default commands with each moved program run by the retained Python it replaced."""
    commands = {}
    programs = {step["program"] for case in corpus["cases"] for step in [case, *case.get("steps", [])] if "program" in step}
    for program in sorted(programs):
        driver = PYTHON_COUNTERPARTS.get(program, program)
        if program in SETUP_PROGRAMS:
            continue
        if driver in PYTHON_ENTROPY:
            commands[program] = entropy_command(PYTHON_ENTROPY[driver])
        elif driver in PYTHON_DRIVERS:
            commands[program] = [sys.executable, "-c", PYTHON_DRIVERS[driver],
                                 *(str(REPO / path) for path in DRIVER_ARGUMENTS.get(driver, ["bench/tools"]))]
        elif driver in PYTHON_PROGRAMS:
            commands[program] = [sys.executable, str(REPO / PYTHON_PROGRAMS[driver])]
        else:
            commands.update({name: command for name, command in default_commands(
                {"cases": [{"program": program}]}).items()})
    return commands


def entropy_command(name):
    """ENTROPY_DRIVER running a driver of PYTHON_DRIVERS by name, or else a script relative to the repository."""
    if name in PYTHON_DRIVERS:
        return [sys.executable, "-c", ENTROPY_DRIVER, "-c", PYTHON_DRIVERS[name],
                *(str(REPO / path) for path in DRIVER_ARGUMENTS.get(name, ["bench/tools"]))]
    return [sys.executable, "-c", ENTROPY_DRIVER, str(REPO / name)]


def with_setup(commands):
    return {**commands, **{name: [sys.executable, "-c", source] for name, source in SETUP_PROGRAMS.items()}}


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
    if set(corpus) - {"format", "decisions", "cases", "snapshot_modes", "gh_stub_b64"}:
        raise ValueError("unknown corpus option")
    decisions = load_decisions(path.parent / corpus["decisions"])
    ids = set()
    for case in corpus["cases"]:
        if case["id"] in ids:
            raise ValueError("duplicate case id")
        ids.add(case["id"])
        if "steps" in case and ({"argv", "stdin_b64", "expected", "baseline_defect"} & set(case)
                                or not any("argv" in step for step in case["steps"])
                                or any("baseline_defect" in step and "argv" not in step for step in case["steps"])):
            raise ValueError(f"{case['id']}: a step case keeps its commands and expectations in its steps")
        if "structured" in case:
            check_structured(case)
        for step in [case, *case.get("steps", [])]:
            for outcome in step.get("expected", {}).get("outcomes", []):
                if set(outcome) not in ({"kind", "path", "pointer", "value"}, {"kind", "stream", "pointer", "value"}) \
                        or outcome["kind"] not in GRADING_OUTCOMES \
                        or outcome.get("stream", "stdout") not in ("stdout", "stderr"):
                    raise ValueError(f"{case['id']}: an outcome states its kind ({', '.join(GRADING_OUTCOMES)}), "
                                     "path or stream, pointer and value")
        decision = decisions[case["id"]]
        if not decision["reason"] or decision["contract_sha256"] != digest(case):
            raise ValueError(f"{case['id']}: changed contract needs an explicit decision")
    if ids != set(decisions):
        raise ValueError("corpus case ids differ from decisions; removal needs an explicit decision")
    return corpus


REAL_RECORD = REPO / "tools/behavior_contracts/grading-real-record.v1.json"
RECORD_LOCK = Path(".cache/current-record.lock")


def load_real_record(path):
    listing = json_value(path.read_bytes())
    if listing.get("format") != "behavior-real-record/1" or set(listing) - {"format", "description", "commands"}:
        raise ValueError(f"{path}: unsupported real-record list")
    ids = set()
    for command in listing["commands"]:
        if set(command) - {"id", "program", "argv", "outputs", "timeout"} or command["id"] in ids:
            raise ValueError(f"{path}: each command has a unique id, a program, argv and optional outputs")
        ids.add(command["id"])
        for name in [*command.get("outputs", []), *(arg["output"] for arg in command["argv"] if isinstance(arg, dict))]:
            relative(base64.b64encode(os.fsencode(name)).decode("ascii"))
    return listing


def root_state(root):
    """Every entry below root, tracked, ignored or Git's own, by type, permissions, size, inode and change time.
    Writing a file moves its change time even when its modification time and bytes are put back."""
    state = []
    for directory, dirs, files in os.walk(root):
        for name in dirs + files:
            path = os.path.join(directory, name)
            info = os.lstat(path)
            state.append((os.path.relpath(path, root), info.st_mode, info.st_size, info.st_ino, info.st_mtime_ns,
                          info.st_ctime_ns))
    return sorted(state)


def establish_record_lock(root):
    """current_grading.record_lock creates its lock file on first use, which a fresh checkout lacks; creating it
    before the first snapshot keeps that creation from reading as a change to the record. Opening in append mode
    leaves an existing lock in place, byte- and inode-unchanged."""
    path = Path(root).resolve() / RECORD_LOCK
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a"):
        pass


def run_real_record(command, commands, root):
    """Run one listed command on the checked-out record, with its named outputs in a disposable directory."""
    with tempfile.TemporaryDirectory(prefix="behavior-contract-real-") as temporary:
        out, home = Path(temporary) / "out", Path(temporary) / "home"
        out.mkdir()
        home.mkdir()
        argv = [str(out / arg["output"]) if isinstance(arg, dict) else arg for arg in command["argv"]]
        environment = {"PATH": os.defpath, "HOME": str(home), "LC_ALL": "C.UTF-8", "TZ": "UTC",
                       "PYTHONDONTWRITEBYTECODE": "1", "PYTHONUTF8": "1"}
        before = root_state(root)
        result = subprocess.run([*commands[command["program"]], *argv], cwd=root, capture_output=True,
                                env=environment, timeout=command.get("timeout", 300))
        changed = root_state(root) != before
        outputs = {name: (out / name).read_bytes() if (out / name).is_file() else None
                   for name in command.get("outputs", [])}
        return {"exit": result.returncode, "stdout": result.stdout, "stderr": result.stderr, "outputs": outputs,
                "changed": changed}


def real_record_differences(left, right):
    differences = ["candidate changed the checked-out record"] if right["changed"] else []
    if left["exit"] != right["exit"]:
        differences.append(f"exit: primary {left['exit']}, candidate {right['exit']}")
    for stream in ("stdout", "stderr"):
        if left[stream] != right[stream]:
            differences.append(f"{stream} bytes: primary {left[stream]!r}, candidate {right[stream]!r}")
    for name, data in left["outputs"].items():
        if data != right["outputs"][name]:
            differences.append(f"output {name}: primary {data!r}, candidate {right['outputs'][name]!r}")
    return differences


def real_record(listing, commands, other, root):
    """A command that changes the checkout stops the run, so no later run reads inputs it changed."""
    if not listing["commands"]:
        print("real record: no commands are listed")
        return 0
    establish_record_lock(root)
    failed = 0
    for command in listing["commands"]:
        primary = run_real_record(command, commands, root)
        if primary["changed"]:
            print(f"DIFFERENTIAL {command['id']}\nprimary changed the checked-out record; stopped before the candidate")
            return 1
        candidate = run_real_record(command, other, root)
        differences = real_record_differences(primary, candidate)
        if candidate["changed"]:
            print(f"DIFFERENTIAL {command['id']}\n" + "\n".join(differences) + "\nstopped before the next command")
            return 1
        if differences:
            failed += 1
            print(f"DIFFERENTIAL {command['id']}\n" + "\n".join(differences))
        else:
            print(f"PASS {command['id']}")
    return 1 if failed else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=CORPUS)
    parser.add_argument("--implementations", type=Path)
    parser.add_argument("--differential", type=Path)
    parser.add_argument("--allow-known-defects", action="store_true")
    parser.add_argument("--verify-decision-history", action="store_true")
    parser.add_argument("--real-record", type=Path, nargs="?", const=REAL_RECORD)
    parser.add_argument("--root", type=Path, default=REPO)
    args = parser.parse_args()
    try:
        if args.verify_decision_history:
            verify_decision_history(REPO)
            print("Published decision history verified")
            return 0
        if args.real_record:
            if not args.differential:
                raise ValueError("--real-record compares two implementations; name the second with --differential")
            listing = load_real_record(args.real_record)
            commands = (json.loads(args.implementations.read_text(encoding="utf-8")) if args.implementations
                        else default_commands({"cases": listing["commands"]}))
            return real_record(listing, commands, json.loads(args.differential.read_text(encoding="utf-8")),
                               args.root.resolve())
        corpus = load_corpus(args.corpus)
        commands = (json.loads(args.implementations.read_text(encoding="utf-8")) if args.implementations
                    else default_commands(corpus))
        other = json.loads(args.differential.read_text(encoding="utf-8")) if args.differential else None
        failed = 0
        for case in corpus["cases"]:
            try:
                actual = execute(case, commands, corpus)
            except UnavailableFixture as error:
                print(f"SKIP UNAVAILABLE {case['id']}: {error}")
                continue
            differences = compare_case(case, actual)
            known = explained(case, actual) if differences and args.allow_known_defects else None
            if known:
                print(f"KNOWN DEFECT {case['id']}: {known}\n" + "\n".join(differences))
            elif differences:
                failed += 1
                print(f"FAIL {case['id']}\n" + "\n".join(differences))
            else:
                print(f"PASS {case['id']}")
            if other:
                candidate = execute(case, other, corpus)
                differences = differential_case(case, actual, candidate)
                if differences:
                    fixed = approved(case, actual, candidate) if args.allow_known_defects else None
                    if fixed:
                        print(f"APPROVED FIX {case['id']}: {fixed}\n" + "\n".join(differences))
                    else:
                        failed += 1
                        print(f"DIFFERENTIAL {case['id']}\n" + "\n".join(differences))
        return 1 if failed else 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(f"behavior_contract: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
