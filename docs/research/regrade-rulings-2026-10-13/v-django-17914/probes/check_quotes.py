"""Check every quotation of the case files, dossiers and cards against its sources.

Usage: python3 check_quotes.py <round directory of this pull request> <head checkout> <clone with base and head>

A quotation is text between straight double quotes, 20 characters or longer, or a fenced block.
Each is looked for, with runs of white space folded to one space, in: the packet, the head's
source, documentation and tests, the diff from base to head, every file under upstream/ (JSON
strings are decoded first) and every probe result. Whatever is not found is printed for a person
to look at; some of those are this file's own labels and table text, not quotations.
"""
import json
import pathlib
import re
import subprocess
import sys

root, head, repo = map(pathlib.Path, sys.argv[1:4])


def fold(text):
    return re.sub(r"\s+", " ", text).strip()


def strings(obj):
    if isinstance(obj, str):
        yield obj
    elif isinstance(obj, dict):
        for value in obj.values():
            yield from strings(value)
    elif isinstance(obj, list):
        for value in obj:
            yield from strings(value)


corpus = []
corpus.extend(strings(json.loads((root / "packet.json").read_text())))
for path in sorted((root / "upstream").iterdir()):
    text = path.read_text(errors="replace")
    if path.suffix == ".json":
        try:
            corpus.extend(strings(json.loads(text)))
            continue
        except ValueError:
            pass
    if path.suffix == ".html":
        import html

        text = html.unescape(re.sub(r"<[^>]+>", " ", text))
    corpus.append(text)
for path in sorted(root.glob("probes/*/result-*.txt")):
    corpus.append(path.read_text(errors="replace"))
for rel in (
    "django/db/backends/postgresql/base.py",
    "django/db/backends/postgresql/operations.py",
    "django/db/backends/postgresql/psycopg_any.py",
    "django/db/backends/base/base.py",
    "docs/ref/databases.txt",
    "docs/releases/5.1.txt",
    "docs/releases/4.2.txt",
    "docs/topics/settings.txt",
    "docs/topics/async.txt",
    "tests/backends/postgresql/tests.py",
):
    corpus.append((head / rel).read_text())
corpus.append(
    subprocess.run(
        ["git", "-C", str(repo), "diff", "bcccea3ef31c777b73cba41a6255cd866bf87237", "fad334e1a9b54ea1acb8cce02a25934c5acfe99f"],
        capture_output=True, text=True, check=True,
    ).stdout
)
for extra in ("docs/research/cohort-rebuild-2026-10-05/rulings/43-D7a.md", "bench/rubric/scoring.next.md",
              "bench/targets/v-django-17914/register.v1.json",
              "docs/research/cohort-rebuild-2026-10-05/second-pass/terms/two-questions.v8.md",
              "docs/research/impact-boundary-2026-10-04/impact-boundary.v4.md"):
    path = root.parents[3] / extra
    if path.exists():
        corpus.append(path.read_text())
folded = fold("\n".join(corpus))
# A diff or a cat -n style listing prefixes lines; also keep a copy with those prefixes removed.
stripped = fold("\n".join(re.sub(r"^[+\- ]?\s*(# )?", "", line) for text in corpus for line in text.splitlines()))

missing = 0
for path in sorted(list(root.glob("cases/*.md")) + list(root.glob("dossiers/*.md")) + list(root.glob("cards/*.md"))):
    text = path.read_text()
    quotes = [m.group(1) for m in re.finditer(r'"([^"\n]{20,})"', text)]
    for quote in quotes:
        q = fold(quote)
        variants = {q, q.rstrip(".,;:"), re.sub(r"\s*\.\.\.\s*", " ", q)}
        parts = [p for p in re.split(r"\s*\.\.\.\s*", q) if len(p) > 12]
        if any(v in folded or v in stripped for v in variants):
            continue
        if parts and all(p in folded or p in stripped for p in parts):
            continue
        missing += 1
        print(f"{path.relative_to(root)}: not found: {quote[:160]}")
print(f"{missing} quotations not found")
