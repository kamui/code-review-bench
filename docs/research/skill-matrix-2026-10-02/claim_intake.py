#!/usr/bin/env python3
"""Link the skill-matrix reviews' repeated claims to the shared registry before grading.

Usage::

    python3 docs/research/skill-matrix-2026-10-02/claim_intake.py inventory --out DIR --key KEY [--run RUN ...]
    python3 docs/research/skill-matrix-2026-10-02/claim_intake.py apply --proposals DIR --key KEY --record OUT

``inventory`` writes, for every target that has a registered claim, ``DIR/<target>.json``: the
target's canonical claims without their decisions, and every item of a 2026-10-02 review that no
claim links yet, under a random token with run and attempt paths removed; ``--run`` limits the
reviews to the named runs. KEY maps tokens to their
sources and stays outside the assessor's directory.

An assessor writes ``DIR/<target>.proposals.json``, one entry per token, each with a ``matches``
list of ``{"claim_id", "relation": "equivalent"|"related", "reason"}``; an empty list records an
inspected item with no match. ``apply`` refuses a missing or unknown token, writes the next version
of every claim that gains a link, points the registry at it and saves the intake record. A link's
relation is a proposed intake judgment; it changes no eligibility decision.
"""

import argparse
import hashlib
import json
import re
import secrets
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / "bench/tools"))
import claims  # noqa: E402

REGISTRY = ROOT / "bench/claims/registry.json"
RUN_PREFIX = "bench/runs/2026-10-02-"
ATTEMPT_PATH = re.compile(r"/[^\s)`'\"]*?/bench-runs/[^/\s)]+/att-\d+/")
REASON = ("Inventory the reviews of the 2026-10-02 skill-matrix runs. Add assessed equivalent or related source-item "
          "links while preserving the saved eligibility ruling, evidence and every earlier link.")


def ref(path):
    return {"path": Path(path).resolve().relative_to(ROOT).as_posix(), "sha256": hashlib.sha256(Path(path).read_bytes()).hexdigest()}


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def scrub(value):
    return ATTEMPT_PATH.sub("<attempt>/", value) if isinstance(value, str) else value


def inventory(args):
    _refs, cases = claims.load_registry(REGISTRY)
    out = Path(args.out)
    out.mkdir(parents=True)
    key = {}
    for target in sorted({case["target"] for case in cases}):
        linked = {(link["review"]["path"], link["item_id"]) for case in cases for link in case["links"]}
        rows = [row for row in claims.inventory(target) if row["review"]["path"].startswith(RUN_PREFIX)
                and (not args.run or row["review"]["path"].split("/")[2] in args.run)
                and (row["review"]["path"], row["item_id"]) not in linked]
        secrets.SystemRandom().shuffle(rows)
        items = []
        for row in rows:
            token = f"item-{secrets.token_hex(4)}"
            key[token] = {"target": target, "review": row["review"], "attempt_id": row["attempt_id"], "item_id": row["item_id"]}
            item = row["item"]
            items.append({"token": token, "file": item.get("file"), "line_start": item.get("line_start"),
                          "line_end": item.get("line_end"), "kind": item.get("kind"),
                          "claim": scrub(item.get("claim")), "consequence": scrub(item.get("consequence")),
                          "proposed_fix": scrub(item.get("proposed_fix"))})
        write(out / f"{target}.json", {"target": target, "claims": [
            {"claim_id": case["claim_id"], **case["claim"]} for case in cases if case["target"] == target], "items": items})
        print(f"{target}: {len(items)} unlinked items, {sum(case['target'] == target for case in cases)} claims")
    with Path(args.key).open("x", encoding="utf-8") as handle:
        json.dump(key, handle, indent=2)


def apply(args):
    key = json.loads(Path(args.key).read_text(encoding="utf-8"))
    registry = json.loads(REGISTRY.read_text(encoding="utf-8"))
    paths = {json.loads((ROOT / entry["path"]).read_text())["claim_id"]: ROOT / entry["path"] for entry in registry["cases"]}
    proposals, added, record = {}, {}, []
    for path in sorted(Path(args.proposals).glob("*.proposals.json")):
        for token, value in json.loads(path.read_text(encoding="utf-8")).items():
            if token in proposals or token not in key:
                raise SystemExit(f"{path}: unknown or repeated token {token}")
            proposals[token] = value["matches"]
    missing = sorted(set(key) - set(proposals))
    if missing:
        raise SystemExit(f"{len(missing)} inventory items have no assessment, first {missing[0]}")
    for token, matches in proposals.items():
        source = key[token]
        if claims.digest(ROOT / source["review"]["path"]) != source["review"]["sha256"]:
            raise SystemExit(f"{source['review']['path']} changed since the inventory")
        for match in matches:
            case = json.loads(paths[match["claim_id"]].read_text())
            if case["target"] != source["target"] or match["relation"] not in ("equivalent", "related") or not match["reason"].strip():
                raise SystemExit(f"{token}: invalid match {match}")
            added.setdefault(match["claim_id"], []).append({
                "mapping": None, "review": source["review"], "attempt_id": source["attempt_id"],
                "item_id": source["item_id"], "relation": match["relation"], "reason": match["reason"].strip()})
        record.append({"review": source["review"], "attempt": source["attempt_id"], "item": source["item_id"],
                       "matches": matches, "status": "Inspected inventory item" + ("" if matches else "; no match to this target's canonical claims")})
    for claim_id, links in sorted(added.items()):
        old = paths[claim_id]
        case = json.loads(old.read_text())
        case.update(version=case["version"] + 1, supersedes=ref(old), revision_reason=REASON,
                    links=case["links"] + sorted(links, key=lambda link: (link["review"]["path"], link["item_id"])))
        new = old.with_name(f"{claim_id}.v{case['version']}.json")
        with new.open("x", encoding="utf-8") as handle:
            json.dump(case, handle, indent=2)
            handle.write("\n")
        entry = next(entry for entry in registry["cases"] if ROOT / entry["path"] == old)
        entry.update(ref(new))
    write(REGISTRY, registry)
    claims.load_registry(REGISTRY)
    links = [match for matches in proposals.values() for match in matches]
    with Path(args.record).open("x", encoding="utf-8") as handle:
        json.dump({"schemaVersion": 1, "registry": ref(REGISTRY), "inspectedInventoryItems": len(record),
                   "addedLinks": len(links), "equivalentLinks": sum(m["relation"] == "equivalent" for m in links),
                   "relatedLinks": sum(m["relation"] == "related" for m in links),
                   "status": "Proposed intake judgments by blinded automated assessment; eligibility decisions unchanged",
                   "items": record}, handle, indent=2)
        handle.write("\n")
    print(f"{len(record)} items inspected, {len(links)} links added to {len(added)} claims")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="command", required=True)
    inv = commands.add_parser("inventory")
    inv.add_argument("--out", required=True)
    inv.add_argument("--key", required=True)
    inv.add_argument("--run", nargs="*", default=[])
    app = commands.add_parser("apply")
    app.add_argument("--proposals", required=True)
    app.add_argument("--key", required=True)
    app.add_argument("--record", required=True)
    args = parser.parse_args()
    {"inventory": inventory, "apply": apply}[args.command](args)


if __name__ == "__main__":
    main()
