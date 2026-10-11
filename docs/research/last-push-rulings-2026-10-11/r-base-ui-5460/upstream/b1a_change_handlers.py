#!/usr/bin/env python3
"""Second pass over the files of b1a-code-extract.json: does any change-type handler prevent the event?

    python3 b1a_change_handlers.py CACHE_DIR

For every fetched file it looks at each onChange, onInput, onValueChange, onBeforeInput and onCheckedChange
prop. An inline handler is searched for `preventDefault` and `defaultPrevented`; a named handler is looked up
in the same file and its body searched. It also lists every addEventListener('input' | 'change' |
'beforeinput') that prevents the event. Writes b1a-code-change-handlers.json. A person reads what it lists."""
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
WORDS = re.compile(r"preventDefault|defaultPrevented")


def body(text, start):
    depth, index = 0, start
    while index < len(text):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return text[start:index + 1]
        index += 1
    return text[start:start + 2000]


def main():
    cache = Path(sys.argv[1])
    files = json.loads((HERE / "b1a-code-extract.json").read_text(encoding="utf-8"))["files"]
    found, calls = [], 0
    for record in files:
        if "found" not in record:
            continue
        calls += len(record["found"])
        text = (cache / (record["sha"] + ".txt")).read_text(encoding="utf-8")
        where = {"repository": record["repository"], "path": record["path"], "html_url": record["html_url"]}
        for match in re.finditer(r"\bon(Change|Input|ValueChange|BeforeInput|CheckedChange)\s*=\s*\{", text):
            inline = body(text, match.end() - 1)
            if WORDS.search(inline):
                found.append({**where, "prop": "on" + match.group(1), "kind": "inline", "text": " ".join(inline.split())[:500]})
            named = re.match(r"\{\s*([A-Za-z_][\w.]*)\s*\}", inline)
            if not named:
                continue
            name = named.group(1).split(".")[-1]
            for definition in re.finditer(r"(?:function\s+%s\s*\(|(?:const|let)\s+%s\s*=)" % (re.escape(name), re.escape(name)), text):
                opening = text.find("{", definition.end())
                if opening != -1 and WORDS.search(body(text, opening)[:3000]):
                    found.append({**where, "prop": "on" + match.group(1), "kind": "named " + name,
                                  "text": " ".join(body(text, opening).split())[:500]})
        for match in re.finditer(r"addEventListener\(\s*['\"](input|change|beforeinput)['\"]", text):
            segment = text[match.start():match.start() + 400]
            if "preventDefault" in segment:
                found.append({**where, "prop": "listener " + match.group(1), "kind": "listener", "text": " ".join(segment.split())[:300]})
    unique = {(item["repository"], item["path"], item["prop"], item["kind"]): item for item in found}
    output = {"files_checked": sum(1 for record in files if "found" in record), "preventDefault_calls_in_those_files": calls,
              "change_type_handlers_or_listeners_that_prevent_or_test_prevention": list(unique.values())}
    (HERE / "b1a-code-change-handlers.json").write_text(json.dumps(output, indent=1) + "\n", encoding="utf-8")
    print(output["files_checked"], calls, len(unique))
    for item in unique.values():
        print(" ", item["repository"], item["path"], item["prop"], item["kind"])


if __name__ == "__main__":
    main()
