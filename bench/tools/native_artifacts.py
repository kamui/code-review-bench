#!/usr/bin/env python3
"""Pack indexed scratch files once per content hash; verify or restore a filed artifact tree.

Usage: native_artifacts.py verify ATTEMPT
       native_artifacts.py restore ATTEMPT --out NEW_DIRECTORY
"""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import zipfile

STORAGE = "native-artifact-storage.v1.json"
ARCHIVE = "native-scratch.v1.zip"
INDEX = "native-artifacts.json"


class ArtifactError(ValueError):
    pass


def digest(path):
    with Path(path).open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def reference(path):
    return {"path": path.name, "sha256": digest(path)}


def relative_path(value):
    path = Path(value)
    if not value or path.is_absolute() or ".." in path.parts or path.as_posix() != value:
        raise ArtifactError(f"unsafe artifact path: {value!r}")
    return path


def indexed_files(index):
    rows = json.loads(index.read_text())["files"]
    files = {}
    for row in rows:
        name = row["path"]
        relative_path(name)
        if name in files:
            raise ArtifactError(f"duplicate indexed artifact: {name}")
        files[name] = row
    return files


def check_file(path, row):
    if path.is_symlink() or not path.is_file() or path.stat().st_size != row["bytes"] or digest(path) != row["sha256"]:
        raise ArtifactError(f"artifact differs from index: {row['path']}")


def check_reference(directory, ref):
    path = directory / relative_path(ref["path"])
    if not path.resolve().is_relative_to(directory.resolve()) or digest(path) != ref["sha256"]:
        raise ArtifactError(f"artifact checksum mismatch: {ref['path']}")
    return path


def verify(directory, pin=None):
    directory = Path(directory)
    if pin is None:
        pin = json.loads((directory / "attempt.json").read_text())["native_artifact_storage"]
    storage = json.loads(check_reference(directory, pin).read_text())
    if storage["schema_version"] != 1:
        raise ArtifactError("unsupported native artifact storage version")
    files = indexed_files(check_reference(directory, storage["index"]))
    archived = storage["archived_paths"]
    archived_set = set(archived)
    if len(archived_set) != len(archived) or not archived_set <= files.keys():
        raise ArtifactError("archive paths must be distinct indexed files")
    if not storage["modes"].keys() <= set(archived):
        raise ArtifactError("archive modes must refer to archived files")
    if any(type(mode) is not int or not 0 <= mode <= 0o777 for mode in storage["modes"].values()):
        raise ArtifactError("invalid archived file permissions")
    blobs = {files[name]["sha256"]: files[name]["bytes"] for name in archived}
    if any(blobs[files[name]["sha256"]] != files[name]["bytes"] for name in archived):
        raise ArtifactError("the same content hash has conflicting indexed sizes")
    with zipfile.ZipFile(check_reference(directory, storage["archive"])) as archive:
        if len(archive.namelist()) != len(blobs) or set(archive.namelist()) != blobs.keys():
            raise ArtifactError("archive contents differ from indexed scratch files")
        for sha, size in blobs.items():
            with archive.open(sha) as handle:
                actual = hashlib.file_digest(handle, "sha256").hexdigest()
            if actual != sha or archive.getinfo(sha).file_size != size:
                raise ArtifactError(f"archived content differs from index: {sha}")
    artifact_root = directory / relative_path(storage["artifact_root"])
    if artifact_root.is_symlink() or not artifact_root.resolve().is_relative_to(directory.resolve()):
        raise ArtifactError("native artifact root escapes filed directory")
    paths = list(artifact_root.rglob("*"))
    if any(path.is_symlink() or not (path.is_dir() or path.is_file()) for path in paths):
        raise ArtifactError("filed artifacts must be regular files and directories")
    if {path.relative_to(artifact_root).as_posix() for path in paths if path.is_file()} != files.keys() - archived_set:
        raise ArtifactError("loose artifacts differ from indexed file inventory")
    for name, row in files.items():
        if name not in archived_set:
            path = artifact_root / name
            if not path.resolve().is_relative_to(artifact_root.resolve()):
                raise ArtifactError(f"artifact escapes root: {name}")
            check_file(path, row)
    return storage, files


