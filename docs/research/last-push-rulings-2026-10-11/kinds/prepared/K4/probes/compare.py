"""Usage: python compare.py outcomes-base.json outcomes-head.json
Compares, request by request, what one visitor gets before and after the change."""
import json
import sys

base = json.load(open(sys.argv[1]))
head = json.load(open(sys.argv[2]))
print(f"base: django {base['django']}   head: django {head['django']}")
assert base["outcomes"].keys() == head["outcomes"].keys()
same = better = worse = 0
worse_cases, better_by_backend, total_by_backend = [], {}, {}
for name in sorted(base["outcomes"]):
    b, h = base["outcomes"][name], head["outcomes"][name]
    backend = name.split(" | ")[0]
    total_by_backend[backend] = total_by_backend.get(backend, 0) + 1
    lost = any(
        (bs["signed_in"] and not hs["signed_in"]) or (bs["cart"] and not hs["cart"])
        for bs, hs in zip(b, h)
    )
    gained = any(
        (hs["signed_in"] and not bs["signed_in"]) or (hs["cart"] and not bs["cart"])
        for bs, hs in zip(b, h)
    )
    if lost:
        worse += 1
        worse_cases.append((name, b, h))
    elif gained:
        better += 1
        better_by_backend[backend] = better_by_backend.get(backend, 0) + 1
    else:
        same += 1
print(f"paths compared: {len(base['outcomes'])}")
print(f"  the same at every request before and after the change: {same}")
print(f"  signed in or session value kept at some request after the change and not before, never the reverse: {better}")
print(f"  signed in or session value kept at some request before the change and not after: {worse}")
for name, b, h in worse_cases:
    print(f"    WORSE {name}\n      before {b}\n      after  {h}")
print("by session backend (paths, better after the change):")
for backend, total in total_by_backend.items():
    print(f"  {backend}: {total}, {better_by_backend.get(backend, 0)}")


def show(name):
    fmt = lambda steps: ", ".join(
        ("signed in" if s["signed_in"] else "signed out") + ("+value" if s["cart"] else "") for s in steps
    )
    print(f"  {name}\n      before: {fmt(base['outcomes'][name])}\n      after:  {fmt(head['outcomes'][name])}")


print("the claim's path and its neighbours, cache sessions:")
for name in [
    "cache | sign in on O | then N",
    "cache | sign in on O | then N O",
    "cache | sign in on O | then N O N",
    "cache | sign in on O | then O N O",
    "cache | sign in on O | then N N",
    "cache | sign in on O | then P N P",
    "cache | sign in on N | then O",
    "cache | sign in on N | then P",
    "cache | sign in on O | then N X",
    "cache | sign in on O | then X",
]:
    show(name)
print("the same with database sessions:")
for name in [
    "db | sign in on O | then N O",
    "db | sign in on O | then N O N",
    "db | sign in on O | then P N P",
    "db | sign in on N | then O",
]:
    show(name)
