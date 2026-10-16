"""Select immutable classifier evidence for the repository's release store."""

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
EXPERIMENTS = ("jev-grading-2026-10-15", "luna-grading-2026-10-15")


def exclusion(path):
    parts = path.relative_to(ROOT).parts
    if path.name in {"auth.json", ".credentials.json"}:
        raise ValueError(f"Credential file requires removal before packaging: {path}")
    if path.is_symlink():
        return "Runtime executable link; executable identity is pinned in dispatch receipts"
    if "__pycache__" in parts:
        return "Rebuildable Python bytecode"
    if ".git" in parts:
        return "Synthetic probe Git metadata; marker files and captured requests are retained"
    if "home" in parts:
        relative = parts[parts.index("home") + 1:]
        if relative[:2] == (".codex", "sessions"):
            return None
        if relative in ((".codex", "config.toml"), (".codex", "classifier-models.json")):
            return None
        return "Generated client runtime/cache/account state; configuration, requests, diagnostics and complete session JSONL retained"
    return None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    files, omitted = [], []
    for experiment in EXPERIMENTS:
        research = Path("docs/research") / experiment
        for source_root, destination in ((ROOT / research / "results", research / "results"),
                                         (ROOT / ".local" / experiment, research / "probes")):
            if not source_root.is_dir():
                raise ValueError(f"Missing original evidence directory: {source_root}")
            for source in sorted(source_root.rglob("*")):
                if source.is_dir() and not source.is_symlink():
                    continue
                logical = destination / source.relative_to(source_root)
                reason = exclusion(source)
                if reason:
                    omitted.append({"path": str(logical), "reason": reason})
                elif source.is_file():
                    files.append({"source": str(source), "path": str(logical), "kind": "evidence"})
                else:
                    raise ValueError(f"Unclassified special file: {source}")
    if not files:
        raise ValueError("No evidence selected")
    selection = {"repository": "kamui/code-review-bench", "tag": "evidence-classifier-experiments-2026-10-15-v1",
                 "subjects": list(EXPERIMENTS), "files": files}
    (args.out / "selection.json").write_text(json.dumps(selection, indent=2) + "\n")
    (args.out / "selection-notes.json").write_text(json.dumps({
        "selected_files": len(files), "selected_bytes": sum(Path(f["source"]).stat().st_size for f in files),
        "probe_mapping": ".local/<experiment>/ maps to docs/research/<experiment>/probes/ without rewriting bytes",
        "excluded_files": omitted, "local_retirement": False,
    }, indent=2) + "\n")
    print(f"Selected {len(files)} evidence files; classified {len(omitted)} excluded runtime files")


if __name__ == "__main__":
    main()
