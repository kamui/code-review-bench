import json, sys
run = sys.argv[1]
mech = json.load(open(f"{run}/mechanical-findings.json"))
detail = json.load(open(f"{run}/merge-scratch/source-detail-map.json"))
mm = json.load(open(f"{run}/merge-scratch/merge-map.json"))
fi = json.load(open(f"{run}/finish-input.json"))
by_title = {v["title"]: (k, v) for k, v in mm.items()}

def dkey(src):
    return "|".join([src[0], src[1], src[2], src[3].lower()])

primary, demoted, residual_from_demotion = [], [], []
for f in mech["findings"]:
    key, m = by_title[f["title"]]
    # hydrate
    ev = []
    for s in m["sources"]:
        for e in detail[dkey(s)]["evidence"]:
            if e not in ev:
                ev.append(e)
    why = detail[dkey(m["why_source"])]["why_it_matters"]
    assert why and ev, f["title"]
    f["why_it_matters"] = why
    f["evidence"] = ev
    f["reviewers"] = m["reviewers"]
    f["independent_reviewers"] = m["independent_reviewers"]
    f["merged_from"] = [{"reviewer": s[0], "file": s[1], "line": int(s[2]), "title": s[3]} for s in m["sources"]]
    f["synthesis_note"] = m["severity_note"]
    f["cross_model_corroborated"] = False
    f["validation_status"] = None
    f["validation_reason"] = None
    f["protected_subject_candidate"] = key in ("A", "B", "C", "D", "E")  # TLS trust / credential handling; merge-leaf pre-classification hint only
    if key == "H":
        demoted.append(f)
    else:
        primary.append(f)

# Step 4 soft-bucket demotion: #8 -> residual_risks (single-reviewer P3 advisory from performance; not a regression vs base)
for f in demoted:
    residual_from_demotion.append(
        f"[demoted from primary #{f['#']}, performance P3] The optimisation does not apply when REQUESTS_CA_BUNDLE/CURL_CA_BUNDLE is set: Session.merge_environment_settings (src/requests/sessions.py:766-771) rewrites verify=True into the bundle path string, which reaches _urllib3_request_context as pool_kwargs['ca_certs'] (src/requests/adapters.py:96-98), so urllib3 still builds a fresh SSLContext and calls load_verify_locations() per connection (offline: 3 connections -> 3 calls vs 0 on bare verify=True). Not a regression versus base; a gap between the claimed and delivered improvement. The reviewer's extension (one cached context per bundle path) would widen the shared-context hazard in #1 unless client-cert pools get a non-shared context."
    )

# Step 5 partition
for f in primary:
    f["actionable"] = f["autofix_class"] in ("gated_auto", "manual") and f["owner"] == "downstream-resolver"
actionable_numbers = [f["#"] for f in primary if f["actionable"]]

# Step 6 triage groups (grouping:auto)
triage_groups = [
    {
        "title": "Shared mutable SSLContext handed to urllib3 unconditionally",
        "findings": [1, 4, 3],
        "kind": "apply-queue",
        "context": "All three stem from src/requests/adapters.py:95 setting pool_kwargs['ssl_context'] = _preloaded_ssl_context whenever verify is True, regardless of client_cert (#1), the adapter's own TLS policy in connection_pool_kw (#4), or the request scheme (#3); urllib3 then mutates or overrides from that one object.",
        "preferred_resolution": "Gate the injection once, where the adapter's manager is visible (_get_connection): only when scheme == 'https', client_cert is None, and the target manager carries no ssl_context/ssl_version/ca_* kwargs; otherwise fall back to ca_certs=extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH). Handle #1 first (credential leak), then #4, then #3; one guarded injection site resolves all three.",
        "why": "One root cause and one fix site; fixing #1 alone with the client_cert gate leaves #4 and #3 open, while a single scheme+cert+manager-aware gate closes all three and keeps the perf win on the common direct verify=True path.",
    },
    {
        "title": "verify=True pools that never receive the certifi bundle",
        "findings": [2, 5],
        "kind": "apply-queue",
        "context": "cert_verify (src/requests/adapters.py:304) no longer sets conn.ca_certs for verify=True and _urllib3_request_context no longer passes ca_certs, so any pool that does not use the shared context for its handshake falls to urllib3's load_default_certs(): pools from the public get_connection() hook (#2) and the HTTPS CONNECT proxy leg, which urllib3 wraps with ssl_context=None (#5).",
        "preferred_resolution": "Do #2 first: make cert_verify attach extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH) (with the existence check) whenever the pool's conn_kw ssl_context is not _preloaded_ssl_context. Then #5: in _get_connection's proxy branch add pool_kwargs.setdefault('ca_certs', ...) for verify=True, or decide on proxy_ssl_context in proxy_manager_for (design call, also affects verify=False).",
        "why": "Same root cause (ca_certs removed for verify=True) but different fix paths: the #2 fix leaves pools carrying the shared context untouched, so it does not cover the proxy leg in #5; both are needed to keep certifi as the documented trust store on every verify=True handshake.",
    },
]
# #6 (import-time build) and #7 (tests) are distinct concerns and stay ungrouped.

