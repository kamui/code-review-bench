#!/usr/bin/env python3
"""Classify the public files found by the code searches: where each one loads ripgrep's zsh completion, and whether
the same file runs compinit or loads a plugin framework that runs it.

Usage: classify_code.py upstream/public-code-files.json OUT.json

Lines that start with `#` are ignored. A file is put in one class:
  compinit-in-file      a line runs compinit (also zicompinit or `zcomet compinit`)
  framework-in-file     no such line, but the file loads a framework that runs compinit (list below)
  neither-in-file       neither; compinit may still run from another file this one sources, or from a system file
For a file that runs compinit, `order` says whether its first ripgrep line comes before or after its first compinit
line."""
import json
import re
import sys

RG = re.compile(r"rg --generate[ =]complete-zsh")
COMPINIT = re.compile(r"(^|[\s;&|({])(compinit|zicompinit)\b|zcomet compinit")
FRAMEWORKS = {
    "oh-my-zsh": re.compile(r"oh-my-zsh\.sh"),
    "prezto": re.compile(r"zprezto/init\.zsh"),
    "zinit": re.compile(r"zinit\.zsh|\bzinit (light|load|cdreplay)|\bzi (light|load)"),
    "antidote": re.compile(r"antidote (load|bundle)"),
    "zim": re.compile(r"zim/init\.zsh|zimfw"),
    "zplug": re.compile(r"zplug load"),
    "antigen": re.compile(r"antigen apply"),
    "znap": re.compile(r"znap\.zsh|\bznap source"),
    "sheldon": re.compile(r"sheldon source"),
    "zgen": re.compile(r"zgenom|zgen (load|save)"),
}


def classify(rec):
    lines = rec["text"].splitlines()
    live = [(n, line) for n, line in enumerate(lines, 1) if not line.lstrip().startswith("#")]
    rg = [(n, line.strip()) for n, line in live if RG.search(line)]
    ci = [(n, line.strip()) for n, line in live if COMPINIT.search(line)]
    fw = sorted({name for name, pat in FRAMEWORKS.items() for _, line in live if pat.search(line)})
    forms = []
    for _, line in rg:
        if re.search(r"source\s*<\(|\.\s*<\(", line):
            forms.append("source")
        elif "eval" in line:
            forms.append("eval")
        elif re.search(r">\s*\S", line.split("complete-zsh", 1)[1]):
            forms.append("saved to a file")
        else:
            forms.append("other")
    cls = "compinit-in-file" if ci else ("framework-in-file" if fw else "neither-in-file")
    order = None
    if ci and rg:
        order = "ripgrep line after compinit" if rg[0][0] > ci[0][0] else "ripgrep line before compinit"
    return {
        "repo": rec["repo"], "path": rec["path"], "html_url": rec["html_url"],
        "last_commit_date": rec.get("last_commit_date"), "class": cls, "frameworks": fw, "order": order,
        "ripgrep_lines": [f"{n}: {t}" for n, t in rg], "forms": forms, "compinit_lines": [f"{n}: {t}" for n, t in ci],
    }


def main():
    records = json.load(open(sys.argv[1]))
    rows = [classify(r) for r in records if "text" in r]
    json.dump(rows, open(sys.argv[2], "w"), indent=1)
    counts = {}
    for r in rows:
        key = (r["class"], r["order"] or "-", "/".join(sorted(set(r["forms"]))) or "-")
        counts[key] = counts.get(key, 0) + 1
    for key, n in sorted(counts.items()):
        print(n, *key, sep="\t")
    print("files:", len(rows))


if __name__ == "__main__":
    main()
