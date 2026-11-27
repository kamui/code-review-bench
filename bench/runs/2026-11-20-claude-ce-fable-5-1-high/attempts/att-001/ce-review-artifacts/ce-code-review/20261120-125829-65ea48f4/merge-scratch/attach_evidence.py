import json, re, sys
run = sys.argv[1]
def key(reviewer, f):
    return (reviewer, f["file"].replace("\\", "/").strip(), str(f["line"]), re.sub(r"\s+", " ", f["title"]).strip().lower())
detail = {}
for name in ["correctness", "security", "adversarial", "testing", "performance"]:
    art = json.load(open(f"{run}/{name}.json"))
    for f in art["findings"]:
        detail[key(art["reviewer"], f)] = {"why_it_matters": f.get("why_it_matters"), "evidence": f.get("evidence", [])}
returns = json.load(open(f"{run}/raw-returns.json"))
missing = []
for r in returns:
    for f in r["findings"]:
        k = key(r["reviewer"], f)
        if k in detail:
            f["evidence"] = list(detail[k]["evidence"])
        else:
            missing.append(k)
json.dump(returns, open(f"{run}/merge-scratch/helper-input-1.json", "w"), indent=1)
json.dump({"|".join(k): v for k, v in detail.items()}, open(f"{run}/merge-scratch/source-detail-map.json", "w"), indent=1)
print("detail entries:", len(detail), "unmatched compact findings:", missing)