# Step 7 coverage and advisory assets
residual_risks = [
    "Shared-context mutation beyond load_cert_chain: urllib3 writes `context.check_hostname = False` (urllib3/connection.py:1045-1056) onto whatever context it is handed when pyOpenSSL is injected or an adapter passes assert_hostname/assert_fingerprint; on `_preloaded_ssl_context` this is persistent and process-wide. urllib3's `_match_hostname` fallback (connection.py:1098-1119) still verifies hostnames, so no bypass was constructible, but once check_hostname is False the CERT_NONE write from #3 would succeed silently instead of raising, and a concurrent verify=True handshake in another thread could run on the shared context before its reset at connection.py:1042. Not reproduced (pyOpenSSL not installed); the fixes to #1 and #3 remove the preconditions. (correctness, security, adversarial, testing)",
    "Thread-safety of mutating one ssl.SSLContext (load_cert_chain, verify_mode/check_hostname, set_alpn_protocols) from many threads while other threads call wrap_socket on it was not exercised; concurrent multi-thread use is the PR's target scenario and CPython does not document these writes as safe on a shared context. (security, testing)",
    "pyOpenSSL (`urllib3.contrib.pyopenssl.inject_into_urllib3`) is not installed here, so two behaviours were reasoned from source only: injecting after `import requests` leaves `_preloaded_ssl_context` a stdlib context (verify=True then bypasses pyOpenSSL entirely), and `PyOpenSSLContext.verify_mode` has no check_hostname guard, which would turn the #3 ValueError into a silent CERT_NONE flip of the shared context racing concurrent verify=True handshakes. (adversarial, correctness)",
    "Callsite completeness for the subclassing hooks (`get_connection`, `cert_verify`, `init_poolmanager`, `proxy_manager_for`) is grep-only inside this repo; third-party adapters affected by #2 and #4 (requests-toolbelt, truststore recipe, pyOpenSSL adapters, corporate adapters) were reasoned from known patterns, not enumerated. (all reviewers)",
    "test_different_connection_pool_for_mtls_settings fails identically at HEAD and at the merge-base on this machine (expired tests/certs/mtls client certificate), so the suite currently provides no executable mTLS coverage for the shared-context change in #1. (correctness)",
] + residual_from_demotion

testing_gaps = [
    "No test asserts that a Session without `cert=` does NOT present a client certificate after another Session/adapter used `cert=` with verify=True (tests/testserver TLSServer(mutual_tls=True) with a second plain Session, or a spy on ssl.SSLContext.load_cert_chain asserting no call targets requests.adapters._preloaded_ssl_context) -- would have caught #1.",
    "tests/test_requests.py:2963 (`s.get(url, cert=cert)` with verify=True) now leaves the mtls client cert loaded in `_preloaded_ssl_context` for all later tests in the pytest process; no fixture rebuilds or isolates the shared context.",
    "No test covers an HTTPAdapter subclass passing `ssl_context`, `ssl_version` or `ssl_minimum_version` via `init_poolmanager(**pool_kwargs)` and asserts the pool created for a verify=True request honours it (#4).",
    "No test covers verify=True through an https:// proxy for either an http:// target (#3, ValueError path) or an https:// target (#5, trust-store switch); the suite only exercises proxy selection via environment variables, and no test covers verify=True through a plain HTTP proxy with the shared context either.",
    "No test that `adapter.get_connection(url)` followed by `adapter.cert_verify(pool, url, True, None)` yields a pool verifying against DEFAULT_CA_BUNDLE_PATH (#2).",
    "No test that `import requests` succeeds without the ssl module, or that a missing default CA bundle produces the documented 'Could not find a suitable TLS CA certificate bundle' OSError at request time rather than a failure at import (#6).",
    "verify=True, verify=<file> and verify=False against one host on one Session should yield three distinct pools (PoolKey includes key_ssl_context); existing tests assert two pools only for False-vs-True and False-vs-file, never True-vs-file.",
    "Pickle round-trip (`__setstate__` -> init_poolmanager) followed by a verify=True _get_connection is untested; the module-level context should still be injected after unpickling.",
    "No test documents that the env-var path (REQUESTS_CA_BUNDLE/CURL_CA_BUNDLE set, verify=True through a Session) intentionally still takes the ca_certs per-connection route.",
]

