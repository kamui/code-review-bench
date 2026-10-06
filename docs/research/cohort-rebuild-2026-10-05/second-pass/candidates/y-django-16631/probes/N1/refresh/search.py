import concurrent.futures
import json
import subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[3]
out = root / "upstream"
queries = {
    "maintainers-fallbacks": ("issues", 'repo:django/django "SECRET_KEY_FALLBACKS" created:<=2023-03-08'),
    "maintainers-get-user": ("issues", 'repo:django/django "get_user" "rotation" created:<=2023-03-08'),
    "maintainers-rolling": ("issues", 'repo:django/django "secret key" "rolling" created:<=2023-03-08'),
    "public-rolling": ("code", '"SECRET_KEY_FALLBACKS" "rolling" extension:py -repo:django/django'),
    "public-staged": ("code", '"SECRET_KEY_FALLBACKS" "staged" extension:py -repo:django/django'),
}


def capture(name, command):
    response = subprocess.run(command, capture_output=True, text=True)
    path = out / f"refresh-N1-{name}.json"
    with path.open("x") as stream:
        stream.write(response.stdout if response.stdout else json.dumps({"stderr": response.stderr}))
    return {"name": name, "command": command, "exit": response.returncode, "stderr": response.stderr, "saved": str(path.relative_to(root))}


tasks = [
    (name, ["gh", "api", "-X", "GET", f"search/{kind}", "-f", f"q={query}", "-f", "per_page=100"])
    for name, (kind, query) in queries.items()
]
tasks += [
    ("pr-13850-comments", ["gh", "api", "repos/django/django/issues/13850/comments?per_page=100"]),
    ("pr-16631", ["gh", "api", "repos/django/django/pulls/16631"]),
    ("pr-16631-comments", ["gh", "api", "repos/django/django/pulls/16631/comments?per_page=100"]),
    ("itsdangerous-docs", ["gh", "api", "repos/pallets/itsdangerous/contents/docs?ref=2.1.2"]),
]
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    records = list(pool.map(lambda task: capture(*task), tasks))
with (out / "refresh-N1-capture.json").open("x") as stream:
    json.dump(records, stream, indent=2)
for record in records:
    data = json.loads((root / record["saved"]).read_text())
    print(record["name"], record["exit"], data.get("total_count") if isinstance(data, dict) else len(data))
    if isinstance(data, dict):
        for item in data.get("items", []):
            print(item.get("repository", {}).get("full_name", ""), item.get("path", item.get("number")), item.get("title", ""))
