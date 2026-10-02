# TLS initialization, compatibility, and verification limits

## Scope and judgment

This subsystem contains F3 from [summary.md](summary.md).
It also records size measurements and the existing-suite verification needed
to distinguish new regressions from unusable fixture certificates.

## Source evidence for F3

At `src/requests/adapters.py:75–78`, the module now calls
`create_urllib3_context()`, extracts the default certificate bundle path, and
calls `load_verify_locations()` without deferring or containing failure.

Requests imports its API and Session classes during package initialization.
Session imports the transport adapters. The module is consequently evaluated
during `import requests`, before the caller can choose HTTP, a custom CA file,
an environment-provided CA file, or disabled verification.

At the base, `cert_verify()` extracted and checked the default CA bundle only
inside the verified HTTPS branch. HTTP and unverified HTTPS never needed that
file. A request specifying a different CA path could use that path instead.

The head keeps the canonical `extract_zipped_paths()` helper, which is correct
for zipped packaging. The problem is when it is invoked and when loading is
required. Extraction can also need temporary storage, so eager invocation
widens import's resource boundary even where the trust bundle is packaged.

The missing-file reproduction establishes the change in failure boundary.
The unreadable/malformed alternatives are consequences of calling the same
loading API unconditionally; they were not separately executed.
A no-SSL Python environment was not provisioned or tested and is not an
additional indexed finding.

## Differential reproduction

In [../test_review_tls.py](../test_review_tls.py),
`test_import_does_not_require_default_bundle` temporarily patches
`requests.utils.DEFAULT_CA_BUNDLE_PATH` to an absent path in the work directory.
It then executes the selected adapter source as a new module with working
Requests relative imports.

The base source imports successfully without accessing that file.
The head raises `FileNotFoundError` from its top-level
`_preloaded_ssl_context.load_verify_locations()` call.
The bundle is not removed, the installed certifi package is not modified, and
the checkout remains unchanged.

This is a module-initialization differential test rather than a full clean
subprocess import of the package. The package-level implication follows from
the inspected import chain. The test isolates precisely the changed module
and avoids unrelated startup differences.

The complete invocation and environment are recorded in
[01_tls_context_ownership.md](01_tls_context_ownership.md).
The log is [../review-tests.log](../review-tests.log).
Verification status: reproduced at the head; corresponding assertion passes
at the pinned base.

## Code-judo proposal: publish a ready context on demand

Keep certificate trust loading behind a private getter whose only callers are
eligible default-verified HTTPS paths. Reuse the canonical extraction helper
and perform initialization under a lock so concurrent first use does not
multiply the expensive load.

```python
_default_ssl_context = None
_default_ssl_context_lock = threading.Lock()

def get_default_ssl_context():
    global _default_ssl_context
    with _default_ssl_context_lock:
        if _default_ssl_context is None:
            ca_path = extract_zipped_paths(DEFAULT_CA_BUNDLE_PATH)
            if not ca_path or not os.path.exists(ca_path):
                raise OSError(
                    "Could not find a suitable TLS CA certificate bundle, "
                    f"invalid path: {ca_path}"
                )
            context = create_urllib3_context()
            context.load_verify_locations(ca_path)
            _default_ssl_context = context
        return _default_ssl_context
```

This sketch illustrates synchronized, atomic publication; it is not an applied
patch. The lock serializes initialization and retrieval, not network work.
Keeping construction in a local variable means a failed load does not publish
a half-configured context. No broad exception fallback should quietly select
another trust store.

Before using the getter, the adapter must establish the eligibility boundary
from F1 and F2. Merely replacing eager initialization with this getter would
leave authentication contamination and custom-context replacement intact.

For an adapter-supplied context or client-certificate request, preserve the
existing trust-path flow. If default CA extraction needs validation, perform
it only for the selected policy. For custom bundles or directories, validate
the selected user path rather than trying the default bundle first.

