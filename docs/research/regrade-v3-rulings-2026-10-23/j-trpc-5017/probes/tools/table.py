import re, sys, pathlib
root = pathlib.Path(sys.argv[1])
order = ['base','head','pe-j1-copy-intersection','fn-check','both-copy-intersection-and-fn-check','gt-j7-array-check','array-or-fn-check','both-copy-intersection-and-array-or-fn-check','plain-record-guard']
for s in ['known','C1n','C1']:
    rows = {}
    for t in order:
        f = root/s/f'result-{t}.txt'
        txt = f.read_text()
        for m in re.finditer(r'^## (\S+): (.*)$', txt, re.M):
            body = txt[m.end():].split('\n## ')[0]
            codes = sorted(set(re.findall(r'TS(\d+)', body)))
            val = 'ok' if m.group(2)=='compiles' else 'FAIL(' + ','.join(codes) + ')'
            rows.setdefault(m.group(1), {})[t]=val
    print(f'### {s}')
    print('| probe | ' + ' | '.join(order) + ' |')
    print('|' + ' --- |'*(len(order)+1))
    for p,v in rows.items():
        print(f'| {p} | ' + ' | '.join(v.get(t,'?') for t in order) + ' |')
    print()
