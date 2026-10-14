#!/usr/bin/env python3
"""Screen the files fetched by C3n-code-search.py for the exact operation: a JSX element with
type="number" and an onValueChange that stores a parsed number, in a file that imports Base UI.
Writes C3n-code-screen.json. Run from this directory."""
import json
import re
from pathlib import Path

index = json.load(open("C3n-code-files-index.json"))
out = []
for info in index:
    if info["repo"] == "kamui/code-review-bench":
        info["dropped"] = "this benchmark's own repository, not read"
        out.append(info)
        continue
    if "saved" not in info:
        out.append(info)
        continue
    text = Path(info["saved"]).read_text()
    elements = []
    for m in re.finditer(r"<(Field\.Control|FieldControl|Input|InputPrimitive|BaseInput|Field\.Input)\b", text):
        start = m.start()
        # take the opening tag: up to the first '>' that is not part of '=>' and at brace depth 0
        depth = 0
        i = m.end()
        while i < len(text):
            ch = text[i]
            if ch == "{":
                depth += 1
            elif ch == "}":
                depth -= 1
            elif ch == ">" and depth == 0 and text[i - 1] != "=":
                break
            i += 1
        tag = text[start : i + 1]
        if re.search(r"type=(\"number\"|\{?'number'\}?|\{\"number\"\})", tag) and "onValueChange" in tag:
            numeric = bool(re.search(r"onValueChange=\{[^>]*?(Number\(|parseFloat\(|parseInt\(|\+\s*v\b|valueAsNumber)", tag, re.S))
            line = text.count("\n", 0, start) + 1
            elements.append({"line": line, "tag": m.group(1), "stores_parsed_number_in_tag": numeric, "text": tag[:700]})
    info["number_elements_with_onvaluechange"] = elements
    info["file_imports_base_ui"] = bool(re.search(r"from ['\"]@base-ui(-components)?/react", text))
    info["file_has_field_root"] = "Field.Root" in text
    info["file_has_form_errors"] = bool(re.search(r"<Form\b[^>]*errors=", text, re.S))
    info["file_has_validate"] = bool(re.search(r"\bvalidate=\{", text))
    out.append(info)
json.dump(out, open("C3n-code-screen.json", "w"), indent=1)
cands = [i for i in out if i.get("number_elements_with_onvaluechange")]
print(len(out), "files;", len(cands), "have a number-typed element with onValueChange")
for i in cands:
    n = sum(1 for e in i["number_elements_with_onvaluechange"] if e["stores_parsed_number_in_tag"])
    print(("B" if i["file_imports_base_ui"] else "-") + ("R" if i["file_has_field_root"] else "-") + ("E" if i["file_has_form_errors"] else "-") + ("V" if i["file_has_validate"] else "-"), n, "of", len(i["number_elements_with_onvaluechange"]), i["repo"], i["path"])
