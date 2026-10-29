#!/usr/bin/env python3
"""Print, for each pull request of this round, the batches a ruling would make stale and their measured list-price grading cost.

    python3 -I docs/research/rule-v4-post-filing-candidates-2026-10-29/stale.py .

The Sol cost is each batch's grade in grades.json; the Opus cost is the latest Opus receipt for the same batch."""
import json,glob,collections,sys
root=sys.argv[1]
def L(p): return json.load(open(f"{root}/{p}"))
grades=L("bench/grading/current/grades.json")
inv=L("docs/research/rule-v4-post-filing-candidates-2026-10-29/inventory.v1.json")
batches=[(b["run"],b["target"],b["assessor"]["receipt"]["path"]) for b in grades["batches"]]
def cost(path):
    u=L(path)["provenance"].get("usage") or {}
    return u.get("priced_total_usd")
opus={}
for p in glob.glob(f"{root}/bench/grading/current/assessments/*/*/assessment-*/receipt.json"):
    d=json.load(open(p));pr=d["provenance"]
    if pr.get("model")=="claude-opus-5-5":
        parts=p.split("/"); key=("runs/"+parts[-4],parts[-3]); n=int(parts[-2].split("-")[1])
        if key not in opus or n>opus[key][0]: opus[key]=(n,(pr.get("usage") or {}).get("priced_total_usd"))
out={}
for t in ("v-django-17914","n-ripgrep-2957","p-hono-5067"):
    tb=[(r,tt,c) for r,tt,c in batches if tt==t]
    sol=[cost(c) for _,_,c in tb]
    op=[opus.get((r,t),(None,None))[1] for r,_,_ in tb]
    out[t]={"batches":len(tb),"sol":sum(x for x in sol if x),"sol_missing":sum(1 for x in sol if x is None),"opus":sum(x for x in op if x),"opus_missing":sum(1 for x in op if x is None)}
    print(t,out[t])
for q in inv["questions"]:
    if q["decision_type"]!="candidate": continue
    runs=set()
    for row in q["comments"]+q["anchors"]:
        run=row["review"].split("/attempts/")[0].replace("bench/","")
        runs.add(run)
    tb=[(r,tt,c) for r,tt,c in batches if tt==q["target"] and r in runs]
    sol=[cost(c) for _,_,c in tb]; op=[opus.get((r,q["target"]),(None,None))[1] for r,_,_ in tb]
    print(q["target"],q["group"],"linked batches",len(tb),sorted(r for r,_,_ in tb),"sol %.2f"%sum(x for x in sol if x),"opus %.2f"%sum(x for x in op if x), "opus missing",sum(1 for x in op if x is None))
