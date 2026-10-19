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
import fnmatch
import hashlib
import io
import json
from json.decoder import WHITESPACE, scanstring
from json.scanner import NUMBER_RE
import math
import os
from pathlib import Path
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
                   "check-manifest": "bench/tools/check_manifest.py", "evidence-store": "bench/tools/evidence_store.py",
                   "evidence-inventory": "bench/tools/evidence_inventory.py",
                   "recover-evidence": "bench/tools/recover_evidence.py"}
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
    # file_artifacts has no command line; file_attempt.py calls it in process. `file` exposes it with the
    # module's own refusals and exit status, as `bench-tools native-artifacts file` will.
    "native-artifacts": """import sys
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
except (native_artifacts.ArtifactError, OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
    parser.exit(1, f"native_artifacts.py: {error}\\n")
print(json.dumps(pin))
""",
    # restore-local.py is a research record that imports from the checkout it runs in; preloading the retained
    # modules makes it use them while it reads the case directory as that checkout.
    "restore-local": """import runpy, sys
sys.path.insert(0, sys.argv.pop(1))
script = sys.argv.pop(1)
import evidence_store, local_evidence_resolution
sys.argv[0] = script
runpy.run_path(script, run_name="__main__")
""",
    "collect-run": """import argparse, sys
from pathlib import Path
sys.path.insert(0, sys.argv.pop(1))
sys.argv[0] = "collect_run.py"
import collect_run
collect_run.ROOT = Path.cwd()
parser = argparse.ArgumentParser(description=collect_run.__doc__)
parser.add_argument("--run", type=Path, required=True)
parser.add_argument("--missing", action="append", default=[])
args = parser.parse_args()
collect_run.collect(args.run.resolve(), args.missing)
""",
    "import-benchmark": """import sys
from pathlib import Path
sys.path.insert(0, sys.argv.pop(1))
sys.argv[0] = "import_benchmark.py"
import import_benchmark
import_benchmark.ROOT = Path.cwd()
import_benchmark.MANIFEST = import_benchmark.ROOT / "bench/import-manifest.json"
sys.exit(import_benchmark.main())
""",
}
DRIVER_ARGUMENTS = {"native-artifacts": ["bench/tools"], "restore-local": [
    "bench/tools", "docs/research/evidence-migration-2026-10-08/restore-local.py"], "collect-run": ["tools"],
    "import-benchmark": ["tools"]}
RUST_PROGRAMS = {"roster": ("bench-tools", ["roster"]), "normalize-thermo": ("bench-tools", ["normalize-thermo"]),
                 "derive-packet": ("bench-tools", ["derive-packet"]), "build-packet": ("bench-tools", ["build-packet"]),
                 "export-explorer": ("bench-tools", ["export-explorer"]), "archive": ("bench-tools", ["archive"])}
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
# asset download that evidence storage runs, records every argument list in calls.jsonl, and takes its release,
# assets and injected faults from that directory, so no case reaches the network.
GH_STUB = r"""import json, os, sys
from pathlib import Path
from urllib.parse import quote
state = Path(os.environ["CONTRACT_GH_STATE"])
args = sys.argv[1:]
state.mkdir(parents=True, exist_ok=True)
with (state / "calls.jsonl").open("a", encoding="utf-8") as log:
    log.write(json.dumps(args) + "\n")
def fail(message, code=1):
    sys.stderr.write(message + "\n")
    sys.exit(code)
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


def execute(case, commands, options=None):
    """Run a case's command, or each of its steps on one shared root, and observe the results."""
    options = options or {}
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
            result = subprocess.run([*commands[step.get("program", case.get("program"))], *argv], cwd=root,
                                    input=decode(step["stdin_b64"]), capture_output=True,
                                    env=environment, timeout=10, **({"umask": 0o022} if modes else {}))
            results.append(normalize(case, root, {"exit": result.returncode, "stdout": result.stdout,
                                                  "stderr": result.stderr, "filesystem": snapshot(root, modes)}))
        return {"steps": results} if "steps" in case else results[0]


def json_spans(text, pointers):
    """Character spans of the values at the given RFC 6901 pointers in valid JSON text."""
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
                        key, position = scanstring(text, skip(position) + 1)
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
                member = {"name": item.name, "type": item.type.decode("latin-1"), "size": item.size, "mode": item.mode}
                if item.isfile():
                    member["sha256"] = hashlib.sha256(bundle.extractfile(item).read()).hexdigest()
                members.append(member)
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
                except (tarfile.TarError, zipfile.BadZipFile, OSError, EOFError, ValueError) as error:
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
    if expected["filesystem"] != actual["filesystem"]:
        differences.append("filesystem effects:\n" + "".join(difflib.unified_diff(
            json.dumps(expected["filesystem"], indent=2, sort_keys=True).splitlines(True),
            json.dumps(actual["filesystem"], indent=2, sort_keys=True).splitlines(True),
            fromfile="expected", tofile="actual")))
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
    programs = {step["program"] for case in corpus["cases"] for step in [case, *case.get("steps", [])] if "program" in step}
    for program in sorted(programs):
        if program in PYTHON_PROGRAMS:
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
                                or not any("argv" in step for step in case["steps"])):
            raise ValueError(f"{case['id']}: a step case keeps its commands and expectations in its steps")
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
                actual = execute(case, commands, corpus)
            except UnavailableFixture as error:
                print(f"SKIP UNAVAILABLE {case['id']}: {error}")
                continue
            differences = compare_case(case, actual)
            known = case.get("baseline_defect")
            if differences and args.allow_known_defects and known and not compare(known["observed"], actual):
                print(f"KNOWN DEFECT {case['id']}: {known['resolution']}\n" + "\n".join(differences))
            elif differences:
                failed += 1
                print(f"FAIL {case['id']}\n" + "\n".join(differences))
            else:
                print(f"PASS {case['id']}")
            if other:
                candidate = execute(case, other, corpus)
                differences = differential_case(case, actual, candidate)
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
