#!/usr/bin/env python3
"""Build, retain, select and check pinned Rust executables, so a Rust-backed operation is as attributable as a frozen Python one.

Usage::

    python3 bench/tools/rust_runtime.py install --crate rust/usage-pilot [--store DIR] [--select]
    python3 bench/tools/rust_runtime.py restore --receipt RECEIPT --executable FILE [--store DIR]
    python3 bench/tools/rust_runtime.py verify NAME SHA256 [--store DIR]
    python3 bench/tools/rust_runtime.py select NAME SHA256 [--store DIR]
    python3 bench/tools/rust_runtime.py rollback NAME [--store DIR]
    python3 bench/tools/rust_runtime.py pin NAME [--store DIR]
    python3 bench/tools/rust_runtime.py list NAME [--store DIR]
    python3 bench/tools/rust_runtime.py deviation --run RUN_DIR --name NAME --from SHA256 --to SHA256
        --reason TEXT --scope TEXT --validation TEXT --accounting TEXT [--store DIR]

The store (default ``~/.t3/bench-runtime``) keeps each built executable under ``NAME/SHA256/`` beside its
``receipt.json``. Entries are never overwritten or removed by this tool, so every retained version stays
runnable offline for rollback. ``selected.json`` names the version new operations take: ``pin`` prints it as
the pin a queue file lists under ``runtimes``. ``select`` and ``rollback`` change only that file. An operation
that already holds a pin keeps it through restart and recovery; nothing here falls back to another version
or to Python when the pinned one is missing or changed.

``install`` builds from a clean, committed crate with ``cargo build --locked --offline --release`` for this
host's target, with build-affecting environment variables cleared and source paths remapped, after checking
that the active ``rustc`` is the release ``rust-toolchain.toml`` pins. Cargo configuration files outside the
crate's tracked tree are refused, because a receipt could not restore them, and so is a build whose dep-info
lists a local file that Git does not track in the crate, such as a path dependency outside it or an ignored file,
or a crate that tracks a symbolic link. ``install`` needs
Python 3.11 or later; the other commands do not. ``restore`` retains an executable from elsewhere, such as
another store or a rebuild, only when it hashes to the receipt.

A receipt holds no absolute path: the crate path is relative to the repository and the executable to its
entry. It names the source revision, the crate's Git tree, the hashes of ``Cargo.toml``, ``Cargo.lock`` and
``rust-toolchain.toml``, ``rustc -vV``, ``cargo -V``, the target, the C library the build linked against,
the build command and its flags, and the executable's SHA-256 and size.

``deviation`` writes a versioned record of a required runtime change into a run's ``deviations/``, with both
identities. It names no ``predecessor`` or ``cause``, so it is not an attempt diagnosis, and no check reads it
as permission: a run still runs only on the runtime its queue pins.

Exit codes: 0 done; 1 a check refused (missing, changed or incompatible executable, unclean source, a
conflicting entry); 2 an input could not be read or a subprocess failed.
"""
from __future__ import annotations

import argparse
import contextlib
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import uuid
from datetime import datetime, timezone

REPO = Path(__file__).resolve().parents[2]
DEFAULT_STORE = Path("~/.t3/bench-runtime")
RECEIPT = "rust-runtime-receipt/1"
DEVIATION = "rust-runtime-deviation/1"
# Hosts whose installation, pinning and rollback were exercised; see docs/python-migration/runtime.md.
VERIFIED_TARGETS = frozenset({"x86_64-unknown-linux-gnu"})
HOST_TARGETS = {("Linux", "x86_64"): "x86_64-unknown-linux-gnu", ("Linux", "aarch64"): "aarch64-unknown-linux-gnu",
                ("Darwin", "x86_64"): "x86_64-apple-darwin", ("Darwin", "arm64"): "aarch64-apple-darwin"}
CLEARED = re.compile(r"RUSTFLAGS|RUSTDOCFLAGS|CARGO_ENCODED_RUSTFLAGS|CARGO_ENCODED_RUSTDOCFLAGS|RUSTC|RUSTC_WRAPPER|"
                     r"RUSTC_WORKSPACE_WRAPPER|CARGO_INCREMENTAL|CARGO_TARGET_DIR|CARGO_BUILD_\w+|CARGO_PROFILE_\w+|"
                     r"CARGO_TARGET_\w+")
