"""Run one GitHub search, drop unread any item from this benchmark's own repository, save request and response."""
import json, subprocess, sys, time
kind, upstream, name, query = sys.argv[1:5]
pages = int(sys.argv[5]) if len(sys.argv) > 5 else 1
for page in range(1, pages + 1):
    suffix = "" if page == 1 else f"-page{page}"
    command = ["gh", "api", "-X", "GET", f"search/{kind}", "-f", f"q={query}", "-f", "per_page=100", "-f", f"page={page}"]
    result = subprocess.run(["timeout", "90"] + command, capture_output=True, text=True)
    open(f"{upstream}/{name}{suffix}-request.txt", "w").write("gh api -X GET search/%s -f q='%s' -f per_page=100 -f page=%d\n" % (kind, query, page))
    if result.returncode != 0:
        print(name, page, "FAILED", result.stderr[:300]); break
    data = json.loads(result.stdout)
    before = len(data.get("items", []))
    def repo(item):
        return item["repository"]["full_name"] if "repository" in item else item.get("repository_url", "").split("repos/")[-1]
    data["items"] = [i for i in data.get("items", []) if repo(i) != "kamui/code-review-bench"]
    dropped = before - len(data["items"])
    if dropped:
        data["_note"] = f"{dropped} items in the repository kamui/code-review-bench (this benchmark's own saved records) were removed from this saved response unread; total_count is as GitHub returned it."
    json.dump(data, open(f"{upstream}/{name}{suffix}.json", "w"), indent=1)
    print(name, "page", page, "total", data.get("total_count"), "kept", len(data["items"]), "dropped", dropped)
    time.sleep(7 if kind == "code" else 2)
    if before < 100:
        break
