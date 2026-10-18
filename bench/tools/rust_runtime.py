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

``install`` needs a clean, committed crate. It writes the crate's Git tree at ``HEAD`` into a snapshot inside
a private Cargo home that holds no configuration, only links to the host's registry and Git caches. It
selects the toolchain once, from the snapshot's ``rust-toolchain.toml``, and runs that toolchain's own
``cargo`` and ``rustc`` from ``/`` for every later step. Cargo searches its working directory's ancestors and
its home for configuration, and stops its workspace search at its home, so Cargo configuration or a workspace
manifest saved elsewhere, even briefly, cannot change the build; ``/.cargo``, the toolchain and the shared caches
stay trusted. The manifest and metadata checks, the receipt's hashes, the toolchain query and the build use
that snapshot. The build runs
``cargo build --locked --offline --release`` for this host's target, with only ``BUILD_ENVIRONMENT`` passed
through and source paths remapped, once that ``rustc`` is the release ``rust-toolchain.toml`` pins. It
refuses what a receipt could not restore:

- A Cargo configuration file in ``/.cargo`` or tracked in the crate, a relative ``HOME``, ``CARGO_HOME``,
  ``RUSTUP_HOME`` or ``TMPDIR``, a parent Cargo workspace in the checkout, and ``[patch]`` or ``[replace]`` sections.
- A manifest, lockfile or toolchain file Git does not track, a tracked symbolic link or submodule, and a local
  dependency whose manifest is outside the snapshot.
- A local file in the build's dep-info outside the snapshot, such as an absolute ``include_str!`` path. A
  relative path to a file the tree does not hold finds nothing in the snapshot, so that build fails.
- A build that rustc or a build script reports reading one of the passed-through variables.

``install`` needs Python 3.11 or later; the other commands do not. ``restore`` retains an executable from
elsewhere, such as another store or a rebuild, only when it hashes to the receipt.

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
# The supported hosts of docs/agents/scripts.md; .github/workflows/rust-runtime.yml exercises each one.
HOST_TARGETS = {("Linux", "x86_64"): "x86_64-unknown-linux-gnu", ("Linux", "aarch64"): "aarch64-unknown-linux-gnu",
                ("Darwin", "x86_64"): "x86_64-apple-darwin", ("Darwin", "arm64"): "aarch64-apple-darwin"}
