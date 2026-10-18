"""Usage: python3 -I search_maintainers.py GROUP

Searches every issue, pull request, issue comment and review comment of jonhadfield/soba
created at or before the cut-off (2026-07-28T21:06:09Z), from the saved responses
all-issues-and-prs.json, all-issue-comments.json and all-review-comments.json, and prints each
hit with the text around every match."""
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).parent
CUT = "2026-07-28T21:06:09Z"
QUERIES = {
    "P1": r"_FILE|GIT_BACKUP_DIR|secret.{0,40}file|file.{0,40}secret",
    "P2": r"_FILE|_COMPARE|compare method|DiffRemoteMethod",
    "P3": r"no providers defined|provider configuration invalid|APIURL|SOURCEHUT_PAT|GITEA_TOKEN|non-error exit|exit code|all backups failed",
    "C1": r"BACKUP_LFS|_FILE|envTrue|GetEnvOrFile",
}
pat = re.compile(QUERIES[sys.argv[1]], re.I)
items = [x for page in json.loads((HERE / "all-issues-and-prs.json").read_text()) for x in page]
comments = [x for page in json.loads((HERE / "all-issue-comments.json").read_text()) for x in page]
reviews = [x for page in json.loads((HERE / "all-review-comments.json").read_text()) for x in page]
print(f"query: /{QUERIES[sys.argv[1]]}/i over titles and bodies created at or before {CUT}")
print(f"searched: {sum(x['created_at'] <= CUT for x in items)} issues and pull requests, "
      f"{sum(x['created_at'] <= CUT for x in comments)} issue comments, {sum(x['created_at'] <= CUT for x in reviews)} review comments")
hits = 0
for kind, rows in (("item", items), ("comment", comments), ("review comment", reviews)):
    for x in rows:
        if x["created_at"] > CUT:
            continue
        text = ((x.get("title") or "") + "\n" + (x.get("body") or ""))
        found = list(pat.finditer(text))
        if not found:
            continue
        hits += 1
        print(f"\n## {kind} {x['html_url']} {x['created_at']} by {x['user']['login']} ({x.get('author_association')})")
        if x.get("title"):
            print(f"title: {x['title']}")
        for m in found[:6]:
            s = text[max(0, m.start() - 160): m.end() + 160].replace("\n", " ")
            print(f"  ... {s} ...")
print(f"\nhits: {hits}")
