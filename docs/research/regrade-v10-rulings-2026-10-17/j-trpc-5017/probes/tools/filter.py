"""Usage: filter.py <result.txt> <probe-prefix>... ; prints the result file with only the named probes' sections."""
import re, sys
src, keep = sys.argv[1], sys.argv[2:]
lines = open(src).read().splitlines()
out, cur, n, bad = lines[:2], None, 0, 0
for line in lines[2:]:
    m = re.match(r'## (\S+): (.*)', line)
    if m:
        cur = any(m.group(1).startswith(k) for k in keep)
        if cur:
            n += 1; bad += m.group(2) != 'compiles'
            out.append('')
    if line.startswith('probes with errors:'):
        break
    if cur and line != '':
        out.append(line)
out += ['', f'probes with errors: {bad} of {n}  (filtered from {src.split("/")[-1]}; the full file is in the group that holds every probe)']
print('\n'.join(out))
