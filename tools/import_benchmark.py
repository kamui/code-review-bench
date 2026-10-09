"""Extract a pinned benchmark and verify its external transcript archives."""

import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'bench/tools'))
import evidence_store
REVISION = "6457c79f955c2d6740fe730689a16af6f3aefacb"
MANIFEST = ROOT / "bench/import-manifest.json"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def write_checked(path, data):
    if path.exists():
        if path.read_bytes() != data:
            raise ValueError(f"Refusing to overwrite modified file: {path.relative_to(ROOT)}")
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)


def verify():
    manifest = json.loads(MANIFEST.read_text())
    errors = []
    for entry in manifest["files"] + manifest["transcripts"]:
        if entry.get("status") == "missing":
            continue
        logical = entry.get("preserved_path", entry["path"])
        try:
            path = evidence_store.resolve(ROOT, logical, entry['sha256'])
        except evidence_store.EvidenceError:
            errors.append(logical)
            continue
        if not path.is_file() or digest(path.read_bytes()) != entry["sha256"]:
            errors.append(entry["path"])
    if errors:
        message = "Missing or changed imported evidence: " + ", ".join(errors)
        editable = [entry["path"] for entry in manifest["files"] if entry["path"] in errors
                    and "preserved_path" not in entry and not entry["path"].startswith("bench/runs/")]
        if editable:
            message += ("\nTo keep an intended edit, preserve the imported bytes: "
                        "python3 tools/import_benchmark.py --preserve " + " ".join(editable))
        raise ValueError(message)
    missing = sum(item["status"] == "missing" for item in manifest["transcripts"])
    mismatches = sum(item["status"] == "mismatch" for item in manifest["transcripts"])
    print(f"Verified copied bytes for {len(manifest['files'])} source files and {len(manifest['transcripts']) - missing} archives; {mismatches} superseded source-hash mismatches, {missing} unavailable")


def extract(source):
    previous = json.loads(MANIFEST.read_text()) if MANIFEST.exists() else {"files": []}
    preserved = {entry["path"]: entry["preserved_path"] for entry in previous["files"] if "preserved_path" in entry}
    paths = ["bench", "docs/research/bench-suite-design-2026-09-24.md",
             "docs/research/code-review-one-shot-method.md", "docs/agents/scripts.md",
             "docs/research/builtin-review-benchmark-2026-09-24/answers.py",
             "docs/research/builtin-review-benchmark-2026-09-24/prompts/grader-template.md",
             "docs/research/builtin-review-benchmark-2026-09-24/prompts/regrade-template.md"]
    archive = subprocess.check_output(["git", "-C", str(source), "archive", REVISION, *paths])
    entries = []
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        for member in tar:
            if not member.isfile():
                continue
            relative = Path(member.name)
            if relative.is_absolute() or ".." in relative.parts:
                raise ValueError(f"Unsafe archive path: {member.name}")
            stream = tar.extractfile(member)
            if stream is None:
                raise ValueError(f"Unreadable archive member: {member.name}")
            data = stream.read()
            destination = ROOT / preserved.get(relative.as_posix(), relative.as_posix())
            write_checked(destination, data)
            destination.chmod(member.mode & 0o777)
            entry = {"path": relative.as_posix(), "sha256": digest(data), "bytes": len(data)}
            if relative.as_posix() in preserved:
                entry["preserved_path"] = preserved[relative.as_posix()]
            entries.append(entry)

    transcripts = []
    for entry in entries:
        if not entry["path"].endswith("/attempt.json"):
            continue
        attempt_path = Path(entry["path"])
        attempt = json.loads((ROOT / attempt_path).read_text())
        recorded = attempt.get("transcript_archive")
        if not recorded or not recorded.get("path"):
            continue
        destination = Path("artifacts/transcripts") / attempt_path.parent.relative_to("bench/runs") / "transcript.tar.gz"
        item = {"attempt": str(attempt_path), "path": str(destination), "source": recorded["path"],
                "sha256": recorded["sha256"], "status": "missing"}
        original = Path(recorded["path"]).expanduser()
        if original.is_file():
            data = original.read_bytes()
            write_checked(ROOT / destination, data)
            actual = digest(data)
            item.update(status="verified" if actual == recorded["sha256"] else "mismatch",
                        sha256=actual, expected_sha256=recorded["sha256"], bytes=len(data))
        transcripts.append(item)
    manifest = {"schema_version": 1, "repository": "https://github.com/kamui/skills",
                "revision": REVISION, "files": sorted(entries, key=lambda item: item["path"]),
                "transcripts": sorted(transcripts, key=lambda item: item["attempt"])}
    MANIFEST.write_text(json.dumps(manifest, indent=2) + "\n")
    verify()


def imported_bytes(name, sha256):
    path = ROOT / name
    if path.is_file() and digest(data := path.read_bytes()) == sha256:
        return data
    commits = subprocess.check_output(["git", "-C", str(ROOT), "log", "--format=%H", "--", name], text=True).split()
    for commit in reversed(commits):
        shown = subprocess.run(["git", "-C", str(ROOT), "show", f"{commit}:{name}"], capture_output=True)
        if shown.returncode == 0 and digest(shown.stdout) == sha256:
            return shown.stdout
    raise ValueError(f"Cannot recover the imported bytes from Git history: {name}")


def preserve(paths):
    manifest = json.loads(MANIFEST.read_text())
    entries = {entry["path"]: entry for entry in manifest["files"]}
    for name in paths:
        entry = entries[name]
        if "preserved_path" in entry:
            continue
        data = imported_bytes(name, entry["sha256"])
        destination = Path("artifacts/import-source") / name
        write_checked(ROOT / destination, data)
        entry["preserved_path"] = destination.as_posix()
    MANIFEST.write_text(json.dumps(manifest, indent=2) + "\n")
    verify()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, help="Local skills Git repository")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--preserve", nargs="+", help="Preserve the imported originals of files that are edited here")
    args = parser.parse_args()
    if args.preserve:
        preserve(args.preserve)
    elif args.check:
        verify()
    elif args.source:
        extract(args.source)
    else:
        parser.error("provide --source or --check")


if __name__ == "__main__":
    main()
