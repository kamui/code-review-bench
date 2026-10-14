#!/usr/bin/env python3
"""Usage: check-quotes.py <dossier-dir> <checkout-of-head> <file.md>...

Finds every passage in double quotes (12 characters or more) in the given Markdown files and
looks for it, with white space collapsed, in the sources this dossier quotes from: the saved
upstream responses (JSON strings, and base64 file contents decoded), the saved search outputs,
the task packet, the round's packet (the review comments and known problems), the checkout of
the head (documentation, tests, source) and the saved probe results. Prints the passages it
does not find, for reading by hand."""
import base64, json, pathlib, re, sys

root = pathlib.Path(sys.argv[1]); head = pathlib.Path(sys.argv[2]); files = sys.argv[3:]
repo = root.parents[3]
corpus = []
def walk(o):
    if isinstance(o, str): corpus.append(o)
    elif isinstance(o, dict):
        if o.get("encoding") == "base64" and isinstance(o.get("content"), str):
            try: corpus.append(base64.b64decode(o["content"]).decode("utf-8", "replace"))
            except Exception: pass
        for v in o.values(): walk(v)
    elif isinstance(o, list):
        for v in o: walk(v)
for p in sorted((root / "upstream").rglob("*")):
    if p.is_file():
        text = p.read_text(encoding="utf-8", errors="replace")
        if p.suffix == ".json":
            try: walk(json.loads(text))
            except Exception: corpus.append(text)
        else: corpus.append(text)
walk(json.loads((root / "packet.json").read_text()))
corpus.append((repo / "bench/targets/j-trpc-5017/packet.md").read_text())
for sub in ("www/docs", "packages/tests", "packages/server/src"):
    for p in (head / sub).rglob("*"):
        if p.is_file() and p.suffix in (".md", ".mdx", ".ts", ".tsx"):
            corpus.append(p.read_text(encoding="utf-8", errors="replace"))
for p in (root / "probes").rglob("*.txt"): corpus.append(p.read_text(encoding="utf-8", errors="replace"))
for p in (root / "probes").rglob("*.ts"): corpus.append(p.read_text(encoding="utf-8", errors="replace"))
norm = lambda s: re.sub(r"\s+", " ", s.replace("\r", "")).strip()
blob = norm("\n".join(corpus))
blob_loose = blob.replace(" | ", " ")
for f in files:
    text = pathlib.Path(f).read_text()
    text = re.sub(r"```.*?```", "", text, flags=re.S)
    missing = []
    quotes = re.findall(r'"([^"\n]{12,})"', text)
    for q in quotes:
        cand = norm(q).replace("\\|", "|")
        variants = {cand, cand.rstrip(".,;:"), cand.replace(" | ", " ")}
        if not any(v in blob or v in blob_loose for v in variants):
            missing.append(q)
    print(f"{f}: {len(quotes)} quoted passages, {len(missing)} not found")
    for q in missing: print("   NOT FOUND:", q[:200])
