import json
import sys

columns = []
for path in sys.argv[1:]:
    cases = {}
    for line in open(path):
        if line.startswith('PROBE '):
            _, name, payload = line.rstrip('\n').split(' ', 2)
            cases[name] = json.loads(payload)['keystrokes']
    label = path.rsplit('/', 1)[-1].removeprefix('result-').removesuffix('.txt')
    columns.append((label, cases))

print('case | typed | ' + ' | '.join(label for label, _ in columns))
for name, rows in columns[0][1].items():
    for i, row in enumerate(rows):
        cells = []
        for _, cases in columns:
            r = cases[name][i]
            marks = ' '.join(m for m, on in (('D', r['dirty']), ('F', r['filled']), ('I', r['invalid'])) if on)
            cells.append(f"{r['controlRenders']} (v{r['validateCalls']} {marks})".replace(' )', ')'))
        print(f"{name} | {row['typed']!r} | " + ' | '.join(cells))
