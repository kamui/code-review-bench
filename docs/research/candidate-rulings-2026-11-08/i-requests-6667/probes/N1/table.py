"""Sum the threaded N1 runs per tree and kind of other request.

Usage: python -I table.py RESULT_FILE...
A tree's runs are all its threaded runs in the files given. For each tree and kind it prints the runs, the runs
that crashed (exit 139, a segmentation fault), and, over the finished runs, how many of the good thread's cert=A
requests the server saw with client A's certificate, with no certificate, with client B's certificate, and how
many raised.
"""
import collections
import re
import sys

files = sys.argv[1:]
rows = collections.defaultdict(lambda: {"finished": 0, "crashed": 0, "client-a": 0, "none": 0, "client-b": 0, "raised": 0})
order = []
for path in files:
    tree = None
    for line in open(path, encoding="utf-8"):
        header = re.match(r"== (\S+) threaded (\S+):", line)
        if header:
            tree, kind = header.groups()
            key = (tree, kind)
            if key not in order:
                order.append(key)
            continue
        if line.startswith("== "):
            tree = None
            continue
        if tree is None:
            continue
        if line.startswith("  other="):
            rows[key]["finished"] += 1
        elif line.strip() == "exit 139":
            rows[key]["crashed"] += 1
        else:
            count = re.match(r"\s+(\d+)  (server received (\S+)|raises)", line)
            if count:
                rows[key][count.group(3) or "raised"] += int(count.group(1))
print(f"{'tree':10} {'other thread':12} {'runs':>4} {'crashed':>7} {'A':>6} {'none':>5} {'B':>4} {'raised':>6}")
for tree, kind in order:
    row = rows[(tree, kind)]
    print(f"{tree:10} {kind:12} {row['finished'] + row['crashed']:>4} {row['crashed']:>7} {row['client-a']:>6} {row['none']:>5} {row['client-b']:>4} {row['raised']:>6}")
