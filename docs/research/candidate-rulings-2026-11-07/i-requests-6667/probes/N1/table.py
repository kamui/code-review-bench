"""Summarise run.sh outputs into one table. Usage: python table.py <result-*.txt>..."""
import re, sys

rows = {}
order = []
for path in sys.argv[1:]:
    label = state = None
    for line in open(path):
        m = re.match(r"== (\S+), default CA location: (\S+)", line)
        if m:
            label, state = m.groups()
            if label not in order:
                order.append(label)
            rows.setdefault(state, {})[label] = {}
            continue
        key, _, val = line.rstrip("\n").partition("  ")
        key = key.strip(); val = val.strip()
        if not key or label is None:
            continue
        rows[state][label][key] = val

def short(v):
    if v is None:
        return "-"
    if v.startswith("import ok"):
        return "ok (%s load)" % v.rsplit(" ", 1)[1]
    m = re.match(r"raised (\S+?):.*?(\[from [^\]]+\])?$", v)
    if m:
        kind = m.group(1).split(".")[-1]
        if "Could not find a suitable" in v:
            kind = "OSError(Could not find...)"
        if "requests.exceptions.SSLError" in v:
            kind = "requests SSLError"
        return kind + (" " + m.group(2).replace("[from requests/", "@").rstrip("]") if m.group(2) else "")
    return v

actions = ["import requests", "GET http://", "GET https, verify=False", "GET https, verify=<own CA file>",
           "GET https, REQUESTS_CA_BUNDLE=<own CA file>", "GET https, default verify=True"]
for state, by in rows.items():
    print("\n### default CA location: %s\n" % state)
    print("| action | " + " | ".join(order) + " |")
    print("| --- |" + " --- |" * len(order))
    for a in actions:
        cells = []
        for label in order:
            d = by.get(label, {})
            if "import requests" in d and not d["import requests"].startswith("import ok") and a != "import requests":
                cells.append("(import failed)")
            else:
                cells.append(short(d.get(a)))
        if all(c == "-" for c in cells):
            continue
        print("| %s | %s |" % (a, " | ".join(cells)))
