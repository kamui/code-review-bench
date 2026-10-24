"""Print the PROBE lines of several result files side by side, one compact cell per step.

Usage: python3 -I table.py <result file> [<result file> ...]
Cell: baseline / dirty(D) / filled(F) / invalid(I) / validator calls / error text.
"""
import json
import sys


def cell(s):
    flags = ''.join(k for k, v in (('D', s['rootDirty']), ('F', s['rootFilled']), ('I', s['rootInvalid'])) if v)
    err = s['errorText'] or ''
    return f"base={s['baseline']} {flags or '-'} calls={len(s['validateCalls'])}" + (f" err={err!r}" if err else '')


def load(path):
    rows = {}
    for line in open(path, encoding='utf-8'):
        if line.startswith('PROBE '):
            _, name, data = line.rstrip('\n').split(' ', 2)
            rows[name] = json.loads(data)
    return rows


files = sys.argv[1:]
tables = [load(f) for f in files]
print(' | '.join(['case/step'] + files))
for name in tables[0]:
    for step in tables[0][name]:
        cells = []
        for t in tables:
            s = t.get(name, {}).get(step)
            cells.append(cell(s) if s else 'n/a')
        print(f"{name}.{step} | " + ' | '.join(cells))
