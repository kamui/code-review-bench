# TLS context and CA-path policy

## Finding

In `src/requests/adapters.py`, `_urllib3_request_context()` classifies a custom CA path into `ca_certs` or `ca_cert_dir` at lines 97–100, while `HTTPAdapter.cert_verify()` independently repeats the same file-versus-directory choice at lines 314–317. The first result contributes to urllib3's pool key; the second configures the connection. This duplicates a single policy across two stages that must agree. If one branch changes without the other, requests can select a pool whose key does not describe the CA settings later applied to its connection. Centralize classification in a small helper returning the relevant CA keyword/value (or a compact equivalent), and use that result in both places. Keep existence checking in `cert_verify()` if preserving its current error timing is important. Avoid removing the `cert_verify()` override point: the method is documented as exposed for subclassing, so the code-judo opportunity is to remove duplicated policy, not the compatibility boundary.

## Evidence

The diff adds the directory check to `_urllib3_request_context()` at lines 97–100. That function passes its `pool_kwargs` into `connection_from_host()` in `_get_connection()`, so the selected CA values participate in pool construction. Later, `send()` calls `self.cert_verify()` on the selected pool connection. The changed `cert_verify()` at lines 304–317 repeats a truthiness/type assumption and the same `os.path.isdir()` split before assigning connection attributes. There is no shared representation tying the two decisions together.

This is not a claim that the current branches produce different values for ordinary string paths. Rather, the structure makes the pool identity and live connection configuration two separate implementations of the same rule. A helper returning a CA setting gives both call sites one policy source while preserving their different responsibilities: pool identity needs the setting before the pool is chosen, and `cert_verify()` retains its established validation and subclass hook.

The changed file is 616 lines after the patch, so the change does not trigger the skill's 1,000-line decomposition threshold. The diff adds no sprawling conditional structure; the concern is the duplicated TLS policy at a sensitive boundary.

## Worked code-judo proposal

Introduce a private helper whose input is the custom `verify` path and whose output is either `{"ca_certs": path}` or `{"ca_cert_dir": path}` based on `os.path.isdir(path)`. `_urllib3_request_context()` merges that mapping into `pool_kwargs`; `cert_verify()` uses the same classification to set the corresponding connection attribute. Keep the existing existence check in `cert_verify()` so callers continue to receive the established `OSError` at request time. If helper output itself is used for validation, preserve that observable error timing and message.

The helper should encode only classification, not pool construction, connection mutation, or path validation. That keeps it canonical without turning a two-branch rule into a broad abstraction. A follow-up test can exercise both a file path and a directory path and verify that the pool key and connection attributes reflect the same classification.

## Verification status

- `git diff --check main...review-head`: passed.
- Focused pytest selection: three tests passed; `test_different_connection_pool_for_tls_settings_verify_bundle_unexpired_cert` failed with `Missing Authority Key Identifier`. The execution packet explicitly records this fixture mismatch as failing at merge-base as well as head, so it is not attributed to the PR.
- A prior pytest invocation had an incorrect combined `-k` selection and deselected all tests; it is not evidence of behavior.
- No additional test or benchmark was run. The review did not modify the checkout.
