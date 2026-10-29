import json, sys, datetime, os
RD = sys.argv[1]
s = json.load(open(os.path.join(RD, "synthesized-findings.json")))
keep = ["#","title","severity","file","line","confidence","autofix_class","owner","requires_verification","pre_existing","suggested_fix","first_evidence","why_it_matters","evidence","reviewers","independent_reviewers"]
f1, f2 = [{k: f[k] for k in keep} for f in s["findings"]]
f1["evidence"] = [
 "django/contrib/auth/__init__.py:215 -- for fallback_auth_hash in user.get_session_auth_fallback_hash()",
 "django/contrib/auth/__init__.py:200 -- if hasattr(user, \"get_session_auth_hash\"): (the only capability check; nothing checks for get_session_auth_fallback_hash)",
 "django/contrib/auth/base_user.py:141 -- def get_session_auth_fallback_hash(self): (defined only on AbstractBaseUser, new in this diff)",
 "docs/topics/auth/default.txt:920-923 -- 'If your AUTH_USER_MODEL inherits from AbstractBaseUser or implements its own get_session_auth_hash() method, authenticated sessions will include the hash returned by this function.'",
 "docs/topics/auth/customizing.txt:727 -- '.. versionadded:: 4.1.8' (new method added in a patch release; no note that custom non-AbstractBaseUser models must now implement it)",
 "Reproduced in scratch against the reviewed tree (reviewers and validator): backend returning a plain class defining only get_session_auth_hash(), session HASH_SESSION_KEY='stale' -> get_user() raises AttributeError: 'U' object has no attribute 'get_session_auth_fallback_hash' instead of flushing and returning AnonymousUser",
 "Cascade: stale hash -> AttributeError before request.session.flush() -> session persists -> every later request that touches request.user raises again"
]
f1["why_it_matters"] = ("get_user() decides whether to verify the session with hasattr(user, \"get_session_auth_hash\"), and the docs explicitly support user models that do not inherit AbstractBaseUser but implement their own get_session_auth_hash(). "
 "The new mismatch branch unconditionally calls user.get_session_auth_fallback_hash(), which exists only on AbstractBaseUser and is new in the 4.1.8 patch release. "
 "For such a duck-typed user model, any stored hash that does not match (the documented logout-on-password-change path, or a SECRET_KEY rotation) now raises AttributeError instead of flushing the session and returning AnonymousUser. "
 "The session is never flushed, so every later request carrying that cookie (including via AuthenticationMiddleware's lazy request.user / request.auser()) returns a 500 until the cookie is cleared. "
 "This narrows a documented contract in a bugfix release with no release note. No in-tree class hits it; the impact falls on third-party user classes.")
f1["validation_status"] = "confirmed"
f1["validation_reason"] = s and json.load(open(os.path.join(RD, "validator-verdicts.json")))["verdicts"][0]["reason"]
f1["protected_subject"] = "authorization-authentication"
findings = [f1, f2]
review = {
 "status": "complete",
 "verdict": "Ready with fixes",
 "scope": {"base": "9b224579875e30203d079cc2fee83b116d98eb78", "branch": "review-head", "head_sha": "2396933ca99c6bfb53bda9e53968760316646e01", "pr_url": "https://github.com/django/django/pull/16631", "files_changed": 6},
 "intent": "Fix #34384: keep users logged in across SECRET_KEY rotation when the old key is in SECRET_KEY_FALLBACKS. Adds AbstractBaseUser.get_session_auth_fallback_hash(); get_user() accepts a session hash that matches a fallback key, cycles the session key, and rewrites the stored hash with the current key. 4.1.8 bugfix backport; must not weaken invalidation on password change or break custom user models.",
 "intent_confidence": "explicit",
 "reviewers": ["correctness", "testing", "security", "api-contract", "adversarial"],
 "findings": findings,
 "actionable_findings": findings,
 "triage_groups": [{k: g[k] for k in ("title","findings","context","preferred_resolution","why")} for g in s["triage_groups"]],
 "pre_existing_findings": [],
 "requirements_completeness": None,
 "learnings": [],
 "agent_native_gaps": [],
 "deployment_notes": [],
 "residual_risks": s["residual_risks"],
 "testing_gaps": s["testing_gaps"],
 "coverage": {
  "depth": "full",
  "cross_model": "Adversarial lens ran in-process (claude-opus-5-5) because the user prohibited the cross-model peer for this run; no independent cross-model pass ran, so no finding received cross-model corroboration.",
  "validator": {
   "batch": "One validator batch of 2 selected findings (#1, #2); verdicts landed inside the wait bound.",
   "confirmed": 2, "rejected": 0, "unresolved": 0, "malformed": 0, "failed": 0, "shortcut_skipped": 0,
   "shortcut_basis": "No finding had cross-model corroboration (peer outcome in-process-fallback, no independence_verified:true artifact); same-model agreement never licenses a skip.",
   "per_finding": {"1": "confirmed; protected subject authorization-authentication (validator key and report-leaf classification agree); incidence among third-party user models not measured", "2": "confirmed; unprotected (the alleged failure is missing test coverage, not an auth defect); validator did not rerun the cited accept-all mutation, gap established from inspected tests"},
   "protected_subject_reclassifications": [],
   "degraded_p0_p1": []
  },
  "suppressed_by_confidence": {"50": 4},
  "mode_aware_demotions": {"to_residual_risks": 2, "to_testing_gaps": 0, "primary_demoted": 0},
  "quote_check_demotions": 0,
  "first_evidence_backfilled": 0,
  "failed_reviewers": [],
  "applied": "none (report-only; mode:agent, apply_local false)",
  "notes": s["coverage"] + [
   "Validator: 2 confirmed, 0 rejected, 0 unresolved, 0 malformed, 0 failed, 0 shortcut-skipped; no protected-subject reclassification; no P0/P1 kept on degraded validation.",
   "Triage group retained unchanged after validation (both findings confirmed)."
  ]
 },
 "artifact_path": RD,
 "run_id": "20261028-195838-43325055"
}
json.dump(review, open(os.path.join(RD, "review.json"), "w"), indent=1)
json.dump({"run_id": review["run_id"], "branch": "review-head", "head_sha": "2396933ca99c6bfb53bda9e53968760316646e01", "verdict": "Ready with fixes", "completed_at": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")}, open(os.path.join(RD, "metadata.json"), "w"), indent=1)
print("ok")