The resulting `cert_verify()` should distinguish actual known preloaded trust
from a connection that still needs CA settings. The head comment at lines
300–305 incorrectly makes that distinction depend only on a boolean.
Reusing one normalized policy removes duplicate directory classification and
avoids pool configuration that disagrees with later certificate configuration.
This is supporting structural remediation, not an additional finding for
duplication that already existed at the base.

Do not move the entire adapter into additional modules solely to host five
initialization lines. A private getter and a single TLS policy boundary offer
clearer ownership without an unnecessary abstraction hierarchy.

## Existing focused test selection

The equivalent relative-path command was:

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH="$PWD/src" \
  ../clone-cache/venv/bin/python -m pytest \
  tests/test_adapters.py tests/test_requests.py \
  -k 'request_url_trims_leading_path_separators or pyopenssl_redirect or invalid_ca_certificate_path or invalid_ssl_certificate_files or env_cert_bundles or http_with_certificate or certificate_failure or different_connection_pool_for_tls_settings or different_connection_pool_for_mtls_settings' \
  -q -p no:cacheprovider --basetemp=../clone-work/pytest-existing-tmp
```

Each selection and flag set was executed once. Output is in
[../existing-tests.log](../existing-tests.log).
The result is 3 failed, 15 passed, 312 deselected in 3.10 seconds.

`test_pyopenssl_redirect` and
`test_different_connection_pool_for_tls_settings_verify_bundle_unexpired_cert`
fail with `Missing Authority Key Identifier`. These are the two baseline
fixture mismatches disclosed by the packet and are not attributed to the PR.

The third failure is
`test_different_connection_pool_for_mtls_settings`.
It fails at `tests/test_requests.py:2960`, on the first request with
`verify=False`, reporting `SSLV3_ALERT_CERTIFICATE_EXPIRED`.
That branch does not select the new preloaded context.

To establish its baseline behavior without changing branches or the checkout,
[../test_base_mtls_fixture.py](../test_base_mtls_fixture.py) loads the copied
base adapter module and temporarily replaces
`requests.sessions.HTTPAdapter` with that class. It calls the unchanged
existing test method.

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH="$PWD/src:$PWD" \
  ../clone-cache/venv/bin/python -m pytest \
  ../clone-work/test_base_mtls_fixture.py \
  -k test_existing_mtls_fixture_at_base -q -p no:cacheprovider \
  --basetemp=../clone-work/pytest-base-fixture-tmp
```

Output is in [../base-mtls-fixture.log](../base-mtls-fixture.log).
Result: 1 failed, 34 deselected in 0.50 seconds, with the same expired-client
alert. This is a baseline-adapter comparison, not a claim to have run the
entire repository suite on a second checkout.

The imported existing test class is also visible during scratch collection;
the explicit `-k` selects only the intended wrapper, accounting for the 34
deselections. No additional existing tests were rerun inadvertently.

## Measurements and commands

`wc -l src/requests/adapters.py` gives 616.
`git show main:src/requests/adapters.py | wc -l` gives 606.
`git diff --numstat main...review-head` gives 28 additions and 18 deletions.
There is no 1000-line threshold crossing or giant-file regression in this PR.

`git diff --check main...review-head` reports no errors.
The review read the entire changed adapter and relevant nearby source,
including the imported dependency implementation used by the tests.
The checkout started clean and no remedy or test file was placed in it.
Tracked and untracked checkout state was checked again after report generation.

No network hosts, upstream discussions, reference answers, ambient guidance,
or additional skills were consulted. Tests used local fixture servers only.

## Validation still required for implementation

A remedy should establish that importing Requests and using HTTP do not load
the default bundle. A custom-bundle request and disabled verification must also
remain independent of a missing default bundle.
An eligible default-verified request should give the existing useful path error
when default resources are unavailable.

Exercise successful zipped-path extraction at first eligible use and verify
that the extracted CA is loaded once. Concurrent first use should obtain the
same fully constructed context. Initialization failure must leave the cache
empty and must not silently downgrade verification.

Those are remediation acceptance criteria, not reports of executed checks.
The review has not benchmarked the concurrent speedup or tested every supported
Python, urllib3, TLS backend, or packaging arrangement.

