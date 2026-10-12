#!/usr/bin/env python3
"""Fetch every file a saved GitHub code search returned and record what each one does.

    python3 code_extract.py CACHE_DIR OUTPUT.json MODE SEARCH.json [SEARCH.json ...]

MODE chooses what is looked for in each file:
  prevent   every `preventDefault` call, with the nearest handler name above it (onChange, onSubmit, ...)
  disabled  every JSX element named Field.Control, Input or *Input* that has both `disabled` and `value=`
  range     every JSX element with type="range" and the import lines that name @base-ui
  controlled  in a file whose <Form> receives `errors`: every Field.Control or Input with `value=` and a
              change handler
The fetched files are kept in CACHE_DIR, outside the repository. OUTPUT.json is the index: one record per
distinct file with its repository, path, blob sha, whether it is this benchmark's own repository, and the
excerpts found. A person then reads the excerpts; the script sorts nothing as a practice by itself.
"""
import base64
import json
import re
import subprocess
import sys
from pathlib import Path

HANDLER = re.compile(r"\b(on[A-Z][A-Za-z]*|handle[A-Z][A-Za-z]*|function\s+[A-Za-z]+|const\s+[A-Za-z]+\s*=\s*(?:async\s*)?\()")
ELEMENT = re.compile(r"<([A-Z][A-Za-z.]*|input)\b([^<>]*(?:\{[^{}]*(?:\{[^{}]*\}[^{}]*)*\}[^<>]*)*)/?>", re.S)


def fetch(item, cache):
    target = cache / (item["sha"] + ".txt")
    if target.exists():
        return target.read_text(encoding="utf-8", errors="replace")
    result = subprocess.run(["gh", "api", item["url"]], capture_output=True, text=True)
    if result.returncode != 0:
        return None
    text = base64.b64decode(json.loads(result.stdout).get("content", "")).decode("utf-8", errors="replace")
    target.write_text(text, encoding="utf-8")
    return text


def prevent(text):
    lines = text.splitlines()
    found = []
    for number, line in enumerate(lines):
        if "preventDefault" not in line:
            continue
        handler = None
        for back in range(number, max(-1, number - 25), -1):
            matches = HANDLER.findall(lines[back] if back != number else line.split("preventDefault")[0])
            if matches:
                handler = matches[-1].strip()
                break
        found.append({"line": number + 1, "nearest_handler": handler, "text": "\n".join(lines[max(0, number - 4): number + 2])[:700]})
    return found


def elements(text, wanted):
    found = []
    for match in ELEMENT.finditer(text):
        name, attributes = match.group(1), match.group(2)
        if wanted(name, attributes):
            found.append({"line": text.count("\n", 0, match.start()) + 1, "element": name, "text": match.group(0)[:900]})
    return found


def disabled(text):
    return elements(text, lambda name, attributes: ("Control" in name or "Input" in name or name == "input")
                    and re.search(r"\bdisabled\b", attributes) and re.search(r"\bvalue=", attributes))


def range_inputs(text):
    return elements(text, lambda name, attributes: re.search(r"type=[\"']range[\"']", attributes))


def controlled(text):
    """Controlled Base UI text controls in a file whose Form receives `errors`, with each one's handlers."""
    if not re.search(r"<Form\b[^>]*\berrors=\{", text, re.S):
        return []
    return elements(text, lambda name, attributes: (name.endswith("Field.Control") or name in ("Input", "FieldControl"))
                    and re.search(r"\bvalue=\{", attributes) and re.search(r"\bon(?:Value)?Change=", attributes))


MODES = {"prevent": prevent, "disabled": disabled, "range": range_inputs, "controlled": controlled}


def main():
    cache, output, mode = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
    cache.mkdir(parents=True, exist_ok=True)
    seen, records = set(), []
    for name in sys.argv[4:]:
        for item in json.loads(Path(name).read_text(encoding="utf-8"))["items"]:
            key = (item["repository"]["full_name"], item["path"])
            if key in seen:
                continue
            seen.add(key)
            record = {"repository": key[0], "path": key[1], "sha": item["sha"], "html_url": item["html_url"],
                      "from": Path(name).name}
            if key[0] == "kamui/code-review-bench":
                record["skipped"] = "this benchmark's own repository"
                records.append(record)
                continue
            text = fetch(item, cache)
            if text is None:
                record["skipped"] = "could not be fetched"
            else:
                record["imports_base_ui"] = sorted(set(re.findall(r"@base-ui(?:-components)?/react[\w/-]*", text)))
                record["found"] = MODES[mode](text)
            records.append(record)
    output.write_text(json.dumps(records, indent=1) + "\n", encoding="utf-8")
    fetched = [r for r in records if "found" in r]
    print(f"{output.name}: {len(records)} distinct files, {len(fetched)} fetched, {sum(1 for r in fetched if r['found'])} with a match")


if __name__ == "__main__":
    main()
