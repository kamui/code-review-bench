import json, sys, datetime, os
rd = sys.argv[1]
fi = json.load(open(os.path.join(rd, "finish-input.json")))
sf = json.load(open(os.path.join(rd, "synthesized-findings.json")))
vv = json.load(open(os.path.join(rd, "validator-verdicts.json")))
verd = {v["#"]: v for v in vv["verdicts"]}
keys = ["#","title","severity","file","line","confidence","autofix_class","owner","requires_verification","pre_existing","suggested_fix","first_evidence","why_it_matters","evidence","reviewers","independent_reviewers"]
findings = []
for f in sf["findings"]:
    o = {k: f[k] for k in keys}
    seen = []
    for e in o["evidence"]:
        if e not in seen:
            seen.append(e)
    o["evidence"] = seen
    v = verd[f["#"]]
    assert v["status"] == "confirmed"
    if f["#"] == 1:
        o["validation_status"] = "confirmed"
        o["validation_reason"] = ("Validator confirmed with a read-only repro at 2396933ca9 (duck-typed user, stale session hash -> AttributeError, session not flushed); "
                                  "how often real deployments use duck-typed users that do not inherit AbstractBaseUser was not measured.")
        o["protected_subject"] = "authorization-authentication"
    findings.append(o)
actionable = [f for f in findings if f["owner"] == "downstream-resolver" and f["autofix_class"] in ("gated_auto","manual") and f.get("validation_status") != "unresolved"]
tg = []
for g in sf["triage_groups"]:
    tg.append({"title": g["title"], "findings": g["findings"], "context": g["context"],
               "preferred_resolution": g["preferred_resolution"] + " Apply-queue: both findings are concrete fixes with no design decision.",
               "why": g["why"]})
cov_notes = list(sf["coverage"]) + [
    "Validator: one batch of 2 selected findings (#1 P1, #2 actionable P2); verdicts landed inside the bound. Outcomes: 2 confirmed, 0 rejected, 0 unresolved, 0 malformed, 0 failed, 0 shortcut-skipped.",
    "Validator marked #1 protected_subject authorization-authentication; the report leaf's own classification agrees (a session-invalidation failure in get_user). #2 is a missing regression test for the same auth branch; it was confirmed, so no protected-subject reclassification was needed.",
    "#1 carries validation_status confirmed with an unmeasured-incidence reason: the defect is reproduced, but how many deployments use duck-typed users was not measured.",
    "No P0/P1 finding has degraded validation.",
    "Stage 5c not run: mode:agent is report-only (apply_local false); nothing was applied to the source checkout.",
]
review = {
  "status": "complete",
  "verdict": "Ready with fixes",
  "verdict_reasoning": "One open P1 (#1, validator-confirmed): get_user() calls get_session_auth_fallback_hash() on any user that has get_session_auth_hash(), so duck-typed users that do not inherit AbstractBaseUser get an AttributeError (HTTP 500) and their session is never flushed on a hash mismatch. Fix #1 first by guarding the fallback lookup with hasattr/getattr and adding a duck-typed user test. Then add the #2 negative tests (password changed while SECRET_KEY_FALLBACKS is set; hash signed with a key outside SECRET_KEY/SECRET_KEY_FALLBACKS) so the session-invalidation guarantee is covered before the backport.",
  "scope": {
    "base": fi["scope"]["base"],
    "branch": fi["scope"]["branch"],
    "head_sha": fi["scope"]["head_sha"],
    "pr_url": fi["scope"]["pr"]["url"],
    "files_changed": sum(1 for l in open(fi["scope"]["files"]) if l.strip()),
  },
  "intent": fi["intent"]["summary"],
  "intent_confidence": fi["intent"]["confidence"],
  "reviewers": [r["reviewer"] for r in fi["roster"]["selected"]],
  "findings": findings,
  "actionable_findings": actionable,
  "triage_groups": tg,
  "pre_existing_findings": sf["pre_existing_findings"],
  "requirements_completeness": None,
  "learnings": [],
  "agent_native_gaps": [],
  "deployment_notes": [],
  "residual_risks": sf["residual_risks"],
  "testing_gaps": sf["testing_gaps"],
  "coverage": {
    "depth": "full",
    "cross_model": {"outcome": sf["peer"]["outcome"], "note": sf["peer"]["coverage"]},
    "validator": {"outcome": "verdicts", "batch_size": 2, "confirmed": 2, "rejected": 0, "unresolved": 0, "malformed": 0, "failed": 0, "shortcut_skipped": 0, "per_finding": {"1": "confirmed", "2": "confirmed"}, "protected_subject_reclassifications": []},
    "suppressed_by_confidence": sf["counts"]["suppressed_by_confidence"],
    "mode_aware_demotions": sf["counts"]["mode_aware_demotions"],
    "quote_gate_demotions": sf["counts"]["quote_gate_demotions"],
    "first_evidence_backfilled": sf["counts"]["first_evidence_backfilled"],
    "failed_reviewers": fi["collection"]["failed_reviewers"],
    "applied": 0,
    "standards": "not run (no applicable standards files)",
    "learnings": "not run (no docs/solutions corpus and no declared Compound Packs)",
    "settlement_suppression": "not evaluated (no plan discovered)",
    "notes": cov_notes,
  },
  "artifact_path": rd,
  "run_id": fi["run_id"],
}
s = json.dumps(review, ensure_ascii=True, indent=None)
open(os.path.join(rd, "review.json"), "w").write(s)
meta = {"run_id": fi["run_id"], "branch": "review-head", "head_sha": "2396933ca99c6bfb53bda9e53968760316646e01",
        "verdict": review["verdict"], "completed_at": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")}
open(os.path.join(rd, "metadata.json"), "w").write(json.dumps(meta, indent=2))
print(len(s), [f["#"] for f in actionable])
