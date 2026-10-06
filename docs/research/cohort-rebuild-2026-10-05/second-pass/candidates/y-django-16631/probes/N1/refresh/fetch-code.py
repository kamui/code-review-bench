import base64
import concurrent.futures
import json
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[3]
out = root / "upstream"
items = []
for name in ("public-rolling-retry", "public-staged-retry"):
    response = json.loads((out / f"refresh-N1-{name}.json").read_text())
    items.extend(response["items"])


def api(name, endpoint, fields=()):
    command = ["gh", "api", "-X", "GET", endpoint]
    for key, value in fields:
        command += ["-f", f"{key}={value}"]
    result = subprocess.run(command, capture_output=True, text=True)
    path = out / f"refresh-N1-code-{name}.json"
    with path.open("x") as stream:
        stream.write(result.stdout)
    if result.returncode:
        raise RuntimeError(result.stderr)
    return json.loads(result.stdout), {"command": command, "saved": str(path.relative_to(root))}


def fetch(index_item):
    index, item = index_item
    repo = item["repository"]["full_name"]
    commit = item["html_url"].split("/blob/")[1].split("/")[0]
    blob, capture = api(f"{index}-blob", f"repos/{repo}/git/blobs/{item['sha']}")
    content = base64.b64decode(blob["content"]).decode()
    latest, recent_capture = api(f"{index}-date", f"repos/{repo}/commits", [("path", item["path"]), ("sha", commit), ("per_page", "1")])
    before, before_capture = api(f"{index}-before-cutoff", f"repos/{repo}/commits", [("path", item["path"]), ("until", "2023-03-08T09:48:04Z"), ("per_page", "1")])
    record = {"repo": repo, "path": item["path"], "url": item["html_url"], "latest_path_change": latest[0]["commit"]["committer"]["date"] if latest else None, "pre_cutoff_path_change": before[0]["commit"]["committer"]["date"] if before else None, "captures": [capture, recent_capture, before_capture]}
    lines = content.splitlines()
    selected = set()
    for number, line in enumerate(lines):
        if "SECRET_KEY_FALLBACKS" in line or "rolling" in line.lower() or "staged" in line.lower():
            selected.update(range(max(0, number - 5), min(len(lines), number + 6)))
    record["excerpts"] = [{"line": number + 1, "text": lines[number]} for number in sorted(selected)]
    return record


with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
    records = list(pool.map(fetch, enumerate(items, 1)))
with (out / "refresh-N1-code-read.json").open("x") as stream:
    json.dump(records, stream, indent=2)
for record in records:
    print(record["repo"], record["path"], record["latest_path_change"], "before-cutoff", record["pre_cutoff_path_change"])
    for line in record["excerpts"]:
        print(f"{line['line']}: {line['text']}")