def file_artifacts(root, index, out, native_relative=None):
    """File a new native tree, leaving existing filed attempts on their original layout."""
    root, index, out = Path(root), Path(index), Path(out)
    files = indexed_files(index)
    archived = sorted(name for name in files if "scratch" in Path(name).parts[:-1] and name != native_relative)
    record_path = out / "attempt.json"
    record = json.loads(record_path.read_text()) if record_path.exists() else {}
    if "native_artifact_storage" in record or (out / STORAGE).exists():
        pin = record.get("native_artifact_storage") or reference(out / STORAGE)
        storage, _ = verify(out, pin)
        if storage["index"]["sha256"] != digest(index) or storage["artifact_root"] != root.name:
            raise ArtifactError("refusing to change an existing native artifact filing")
        return pin
    if not archived or record_path.exists():
        shutil.copytree(root, out / root.name, dirs_exist_ok=True)
        shutil.copy2(index, out / INDEX)
        return None
    if root.is_symlink():
        raise ArtifactError("scratch packing requires a regular artifact root")
    paths = list(root.rglob("*"))
    if any(path.is_symlink() or not (path.is_dir() or path.is_file()) for path in paths):
        raise ArtifactError("scratch packing requires regular files and directories")
    actual = {path.relative_to(root).as_posix() for path in paths if path.is_file()}
    if actual != files.keys():
        raise ArtifactError("native artifact tree differs from indexed file inventory")
    for name, row in files.items():
        check_file(root / name, row)
    out.mkdir(parents=True, exist_ok=True)
    if root.name in (ARCHIVE, STORAGE, INDEX) or any((out / name).exists() for name in (root.name, ARCHIVE, STORAGE, INDEX)):
        raise ArtifactError("scratch packing needs unused artifact output paths")
    with tempfile.TemporaryDirectory(prefix="native-artifacts-", dir=out) as temp:
        staged = Path(temp)
        shutil.copy2(index, staged / INDEX)
        archived_set = set(archived)
        modes = {}
        written = set()
        with zipfile.ZipFile(staged / ARCHIVE, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for name in archived:
                path, sha = root / name, files[name]["sha256"]
                mode = path.stat().st_mode & 0o777
                if mode != 0o644:
                    modes[name] = mode
                if sha not in written:
                    info = zipfile.ZipInfo(sha)
                    info.compress_type = zipfile.ZIP_DEFLATED
                    archive.writestr(info, path.read_bytes())
                    written.add(sha)
        (staged / root.name).mkdir()
        for name in files.keys() - archived_set:
            dest = staged / root.name / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(root / name, dest)
        storage = {"schema_version": 1, "artifact_root": root.name, "index": reference(staged / INDEX),
                   "archive": reference(staged / ARCHIVE), "archived_paths": archived, "modes": modes}
        (staged / STORAGE).write_text(json.dumps(storage, indent=2) + "\n")
        pin = reference(staged / STORAGE)
        verify(staged, pin)
        for name in (root.name, INDEX, ARCHIVE, STORAGE):
            (staged / name).rename(out / name)
    return pin


def restore(directory, out):
    directory, out = Path(directory), Path(out)
    storage, files = verify(directory)
    if out.exists() or out.is_symlink():
        raise ArtifactError("restoration requires a new output directory")
    out.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="native-restore-", dir=out.parent) as temp:
        staged = Path(temp) / "artifacts"
        shutil.copytree(directory / storage["artifact_root"], staged)
        with zipfile.ZipFile(directory / storage["archive"]["path"]) as archive:
            for name in storage["archived_paths"]:
                dest = staged / name
                dest.parent.mkdir(parents=True, exist_ok=True)
                with archive.open(files[name]["sha256"]) as source, dest.open("xb") as target:
                    shutil.copyfileobj(source, target)
                dest.chmod(storage["modes"].get(name, 0o644))
        for name, row in files.items():
            check_file(staged / name, row)
        staged.rename(out)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["verify", "restore"])
    parser.add_argument("attempt", type=Path)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "restore":
            if args.out is None:
                parser.error("restore requires --out")
            restore(args.attempt, args.out)
        else:
            verify(args.attempt)
    except (ArtifactError, OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        parser.exit(1, f"native_artifacts.py: {error}\n")
    print(f"{args.command}: verified all indexed artifact bytes")


if __name__ == "__main__":
    main()
