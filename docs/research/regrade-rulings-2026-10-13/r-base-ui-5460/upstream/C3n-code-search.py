#!/usr/bin/env python3
"""Public code search for programs that give a Base UI Field.Control or Input type="number" and
store a number from onValueChange. Saves each request line and raw response, then fetches every
distinct file found into C3n-code-files/ and screens it. Run from this directory:
    python3 C3n-code-search.py
GitHub code search allows about ten requests a minute, so the queries are spaced out and a
rate-limited query is retried after a minute."""
import base64
import json
import re
import subprocess
import time
from pathlib import Path

QUERIES = {
    "field-number-onvaluechange-number": '"@base-ui/react" "type=\\"number\\"" "onValueChange" "Number("',
    "field-subpath-number": '"@base-ui/react/field" "type=\\"number\\"" "Number("',
    "input-subpath-number": '"@base-ui/react/input" "type=\\"number\\"" "onValueChange"',
    "old-package-number": '"@base-ui-components/react" "type=\\"number\\"" "onValueChange" "Number("',
    "field-control-valueasnumber": '"Field.Control" "type=\\"number\\"" "valueAsNumber"',
    "input-parsefloat": '"@base-ui/react" "type=\\"number\\"" "onValueChange" "parseFloat("',
    "field-control-number-value": '"@base-ui/react" "Field.Control" "type=\\"number\\"" "value={"',
}


def gh(args):
    return subprocess.run(["gh", "api"] + args, capture_output=True, text=True)


files = {}
for name, query in QUERIES.items():
    request = f"gh api -X GET search/code -f q='{query}' -f per_page=100"
    Path(f"C3n-code-{name}-request.txt").write_text(request + "\n")
    for attempt in range(3):
        out = gh(["-X", "GET", "search/code", "-f", f"q={query}", "-f", "per_page=100"])
        if out.returncode == 0:
            break
        print(name, "attempt", attempt + 1, "failed:", out.stderr.strip()[:120])
        time.sleep(65)
    if out.returncode != 0:
        Path(f"C3n-code-{name}.json").write_text(json.dumps({"blocked": out.stderr}))
        print(name, "BLOCKED")
        continue
    Path(f"C3n-code-{name}.json").write_text(out.stdout)
    data = json.loads(out.stdout)
    print(name, data["total_count"], len(data["items"]))
    for item in data["items"]:
        key = item["repository"]["full_name"] + ":" + item["path"]
        files.setdefault(key, {"repo": item["repository"]["full_name"], "path": item["path"], "url": item["url"], "html_url": item["html_url"], "queries": []})["queries"].append(name)
    time.sleep(8)

Path("C3n-code-files").mkdir(exist_ok=True)
index = []
for key, info in sorted(files.items()):
    out = gh([info["url"].replace("https://api.github.com/", "")])
    if out.returncode != 0:
        info["fetch_error"] = out.stderr.strip()[:200]
        index.append(info)
        continue
    blob = json.loads(out.stdout)
    text = base64.b64decode(blob.get("content", "")).decode("utf-8", "replace")
    saved = "C3n-code-files/" + re.sub(r"[^A-Za-z0-9._-]+", "__", key)
    Path(saved).write_text(text)
    info["saved"] = saved
    info["imports_base_ui"] = bool(re.search(r"@base-ui(-components)?/react", text))
    info["has_number_type"] = bool(re.search(r"type=\"number\"|type=\{?'number'", text))
    info["has_onvaluechange"] = "onValueChange" in text
    info["stores_number"] = bool(re.search(r"Number\(|parseFloat\(|parseInt\(|valueAsNumber", text))
    info["has_field_or_form"] = bool(re.search(r"Field\.Root|<Field\b|<Form\b|errors=\{", text))
    index.append(info)
    time.sleep(0.4)
Path("C3n-code-files-index.json").write_text(json.dumps(index, indent=1))
print(len(index), "distinct files")
for info in index:
    flags = "".join("Y" if info.get(k) else "-" for k in ("imports_base_ui", "has_number_type", "has_onvaluechange", "stores_number", "has_field_or_form"))
    print(flags, info["repo"], info["path"], ",".join(info["queries"]))