NAME = re.compile(r"[a-z0-9][a-z0-9-]*")
SHA256 = re.compile(r"[0-9a-f]{64}")


class Refused(Exception):
    """A check refused the runtime; exit code 1."""


class InputError(Exception):
    """An input is missing or a subprocess failed; exit code 2."""


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds").replace("+00:00", "Z")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def publish(path: Path, text: str, *, exclusive: bool = False) -> None:
    """Write ``path`` in one step; with ``exclusive``, refuse a path that exists."""
    partial = path.with_name(f".{path.name}.{uuid.uuid4().hex}.partial")
    try:
        with open(partial, "x", encoding="utf-8") as handle:
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        if exclusive:
            try:
                os.link(partial, path)
            except FileExistsError:
                raise Refused(f"{path} exists; a record is never overwritten") from None
        else:
            os.replace(partial, path)
    finally:
        partial.unlink(missing_ok=True)


def dumps(value) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def run(command: list, cwd: Path, env: dict | None = None) -> str:
    try:
        done = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True, encoding="utf-8")
    except OSError as error:
        raise InputError(f"{command[0]} cannot run: {error}") from error
    if done.returncode:
        raise InputError(f"{' '.join(command)} exited {done.returncode}: {done.stderr.strip()[-2000:]}")
    return done.stdout


def host_target() -> str | None:
    return HOST_TARGETS.get((platform.system(), platform.machine()))


def host_libc() -> dict:
    name, version = platform.libc_ver()
    return {"name": name or None, "version": version or None}


def version_tuple(text: str) -> tuple:
    return tuple(int(part) for part in re.findall(r"\d+", text))


def located(store: Path | str) -> Path:
    return Path(store).expanduser().resolve()


@contextlib.contextmanager
def locked(store: Path, name: str):
    """One writer of a runtime's entries and selection at a time; a second waits."""
    directory = store / name
    directory.mkdir(parents=True, exist_ok=True)
    with open(directory / ".lock", "a", encoding="utf-8") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        yield directory


# --- receipts and checks ------------------------------------------------------------------------

def read_json(path: Path) -> dict:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError:
        raise Refused(f"{path} is missing") from None
    except (OSError, ValueError) as error:
        raise Refused(f"{path} cannot be read: {error}") from None
    if not isinstance(value, dict):
        raise Refused(f"{path} is not a JSON object")
    return value


def receipt_problem(receipt: dict) -> str | None:
    """Why ``receipt`` is not a complete, portable receipt, or None."""
    try:
        if receipt["schema"] != RECEIPT:
            return f"schema is {receipt['schema']!r}, not {RECEIPT}"
        crate, source, artifact = receipt["crate"], receipt["source"], receipt["artifact"]
        if not NAME.fullmatch(crate["name"]) or not crate["bin"] or Path(crate["path"]).is_absolute() or ".." in Path(crate["path"]).parts:
            return "the crate needs a name, a binary and a repository-relative path"
        if not re.fullmatch(r"[0-9a-f]{40,64}", source["revision"]) or not re.fullmatch(r"[0-9a-f]{40,64}", source["tree"]):
            return "the source needs a commit revision and a crate tree"
        if not all(SHA256.fullmatch(source[key]) for key in ("cargo_toml_sha256", "cargo_lock_sha256", "toolchain_file_sha256")):
            return "the source needs the SHA-256 of Cargo.toml, Cargo.lock and rust-toolchain.toml"
        if not receipt["toolchain"]["rustc"].get("release") or not receipt["target"] or not receipt["build"]["command"]:
            return "the receipt needs the rustc release, the target and the build command"
        if (not SHA256.fullmatch(artifact["sha256"]) or artifact["file"] in ("", ".", "..") or "/" in artifact["file"]
                or artifact["size"] < 0):
            return "the artifact needs a SHA-256, a size and a file name within its entry"
    except (KeyError, TypeError, AttributeError) as error:
        return f"a field is missing or malformed: {error!r}"
    return None


def entry(store: Path, name: str, sha256: str) -> Path:
    if not NAME.fullmatch(name) or not SHA256.fullmatch(sha256):
        raise InputError(f"{name!r} is not a runtime name or {sha256!r} is not a SHA-256")
    return store / name / sha256


