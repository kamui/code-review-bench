"""How n3-code-files-index.json and n3-code-controlled-fields.json were made.

    python3 n3-code-extract.py <upstream directory> <scratch cache directory> <search name>...

Fetches every file the named code searches returned, then, for each file, records whether a Base UI
Form receives an `errors` prop and lists every controlled Field.Control or Input (one with a value
prop) together with its onValueChange or onChange handler. The fetched files stay in the scratch
cache; only the ones quoted in the dossier are kept under n3-code-files/."""
import base64, json, re, subprocess, sys, time
from pathlib import Path

upstream, cache, names = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3:]
cache.mkdir(parents=True, exist_ok=True)
seen = {}
for name in names:
    for path in sorted(upstream.glob(name + "*.json")):
        if path.name.endswith("-request.txt"):
            continue
        for item in json.loads(path.read_text())["items"]:
            key = (item["repository"]["full_name"], item["path"])
            seen.setdefault(key, {"url": item["url"], "sha": item["sha"], "html_url": item["html_url"], "queries": []})
            if name not in seen[key]["queries"]:
                seen[key]["queries"].append(name)

ALIAS = re.compile(r"import\s*\{([^}]*)\}\s*from\s*['\"]@base-ui(?:-components)?/react(?:/(?:field|input|form))?['\"]")

def names_for(text):
    controls, forms = {"Field.Control", "FieldControl", "Input"}, {"Form"}
    for block in ALIAS.findall(text):
        for part in block.split(","):
            pieces = part.replace("type ", "").strip().split(" as ")
            if len(pieces) == 2:
                original, alias = pieces[0].strip(), pieces[1].strip()
                if original == "Field":
                    controls.add(alias + ".Control")
                elif original == "Input":
                    controls.add(alias)
                elif original == "Form":
                    forms.add(alias)
    return controls, forms

def opening(names):
    return re.compile(r"<(" + "|".join(re.escape(n) for n in sorted(names, key=len, reverse=True)) + r")(?![\w.])")

def tag_body(text, start):
    depth, quote, i = 0, None, start
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
    return text[start:start + 3000]

def attribute(body, name):
    found = re.search(r"(?<![\w-])%s=" % re.escape(name), body)
    if not found:
        return None
    rest = body[found.end():]
    if rest[:1] in "\"'":
        return rest[: rest.find(rest[0], 1) + 1]
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
    return " ".join(rest[1:300].split())

index, rows = [], []
for (repo, path), info in sorted(seen.items()):
    target = cache / (repo.replace("/", "__") + "__" + path.replace("/", "__"))
    if not target.exists():
        result = subprocess.run(["timeout", "60", "gh", "api", info["url"]], capture_output=True, text=True)
        if result.returncode != 0:
            index.append({"repo": repo, "path": path, **info, "fetched": False})
            continue
        target.write_text(base64.b64decode(json.loads(result.stdout).get("content", "")).decode("utf-8", "replace"))
        time.sleep(0.15)
    text = target.read_text()
    controls, forms = names_for(text)
    form_errors = [attribute(tag_body(text, m.end()), "errors") for m in opening(forms).finditer(text)]
    form_errors = [e for e in form_errors if e is not None]
    fields = []
    for m in opening(controls).finditer(text):
        body = tag_body(text, m.end())
        value = attribute(body, "value")
        if value is None:
            continue
        fields.append({"tag": m.group(1), "value": value[:200],
                       "onValueChange": (attribute(body, "onValueChange") or "")[:400] or None,
                       "onChange": (attribute(body, "onChange") or "")[:400] or None})
    index.append({"repo": repo, "path": path, **info, "fetched": True})
    rows.append({"repo": repo, "path": path, "html_url": info["html_url"],
                 "imports_base_ui_form": bool(re.search(r"@base-ui(-components)?/react(/form)?['\"]", text)) and "Form" in text,
                 "form_errors": form_errors, "controlled_fields": fields})
(upstream / "n3-code-files-index.json").write_text(json.dumps(index, indent=1))
(upstream / "n3-code-controlled-fields.json").write_text(json.dumps(rows, indent=1))
both = [r for r in rows if r["form_errors"] and r["controlled_fields"]]
print(len(index), "distinct files;", sum(1 for i in index if not i["fetched"]), "not fetched;",
      sum(1 for r in rows if r["form_errors"]), "with a Form that receives errors;",
      sum(1 for r in rows if r["controlled_fields"]), "with a controlled Field.Control or Input;", len(both), "with both")
