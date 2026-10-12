#!/usr/bin/env python3
"""Fetch every file the D1 code searches returned and list the lines that use Base UI's focus mark.

    python3 d1_code_focused.py CACHE_DIR SEARCH.json [SEARCH.json ...]

Items from this benchmark's own repository are dropped unread and counted. For each other file it
records the lines that contain `data-focused` or `data-[focused]`, and whether the file is prose
(Markdown or text) or code. Writes d1-code-extract.json. A person reads the code files it lists."""
import base64
import json
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def main():
    cache = Path(sys.argv[1])
    cache.mkdir(parents=True, exist_ok=True)
    seen, files, own = set(), [], 0
    for name in sys.argv[2:]:
        for item in json.loads((HERE / name).read_text(encoding="utf-8"))["items"]:
            key = (item["repository"]["full_name"], item["path"])
            if key in seen:
                continue
            seen.add(key)
            if key[0] == "kamui/code-review-bench":
                own += 1
                continue
            target = cache / (item["sha"] + ".txt")
            if not target.exists():
                result = subprocess.run(["gh", "api", item["url"]], capture_output=True, text=True)
                if result.returncode != 0:
                    files.append({"repository": key[0], "path": key[1], "skipped": "could not be fetched"})
                    continue
                target.write_text(base64.b64decode(json.loads(result.stdout).get("content", "")).decode("utf-8", errors="replace"), encoding="utf-8")
            text = target.read_text(encoding="utf-8")
            lines = [{"line": number + 1, "text": line.strip()[:400]} for number, line in enumerate(text.splitlines())
                     if re.search(r"data-focused|data-\[focused\]", line)]
            files.append({"repository": key[0], "path": key[1], "html_url": item["html_url"],
                          "kind": "prose" if re.search(r"\.(md|mdx|txt)$", key[1]) else "code",
                          "copy_of_base_ui": bool(re.search(r"(^|/)(packages/react/src|docs/src/app)/", key[1])) or key[0] == "mui/base-ui",
                          "imports_base_ui": sorted(set(re.findall(r"@base-ui(?:-components)?/react[\w/-]*", text)))[:6],
                          "focus_mark_lines": lines})
    output = {"note": "Items from this benchmark's own repository were dropped unread and are not counted.",
              "dropped_from_this_benchmarks_own_repository": own, "files": files}
    (HERE / "d1-code-extract.json").write_text(json.dumps(output, indent=1) + "\n", encoding="utf-8")
    code = [f for f in files if f.get("kind") == "code" and f["focus_mark_lines"] and not f["copy_of_base_ui"]]
    print(len(files), "files kept;", own, "dropped;", len(code), "code files outside Base UI with a focus-mark line")
    for record in code:
        print("==", record["repository"], record["path"])
        for line in record["focus_mark_lines"][:2]:
            print("    L%d %s" % (line["line"], line["text"][:230]))


if __name__ == "__main__":
    main()
