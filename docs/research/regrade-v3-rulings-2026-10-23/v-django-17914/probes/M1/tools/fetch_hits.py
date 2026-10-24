import json, subprocess, sys, base64, os, hashlib
up, out = sys.argv[1], sys.argv[2]
seen = {}
for q in sys.argv[3:]:
    for it in json.load(open(f"{up}/M1-code-{q}.json"))["items"]:
        key = (it["repository"]["full_name"], it["path"])
        if key in seen:
            continue
        r = subprocess.run(["gh", "api", it["url"]], capture_output=True, text=True)
        try:
            d = json.loads(r.stdout)
            text = base64.b64decode(d["content"]).decode("utf-8", "replace")
        except Exception as e:
            text = None
        fn = os.path.join(out, hashlib.sha1("/".join(key).encode()).hexdigest()[:12] + ".py")
        if text is not None:
            open(fn, "w").write(text)
        seen[key] = {"query": q, "repo": key[0], "path": key[1], "file": fn if text else None, "html_url": it["html_url"]}
json.dump(list(seen.values()), open(os.path.join(out, "index.json"), "w"), indent=1)
print(len(seen))
