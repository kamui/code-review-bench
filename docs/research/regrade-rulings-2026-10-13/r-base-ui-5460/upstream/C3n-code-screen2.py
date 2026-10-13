#!/usr/bin/env python3
"""Second screen of the files fetched by C3n-code-search.py: every JSX element with type="number"
whose tag is Base UI's own Field.Control, or an Input the same file imports from Base UI, with the
handler it uses (onValueChange or the native onChange). Writes C3n-code-screen2.json and prints
one line per element for reading by hand. Run from this directory."""
import json
import re
from pathlib import Path

index = json.load(open("C3n-code-files-index.json"))
rows = []
for info in index:
    if info["repo"] == "kamui/code-review-bench" or "saved" not in info:
        continue
    text = Path(info["saved"]).read_text()
    base_input_names = set()
    for m in re.finditer(r"import\s+(?:type\s+)?\{([^}]*)\}\s+from\s+['\"]@base-ui(?:-components)?/react(?:/input)?['\"]", text):
        for part in m.group(1).split(","):
            part = part.strip()
            if re.match(r"Input(\s+as\s+\w+)?$", part):
                base_input_names.add(part.split(" as ")[-1].strip())
    names = ["Field\\.Control"] + sorted(base_input_names)
    for m in re.finditer(r"<(" + "|".join(names) + r")\b", text):
        depth, i = 0, m.end()
        while i < len(text):
            ch = text[i]
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
            elif ch == ">" and depth == 0 and text[i - 1] != "=":
                break
            i += 1
        tag = text[m.start() : i + 1]
        if not re.search(r"type=(\"number\"|\{?'number'\}?|\{\"number\"\})", tag):
            continue
        handler = "onValueChange" if "onValueChange" in tag else ("onChange" if "onChange" in tag else "none")
        value = re.search(r"\bvalue=\{([^}]*)\}", tag)
        rows.append({
            "repo": info["repo"], "path": info["path"], "saved": info["saved"], "line": text.count("\n", 0, m.start()) + 1,
            "tag": m.group(1), "handler": handler, "value_expr": value.group(1) if value else None,
            "file_has_field_root": "Field.Root" in text, "file_has_form_errors": bool(re.search(r"<Form\b[^>]*errors=", text, re.S)),
            "file_has_validate": bool(re.search(r"\bvalidate=\{", text)), "text": tag[:900],
        })
json.dump(rows, open("C3n-code-screen2.json", "w"), indent=1)
print(len(rows), "number-typed Base UI elements in", len({(r['repo'], r['path']) for r in rows}), "files")
for r in rows:
    print(("R" if r["file_has_field_root"] else "-") + ("E" if r["file_has_form_errors"] else "-") + ("V" if r["file_has_validate"] else "-"), r["handler"], "| value:", r["value_expr"], "|", r["repo"], r["path"] + ":" + str(r["line"]))