coverage = {
    "depth": "full",
    "fold_in_outcome": fi["peer"]["outcome"],
    "cross_model": fi["peer"]["coverage"],
    "notes_from_dispatch": fi["coverage_notes"],
    "stage5": [
        "Mechanics helper pass 1 over 6 reviewer returns (5 personas + fast-pass): 21 input findings, 19 primary candidates, 0 pre-existing, 3 suppressed by confidence at anchor 50 (2 fast-pass candidates, capped at 50 and never counting toward promotion; 1 correctness import-time candidate), 0 malformed findings, 0 malformed returns.",
        "Semantic reconciliation merged the 19 primary candidates and the 3 suppressed candidates into 8 findings (6 merged groups + 2 single-reviewer); every merge required the same defect and the same fix path, and the HTTPS-proxy-leg and legacy-hook findings were kept separate because their fix paths differ. Merged confidence is the anchor a single reviewer established with a reproduction, never raised by in-process agreement; routing kept the more cautious autofix_class on disagreement (#1 and #4 manual).",
        "Helper pass 2 over the synthetic reconciled return: 8 primary findings, 0 pre-existing, 0 suppressed, 0 malformed; stable # assigned after the deterministic sort.",
        "first_evidence_backfilled: 0; quote-the-line demotions of 75/100 findings for missing first_evidence: 0.",
        "Independent-agreement promotion: none (no adversarial-<provider> peer with independence_verified: true ran; in-process personas share one serving model, and fast-pass never counts). cross_model_corroborated is false on every finding.",
        "Settled decisions: no plan was discovered (plan.source: none), so settlement suppression was not evaluated.",
        "Soft-bucket demotion (mode-aware): 1 finding demoted (#8, performance P3 'Optimization skipped when REQUESTS_CA_BUNDLE/CURL_CA_BUNDLE is set', single-reviewer advisory, not a regression versus base) to residual_risks; stable #8 is retired and not reused. 7 primary findings remain.",
        "Detail hydration: every retained finding carries a non-empty why_it_matters and evidence array from the per-reviewer artifacts (union of contributing evidence; the most specific source why_it_matters kept). 0 findings dropped as malformed.",
        "Testing umbrella: one absence-of-coverage finding (#7) kept primary for the single changed subsystem; narrower coverage cases carried in testing_gaps.",
        "Import-time cost (performance): module-level create_urllib3_context()+load_verify_locations on the 240 KB certifi bundle measured at ~3 ms once per process (`import requests` ~52 ms total); judged immaterial and not a finding. The failure-mode half of that observation is #6.",
        "Learnings-researcher, agent-native and deployment-verification reviewers were not selected; no unstructured returns to fold.",
        "Validator shortcut (Stage 5b step 1): 0 findings skipped. Evidence basis: no finding has an `adversarial-<provider>` corroborator with independence_verified: true (single-model configuration; same-model corroboration never licenses the shortcut).",
        "Validator batch (Stage 5b steps 2-3): 7 findings selected into one batch, ordered by severity then stable # (1 P0, 3 P1, 3 P2; every remaining P0/P1 plus every actionable P2). Normal cap of 8 not exceeded.",
    ],
    "counts": {
        "input_findings": 21,
        "pass1_primary_candidates": 19,
        "suppressed_by_anchor": {"50": 3},
        "first_evidence_backfilled": 0,
        "quote_gate_demotions": 0,
        "semantic_merges": 6,
        "primary_after_merge": 8,
        "mode_aware_demotions": 1,
        "primary_final": len(primary),
        "pre_existing": 0,
        "actionable": len(actionable_numbers),
        "validator_shortcut_skipped": 0,
        "validator_selected": 7,
    },
}

