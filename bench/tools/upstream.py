#!/usr/bin/env python3
"""Capture upstream review evidence and validate claim-level maintainer assessments."""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

import check_manifest

ROOT = Path(__file__).resolve().parents[2]
PIN_FIELDS = ("head", "base_sha", "packet_sha256", "diff_manifest_sha256")


def read(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def endpoints(repo, pr):
    return [f"repos/{repo}/pulls/{pr}", f"repos/{repo}/issues/{pr}/comments",
            f"repos/{repo}/pulls/{pr}/comments", f"repos/{repo}/pulls/{pr}/reviews"]


def fetch(endpoint):
    try:
        result = subprocess.run(["gh", "api", "--method", "GET", "--paginate", "--slurp", endpoint],
                                capture_output=True, text=True, timeout=120)
        if result.returncode:
            raise ValueError(result.stderr.strip() or f"gh exit {result.returncode}")
        pages = json.loads(result.stdout)
        if not isinstance(pages, list) or not pages:
            raise ValueError("expected paginated JSON response")
        return {"endpoint": endpoint, "complete": True, "pages": pages, "error": None}
    except (ValueError, OSError, subprocess.TimeoutExpired) as error:
        return {"endpoint": endpoint, "complete": False, "pages": [], "error": str(error)}


def validate_snapshot(snapshot, target):
    errors = check_manifest.validate(read(ROOT / "bench/schema/upstream.schema.json"), snapshot)
    if errors:
        raise ValueError("upstream snapshot: " + "; ".join(errors))
    if (snapshot["target"], snapshot["repo"], snapshot["pr"], snapshot["review_cutoff"]) != (
            target["id"], target["repo"], target["pr"], target["cutoff"]):
        raise ValueError("upstream snapshot belongs to another target or cutoff")
    if snapshot["revision"] != {k: target[k] for k in PIN_FIELDS}:
        raise ValueError("upstream snapshot revision changed")
    required = set(endpoints(target["repo"], target["pr"]))
    observed = set()
    for page in snapshot["coverage"]:
        endpoint = page["endpoint"]
        allowed = re.fullmatch(rf"repos/{re.escape(target['repo'])}/(?:pulls/[0-9]+(?:/comments|/reviews)?|issues/[0-9]+/comments)", endpoint)
        if not allowed or endpoint in observed:
            raise ValueError("unrelated or duplicate upstream endpoint")
        observed.add(endpoint)
        if page["complete"] != bool(page["pages"]) or page["complete"] != (page["error"] is None):
            raise ValueError("inconsistent upstream retrieval coverage")
        if page["complete"] and re.fullmatch(r".*/pulls/[0-9]+", endpoint):
            if len(page["pages"]) != 1 or not isinstance(page["pages"][0], dict):
                raise ValueError("invalid pull-request response")
            pr = page["pages"][0]
            if (pr.get("number") != int(endpoint.rsplit("/", 1)[1])
                    or pr.get("base", {}).get("repo", {}).get("full_name", "").lower() != target["repo"].lower()):
                raise ValueError("pull-request response identity mismatch")
        elif page["complete"] and any(not isinstance(items, list) for items in page["pages"]):
            raise ValueError("invalid comments or reviews response")
    if not required <= observed:
        raise ValueError("upstream snapshot omits required endpoint coverage")


def records(snapshot, endpoint):
    page = next((p for p in snapshot["coverage"] if p["endpoint"] == endpoint), None)
    if page is None or not page["complete"]:
        return []
    return [item for items in page["pages"] for item in (items if isinstance(items, list) else [items])]


def validate_assessment(case, root, resolve):
    assessment = case.get("assessment")
    if assessment is None:
        return
    target = read(root / "bench/targets" / case["target"] / "target.json")
    for name in ("technical", "attribution", "materiality"):
        axis = assessment[name]
        if not axis["reason"].strip() or not axis["evidence"]:
            raise ValueError(f"{name} assessment needs a reason and evidence")
        for ref in axis["evidence"]:
            resolve(ref, root)
    disposition = assessment["maintainer"]
    if not disposition["reason"].strip() or not disposition["snapshots"]:
        raise ValueError("maintainer assessment needs a reason and inspected snapshots")
    snapshots = {}
    for ref in disposition["snapshots"]:
        key = (ref["path"], ref["sha256"])
        if key in snapshots:
            raise ValueError("duplicate upstream snapshot")
        snapshot = read(resolve(ref, root))
        validate_snapshot(snapshot, target)
        snapshots[key] = snapshot
    matched = []
    for judgment in disposition["judgments"]:
        key = (judgment["snapshot"]["path"], judgment["snapshot"]["sha256"])
        if key not in snapshots:
            raise ValueError("judgment uses an uninspected snapshot")
        source = [r for r in records(snapshots[key], judgment["endpoint"]) if r.get("id") == judgment["record_id"]]
        if len(source) != 1 or not judgment["quote"].strip() or judgment["quote"] not in (source[0].get("body") or ""):
            raise ValueError("maintainer judgment quote is not in the archived source")
        if not judgment["reason"].strip() or not judgment["role"]["reason"].strip():
            raise ValueError("maintainer judgment and role need reasons")
        for ref in judgment["role"]["evidence"]:
            resolve(ref, root)
        if judgment["role"]["status"] == "confirmed":
            user = source[0].get("user") or {}
            if user.get("type") != "User" or user.get("login", "").endswith("[bot]"):
                raise ValueError("bot or unidentified author cannot supply maintainer authority")
            if not judgment["role"]["evidence"]:
                raise ValueError("confirmed maintainer role needs saved evidence")
            if judgment["applicability"] == "present-at-head" and judgment["relation"] == "equivalent":
                matched.append(judgment["disposition"])
    status = disposition["status"]
    if status == "unknown" and matched:
        raise ValueError("confirmed matching maintainer judgment conflicts with unknown disposition")
    if status != "unknown":
        if not matched or (status != "mixed" and any(s != status for s in matched)):
            raise ValueError("maintainer disposition requires matching responsible-human judgments")
        if status == "mixed" and len(set(matched)) < 2:
            raise ValueError("mixed disposition needs conflicting judgments")


def route(case):
    assessment = case.get("assessment")
    if assessment is None:
        return "needs-evidence-audit"
    truth, relation, material = (assessment[n]["status"] for n in ("technical", "attribution", "materiality"))
    owner = assessment["maintainer"]["status"]
    conflict = (owner == "mixed" or (owner in ("refuted", "by-design") and truth == "supported")
                or (owner in ("accepted", "deferred") and truth in ("refuted", "unsupported")))
    decision = case["decision"]
    if decision and decision["status"] == "approved":
        eligible_facts = truth == "supported" and relation in ("introduced", "worsened", "new-obligation") and material == "material"
        conflict |= ((decision["outcome"] == "eligible" and (truth in ("refuted", "unsupported")
                                                              or relation == "preexisting" or material == "non-material"))
                     or (decision["outcome"] == "false" and truth == "supported")
                     or (decision["outcome"] == "non-material" and eligible_facts))
    if conflict:
        return "human-conflict"
    if truth == "unsettled" or relation == "unsettled" or material == "unsettled":
        return "human-boundary"
    if decision and decision["status"] == "approved":
        return "settled"
    return "shadow-proposal"


def report(cases, root=ROOT, refs=()):
    rows = [{"claim_id": c["claim_id"], "claim_version": c["version"], "target": c["target"], "route": route(c),
             "maintainer_disposition": c.get("assessment", {}).get("maintainer", {}).get("status", "not-audited"),
             "complete_endpoints": sum(p["complete"] for r in c.get("assessment", {}).get("maintainer", {}).get("snapshots", [])
                                       for p in read(root / r["path"])["coverage"])} for c in cases]
    targets = []
    for path in sorted((root / "bench/targets").glob("*/target.json")):
        target = read(path)
        snapshots = list((root / "bench/claims/upstream" / target["id"]).glob("snapshot.v*.json"))
        latest = max(snapshots, key=lambda p: int(p.stem.removeprefix("snapshot.v"))) if snapshots else None
        coverage = []
        if latest:
            snapshot = read(latest)
            validate_snapshot(snapshot, target)
            coverage = snapshot["coverage"]
        register = read(path.parent / f"register.v{target['current_register']}.json")
        snapshot_ref = {"path": str(latest.resolve().relative_to(root.resolve())),
                        "sha256": hashlib.sha256(latest.read_bytes()).hexdigest()} if latest else None
        targets.append({"target": target["id"], "snapshot": snapshot_ref,
                        "complete_endpoints": sum(e["complete"] for e in coverage), "requested_endpoints": len(coverage),
                        "reference_defects": [d["id"] for d in register["defects"]],
                        "canonical_claims_audited": sum(c["target"] == target["id"] and c.get("assessment") is not None for c in cases)})
    return {"mode": "shadow", "claim_snapshot": list(refs), "claims": rows, "targets": targets}


def capture(target_id, related, out_dir, root=ROOT):
    target = read(root / "bench/targets" / target_id / "target.json")
    requests = list(dict.fromkeys(e for pr in [target["pr"], *related] for e in endpoints(target["repo"], pr)))
    with ThreadPoolExecutor(max_workers=4) as workers:
        coverage = list(workers.map(fetch, requests))
    snapshot = {"schema_version": 1, "target": target_id, "repo": target["repo"], "pr": target["pr"],
                "revision": {k: target[k] for k in PIN_FIELDS}, "review_cutoff": target["cutoff"],
                "retrieved_at": datetime.now(timezone.utc).isoformat(), "coverage": coverage}
    validate_snapshot(snapshot, target)
    directory = out_dir / target_id
    directory.mkdir(parents=True, exist_ok=True)
    versions = [int(p.stem.removeprefix("snapshot.v")) for p in directory.glob("snapshot.v*.json")]
    path = directory / f"snapshot.v{max(versions, default=0) + 1}.json"
    with path.open("x", encoding="utf-8") as handle:
        handle.write(json.dumps(snapshot, indent=2) + "\n")
    complete = sum(p["complete"] for p in coverage)
    print(f"{path}: {complete}/{len(coverage)} endpoints retrieved")
    return complete == len(coverage)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    collect = commands.add_parser("collect")
    choice = collect.add_mutually_exclusive_group(required=True)
    choice.add_argument("--target")
    choice.add_argument("--all", action="store_true")
    collect.add_argument("--related-pr", type=int, action="append", default=[])
    collect.add_argument("--out", type=Path, default=ROOT / "bench/claims/upstream")
    summary = commands.add_parser("report")
    summary.add_argument("--registry", type=Path, default=ROOT / "bench/claims/registry.json")
    args = parser.parse_args()
    try:
        if args.command == "collect":
            if args.all and args.related_pr:
                raise ValueError("related PRs require a single target")
            targets = [args.target] if args.target else [p.parent.name for p in sorted((ROOT / "bench/targets").glob("*/target.json"))]
            succeeded = [capture(t, args.related_pr, args.out) for t in targets]
            return int(not all(succeeded))
        import claims
        refs, cases = claims.load_registry(args.registry)
        print(json.dumps(report(cases, refs=refs), indent=2))
        return 0
    except (ValueError, KeyError, OSError) as error:
        print(f"upstream.py: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
