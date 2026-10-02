import json
from pathlib import Path
from collections import Counter
OUT=Path(__file__).parent
ROOT=Path('/home/jack/.t3/worktrees/code-review-bench/t3code-7fc50325')
a=json.loads((OUT/'audit.json').read_text())
g=json.loads(Path(a['grounding_path']).read_text())
original={r['token']:r for r in g['items']}
assert len(a['items'])==68
assert len(set(r['token'] for r in a['items']))==68
assert set(original)=={r['token'] for r in a['items']}
ids={r['id']:r for r in a['canonical_issues']}
assert len(ids)==len(a['canonical_issues'])
required={'token','target','canonical_issue_ids','proposed_outcomes','route','human_question','evidence_paths','reasoning','counterevidence','limits'}
for r in a['items']:
    assert required<=r.keys()
    assert r['original_item']==original[r['token']]['item']
    assert r['target']==original[r['token']]['target']
    assert len(r['canonical_issue_ids'])==len(r['proposed_outcomes'])
    for id,outcome in zip(r['canonical_issue_ids'],r['proposed_outcomes']):
        assert id in ids
        assert outcome==ids[id]['proposed_outcome']
        assert r['token'] in ids[id]['original_tokens']
    assert r['route'] in ['clear','needs-human','needs-evidence','item-grading']
    assert bool(r['human_question'])==(r['route']=='needs-human')
    assert r['administrative_approval_needed'] is True
for c in a['canonical_issues']:
    assert set(c['original_tokens'])=={r['token'] for r in a['items'] if c['id'] in r['canonical_issue_ids']}
    assert c['proposed_outcome'] in ['eligible','advisory','inconsequential','scope-excluded','refuted','unsupported','unresolved']
    assert c['administrative_approval_needed'] is True
assert len(a['research_only'])==4
assert all(r['original_tokens']==[] for r in a['research_only'])
queued={id for q in a['human_queue'] for id in q['canonical_issue_ids']}
assert queued=={c['id'] for c in a['canonical_issues'] if c['route']=='needs-human'}
paths=set()
for r in a['items']+a['canonical_issues']+a['research_only']:
    for p in r['evidence_paths']:
        path=Path(p)
        if not path.is_absolute():path=ROOT/path
        assert path.is_file(),str(path)
        paths.add(str(path))
for f in ['report.md','rationale.md']:
    text=(OUT/f).read_text()
    assert len(text)>100
    assert chr(8212) not in text and chr(8211) not in text
summary={'original_items':len(a['items']),'canonical_issues':len(ids),'originals_preserved_exactly':True,'unique_evidence_paths':len(paths),'human_queue':len(a['human_queue']),'evidence_queue':len(a['evidence_queue']),'item_routes':dict(Counter(r['route'] for r in a['items'])),'result':'PASS'}
OUT.joinpath('verification.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
