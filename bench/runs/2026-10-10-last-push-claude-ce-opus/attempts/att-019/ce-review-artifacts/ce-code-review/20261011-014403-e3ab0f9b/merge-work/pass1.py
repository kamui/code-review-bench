import json, re, sys
run = sys.argv[1]
def key(reviewer, f):
    file = str(f.get('file','')).strip().replace('\\','/')
    if file.startswith('./'): file = file[2:]
    title = re.sub(r'\s+', ' ', str(f.get('title',''))).strip().lower()
    return f"{reviewer}|{file}|{str(f.get('line'))}|{title}"
raw = json.load(open(run + '/raw-returns.json'))
detail = {}
for r in raw:
    name = r['reviewer']
    try:
        art = json.load(open(f"{run}/{name}.json"))
    except FileNotFoundError:
        continue
    for f in art.get('findings', []):
        detail[key(name, f)] = {'why_it_matters': f.get('why_it_matters'), 'evidence': f.get('evidence')}
attached = 0; missing = []
for r in raw:
    for f in r.get('findings', []):
        k = key(r['reviewer'], f)
        if k in detail and detail[k]['evidence']:
            f['evidence'] = detail[k]['evidence']; attached += 1
        else:
            missing.append(k)
json.dump(detail, open(run + '/merge-work/source-detail-map.json', 'w'), indent=1)
json.dump(raw, open(run + '/merge-work/helper-input-pass1.json', 'w'), indent=1)
print('detail entries', len(detail), 'attached', attached, 'missing', missing)
