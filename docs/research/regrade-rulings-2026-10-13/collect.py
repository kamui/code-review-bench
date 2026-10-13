#!/usr/bin/env python3
"""Copy each blind session's answers and notes from its working folder into the round.

    python3 docs/research/regrade-rulings-2026-10-13/collect.py SESSIONS

SESSIONS is the directory of working folders, one per session, named `<task letter>-<cand|credit|label>-<sol|astra>`.
A folder whose name ends in a further part, such as `-2`, is a rerun and replaces the first run's copy; the first
run's folder and log stay where they are and are counted in the usage."""
import json
from pathlib import Path
import shutil
import sys

HERE = Path(__file__).resolve().parent
TASKS = {"x": "kubernetes", "o": "astro", "u": "grpc-go-6919", "i": "requests", "r": "base-ui", "p": "hono", "n": "ripgrep", "v": "django", "s": "seaweedfs",
         "m": "grpc-go-7390", "j": "trpc", "w": "graphql-js"}
KINDS = {"cand": "candidates", "credit": "credit", "label": "labels"}


def main(sessions):
    for folder in sorted(path for path in Path(sessions).iterdir() if path.is_dir() and (path / "answers.json").is_file()):
        task, kind, model = folder.name.split("-")[:3]
        out = HERE / "assessors" / TASKS[task]
        out.mkdir(exist_ok=True)
        json.loads((folder / "answers.json").read_text(encoding="utf-8"))
        shutil.copyfile(folder / "answers.json", out / f"answers-{KINDS[kind]}-{model}.json")
        if (folder / "notes.md").is_file():
            shutil.copyfile(folder / "notes.md", out / f"notes-{KINDS[kind]}-{model}.md")
        print(folder.name, "->", out.name)


if __name__ == "__main__":
    main(sys.argv[1])
