"""Report leaf: build review.json and metadata.json from the run directory.

Stage 5b step 5 decisions are made by the report leaf and encoded below; this
script only assembles the payload so finding text is carried verbatim from
synthesized-findings.json and validator-verdicts.json.
"""
import datetime
import json
import os
import sys

RUN = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

fi = json.load(open(os.path.join(RUN, "finish-input.json")))
syn = json.load(open(os.path.join(RUN, "synthesized-findings.json")))
outcome = json.load(open(os.path.join(RUN, "validator-outcome.json")))
verdicts = json.load(open(outcome["verdicts"]))["verdicts"]

assert outcome["outcome"] == "verdicts"
selected = syn["validation_selection"]["selected"]
by_num = {}
for v in verdicts:
    assert set(v) == {"#", "status", "protected_subject", "reason"}, v
    assert v["#"] in selected and v["#"] not in by_num, v
    assert v["status"] in ("confirmed", "rejected", "unresolved"), v
    by_num[v["#"]] = v
assert sorted(by_num) == sorted(selected)

# Report leaf's own protected-subject classification, made from the failure
# each finding alleges (validator keys can add protection, never remove it).
OWN_SUBJECT = {
    1: "concurrency",
    2: "data-loss",
    3: "public-contract",
    4: "public-contract",
    10: None,
    11: None,
    12: "public-contract",
}
# Confirmed verdicts whose reason states something that was not measured.
UNMEASURED = {1, 2, 3, 10, 11, 12}

FIELDS = [
    "#", "title", "severity", "file", "line", "confidence", "autofix_class",
    "owner", "requires_verification", "pre_existing", "suggested_fix",
    "first_evidence", "why_it_matters", "evidence", "reviewers",
    "independent_reviewers",
]

findings = []
per_finding = []
for f in syn["findings"]:
    n = f["#"]
    v = by_num[n]
    assert v["status"] == "confirmed"
    out = {k: f[k] for k in FIELDS}
    subject = v["protected_subject"] or OWN_SUBJECT[n]
    if subject:
        out["protected_subject"] = subject
    entry = {
        "#": n,
        "status": v["status"],
        "protected_subject_validator": v["protected_subject"],
        "protected_subject_final": subject,
    }
    if n in UNMEASURED:
        out["validation_status"] = "confirmed"
        out["validation_reason"] = v["reason"]
        entry["reason"] = "See validation_reason on the finding (confirmed; the reason names what was not measured)."
    else:
        entry["reason"] = v["reason"]
    findings.append(out)
    per_finding.append(entry)

actionable = [
    f for f in findings
    if f["autofix_class"] in ("gated_auto", "manual")
    and f["owner"] == "downstream-resolver"
    and f.get("validation_status") != "unresolved"
]
nums = {f["#"] for f in findings}
groups = []
for g in syn["triage_groups"]:
    assert set(g["findings"]) <= nums
    groups.append({
        "title": g["title"],
        "findings": g["findings"],
        "kind": g["kind"],
        "context": g["context"],
        "preferred_resolution": g["preferred_resolution"],
        "why": g["why"],
    })

merge_notes = [s for s in syn["coverage"] if not s.startswith("Stage 5b selection:")]
assert len(merge_notes) == len(syn["coverage"]) - 1
LIMITS_SUFFIX = " (Reviewer-stage statement; the validator later reproduced #2 offline through the real _create_test_db() and the _nodb_cursor fallback, still without a live server.)"
limits = [i for i, s in enumerate(merge_notes) if s.startswith("Verification limits:")]
assert len(limits) == 1
merge_notes[limits[0]] += LIMITS_SUFFIX

validation_notes = [
    "Stage 5b: 0 findings skipped validation through the cross-model shortcut (it needs an ordinary reviewer plus an adversarial-<provider> reviewer with independence_verified: true; none exists in this run). All 7 primary findings (2 P1, 2 P2, 3 P3, all actionable) went to one validator batch, inside the normal cap of eight.",
    "Validator batch result: the verdicts file landed inside the wait bound (validator-outcome.json records outcome 'verdicts'). 7 confirmed, 0 rejected, 0 unresolved, 0 malformed, 0 failed. Every entry matched one input # exactly once, so no finding was dropped, none is a validation-degraded gate, and triage groups were not pruned.",
    "Confirmed with something left unmeasured (carried as validation_status 'confirmed' plus validation_reason): #1 was reproduced offline with real psycopg_pool 3.3.3 and a fake connection class, not against a live PostgreSQL server; #2 was reproduced offline through the _nodb_cursor fallback, and how often a pool exists before the test NAME switch was not measured; #3, #10, #11 and #12 are confirmed on the code with incidence not measured. #4 is confirmed with no such caveat.",
    "Protected subjects: validator keys kept for #1 (concurrency), #2 (data-loss), #4 and #12 (public-contract). The report leaf classified #3 as public-contract (the validator returned null): base reconnects and head raises ProgrammingError on a public connection API, which is an evidenced compatibility concern on an error path. #10 and #11 are unprotected (a new option with no earlier consumers). No verdict was a rejection, so no protected-subject rejection had to be rerouted and no reclassification changed a route.",
    "Report leaf line check (read-only): the anchor line of each of the 7 findings and the file:line citations in their evidence were re-read (checkout at head fad334e1a9, base bcccea3ef3 through git show, installed psycopg_pool 3.3.3) and match the quoted text.",
    "Stage 5c did not run: mode:agent is report-only and local apply was not authorized. Nothing was applied and the checkout was not modified.",
]

