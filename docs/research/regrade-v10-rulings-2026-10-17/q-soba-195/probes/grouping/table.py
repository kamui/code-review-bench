"""Usage: python3 -I table.py RESULTS_DIR > table.md

One row per variant: what each group's failing scenario shows under it."""
import re
import sys
from pathlib import Path

ORDER = ["head", "fix-peq1-envtrue", "fix-peq1-lfslines", "fix-p1", "fix-p2", "fix-p3", "fix-c1n", "fix-wide",
         "both-peq1-envtrue+p1", "both-peq1-lfslines+p1", "both-peq1-envtrue+p2", "both-peq1-lfslines+p2",
         "both-p1+p2", "both-peq1-envtrue+c1n"]


def blocks(text):
    out = {}
    for part in text.split("=== scenario ")[1:]:
        name, _, body = part.partition("\n")
        out[name.strip()] = body
    return out


def section(body, title):
    m = re.search(r"--- " + re.escape(title) + r"[^\n]*\n(.*?)(?=\n--- |\Z)", body, re.S)
    return m.group(1) if m else ""


def p1(b):
    s = b["P1-file-only"]
    code = section(s, "exit code").strip() or re.search(r"exit code: (\d+)", s).group(1)
    if "must be set" in s:
        return "refused at start-up"
    return "backup made" if "bundle" in section(s, "files in the backup directory") else f"exit {code}"


def p2(b):
    s = b["P2-file run 2"]
    method = "refs" if "using diff remote method: refs" in s else "clone"
    action = "skipped the clone" if "skipping clone" in s else "cloned again"
    return f"used {method}; second run {action}"


def peq1(b):
    s = b["PEq1-file"]
    logged = "LFS line" if "backup LFS: true" in s else "no LFS line"
    called = "LFS asked for" if "git-lfs ls-files" in s else "LFS not asked for"
    return f"{logged}; {called}"


def p3(b, key):
    s = b[key]
    code = re.search(r"exit code: (\d+)", s).group(1)
    return "refused: provider configuration invalid" if "provider configuration invalid" in s else f"ran nothing, exit {code}"


def c1n(b):
    s = b["C1n-open-count"]
    opens = section(s, "opens of").strip()
    used = "LFS line" if "backup LFS: true" in s else "no LFS line"
    m = b["C1n-missing-lfs-file"]
    missing = "missing-file line" if "missing-lfs does not exist" in m else "no missing-file line"
    return f"{opens} open(s), {used}; {missing}"


def skip(b):
    s = b["C1n-skip-user-repos-file"]
    return "user repos skipped" if "skipping user repos: true" in s and "query=viewer" not in s else "user repos still listed"


rows = ["| Variant | P1: `GIT_BACKUP_DIR_FILE` only | P2: `GITHUB_COMPARE_FILE` = refs, second run | PE-q1: `GITHUB_BACKUP_LFS_FILE` = true | P3: `GITEA_APIURL` only | P3: `SOURCEHUT_APIURL` only | C1n: LFS file opens by the run; log line; missing file | `GITHUB_SKIP_USER_REPOS_FILE` = true |",
        "| --- | --- | --- | --- | --- | --- | --- | --- |"]
results = Path(sys.argv[1])
for v in ORDER:
    path = results / f"{v}.txt"
    if not path.exists():
        continue
    b = blocks(path.read_text())
    rows.append(f"| `{v}` | {p1(b)} | {p2(b)} | {peq1(b)} | {p3(b, 'P3-gitea-url-only')} | {p3(b, 'P3-sourcehut-url-only')} | {c1n(b)} | {skip(b)} |")
print("\n".join(rows))
