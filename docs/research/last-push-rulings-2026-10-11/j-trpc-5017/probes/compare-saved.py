"""Usage: compare-saved.py <saved-result> <new-result> <label>; compares the diagnostics (position and code)."""
import re, sys
def diags(path):
    text = open(path).read().split('### runtime')[0]
    found = re.findall(r'^(\S+:\d+:\d+ TS\d+)', text, re.M) or re.findall(r'^(line \d+ TS\d+)', text, re.M)
    return found
a, b = diags(sys.argv[1]), diags(sys.argv[2])
print(f"{sys.argv[3]}: saved {len(a)} diagnostics, rerun {len(b)}, same positions and codes: {a == b}")