SUPPORTED_TARGETS = frozenset(HOST_TARGETS.values())
# The only variables a build sees. Any other variable a crate reads is absent, so it reads the same everywhere.
BUILD_ENVIRONMENT = ("PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "TMPDIR")
# Cargo reads configuration from every ancestor of its working directory; this one has no ancestor but itself.
CARGO_CWD = Path("/")
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
    if host not in SUPPORTED_TARGETS:
        return f"{host} is not a supported Rust runtime target ({', '.join(sorted(SUPPORTED_TARGETS))})"
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

def cargo_configs(source: Path) -> list:
    """Cargo configuration files the build would read from ``CARGO_CWD``, or that the crate tracks and the build would
    ignore. The private Cargo home holds none."""
    return sorted(path for directory in (CARGO_CWD, source) for name in ("config", "config.toml")
                  if (path := directory / ".cargo" / name).is_file())


def private_home(scratch: Path, host: dict) -> Path:
    """A Cargo home only this build reads, linking the host's registry and Git caches and the locks that guard them.
    A missing lock is linked anyway, so cargo creates it in the host home and other cargo processes share it."""
    shared = Path(host["CARGO_HOME"]) if host.get("CARGO_HOME") else Path(host.get("HOME") or Path.home()) / ".cargo"
    home = Path(tempfile.mkdtemp(prefix="cargo-home-", dir=scratch))
    caches = [name for name in ("registry", "git") if (shared / name).exists()]
    for name in caches + ([".package-cache", ".package-cache-mutate"] if caches else []):
        (home / name).symlink_to(shared / name)
    return home.resolve()


def isolated_environment(source: Path, host: dict) -> dict:
    """The build environment: ``host`` with the private Cargo home, and ``PATH`` led by the bin directory of the
    toolchain that rustup or another proxy selects for the snapshot, chosen once here."""
    tools = Path(run(["rustc", "--print", "sysroot"], source, host).strip()) / "bin"
    missing = [name for name in ("cargo", "rustc") if not (tools / name).is_file()]
    if missing:
        raise InputError(f"the selected toolchain {tools.parent} has no {' or '.join(missing)}")
    return {**host, "CARGO_HOME": str(source.parent), "PATH": os.pathsep.join(filter(None, (str(tools), host.get("PATH"))))}


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
    """The passed-through variables; a relative directory would resolve against cargo's directory, not this one."""
    env = {name: os.environ[name] for name in BUILD_ENVIRONMENT if name in os.environ}
    relative = sorted(name for name in ("HOME", "CARGO_HOME", "RUSTUP_HOME", "TMPDIR") if name in env and not Path(env[name]).is_absolute())
    if relative:
        raise Refused(f"{', '.join(relative)} must be absolute for a build whose configuration a receipt can pin")
    return env


def consumed_variables(scratch: Path) -> set:
    """The environment variables rustc's dep-info or a build script's ``rerun-if-env-changed`` says the build read."""
    names = set()
    for path in scratch.rglob("*.d"):
        names |= {line[len("# env-dep:"):].split("=", 1)[0] for line in path.read_text(encoding="utf-8", errors="replace").splitlines()
                  if line.startswith("# env-dep:")}
    for path in scratch.rglob("build/*/output"):
        names |= {line.split("=", 1)[1].strip() for line in path.read_text(encoding="utf-8", errors="replace").splitlines()
                  if line.startswith(("cargo:rerun-if-env-changed=", "cargo::rerun-if-env-changed="))}
    return names


def snapshot(repo: Path, tree: str, relative: str, destination: Path) -> set:
    """Write the crate's Git tree into ``destination``, so later edits to the checkout cannot reach the build; returns
    the files written, resolved. Refuses a tracked symbolic link or submodule, whose target the tree does not hold."""
    entries = [row.split("\t", 1) for row in run(["git", "ls-tree", "-r", "-z", tree], repo).split("\0") if row]
    others = [f"{relative}/{name}" for meta, name in entries if not meta.startswith(("100644 blob ", "100755 blob "))]
    if others:
        raise Refused(f"{relative} tracks symbolic links or submodules, whose targets its receipt cannot pin: " + ", ".join(others))
    written = set()
    for meta, name in entries:
        mode, _, oid = meta.split(" ")
        done = subprocess.run(["git", "-C", str(repo), "cat-file", "blob", oid], capture_output=True)
        if done.returncode:
            raise InputError(f"git cat-file blob {oid} exited {done.returncode}: {done.stderr.decode(errors='replace').strip()}")
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        with open(path, "xb") as handle:
            handle.write(done.stdout)
        path.chmod(0o755 if mode == "100755" else 0o644)
        written.add(path.resolve())
    return written


def describe(crate_dir: Path, repo: Path, source: Path, host: dict) -> tuple:
    """The committed source and the toolchain a build would use, the files of the crate's snapshot, and the build's
    environment.

    Writes the crate tree at ``HEAD`` into ``source``, directly inside the private Cargo home, then checks and
    describes that snapshot, so the receipt names exactly what cargo compiles. Refuses an unclean crate, local build
    inputs Git does not track there, or another toolchain."""
    relative = crate_dir.resolve().relative_to(repo.resolve()).as_posix()
    dirty = run(["git", "status", "--porcelain", "--untracked-files=all", "--", relative], repo)
    if dirty.strip():
        raise Refused(f"{relative} has uncommitted or untracked files:\n{dirty.rstrip()}")
    try:
        import tomllib
    except ModuleNotFoundError:
        raise InputError("install needs Python 3.11 or later to read Cargo.toml") from None
    revision = run(["git", "rev-parse", "HEAD"], repo).strip()
    tree = run(["git", "rev-parse", f"{revision}:{relative}"], repo).strip()
    tracked = snapshot(repo, tree, relative, source)
    configs = cargo_configs(source)
    if configs:
        raise Refused("cargo configuration a receipt cannot restore would apply: " + ", ".join(map(str, configs)))
    untracked = [name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml") if (source / name).resolve() not in tracked]
    if untracked:
        raise Refused(f"{relative} does not track {', '.join(untracked)}, so its crate tree does not hold them")
    try:
        manifest = tomllib.loads((source / "Cargo.toml").read_text(encoding="utf-8"))
        channel = tomllib.loads((source / "rust-toolchain.toml").read_text(encoding="utf-8"))["toolchain"]["channel"]
        package = manifest["package"]
    except (OSError, KeyError, tomllib.TOMLDecodeError) as error:
        raise InputError(f"{relative} needs Cargo.toml with a package and rust-toolchain.toml with a channel: {error!r}") from error
    overrides = sorted(key for key in ("patch", "replace") if key in manifest)
    if overrides:
        raise Refused(f"{relative}/Cargo.toml has [{'], ['.join(overrides)}] sections, whose sources its receipt cannot pin")
    env = isolated_environment(source, host)
    for directory in (crate_dir, source):
        workspace = Path(run(["cargo", "locate-project", "--workspace", "--message-format", "plain", "--offline",
                              "--manifest-path", str(directory / "Cargo.toml")], CARGO_CWD, env).strip())
        if workspace.resolve() != (directory / "Cargo.toml").resolve():
            raise Refused(f"{relative} belongs to the Cargo workspace {workspace}, whose manifest and lockfile its receipt cannot pin")
    metadata = json.loads(run(["cargo", "metadata", "--offline", "--locked", "--all-features", "--format-version", "1",
                               "--manifest-path", str(source / "Cargo.toml")], CARGO_CWD, env))
    local = sorted(str(package["manifest_path"]) for package in metadata["packages"] if package["source"] is None
                   and Path(package["manifest_path"]).resolve() not in tracked)
    if local:
        raise Refused(f"{relative} depends on local packages whose manifests it does not track: {', '.join(local)}")
    rustc = dict(line.split(": ", 1) for line in run(["rustc", "-vV"], CARGO_CWD, env).splitlines() if ": " in line)
    if rustc.get("release") != channel:
        raise Refused(f"the active rustc is {rustc.get('release')}; {relative}/rust-toolchain.toml pins {channel}")
    target = host_target()
    if target not in SUPPORTED_TARGETS or rustc.get("host") != target:
        raise Refused(f"this host ({rustc.get('host')}) is not a supported Rust runtime target ({', '.join(sorted(SUPPORTED_TARGETS))})")
    bins = [row["name"] for row in manifest.get("bin", [])] or [package["name"]]
    return {"crate": {"name": package["name"], "version": package.get("version"), "path": relative, "bin": bins[0]},
            "source": {"repository": origin(repo), "revision": revision, "tree": tree,
                       "cargo_toml_sha256": sha256_file(source / "Cargo.toml"),
                       "cargo_lock_sha256": sha256_file(source / "Cargo.lock"),
                       "toolchain_file_sha256": sha256_file(source / "rust-toolchain.toml")},
            "toolchain": {"channel": channel, "rustc": rustc, "cargo": run(["cargo", "-V"], CARGO_CWD, env).strip()},
            "target": target, "libc": host_libc()}, tracked, env


def origin(repo: Path) -> str | None:
    done = subprocess.run(["git", "-C", str(repo), "remote", "get-url", "origin"], capture_output=True, text=True, encoding="utf-8")
    url = re.sub(r"(?<=://)[^/@]+@", "", done.stdout.strip())
    return url if re.match(r"(https?|ssh|git)://|[\w.-]+@[\w.-]+:", url) else None


def build(crate_dir: Path, repo: Path, scratch: Path) -> tuple:
    """Build the crate's binary from a snapshot of its committed tree in ``scratch``; returns the executable and its
    receipt, without retaining it."""
    host = build_environment()
    home = private_home(scratch, host)
    source, target_dir = home / "source", scratch / "target"
    identity, tracked, env = describe(crate_dir, repo, source, host)
    # The snapshot lies inside the Cargo home, and rustc applies the last matching prefix.
    flags = [f"--remap-path-prefix={home}=/cargo", f"--remap-path-prefix={source}=/source"]
    command = ["cargo", "build", "--locked", "--offline", "--release", "--target", identity["target"], "--bin", identity["crate"]["bin"]]
    passed = sorted(env)
    env.update(CARGO_TARGET_DIR=str(target_dir), CARGO_ENCODED_RUSTFLAGS="\x1f".join(flags))
    run([*command, "--manifest-path", str(source / "Cargo.toml")], CARGO_CWD, env)
    executable = target_dir / identity["target"] / "release" / identity["crate"]["bin"]
    read = sorted(consumed_variables(target_dir) & set(env))
    if read:
        raise Refused(f"the build read host environment variables its receipt cannot pin: {', '.join(read)}")
    unpinned = [path for path in local_inputs(executable.with_name(executable.name + ".d"))
                if not path.is_relative_to(target_dir.resolve()) and path not in tracked]
    if unpinned:
        raise Refused(f"the build read local files that are not tracked in {identity['crate']['path']}, which its receipt cannot pin: "
                      + ", ".join(map(str, unpinned)))
    receipt = {"schema": RECEIPT, **identity,
               "build": {"command": command, "rustflags": ["--remap-path-prefix=<cargo home>=/cargo", "--remap-path-prefix=<crate>=/source"],
                         "environment": passed, "cargo_config_files": []},
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


def publish_selection(store: Path, name: str, pin: dict, action: str) -> dict:
    """Record ``pin`` as the selection; the caller holds the runtime's lock."""
    check(store, name, pin)
    current = selected(store, name)
    previous = current and current["pin"]
    if previous == pin:
        return current
    row = {"pin": pin, "previous": previous, "action": action, "at": now()}
    with open(store / name / "selections.jsonl", "a", encoding="utf-8") as history:
        history.write(json.dumps(row, sort_keys=True) + "\n")
        history.flush()
        os.fsync(history.fileno())
    publish(store / name / "selected.json", dumps(row))
    return row


def select(store: Path, name: str, pin: dict) -> dict:
    """Make ``pin`` the version new operations take. Operations holding a pin are unaffected."""
    with locked(store, name):
        return publish_selection(store, name, pin, "select")


def rollback(store: Path, name: str) -> dict:
    """Select the version the current selection replaced, reading and writing under one lock."""
    with locked(store, name):
        current = selected(store, name)
        if not current or not current.get("previous"):
            raise Refused(f"{name} has no previously selected version to return to")
        return publish_selection(store, name, current["previous"], "rollback")


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