def host_problem(name: str, receipt: dict) -> str | None:
    """Why the receipt's executable may not run on this host, or None."""
    host = host_target()
    if receipt["target"] != host:
        return f"{name} was built for {receipt['target']}; this host is {host or platform.system() + ' ' + platform.machine()}"
    if host not in VERIFIED_TARGETS:
        return f"{host} is not a verified Rust runtime target ({', '.join(sorted(VERIFIED_TARGETS))})"
    built, libc = receipt.get("libc") or {}, host_libc()
    if built.get("name") and (built["name"] != libc["name"]
                              or version_tuple(libc["version"] or "") < version_tuple(built["version"] or "")):
        return f"{name} links {built['name']} {built['version']}; this host has {libc['name']} {libc['version']}"
    return None


def check(store: Path, name: str, pin: dict, *, here: bool = True) -> tuple:
    """The pinned executable's path and receipt, once both hash to the pin and, with ``here``, the executable suits this host."""
    directory = entry(store, name, str(pin.get("sha256")))
    receipt_path = directory / "receipt.json"
    receipt = read_json(receipt_path)
    if sha256_file(receipt_path) != pin.get("receipt_sha256"):
        raise Refused(f"{receipt_path} no longer hashes to its pin")
    problem = receipt_problem(receipt)
    if problem:
        raise Refused(f"{receipt_path}: {problem}")
    if receipt["crate"]["name"] != name or receipt["artifact"]["sha256"] != pin["sha256"]:
        raise Refused(f"{receipt_path} describes {receipt['crate']['name']} {receipt['artifact']['sha256']}, not the pinned {name}")
    problem = here and host_problem(name, receipt)
    if problem:
        raise Refused(problem)
    executable = directory / receipt["artifact"]["file"]
    if executable.is_symlink() or not executable.is_file():
        raise Refused(f"the pinned {name} executable {executable} is missing")
    if sha256_file(executable) != pin["sha256"]:
        raise Refused(f"the pinned {name} executable {executable} no longer hashes to its pin")
    if not os.access(executable, os.X_OK):
        raise Refused(f"the pinned {name} executable {executable} is not executable")
    return executable, receipt


def source_problem(receipt: dict, freeze_commit: str | None, repo: Path = REPO) -> str | None:
    """Why the executable was not built from the crate tree at ``freeze_commit``, or None."""
    if not freeze_commit:
        return "the manifest has no freeze_commit to compare the crate with"
    path = receipt["crate"]["path"]
    done = subprocess.run(["git", "-C", str(repo), "rev-parse", "--verify", "--quiet", f"{freeze_commit}^{{commit}}:{path}"],
                          capture_output=True, text=True, encoding="utf-8")
    if done.returncode:
        return f"freeze commit {freeze_commit} has no crate {path}"
    if done.stdout.strip() != receipt["source"]["tree"]:
        return (f"{receipt['crate']['name']} was built from crate tree {receipt['source']['tree']}; "
                f"freeze commit {freeze_commit} has {done.stdout.strip()}")
    return None


def pin_of(directory: Path) -> dict:
    receipt = read_json(directory / "receipt.json")
    return {"sha256": receipt["artifact"]["sha256"], "receipt_sha256": sha256_file(directory / "receipt.json")}


# --- building and retaining ---------------------------------------------------------------------

def cargo_configs(crate_dir: Path, env: dict) -> list:
    """Cargo configuration files a build in ``crate_dir`` would read."""
    home = Path(env.get("CARGO_HOME") or Path.home() / ".cargo")
    found = []
    for directory in (crate_dir, *crate_dir.parents, home):
        for name in ("config", "config.toml"):
            path = (directory if directory == home else directory / ".cargo") / name
            if path.is_file():
                found.append(path)
    return found


def local_inputs(dep_info: Path) -> list:
    """The local files cargo's dep-info lists for a build; registry and Git dependencies are pinned by Cargo.lock instead."""
    try:
        text = dep_info.read_text(encoding="utf-8")
    except OSError as error:
        raise InputError(f"cargo wrote no dep-info at {dep_info}: {error}") from error
    paths = []
    for line in text.splitlines():
        if line.startswith("#") or ": " not in line:
            continue
        _, _, inputs = line.partition(": ")
        paths += [Path(part.replace("\\ ", " ")).resolve() for part in re.split(r"(?<!\\) ", inputs.strip()) if part]
    return paths


