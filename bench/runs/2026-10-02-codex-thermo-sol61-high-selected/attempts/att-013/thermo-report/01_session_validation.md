# Session validation and migration

## Scope and verdict

The reviewed change is `django/contrib/auth/__init__.py` in the pinned range. Related evidence came from `login()`, `update_session_auth_hash()`, `django/contrib/sessions/backends/base.py`, the database and signed-cookie backends, session response middleware, and the existing custom-user contract in `docs/topics/auth/default.txt`.

Request changes for the optional-interface regression below. The built-in rotation algorithm itself passed the exercised cases. There is no separate finding about the existence of fallback branching or key cycling.

## Finding: preserve the optional custom-user hashing contract

The changed call at `django/contrib/auth/__init__.py:215` assumes that any user with `get_session_auth_hash()` also has `get_session_auth_fallback_hash()`. The surrounding guard at line 200 establishes only the former. Before this PR, a custom user could implement session invalidation by providing that one method; subclassing `AbstractBaseUser` was unnecessary. This is an explicit supported boundary in `docs/topics/auth/default.txt:920–927`, and `login()` also checks only the existing method before storing the hash.

For a custom user with only that method, a nonempty stored hash that differs from the current hash enters the new fallback generator call. Attribute lookup occurs before any fallback iterable can be empty, so an empty `SECRET_KEY_FALLBACKS` setting does not avoid the failure. A password change is a normal trigger independent of secret rotation. The exception interrupts authentication resolution instead of invalidating the session, so a request that resolves its authenticated user can fail rather than becoming anonymous. Do not turn an optional feature into an implicit mandatory interface for existing integrations.

The precise changed anchor is lines 213–215. The remedy is to require the fallback capability as part of this branch's condition, and otherwise use the existing flush/anonymous path. Add a test for a custom user outside `AbstractBaseUser` that implements only `get_session_auth_hash()`.

## Reproduction and verification status

Scratch tests are retained at `../review-checks/thermo_checks.py`. They use the real head `auth.get_user()` with controlled backend retrieval and session-key decoding, a hash-only user object, and a recording session. The pinned base function was extracted from the supplied checkout using `git show main:django/contrib/auth/__init__.py`, retained as `../review-checks/base_auth.py`, parsed with `ast`, and executed in the same auth-module context. No repository code was changed.

For each of `SECRET_KEY_FALLBACKS=[]` and `SECRET_KEY_FALLBACKS=["oldsecret"]`, the user returned `"new-password-hash"` and the session contained `"old-password-hash"`. The base returned `AnonymousUser` and called `flush()` once. The head raised `AttributeError` naming `get_session_auth_fallback_hash`, and its flush count remained zero. This is a confirmed runtime difference at the advertised model boundary, not a hypothetical missing method.

The reproduction test deliberately asserts that the head raises. Its passing result confirms the finding; it is not an acceptance test claiming that the head honors the contract. Backend and session-key mocks isolate this boundary from model retrieval. The probe is not a full HTTP integration test for a concrete alternate user model, but it executes the actual changed function and the exact previously documented user protocol.

Five other head probes confirmed the following behavior. A current hash succeeds without evaluating fallbacks or modifying the session. A missing hash flushes without computing either hash method. A later fallback key matches after an earlier wrong key and migrates the hash once; after removing fallbacks, the migrated hash succeeds without another cycle. Changing the password makes an old fallback-signed hash invalid and flushes the session. A signed-cookie session survives rotation, response middleware saves the updated hash, and the resulting cookie authenticates after the fallback is removed.

The signed-cookie probe exercises the real cookie store and session response middleware. The other probes use a recording session to verify validation and mutation choices. The PR's own database-backed happy-path test separately checks that the session key changes and the updated hash survives fallback removal within the request session object.

## Worked code-judo proposal

Keep the feature optional at the caller's existing duck-typed boundary. This avoids imposing a model inheritance requirement or introducing a universal verifier, adapters, or broad exception handling. The candidate is a narrow change to the existing fallback condition:

```python
if (
    session_hash
    and hasattr(user, "get_session_auth_fallback_hash")
    and any(
        constant_time_compare(session_hash, fallback_auth_hash)
        for fallback_auth_hash in user.get_session_auth_fallback_hash()
    )
):
    request.session.cycle_key()
    request.session[HASH_SESSION_KEY] = session_auth_hash
else:
    request.session.flush()
    user = None
```

The current-key check still runs first. A valid legacy user needs no fallback support. An invalid legacy user cannot establish a fallback match and takes the established invalidation path. A capable built-in user retains lazy fallback iteration, short-circuit matching, key cycling, and current-hash replacement. An exception raised inside a supplied fallback implementation still propagates; the remedy does not hide implementation failures.

`ContractRemedyProbes` constructs this candidate only in memory by inserting that capability check into the head function's AST. Two test methods passed thirteen cases: eight combinations of legacy users, fallback settings, and absent/empty/invalid/current hashes, followed by five built-in-user cases with absent/empty/invalid/old/current hashes and multiple fallback keys. This supports the precise guard proposal. The candidate has not been committed or subjected to the full Django suite.

## Control-flow assessment

The source measurement is 28 to 42 lines for `get_user()`, and its explicit `if` count grows from three to five. The maximum nesting of those conditionals increases from three to four. The added no-hash branch avoids computing the current HMAC when there is no session hash; the fallback branch represents a distinct migration outcome. `session_auth_hash` is assigned only in the nonempty-hash branch, but the later use is protected by the same `session_hash` condition, so no unbound-variable bug was found.

An ambitious simplification considered was to restore the original compact current-hash comparison and recompute the current hash only after a successful fallback. That removes the explicit no-hash `if/else` and the cached-hash lifetime, but repeats an HMAC and invokes custom hashing twice on migration. It is a small tradeoff rather than a dramatic simplification, so it is not a separate actionable finding. An enum-valued verifier or policy object would add a layer for a small three-outcome flow and make session-mutation ownership less direct. The existing secret-aware model helper and a clear capability guard are a better fit.

Key cycling precedes hash replacement, and the session middleware persists the modified session on a successful response. This ordering matches the existing `update_session_auth_hash()` pattern. No independent atomicity defect was established. The operations depend on one another and do not benefit from parallel execution.

## Commands and results

Commands ran from the clone root. `PYTHONDONTWRITEBYTECODE=1` prevented Python cache writes in the checkout. Every test command had a 300-second process limit.

```sh
git diff main...review-head
git show main:django/contrib/auth/__init__.py > /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-013/clone-work/review-checks/base_auth.py

PYTHONDONTWRITEBYTECODE=1 \
PYTHONPATH=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-013/clone:/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-013/clone-work/review-checks \
timeout 300 /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-013/clone-cache/venv/bin/python \
tests/runtests.py thermo_checks --settings=test_sqlite --parallel=1 --verbosity=2

PYTHONDONTWRITEBYTECODE=1 \
PYTHONPATH=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-013/clone:/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-013/clone-work/review-checks \
timeout 300 /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-013/clone-cache/venv/bin/python \
tests/runtests.py thermo_checks.ContractRemedyProbes --settings=test_sqlite --parallel=1 --verbosity=2
```

The first scratch selection ran six tests in 0.009 seconds, all passing. The remedy class was added afterward and selected separately; its two tests ran in 0.016 seconds, all passing. The original six probes were not rerun. No test selection was repeated with the same flags.
