import re, sys, pathlib
here = pathlib.Path(sys.argv[1])
trees = ['release-v4.5.0', 'release-v4.5.1', 'base', 'head', 'head-fix-gtp5', 'head-fix-json', 'head-fix-both', 'head-fix-gtp4', 'base-fix-json']
rows = {}
order = []
for t in trees:
    rt = section = case = None
    for line in (here / f'result-{t}.txt').read_text().splitlines():
        if line.startswith('### '):
            rt = line.split()[1]
        elif line.startswith('# '):
            section = line[2:]
        elif line.startswith('## '):
            case = line[3:]
        else:
            m = re.match(r"\s+route validator\('(json|form)'\):\s+(.*)", line)
            if m and m.group(1) in section:
                key = (section, case)
                if key not in rows:
                    rows[key] = {}
                    order.append(key)
                rows[key].setdefault(t, {})[rt] = m.group(2)
print('| target | Content-Type and body | ' + ' | '.join(trees) + ' |')
print('|' + ' --- |' * (len(trees) + 2))
for key in order:
    cells = []
    for t in trees:
        vals = rows[key].get(t, {})
        uniq = set(vals.values())
        cells.append(next(iter(uniq)) if len(uniq) == 1 and len(vals) == 3 else '; '.join(f'{k}: {v}' for k, v in vals.items()))
    print(f'| {key[0]} | `{key[1]}` | ' + ' | '.join(c.replace('|', '\\|') for c in cells) + ' |')
