# Verification, measurements, and documentation

## Scope and identity

The checkout's `HEAD` and `review-head` both resolve to `2396933ca99c6bfb53bda9e53968760316646e01`; `main` resolves to `9b224579875e30203d079cc2fee83b116d98eb78`. The review used the committed `git diff main...review-head` and read all six manifest files. Related source reads covered authentication tokens, the signing implementation, crypto utilities, session middleware, and database, cached-database, cache, and signed-cookie session implementations.

The frozen skill was read at the user-supplied `clone-work/frozen-skill/SKILL.md`. It does not require child reviewers or reference-resource loads for this review, so neither was used. No repository guidance files, upstream discussions, dependency downloads, or network requests were loaded. All tests used the pre-provisioned virtualenv with SQLite and bytecode writing disabled.

## File-size and change measurements

Measurements came from `git diff --numstat main...review-head`, `wc -l` on head files, and `git show main:<path> | wc -l` for their base versions.

| File | Base lines | Head lines | Added / removed |
| --- | ---: | ---: | ---: |
| `django/contrib/auth/__init__.py` | 230 | 244 | 19 / 5 |
| `django/contrib/auth/base_user.py` | 158 | 167 | 9 / 0 |
| `docs/ref/contrib/auth.txt` | 704 | 711 | 8 / 1 |
| `docs/releases/4.1.8.txt` | 12 | 13 | 2 / 1 |
| `docs/topics/auth/customizing.txt` | 1199 | 1206 | 7 / 0 |
| `tests/auth_tests/test_basic.py` | 140 | 164 | 24 / 0 |

The net change is 62 lines across six files. No file crosses the 1,000-line threshold. The customizing guide was already larger than that threshold and remains a documentation reference organized around user-model contracts; seven lines documenting a method do not create a new decomposition requirement.

The changed authentication region expands a short rejection check into primary-key validation and fallback-key migration. The expansion has a concrete behavioral purpose. It stays in the canonical authentication path rather than adding rotation branches to each session store. The one actionable problem is the unguarded optional method invocation described in [01_authentication.md](01_authentication.md).

## Test commands and results

Commands were run from the clone root. Each invocation finished within five minutes. Each selection was invoked once with its shown flags. No test source was added or edited.

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-008/clone /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-008/clone-cache/venv/bin/python tests/runtests.py auth_tests.test_basic auth_tests.test_models auth_tests.test_auth_backends auth_tests.test_views --settings=test_sqlite --parallel=1
```

Result: 239 tests, `OK`, exit code 0. The reported test execution time was 2.144 seconds. This selection includes the PR's fallback rotation test, model behavior, backend behavior, password-change views, and existing login/session behavior.

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-008/clone /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-008/clone-cache/venv/bin/python tests/runtests.py sessions_tests signing tests.test_crypto auth_tests.test_tokens --settings=test_sqlite --parallel=1
```

Result: 410 loaded tests including one failed-import placeholder; exit code 1 with one error, two skips, and one expected failure. The error was `ModuleNotFoundError: No module named 'tests.test_crypto'`, caused by an incorrect test label supplied by this review. The other 409 loaded tests reported no unexpected failures or errors. Their success is useful evidence for the underlying session, signing, and token primitives, but this invocation must not be described as an entirely passing command.

The module was located with `rg --files tests -g '*crypto*'`, and only the corrected crypto selection was run afterward:

```sh
PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-008/clone /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-008/clone-cache/venv/bin/python tests/runtests.py utils_tests.test_crypto --settings=test_sqlite --parallel=1
```

Result: six tests, `OK`, exit code 0. The reported test execution time was 0.004 seconds.

`git diff --check main...review-head` also passed. No test suite was fetched, no dependencies were installed, and no network was used.

## Coverage and its limits

The new test creates a real default user, logs in, changes `SECRET_KEY`, supplies the old key as a fallback, and checks that authentication remains valid and the session key changes. It then removes the fallback under the same new primary key and verifies that authentication still succeeds. This latter check exercises updating the stored authentication hash rather than merely accepting old hashes repeatedly.

The test uses `User`, which inherits the new fallback method. It therefore does not cover a preexisting custom user implementing only the original hash method. The finding is source-verified, not dynamically reproduced. Its proposed regression test and remedy were not executed or applied.

The default rotation test keeps the request's session object in memory rather than reconstructing it after middleware persistence. Backend source inspection shows that `cycle_key()` preserves data and the assignment of `HASH_SESSION_KEY` marks the session modified; session middleware saves the final modified session on a successful response. This makes the operation order coherent for database, cache, cached-database, and signed-cookie stores. The review did not execute a new end-to-end rotation test for each backend and does not claim the existing tests cover that full matrix.

For signed cookies, `cycle_key()` saves an intermediate representation, then setting the new hash marks the session modified so middleware saves the final representation. For database-backed sessions, key creation precedes the final hash assignment, but the existing session middleware persists that assignment. These are existing backend lifecycle semantics, not new bespoke orchestration introduced by this PR. A proposed transaction spanning arbitrary session stores would add a cross-backend abstraction without a demonstrated failure requiring it.

## Documentation judgment

The reference guide explains fallback verification and adds a version-change note. The customizing guide documents the new generator as yielding password HMACs under fallback secrets. The release note describes the rotation bug. Those statements match the default `AbstractBaseUser` implementation and the supplied test.

The unchanged default-authentication guide explicitly preserves the older independent-user hashing contract, which is the source evidence for the actionable finding. Guarding the new capability reconciles implementation with that existing contract without making inheritance mandatory in a patch release.

The docs were reviewed as text, including method references and their destinations in the repository. A Sphinx build was not run. No independent documentation finding is asserted.

## Preservation checks

Initial `git status --porcelain=v1` was empty, and pinned identities were checked with `git rev-parse HEAD main review-head`. Final status, working-tree diff, staged diff, and the same identities were checked after the review. The clone remains at the pinned head with no tracked modifications or untracked files. Reports and the finding index are stored only under the requested work-directory report path.
