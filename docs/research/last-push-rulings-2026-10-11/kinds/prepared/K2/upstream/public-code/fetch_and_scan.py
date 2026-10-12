"""Usage: python3 -I fetch_and_scan.py <responses dir> <files dir> <out.json>
Reads saved GitHub code-search responses, fetches every distinct file once, and scans each for a tRPC
procedure chain in which `.input(` holds an inline z.array( or z.tuple( as the whole input and the next
call in the same chain is `.use(`. Also counts chains with such an input and no `.use(` after it."""
import glob
import json
import os
import re
import subprocess
import sys

responses, files, out = sys.argv[1:4]
items = {}
for path in sorted(glob.glob(os.path.join(responses, "q*-p*.json"))):
    query = os.path.basename(path).split("-")[0]
    for item in json.load(open(path))["items"]:
        key = (item["repository"]["full_name"], item["path"])
        entry = items.setdefault(key, {"repo": key[0], "path": key[1], "url": item["url"], "html_url": item["html_url"], "queries": []})
        if query not in entry["queries"]:
            entry["queries"].append(query)

START = re.compile(r"\.input\(\s*z\s*\.\s*(array|tuple)\s*\(")
SKIP = re.compile(r"(\s|//[^\n]*\n|/\*.*?\*/)*", re.S)


def close_paren(text, open_index):
    depth, i, quote = 0, open_index, None
    while i < len(text):
        c = text[i]
        if quote:
            if c == "\\":
                i += 1
            elif c == quote:
                quote = None
        elif c in "'\"`":
            quote = c
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


def scan(text):
    found = []
    for match in START.finditer(text):
        open_index = match.start() + len(".input")
        end = close_paren(text, open_index)
        if end < 0:
            continue
        argument = text[open_index + 1:end]
        # the whole input is the array or tuple when nothing but optional/nullish modifiers follow it
        after = SKIP.match(text, end + 1).end()
        chain = []
        cursor = after
        while text.startswith(".", cursor):
            name = re.match(r"\.\s*([A-Za-z_]\w*)\s*\(", text[cursor:])
            if not name:
                break
            call_open = cursor + name.end() - 1
            call_end = close_paren(text, call_open)
            if call_end < 0:
                break
            chain.append(name.group(1))
            cursor = SKIP.match(text, call_end + 1).end()
        line = text.count("\n", 0, match.start()) + 1
        found.append({"line": line, "kind": match.group(1), "argument": " ".join(argument.split())[:160], "calls_after_input": chain[:6]})
    return found


results = []
for index, entry in enumerate(sorted(items.values(), key=lambda e: (e["repo"], e["path"]))):
    local = os.path.join(files, f"{index:04d}.txt")
    if not os.path.exists(local):
        done = subprocess.run(["gh", "api", entry["url"], "-H", "Accept: application/vnd.github.raw"], capture_output=True)
        open(local, "wb").write(done.stdout if done.returncode == 0 else b"")
    text = open(local, encoding="utf-8", errors="replace").read()
    inputs = scan(text)
    entry.update({
        "fetched_bytes": len(text),
        "inline_array_or_tuple_inputs": inputs,
        "followed_by_use": [i for i in inputs if "use" in i["calls_after_input"]],
    })
    results.append(entry)
json.dump(results, open(out, "w"), indent=1)
with_input = [r for r in results if r["inline_array_or_tuple_inputs"]]
with_use = [r for r in results if r["followed_by_use"]]
print("distinct files", len(results), "| fetched empty", sum(1 for r in results if not r["fetched_bytes"]))
print("files with an inline array or tuple as the whole input", len(with_input))
print("files where .use( follows it in the same chain", len(with_use))
for r in with_use:
    for hit in r["followed_by_use"]:
        print(" ", r["repo"], r["path"], "line", hit["line"], hit["kind"], hit["calls_after_input"], "|", hit["argument"][:80])