synth = {
    "run_id": fi["run_id"],
    "stage": "merge (Stage 5 + Stage 5b steps 1-3)",
    "scope": {"mode": fi["scope"]["mode"], "base": fi["scope"]["base"], "head_sha": fi["scope"]["head_sha"], "branch": fi["scope"]["branch"], "repo_root": fi["scope"]["repo_root"], "files_changed": 1, "pr_url": fi["scope"]["pr"]["url"]},
    "intent": fi["intent"],
    "reviewers": [r["reviewer"] for r in fi["roster"]["selected"]],
    "peer": fi["peer"],
    "findings": primary,
    "pre_existing_findings": [],
    "demoted_findings": demoted,
    "actionable_numbers": actionable_numbers,
    "triage_groups": triage_groups,
    "residual_risks": residual_risks,
    "testing_gaps": testing_gaps,
    "coverage": coverage,
    "helper_output": f"{run}/mechanical-findings.json",
    "helper_inputs": [f"{run}/merge-scratch/helper-input-1.json", f"{run}/merge-scratch/helper-input-2.json"],
}
json.dump(synth, open(f"{run}/synthesized-findings.json", "w"), indent=2)

order = {"P0": 0, "P1": 1, "P2": 2, "P3": 3}
selected = sorted([f for f in primary if f["severity"] in ("P0", "P1") or f["actionable"]], key=lambda f: (order[f["severity"]], f["#"]))
vin = {
    "run_id": fi["run_id"],
    "run_dir": run,
    "verdicts_path": f"{run}/validator-verdicts.json",
    "scope": {
        "mode": fi["scope"]["mode"],
        "repo_root": fi["scope"]["repo_root"],
        "base_sha": fi["scope"]["base"],
        "head_sha": fi["scope"]["head_sha"],
        "branch": fi["scope"]["branch"],
        "diff_path": fi["scope"]["diff"],
        "files_path": fi["scope"]["files"],
        "tree_is_reviewed_head": fi["scope"]["tree_is_reviewed_head"],
        "pr_url": fi["scope"]["pr"]["url"],
        "dependency_source": {"urllib3": "/home/jack/.t3/bench-runs-lanes/ce-fable-51/2026-11-20-claude-ce-fable-5-1-high/att-001/clone-cache/venv/lib/python3.13/site-packages/urllib3/", "version": "urllib3 2.8.0"},
        "environment": fi["scope"]["environment"],
        "constraints": fi["invocation"]["constraints"],
    },
    "intent": fi["intent"]["summary"],
    "skip": {"count": 0, "evidence_basis": "No finding has an `adversarial-<provider>` corroborator with independence_verified: true (single-model configuration; the in-process adversarial reviewer shares the serving model). Same-model corroboration never licenses the shortcut, so every P0/P1 and every actionable finding is in the batch."},
    "selection_rule": "Every remaining P0/P1 plus every remaining actionable (gated_auto/manual + downstream-resolver) finding; ordered by severity then stable #. 7 selected, normal cap 8 not exceeded; one batch.",
    "findings": [
        {k: f[k] for k in ["#", "title", "severity", "file", "line", "confidence", "autofix_class", "owner", "requires_verification", "reviewers", "independent_reviewers", "cross_model_corroborated", "first_evidence", "evidence", "why_it_matters", "suggested_fix"]}
        for f in selected
    ],
}
json.dump(vin, open(f"{run}/validator-input.json", "w"), indent=2)
print("primary:", len(primary), "actionable:", actionable_numbers, "selected:", [f["#"] for f in selected], "demoted:", [f["#"] for f in demoted])
for f in primary:
    print(f["#"], f["severity"], f["confidence"], f["autofix_class"], "->", f["owner"], "| ev:", len(f["evidence"]), "| why:", len(f["why_it_matters"]))
