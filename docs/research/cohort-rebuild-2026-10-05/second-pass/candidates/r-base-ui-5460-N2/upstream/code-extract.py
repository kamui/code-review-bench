"""How code-files-extract.json and code-controlled-values.json were made.

    python3 code-extract.py <upstream directory> <scratch cache directory>

Fetches every file listed in code-files-index.json with `gh api`, then pulls out each value={...}
on a Field.Control or Input, following import aliases. The fetched files stay in the scratch cache;
only the four quoted in the dossier are kept under code-files/."""
import base64, json, re, subprocess, sys, time
from pathlib import Path

upstream = Path(sys.argv[1])
cache = Path(sys.argv[2])
cache.mkdir(parents=True, exist_ok=True)
index = json.loads((upstream / "code-files-index.json").read_text())
ALIAS = re.compile(r"import\s*\{([^}]*)\}\s*from\s*['\"]@base-ui(?:-components)?/react(?:/(?:field|input))?['\"]")

def opening_for(text):
    names = {"Field.Control", "FieldControl", "Input"}
    for block in ALIAS.findall(text):
        for part in block.split(","):
            pieces = part.replace("type ", "").strip().split(" as ")
            if len(pieces) == 2 and pieces[0].strip() in ("Field", "Input"):
                alias = pieces[1].strip()
                names.add(alias + ".Control" if pieces[0].strip() == "Field" else alias)
    return re.compile(r"<(" + "|".join(re.escape(n) for n in sorted(names, key=len, reverse=True)) + r")(?![\w.])")

def tag_body(text, start):
    depth = 0
    quote = None
    i = start
    while i < len(text):
        ch = text[i]
        if quote:
            if ch == quote:
                quote = None
        elif depth == 0 and ch in "\"'":
            quote = ch
        elif ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
        elif ch == ">" and depth == 0:
            return text[start:i]
        i += 1
    return text[start:start + 2000]

def value_expression(body):
    found = re.search(r"\bvalue=", body)
    if not found:
        return None
    rest = body[found.end():]
    if rest[:1] in "\"'":
        end = rest.find(rest[0], 1)
        return rest[: end + 1]
    if rest[:1] != "{":
        return rest[:40]
    depth = 0
    for i, ch in enumerate(rest):
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                return " ".join(rest[1:i].split())
    return " ".join(rest[1:200].split())

rows = []
for entry in index:
    target = cache / (entry["repo"].replace("/", "__") + "__" + entry["path"].replace("/", "__"))
    if not target.exists():
        result = subprocess.run(["timeout", "60", "gh", "api", entry["url"]], capture_output=True, text=True)
        if result.returncode != 0:
            rows.append({"repo": entry["repo"], "path": entry["path"], "fetched": False})
            continue
        target.write_text(base64.b64decode(json.loads(result.stdout).get("content", "")).decode("utf-8", "replace"))
        time.sleep(0.15)
    text = target.read_text()
    values = []
    for match in opening_for(text).finditer(text):
        expression = value_expression(tag_body(text, match.end()))
        if expression is not None:
            values.append({"tag": match.group(1), "value": expression[:200]})
    rows.append({"repo": entry["repo"], "path": entry["path"], "html_url": entry["html_url"], "sha": entry["sha"],
                 "queries": entry["queries"], "fetched": True,
                 "imports_base_ui": bool(re.search(r"@base-ui(-components)?/react", text)),
                 "controlled_values": values})
(upstream / "code-files-extract.json").write_text(json.dumps(rows, indent=1))
flat = [{"repo": r["repo"], "path": r["path"], **v} for r in rows if r["fetched"] for v in r["controlled_values"]]
(upstream / "code-controlled-values.json").write_text(json.dumps(flat, indent=1))
print(len(rows), "files;", sum(1 for r in rows if not r["fetched"]), "not fetched;",
      sum(1 for r in rows if r["fetched"] and r["controlled_values"]), "with a controlled Field.Control or Input;", len(flat), "value expressions")
