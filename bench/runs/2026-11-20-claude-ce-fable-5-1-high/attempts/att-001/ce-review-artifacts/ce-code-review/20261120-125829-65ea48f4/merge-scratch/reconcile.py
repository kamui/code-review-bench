import json, sys
run = sys.argv[1]
F = "src/requests/adapters.py"
# Semantic reconciliation: each merged candidate records the original source-map keys (reviewer|file|line|title) it was built from.
merged = [
 {
  "key": "A",
  "title": "verify=True + cert= loads the client cert and key into the process-wide shared SSLContext (credential leak)",
  "severity": "P0", "file": F, "line": 95, "confidence": 100,
  "autofix_class": "manual", "owner": "downstream-resolver", "requires_verification": True, "pre_existing": False,
  "reviewers": ["adversarial", "security", "correctness", "testing", "fast-pass"],
  "independent_reviewers": ["adversarial", "security", "correctness", "testing"],
  "first_evidence": "src/requests/adapters.py:95: `        pool_kwargs[\"ssl_context\"] = _preloaded_ssl_context` -- set for verify is True regardless of client_cert; lines 103-105 add cert_file/key_file to the same pool_kwargs, and urllib3/util/ssl_.py:431 then runs `context.load_cert_chain(certfile, keyfile)` on the shared object",
  "suggested_fix": "Never hand the shared context to urllib3 together with any kwarg urllib3 applies by mutating the context (cert_file/key_file/key_password, and by extension assert_hostname/assert_fingerprint). Minimal change in `_urllib3_request_context`: `elif verify is True: if client_cert is None: pool_kwargs[\"ssl_context\"] = _preloaded_ssl_context; else: pool_kwargs[\"ca_certs\"] = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)` (per-identity pools get their own urllib3-built context, restoring the pre-PR behaviour for mTLS; `ca_certs` in pool_kwargs flows through HTTPSConnectionPool to the connection). If the perf win is wanted for mTLS callers too, cache one context per (cert_file, key_file) tuple instead of sharing a single object, and never share a context between different client-cert tuples. Add a regression test using tests/testserver TLSServer(mutual_tls=True): Session A with cert=, Session B without; assert B's connection presents no client certificate (or spy ssl.SSLContext.load_cert_chain and assert no call has `self is requests.adapters._preloaded_ssl_context`).",
  "sources": [["adversarial",F,"95","Cascade: cert= loads client identity into process-wide shared SSLContext"],["security",F,"95","CWE-200/CWE-668: client cert+key leak via shared SSLContext"],["correctness",F,"95","Shared SSLContext mutated by load_cert_chain leaks client certs process-wide"],["testing",F,"95","verify=True + cert= mutates shared SSLContext; mTLS test pollutes process state, untested"]],
  "why_source": ["security",F,"95","CWE-200/CWE-668: client cert+key leak via shared SSLContext"],
  "severity_note": "Reviewers split P0 (security, adversarial; both reproduced the leak with a local mTLS server) vs P1 (correctness, testing). Kept P0: a private key and certificate are presented to arbitrary servers process-wide and the mutation is irreversible for the process lifetime. autofix_class kept at the more cautious `manual` (adversarial) because the longer-term shape (per-identity context cache) is a design choice; the minimal gate is concrete, so owner is downstream-resolver."
 },
 {
  "key": "B",
  "title": "verify=True injects _preloaded_ssl_context over a subclass's init_poolmanager ssl_context/ssl_version (silent TLS policy downgrade)",
  "severity": "P1", "file": F, "line": 95, "confidence": 100,
  "autofix_class": "manual", "owner": "downstream-resolver", "requires_verification": True, "pre_existing": False,
  "reviewers": ["adversarial", "security", "correctness", "testing", "fast-pass"],
  "independent_reviewers": ["adversarial", "security", "correctness", "testing"],
  "first_evidence": "src/requests/adapters.py:95: `        pool_kwargs[\"ssl_context\"] = _preloaded_ssl_context` -- passed as pool_kwargs override at adapters.py:400-402; urllib3/poolmanager.py:409 `base_pool_kwargs[key] = value` lets it replace the manager's own ssl_context, and connection.py:1031-1040 consults ssl_version/ssl_minimum_version only when ssl_context is None",
  "suggested_fix": "Only inject the shared context when the target manager has no TLS policy of its own. In `_get_connection`, after selecting `manager` (self.poolmanager or the proxy manager): `if pool_kwargs.get(\"ssl_context\") is _preloaded_ssl_context and any(k in manager.connection_pool_kw for k in (\"ssl_context\", \"ssl_version\", \"ssl_minimum_version\", \"ssl_maximum_version\", \"ca_certs\", \"ca_cert_dir\", \"ca_cert_data\")): del pool_kwargs[\"ssl_context\"]; pool_kwargs[\"ca_certs\"] = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)` so such adapters keep the pre-change behaviour (certifi loaded into the adapter's own context). Equivalent shape: move the shared-context decision out of the module-level helper into the adapter where connection_pool_kw is visible, or make it opt-out via a class attribute. Add a test in tests/test_adapters.py: subclass overriding init_poolmanager to pass `ssl_context=ctx`; assert `adapter._get_connection(prepared, True).conn_kw[\"ssl_context\"] is ctx`; for plain HTTPAdapter assert it `is requests.adapters._preloaded_ssl_context`.",
  "sources": [["adversarial",F,"95","Composition: verify=True silently discards adapter-supplied ssl_context/ssl_version"],["security",F,"95","CWE-295: subclass-provided ssl_context silently replaced for verify=True"],["correctness",F,"400","Injected ssl_context overrides TLS settings from init_poolmanager subclasses"],["testing",F,"95","Subclass-supplied ssl_context silently overridden for verify=True; no test"]],
  "why_source": ["security",F,"95","CWE-295: subclass-provided ssl_context silently replaced for verify=True"],
  "severity_note": "All four reviewers rated P1. autofix_class kept at the more cautious `manual` (adversarial) because the fix location (helper vs adapter) is a small design choice; the gate itself is concrete, so owner is downstream-resolver. Anchor kept at line 95 (the injection); correctness anchored the consumer at line 400."
 },
 {
  "key": "C",
  "title": "http:// target through an https:// proxy with verify=True now raises an uncaught ValueError",
  "severity": "P1", "file": F, "line": 94, "confidence": 100,
  "autofix_class": "gated_auto", "owner": "downstream-resolver", "requires_verification": True, "pre_existing": False,
  "reviewers": ["adversarial", "security"], "independent_reviewers": ["adversarial", "security"],
  "first_evidence": "src/requests/adapters.py:94-95: `    elif verify is True:` / `        pool_kwargs[\"ssl_context\"] = _preloaded_ssl_context` (no scheme check; `scheme` computed at :89 is unused here) + adapters.py:319 `conn.cert_reqs = \"CERT_NONE\"` -> urllib3/connection.py:1042 `context.verify_mode = resolve_cert_reqs(cert_reqs)` on the shared context whose check_hostname is True",
  "suggested_fix": "Attach the shared context only for HTTPS request URLs, matching the `url.lower().startswith('https')` guard in `cert_verify`: `elif verify is True and scheme == \"https\": pool_kwargs[\"ssl_context\"] = _preloaded_ssl_context`. For http:// targets this restores the pre-PR kwargs, so the https-proxy pool is built exactly as before. More robustly, never pass the shared context together with a cert_reqs other than CERT_REQUIRED. Add a test: `requests.get('http://host/', proxies={'http': 'https://127.0.0.1:<dummy>'})` with verify=True must raise a requests exception, not ValueError.",
  "sources": [["adversarial",F,"94","Cascade: http:// URL via https:// proxy now raises uncaught ValueError"],["security",F,"94","http target via https proxy now raises bare ValueError"]],
  "why_source": ["security",F,"94","http target via https proxy now raises bare ValueError"],
  "severity_note": "Both reviewers rated P1 and reproduced the raw ValueError at HEAD vs a requests exception at BASE."
 },
 {
  "key": "D",
  "title": "HTTPS CONNECT proxy leg is now verified against the OS trust store instead of certifi",
  "severity": "P2", "file": F, "line": 304, "confidence": 100,
  "autofix_class": "gated_auto", "owner": "downstream-resolver", "requires_verification": True, "pre_existing": False,
  "reviewers": ["adversarial", "security", "correctness"], "independent_reviewers": ["adversarial", "security", "correctness"],
  "first_evidence": "src/requests/adapters.py:304-317: `            if verify is not True:` ... `                    conn.ca_certs = cert_loc` -- ca_certs is no longer populated when verify is True; urllib3/connection.py:964-967 `_connect_tls_proxy` uses `ssl_context = self.ssl_context if self.proxy_is_forwarding else None` and `ca_certs = self.ca_certs`, then connection.py:1068 `context.load_default_certs()`",
  "suggested_fix": "Keep certifi for proxied verify=True requests. In `_get_connection`, inside the `if proxy:` branch, add `if verify is True: pool_kwargs.setdefault(\"ca_certs\", extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH))` so the proxy handshake loads the bundle (only the proxy leg, which already builds a fresh context per connection, pays load_verify_locations; the optimisation then applies to direct connections only). Alternative with the perf benefit but needing a design decision: pass `proxy_ssl_context=_preloaded_ssl_context` to `proxy_from_url` in `proxy_manager_for` (per-adapter rather than per-request, and would also apply when verify=False). Document the chosen behaviour in HISTORY.md and add a test for the HTTPS CONNECT proxy leg's trust store.",
  "sources": [["adversarial",F,"304","Composition: HTTPS proxy TLS now verified against OS store, not certifi"],["security",F,"304","HTTPS CONNECT proxy leg now verified against OS store, not certifi"],["correctness",F,"304","verify=True no longer sets ca_certs; HTTPS proxy leg verifies with OS store"]],
  "why_source": ["security",F,"304","HTTPS CONNECT proxy leg now verified against OS store, not certifi"],
  "severity_note": "Reviewers split P1 (adversarial) vs P2 (security, correctness). Kept P2: the trust store for the proxy certificate silently changes (and fails hard only where no OS store is visible to OpenSSL), but only for https:// scheme proxies, a narrower population than the direct and legacy-adapter paths. Different fix path from the legacy-hook finding (that fix leaves pools carrying the shared context untouched), so not merged with it."
 },
 {
  "key": "E",
  "title": "Legacy get_connection()+cert_verify() adapters lose the certifi trust store for verify=True",
  "severity": "P1", "file": F, "line": 304, "confidence": 100,
  "autofix_class": "gated_auto", "owner": "downstream-resolver", "requires_verification": True, "pre_existing": False,
  "reviewers": ["security", "testing"], "independent_reviewers": ["security", "testing"],
  "first_evidence": "src/requests/adapters.py:304: `            if verify is not True:` (nothing is set for verify=True; a pool from the public get_connection() at adapters.py:431 `conn = self.poolmanager.connection_from_url(url)` has no ssl_context) -> urllib3/connection.py:1068 `context.load_default_certs()`",
  "suggested_fix": "Make `cert_verify` self-sufficient so only pools that really carry the preloaded context skip the load: when `verify is True`, `if getattr(conn, \"conn_kw\", {}).get(\"ssl_context\") is _preloaded_ssl_context: cert_loc = None` else `cert_loc = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)`; when `cert_loc is not None`, keep the existence check / OSError and set `conn.ca_certs` or `conn.ca_cert_dir` as before (the _get_connection fast path is untouched, so load_verify_locations is still skipped there). Add a test in tests/test_adapters.py: `pool = a.get_connection(\"https://example.com/\"); a.cert_verify(pool, \"https://example.com/\", True, None); assert pool.ca_certs == extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)`.",
  "sources": [["security",F,"304","Legacy get_connection()+cert_verify() adapters lose certifi trust store"],["testing",F,"304","Legacy get_connection()+cert_verify(verify=True) no longer uses certifi; untested"]],
  "why_source": ["security",F,"304","Legacy get_connection()+cert_verify() adapters lose certifi trust store"],
  "severity_note": "Reviewers split P1 (security) vs P2 (testing). Kept P1: cert_verify and get_connection are documented subclassing hooks the intent names as contracts third-party adapters depend on, the new comment in cert_verify asserts a precondition the public get_connection() pool does not satisfy, and every verify=True request through such an adapter silently switches trust store (hard failure where no OS store exists). Reproduced by security at HEAD vs BASE."
 },
 {
  "key": "Fimp",
  "title": "Import-time SSLContext build makes `import requests` fail when the CA bundle or the ssl module is unavailable",
  "severity": "P2", "file": F, "line": 75, "confidence": 100,
  "autofix_class": "gated_auto", "owner": "downstream-resolver", "requires_verification": True, "pre_existing": False,
  "reviewers": ["security", "adversarial", "correctness"], "independent_reviewers": ["security", "adversarial", "correctness"],
  "first_evidence": "src/requests/adapters.py:75-76: `_preloaded_ssl_context = create_urllib3_context()` / `_preloaded_ssl_context.load_verify_locations(` executed at module import; the pre-change `OSError('Could not find a suitable TLS CA certificate bundle, invalid path: ...')` check now runs only for string verify (adapters.py:304-312)",
  "suggested_fix": "Build the context lazily and keep the explicit check: `_preloaded_ssl_context = None` plus a `threading.Lock`, and a `_get_preloaded_ssl_context()` accessor that (under the lock, once) computes `cert_loc = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)`, raises the existing `OSError(f\"Could not find a suitable TLS CA certificate bundle, invalid path: {cert_loc}\")` when `not cert_loc or not os.path.exists(cert_loc)`, then `create_urllib3_context()` + `load_verify_locations(cert_loc)`. Call it from `_urllib3_request_context` only on the `verify is True` (and `scheme == 'https'`) path so ssl-less interpreters doing HTTP never touch it and the failure timing/message match the pre-change behaviour. Add a test that a missing default bundle yields the documented OSError at request time, not at import.",
  "sources": [["security",F,"75","Import-time SSLContext build makes `import requests` fail"],["adversarial",F,"76","Assumption: import requests now fails when CA bundle or ssl unavailable"],["correctness",F,"76","CA bundle now loaded at import; missing bundle breaks `import requests`"]],
  "why_source": ["security",F,"75","Import-time SSLContext build makes `import requests` fail"],
  "severity_note": "All three reviewers rated P2; security and adversarial reproduced both failure modes (ssl-less interpreter: TypeError at import; missing bundle: bare FileNotFoundError at import vs `import ok` at BASE). The correctness candidate (anchor 50, suppressed by confidence in pass 1) is folded in as a contributor; confidence 100 comes from the reproductions, not from agreement."
 },
 {
  "key": "G",
  "title": "Core perf contract and new ca_cert_dir branch have zero tests",
  "severity": "P2", "file": F, "line": 99, "confidence": 100,
  "autofix_class": "gated_auto", "owner": "downstream-resolver", "requires_verification": True, "pre_existing": False,
  "reviewers": ["testing"], "independent_reviewers": ["testing"],
  "first_evidence": "src/requests/adapters.py:97-100: `        if not os.path.isdir(verify):` / `            pool_kwargs[\"ca_certs\"] = verify` / `        else:` / `            pool_kwargs[\"ca_cert_dir\"] = verify`",
  "suggested_fix": "Add to tests/test_adapters.py (no network needed): (1) parametrized `_get_connection` test: for verify=True assert `pool.conn_kw[\"ssl_context\"] is requests.adapters._preloaded_ssl_context` and `pool.ca_certs is None and pool.ca_cert_dir is None`; for verify=\"tests/certs/valid/ca/ca.crt\" assert `pool.ca_certs == path` and `\"ssl_context\" not in pool.conn_kw`; for verify=\"tests/certs/valid/ca\" assert `pool.ca_cert_dir == path`; for verify=False assert `pool.cert_reqs == \"CERT_NONE\"`. (2) In tests/test_requests.py next to test_different_connection_pool_for_tls_settings_verify_True: run `s.get(url)` against the expired-cert TLSServer with `mock.patch.object(ssl.SSLContext, \"load_verify_locations\", autospec=True)` and assert it was not called (the handshake fails after wrap, so the assertion is meaningful), and a sibling with verify=\"tests/certs/expired/ca/ca.crt\" asserting it was called once with that path. (3) Assert `requests.adapters._preloaded_ssl_context.get_ca_certs()` is non-empty so the import-time load is proven to have loaded certifi.",
  "sources": [["testing",F,"99","Core perf contract and new ca_cert_dir branch have zero tests"]],
  "why_source": ["testing",F,"99","Core perf contract and new ca_cert_dir branch have zero tests"],
  "severity_note": "Single-reviewer testing finding kept as the one umbrella absence-of-coverage finding for the changed subsystem: four behaviours on the TLS trust path changed with no test file touched, the author's claim that the context in use is unobservable is shown false, and the fix is a concrete test spec. Narrower coverage cases are carried in testing_gaps."
 },
 {
  "key": "H",
  "title": "Optimization skipped when REQUESTS_CA_BUNDLE/CURL_CA_BUNDLE is set",
  "severity": "P3", "file": F, "line": 98, "confidence": 75,
  "autofix_class": "manual", "owner": "human", "requires_verification": True, "pre_existing": False,
  "reviewers": ["performance"], "independent_reviewers": ["performance"],
  "first_evidence": "src/requests/adapters.py:96-98: `elif isinstance(verify, str):` / `if not os.path.isdir(verify):` / `pool_kwargs[\"ca_certs\"] = verify`",
  "suggested_fix": "Extend the cached-context approach to string `verify` values instead of passing `ca_certs`/`ca_cert_dir`: add an lru_cache'd `_ssl_context_for_bundle(path)` that does `create_urllib3_context()` + `load_verify_locations(cafile=... or capath=...)`, set `pool_kwargs['ssl_context'] = _ssl_context_for_bundle(verify)` in the str branch of `_urllib3_request_context` (after the existing os.path.exists check so the OSError contract is preserved), and drop the `conn.ca_certs`/`conn.ca_cert_dir` assignments in `cert_verify` for the str case. Assumptions: bundle at a path is immutable for the process lifetime (key on (path, mtime) otherwise); the client-cert case must get a non-shared context so load_cert_chain does not mutate the cached one.",
  "sources": [["performance",F,"98","Optimization skipped when REQUESTS_CA_BUNDLE/CURL_CA_BUNDLE is set"]],
  "why_source": ["performance",F,"98","Optimization skipped when REQUESTS_CA_BUNDLE/CURL_CA_BUNDLE is set"],
  "severity_note": "Single-reviewer P3 advisory (performance); not a regression versus base. Carried through the final helper pass for stable numbering, then demoted to residual_risks in step 4."
 },
]
findings = []
for m in merged:
    f = {k: v for k, v in m.items() if k not in ("key", "sources", "why_source", "severity_note")}
    findings.append(f)
json.dump([{"reviewer": "synthesis", "findings": findings, "residual_risks": [], "testing_gaps": []}], open(f"{run}/merge-scratch/helper-input-2.json", "w"), indent=1)
json.dump({m["key"]: {"title": m["title"], "sources": m["sources"], "why_source": m["why_source"], "severity_note": m["severity_note"], "reviewers": m["reviewers"], "independent_reviewers": m["independent_reviewers"]} for m in merged}, open(f"{run}/merge-scratch/merge-map.json", "w"), indent=1)
print("candidates:", len(findings))
