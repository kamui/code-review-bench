"""Side-by-side table of the C1 and C1nb probe results.

Usage: python3 -I table.py <probes dir>
Reads C1/result-*.txt and C1nb/result-*.txt for base, head and the head with each fix, and prints
one row per keystroke.
C1 cells: "control renders/Field.Root renders/Description renders".
C1nb cells: the text and state the app's effect on the text saw ("text|D<dirty>F<filled>V<valid>").
"""

import json
import sys
from pathlib import Path

RUNS = [
    ("base", "result-base.txt"),
    ("head", "result-head.txt"),
    ("fix A", "result-fix-A-head.txt"),
    ("fix B", "result-fix-B-head.txt"),
    ("A and B", "result-fix-A-and-B-head.txt"),
]


def probes(path: Path) -> dict:
    out = {}
    for line in path.read_text().splitlines():
        if line.startswith("PROBE "):
            _, name, payload = line.split(" ", 2)
            out[name] = json.loads(payload)
    return out


def keystrokes(data: dict):
    for key in ("keystrokes", "beforeSubmit", "afterSubmit"):
        for k in data.get(key, []):
            label = k["typed"] or "(empty)"
            if key == "afterSubmit":
                label = f"{label} (after submit)"
            yield label, k


def table(root: Path, group: str, cell) -> list[str]:
    runs = [(name, probes(root / group / file)) for name, file in RUNS]
    lines = ["| Case | Keystroke | " + " | ".join(n for n, _ in runs) + " |"]
    lines.append("| --- | --- | " + " | ".join("---" for _ in runs) + " |")
    for case in runs[0][1]:
        rows = [list(keystrokes(r[case])) for _, r in runs]
        for i, (label, _) in enumerate(rows[0]):
            cells = [cell(row[i][1]) for row in rows]
            lines.append(f"| {case} | `{label}` | " + " | ".join(cells) + " |")
    return lines


def counts(k: dict) -> str:
    return f"{len(k['controlRenders'])}/{len(k['rootRenders'])}/{k['descriptionRenders']}"


def effect(k: dict) -> str:
    return " ".join(f"`{e}`" for e in k["effectOnText"])


def main() -> None:
    root = Path(sys.argv[1])
    print("Renders per keystroke (control/Field.Root/Description), probe C1, jsdom:\n")
    print("\n".join(table(root, "C1", counts)))
    print("\nWhat an app effect that depends on the text saw, probe C1nb, jsdom:\n")
    print("\n".join(table(root, "C1nb", effect)))


main()