coverage = {
    "depth": "full",
    "mode": "agent report-only",
    "applied": 0,
    "suppressed_by_confidence": syn["mechanics"]["suppressed_by_confidence"],
    "first_evidence_backfilled": syn["mechanics"]["first_evidence_backfilled"],
    "quote_gate_demotions": syn["mechanics"]["quote_gate_demotions"],
    "soft_bucket_demotions": syn["mechanics"]["soft_bucket_demotions"],
    "validation": {
        "batches": 1,
        "selected": selected,
        "shortcut_skipped": 0,
        "shortcut_skip_basis": syn["validation_selection"]["shortcut_skip_basis"],
        "collection": "verdicts landed inside the wait bound",
        "confirmed": 7,
        "rejected": 0,
        "unresolved": 0,
        "malformed": 0,
        "failed": 0,
        "per_finding": per_finding,
        "protected_subject_reclassifications": [],
        "degraded_blockers": [],
    },
    "cross_model": {
        "outcome": fi["peer"]["outcome"],
        "coverage": fi["peer"]["coverage"],
    },
    "reviewer_selection": fi["roster"]["selected"],
    "failed_reviewers": fi["collection"]["failed_reviewers"],
    "bound_exceeded": fi["collection"]["bound_exceeded"],
    "notes": validation_notes + merge_notes,
}

verdict = "Ready with fixes"
review = {
    "status": "complete",
    "verdict": verdict,
    "verdict_reasoning": (
        "Two confirmed P1 defects are open, which caps the verdict at Ready with fixes; there is no P0 and no unresolved verification gate. "
        "#1: with OPTIONS 'pool' and 'assume_role' set together no connection can be obtained, every request waits out the pool timeout and fails (reproduced offline with real psycopg_pool, not on a live server). "
        "#2: a pool built for an alias before the test runner switches NAME keeps serving the original database, so a pooled test run can migrate, flush or truncate it (reproduced offline through the _nodb_cursor fallback; incidence not measured). "
        "#3 is a confirmed regression on every backend from the new guard in the shared base wrapper. #4, #10 and #11 align the documented OPTIONS['pool'] contract with the code; #12 is a P3 compatibility decision that shares its edit with #1. "
        "Most important next step: fix #1 by composing SET ROLE on the raw connection passed to the pool configure hook, and add a pooled assume_role test."
    ),
    "fix_order": "#1 (decide #12 in the same edit) -> #2 -> #3 -> #4 -> #10 and #11 together",
    "scope": {
        "base": fi["scope"]["base"],
        "branch": fi["scope"]["branch"],
        "head_sha": fi["scope"]["head_sha"],
        "pr_url": fi["scope"]["pr"]["url"],
        "files_changed": fi["scope"]["files_changed"],
    },
    "intent": fi["intent"]["summary"],
    "intent_confidence": fi["intent"]["confidence"],
    "reviewers": [r["reviewer"] for r in fi["roster"]["selected"]],
    "findings": findings,
    "actionable_findings": actionable,
    "triage_groups": groups,
    "pre_existing_findings": syn["pre_existing_findings"],
    "requirements_completeness": None,
    "learnings": syn["learnings"],
    "agent_native_gaps": syn["agent_native_gaps"],
    "deployment_notes": syn["deployment_notes"],
    "residual_risks": syn["residual_risks"],
    "testing_gaps": syn["testing_gaps"],
    "coverage": coverage,
    "artifact_path": fi["run_dir"],
    "run_id": fi["run_id"],
}

text = json.dumps(review, indent=1, ensure_ascii=True)
assert all(ord(c) < 128 for c in text)
with open(os.path.join(RUN, "review.json"), "w") as fh:
    fh.write(text + "\n")

metadata = {
    "run_id": fi["run_id"],
    "branch": fi["scope"]["branch"],
    "head_sha": fi["scope"]["head_sha"],
    "verdict": verdict,
    "completed_at": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
}
with open(os.path.join(RUN, "metadata.json"), "w") as fh:
    json.dump(metadata, fh, indent=1)
    fh.write("\n")

json.load(open(os.path.join(RUN, "review.json")))
print("findings", [f["#"] for f in findings], "actionable", [f["#"] for f in actionable])
print("review.json bytes", len(text) + 1)
print(json.dumps(metadata))
sys.exit(0)
