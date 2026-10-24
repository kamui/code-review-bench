"""Usage: python3 -I table.py RESULTS_DIR > table.md

Reads each variant's output and prints one row per variant with a short reading of each case."""
import re
import sys
from pathlib import Path

ORDER = ["head", "fix-peq1-envtrue", "fix-peq1-lfslines", "fix-peq2", "fix-peq3", "fix-peq4", "fix-wide",
         "fix-p1", "fix-p2", "fix-c1", "fix-c1n", "fix-one-backupdir-read",
         "both-peq4+p1", "both-peq2+p2", "both-peq1-lfslines+c1n", "both-peq1-envtrue+c1n"]
COLS = ["P1-bb-email-only-with-github", "P1-bb-oauth-partial-with-github", "P1-control-github-only",
        "P2-newline", "P2-control-plain", "PEq2-file-only", "PEq4-gitea-url-only", "PEq1-file", "C1n-skip-file"]


def blocks(text):
    out = {}
    for part in text.split("=== scenario ")[1:]:
        name, _, body = part.partition("\n")
        out[name.strip()] = body
    return out


def exit_code(body):
    m = re.search(r"--- exit code: (\d+)", body)
    return m.group(1) if m else "?"


def read(name, body):
    code = exit_code(body)
    out = body.split("--- soba output", 1)[1].split("--- exit code", 1)[0]
    if "provider configuration invalid" in out:
        return f"refused at start-up (\"provider configuration invalid\"), exit {code}"
    if "must be set" in out:
        return f"refused at start-up (\"GIT_BACKUP_DIR must be set\"), exit {code}"
    summary = "backups complete" if "backups complete" in out else ("all backups failed" if "all backups failed" in out else "?")
    extra = []
    if name.startswith("P1"):
        extra.append("BitBucket not run" if "backing up BitBucket" not in out else "BitBucket run")
    if name.startswith("P2"):
        extra.append("clone failed (chdir)" if "chdir" in out else "bundle written")
    if name == "PEq1-file":
        extra.append("LFS line" if "backup LFS: true" in out else "no LFS line")
        extra.append("LFS asked for" if "git-lfs ls-files" in body else "LFS not asked for")
    if name == "C1n-skip-file":
        extra.append("skip line" if "skipping user repos: true" in out else "no skip line")
        extra.append("user repos listed" if "query=viewer" in body else "user repos not listed")
    return f"{summary}, exit {code}" + ("; " + "; ".join(extra) if extra else "")


results = Path(sys.argv[1])
print("| Variant | " + " | ".join(f"`{c}`" for c in COLS) + " |")
print("| --- | " + " | ".join("---" for _ in COLS) + " |")
for v in ORDER:
    f = results / f"{v}.txt"
    if not f.exists():
        continue
    b = blocks(f.read_text())
    print(f"| `{v}` | " + " | ".join(read(c, b[c]) if c in b else "not run" for c in COLS) + " |")
