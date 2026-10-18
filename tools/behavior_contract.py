#!/usr/bin/env python3
"""Run language-independent process contracts in disposable directories.

Usage: python3 tools/behavior_contract.py [--implementations commands.json]
       [--differential commands.json] [--allow-known-defects]
Inputs: versioned JSON corpus with argv, base64 bytes, filesystem and expectations.
Exit codes: 0 contracts met (or explicitly allowed baseline defects); 1 differences;
2 invalid inputs or failed execution. Commands are trusted local programs, not sandboxed.
"""
from __future__ import annotations

import argparse
import base64
import difflib
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

REPO = Path(__file__).resolve().parents[1]
CORPUS = REPO / "tools/behavior_contracts/usage.v1.json"


def decode(value):
    return base64.b64decode(value, validate=True)


def relative(value):
    raw = decode(value)
    if not raw or raw.startswith(b"/") or any(p in (b"", b".", b"..") for p in raw.split(b"/")) or b"\0" in raw:
        raise ValueError("filesystem paths must be nonempty relative byte paths without dot segments")
    return os.fsdecode(raw)


def digest(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=True, sort_keys=True,
                                     separators=(",", ":")).encode("ascii")).hexdigest()


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
        result = subprocess.run([*commands[case["program"]], *case["argv"]], cwd=root,
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
    for stream in ("stdout", "stderr"):
        if case["expected"][stream]["mode"] in ("json", "json-pointers") and left["exit"] == 0:
            expected[stream] = {"mode": "json", "value": json_value(left[stream])}
        else:
            expected[stream] = {"mode": "bytes", "bytes_b64": base64.b64encode(left[stream]).decode("ascii")}
    return compare(expected, right)


def load_corpus(path):
    corpus = json.loads(path.read_text(encoding="utf-8"))
    if corpus["format"] != "behavior-contract/1":
        raise ValueError("unsupported corpus format")
    decisions = json.loads((path.parent / corpus["decisions"]).read_text(encoding="utf-8"))
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
    args = parser.parse_args()
    try:
        corpus = load_corpus(args.corpus)
        defaults = {name: [sys.executable, str(REPO / "bench/tools" / f"{name}_usage.py")]
                    for name in ("codex", "transcript")}
        commands = json.loads(args.implementations.read_text(encoding="utf-8")) if args.implementations else defaults
        other = json.loads(args.differential.read_text(encoding="utf-8")) if args.differential else None
        failed = 0
        for case in corpus["cases"]:
            actual = execute(case, commands)
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