def build_environment() -> dict:
    return {name: value for name, value in os.environ.items() if not CLEARED.fullmatch(name)}


def describe(crate_dir: Path, repo: Path, env: dict) -> dict:
    """The committed source and the active toolchain a build would use; refuses an unclean crate or another toolchain."""
    relative = crate_dir.resolve().relative_to(repo.resolve()).as_posix()
    dirty = run(["git", "status", "--porcelain", "--untracked-files=all", "--", relative], repo)
    if dirty.strip():
        raise Refused(f"{relative} has uncommitted or untracked files:\n{dirty.rstrip()}")
    configs = cargo_configs(crate_dir, env)
    if configs:
        raise Refused("cargo configuration a receipt cannot restore would apply: " + ", ".join(map(str, configs)))
    try:
        import tomllib
    except ModuleNotFoundError:
        raise InputError("install needs Python 3.11 or later to read Cargo.toml") from None
    try:
        manifest = tomllib.loads((crate_dir / "Cargo.toml").read_text(encoding="utf-8"))
        channel = tomllib.loads((crate_dir / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]["channel"]
        package = manifest["package"]
    except (OSError, KeyError, tomllib.TOMLDecodeError) as error:
        raise InputError(f"{relative} needs Cargo.toml with a package and rust-toolchain.toml with a channel: {error!r}") from error
    if not (crate_dir / "Cargo.lock").is_file():
        raise InputError(f"{relative} has no Cargo.lock")
    rustc = dict(line.split(": ", 1) for line in run(["rustc", "-vV"], crate_dir, env).splitlines() if ": " in line)
    if rustc.get("release") != channel:
        raise Refused(f"the active rustc is {rustc.get('release')}; {relative}/rust-toolchain.toml pins {channel}")
    target = host_target()
    if target not in VERIFIED_TARGETS or rustc.get("host") != target:
        raise Refused(f"this host ({rustc.get('host')}) is not a verified Rust runtime target ({', '.join(sorted(VERIFIED_TARGETS))})")
    revision = run(["git", "rev-parse", "HEAD"], repo).strip()
    bins = [row["name"] for row in manifest.get("bin", [])] or [package["name"]]
    return {"crate": {"name": package["name"], "version": package.get("version"), "path": relative, "bin": bins[0]},
            "source": {"repository": origin(repo), "revision": revision,
                       "tree": run(["git", "rev-parse", f"{revision}:{relative}"], repo).strip(),
                       "cargo_toml_sha256": sha256_file(crate_dir / "Cargo.toml"),
                       "cargo_lock_sha256": sha256_file(crate_dir / "Cargo.lock"),
                       "toolchain_file_sha256": sha256_file(crate_dir / "rust-toolchain.toml")},
            "toolchain": {"channel": channel, "rustc": rustc, "cargo": run(["cargo", "-V"], crate_dir, env).strip()},
            "target": target, "libc": host_libc()}


def origin(repo: Path) -> str | None:
    done = subprocess.run(["git", "-C", str(repo), "remote", "get-url", "origin"], capture_output=True, text=True, encoding="utf-8")
    url = re.sub(r"(?<=://)[^/@]+@", "", done.stdout.strip())
    return url if re.match(r"(https?|ssh|git)://|[\w.-]+@[\w.-]+:", url) else None


def build(crate_dir: Path, repo: Path, scratch: Path) -> tuple:
    """Build the crate's binary into ``scratch``; returns the executable and its receipt, without retaining it."""
    env = build_environment()
    identity = describe(crate_dir, repo, env)
    cargo_home = Path(env.get("CARGO_HOME") or Path.home() / ".cargo").resolve()
    flags = [f"--remap-path-prefix={crate_dir.resolve()}=/source", f"--remap-path-prefix={cargo_home}=/cargo"]
    command = ["cargo", "build", "--locked", "--offline", "--release", "--target", identity["target"], "--bin", identity["crate"]["bin"]]
    env.update(CARGO_TARGET_DIR=str(scratch), CARGO_ENCODED_RUSTFLAGS="\x1f".join(flags))
    run(command, crate_dir, env)
    executable = scratch / identity["target"] / "release" / identity["crate"]["bin"]
    entries = [row.split("\t", 1) for row in run(["git", "ls-files", "-s", "-z", "--", identity["crate"]["path"]], repo).split("\0") if row]
    links = [name for mode, name in entries if mode.startswith("120000 ")]
    if links:
        raise Refused(f"{identity['crate']['path']} tracks symbolic links, whose targets its receipt cannot pin: " + ", ".join(links))
    tracked = {(repo / name).resolve() for _, name in entries}
    unpinned = [path for path in local_inputs(executable.with_name(executable.name + ".d"))
                if not path.is_relative_to(scratch.resolve()) and path not in tracked]
    if unpinned:
        raise Refused(f"the build read local files that are not tracked in {identity['crate']['path']}, which its receipt cannot pin: "
                      + ", ".join(map(str, unpinned)))
    receipt = {"schema": RECEIPT, **identity,
               "build": {"command": command, "rustflags": ["--remap-path-prefix=<crate>=/source", "--remap-path-prefix=<cargo home>=/cargo"],
                         "cleared_environment": CLEARED.pattern, "cargo_config_files": []},
               "artifact": {"file": identity["crate"]["bin"], "sha256": sha256_file(executable), "size": executable.stat().st_size},
               "built_at": now()}
    return executable, receipt


def same_build(left: dict, right: dict) -> bool:
    keep = ("crate", "source", "toolchain", "target", "build", "artifact")
    return {key: left.get(key) for key in keep} == {key: right.get(key) for key in keep}


def retain(store: Path, executable: Path, receipt: dict) -> tuple:
    """Place an executable and its receipt in the store; an existing entry is kept only when it records the same build."""
    problem = receipt_problem(receipt)
    if problem:
        raise Refused(f"the receipt is incomplete: {problem}")
    name, digest = receipt["crate"]["name"], receipt["artifact"]["sha256"]
    if sha256_file(executable) != digest:
        raise Refused(f"{executable} does not hash to the receipt's {digest}")
    with locked(store, name) as directory:
        target = entry(store, name, digest)
        if target.exists():
            kept = read_json(target / "receipt.json")
            if not same_build(kept, receipt):
                raise Refused(f"{target} already retains this executable with another receipt; entries are never replaced")
            check(store, name, pin_of(target), here=False)
            return target, False
        staging = directory / f".partial-{uuid.uuid4().hex}"
        staging.mkdir()
        try:
            shutil.copyfile(executable, staging / receipt["artifact"]["file"])
            os.chmod(staging / receipt["artifact"]["file"], 0o555)
            publish(staging / "receipt.json", dumps(receipt))
            os.chmod(staging / "receipt.json", 0o444)
            os.rename(staging, target)
        finally:
            shutil.rmtree(staging, ignore_errors=True)
        check(store, name, pin_of(target), here=False)
        return target, True


# --- selection ----------------------------------------------------------------------------------

def selected(store: Path, name: str) -> dict | None:
    path = store / name / "selected.json"
    return read_json(path) if path.is_file() else None


def select(store: Path, name: str, pin: dict, *, action: str = "select") -> dict:
    """Make ``pin`` the version new operations take. Operations holding a pin are unaffected."""
    with locked(store, name) as directory:
        check(store, name, pin)
        current = selected(store, name)
        previous = current and current["pin"]
        if previous == pin:
            return current
        row = {"pin": pin, "previous": previous, "action": action, "at": now()}
        with open(directory / "selections.jsonl", "a", encoding="utf-8") as history:
            history.write(json.dumps(row, sort_keys=True) + "\n")
            history.flush()
            os.fsync(history.fileno())
        publish(directory / "selected.json", dumps(row))
        return row


def rollback(store: Path, name: str) -> dict:
    current = selected(store, name)
    if not current or not current.get("previous"):
        raise Refused(f"{name} has no previously selected version to return to")
    return select(store, name, current["previous"], action="rollback")


def current_pin(store: Path, name: str) -> dict:
    current = selected(store, name)
    if not current:
        raise Refused(f"{name} has no selected version in {store}")
    check(store, name, current["pin"])
    return current["pin"]


# --- deviations ---------------------------------------------------------------------------------

def deviation(store: Path, run_dir: Path, name: str, old: str, new: str, notes: dict) -> Path:
    """Record a required runtime change with both identities. It changes no pin and permits nothing."""
    blank = sorted(key for key, value in notes.items() if not str(value or "").strip())
    if blank:
        raise InputError("a runtime deviation states " + ", ".join(blank))
    if old == new:
        raise InputError("a runtime deviation names two different versions")
    if not (run_dir / "manifest.json").is_file():
        raise InputError(f"{run_dir} is not a run directory")
    identities = {}
    for side, digest in (("old", old), ("new", new)):
        directory = entry(store, name, digest)
        identities[side] = {"pin": pin_of(directory), "receipt": read_json(directory / "receipt.json")}
    record = {"schema": DEVIATION, "version": 1, "runtime": name, "recorded_at": now(), **identities, **notes}
    path = run_dir / "deviations" / f"runtime-{name}-{old[:12]}-to-{new[:12]}.v1.json"
    path.parent.mkdir(exist_ok=True)
    publish(path, dumps(record), exclusive=True)
    return path


# --- command line -------------------------------------------------------------------------------

def main(argv: list | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    install = commands.add_parser("install")
    install.add_argument("--crate", required=True, type=Path)
    install.add_argument("--select", action="store_true")
    restore = commands.add_parser("restore")
    restore.add_argument("--receipt", required=True, type=Path)
    restore.add_argument("--executable", required=True, type=Path)
    for name in ("verify", "select"):
        sub = commands.add_parser(name)
        sub.add_argument("name")
        sub.add_argument("sha256")
    for name in ("rollback", "pin", "list"):
        commands.add_parser(name).add_argument("name")
    deviate = commands.add_parser("deviation")
    deviate.add_argument("--run", required=True, type=Path)
    deviate.add_argument("--name", required=True)
    deviate.add_argument("--from", dest="old", required=True)
    deviate.add_argument("--to", dest="new", required=True)
    for note in ("reason", "scope", "validation", "accounting"):
        deviate.add_argument(f"--{note}", required=True)
    for sub in commands.choices.values():
        sub.add_argument("--store", type=Path, default=DEFAULT_STORE)
    args = parser.parse_args(argv)
    store = located(args.store)
    try:
        if args.command == "install":
            crate_dir = (REPO / args.crate).resolve()
            with tempfile.TemporaryDirectory(prefix="rust-runtime-build-") as scratch:
                executable, receipt = build(crate_dir, REPO, Path(scratch))
                directory, added = retain(store, executable, receipt)
            result = {"retained": str(directory), "added": added, "pin": pin_of(directory)}
            if args.select:
                result["selected"] = select(store, receipt["crate"]["name"], pin_of(directory))
        elif args.command == "restore":
            directory, added = retain(store, args.executable, read_json(args.receipt))
            result = {"retained": str(directory), "added": added, "pin": pin_of(directory)}
        elif args.command == "verify":
            directory = entry(store, args.name, args.sha256)
            executable, _ = check(store, args.name, pin_of(directory))
            result = {"verified": str(executable), "pin": pin_of(directory)}
        elif args.command == "select":
            result = select(store, args.name, pin_of(entry(store, args.name, args.sha256)))
        elif args.command == "rollback":
            result = rollback(store, args.name)
        elif args.command == "pin":
            result = {args.name: current_pin(store, args.name)}
        elif args.command == "list":
            versions = sorted(path.name for path in (store / args.name).iterdir() if SHA256.fullmatch(path.name)) \
                if (store / args.name).is_dir() else []
            result = {"versions": versions, "selected": selected(store, args.name)}
        else:
            notes = {note: getattr(args, note) for note in ("reason", "scope", "validation", "accounting")}
            result = {"recorded": str(deviation(store, args.run.resolve(), args.name, args.old, args.new, notes))}
    except Refused as error:
        print(f"rust_runtime: {error}", file=sys.stderr)
        return 1
    except (InputError, OSError) as error:
        print(f"rust_runtime: {error}", file=sys.stderr)
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
