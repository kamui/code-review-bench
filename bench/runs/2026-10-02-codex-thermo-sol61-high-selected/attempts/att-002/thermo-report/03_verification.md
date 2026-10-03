# Scope, measurements, and verification limits

This file records coverage and evidence for the summary's five findings. It
does not introduce additional findings or unanswered questions.

## Identity and execution policy

The pinned base is `bcccea3ef31c777b73cba41a6255cd866bf87237` and the pinned head is
`fad334e1a9b54ea1acb8cce02a25934c5acfe99f`. Local `main` and checked-out
`review-head` match the supplied packet. The head tree object is
`4fc84e9981d6cdc83175d88eaa59c251cda009e7`.

The frozen skill was read from `clone-work/frozen-skill/SKILL.md`. Its workflow
does not require a child reviewer or reference resources, so the review used
one primary context with no delegation and no other skills. Repository guidance
files, personal configuration, prior reviews, reference answers, and upstream
forge material were not loaded. No network access was used.

The clone was treated as read-only. Scratch code and reports were written only
under `clone-work`. Every check that imported Django used
`PYTHONDONTWRITEBYTECODE=1` and the clone's path in `PYTHONPATH`. Each command
completed within the five-minute allowance; the probes took less than a second.

## Changed-file coverage

| File | Review coverage |
| --- | --- |
| `django/db/backends/base/base.py` | Read generic connect, close, health checks, thread ownership, copy, timezone state, and atomic teardown. Exercised the new guard through an actual atomic lifecycle. |
| `django/db/backends/postgresql/base.py` | Read the full module and traced every new pool lookup, raw configuration helper, close path, isolation handling, cursor adaptation, and administrative cursor path. |
| `django/db/backends/postgresql/creation.py` | Read clone/drop cleanup and traced base creation, restoration, mirrors, and worker setup. Executed initial test-name switching with external I/O mocked. |
| `django/db/backends/postgresql/features.py` | Read skip selection and compared the pool-enabled predicate with acquisition and driver validation. Existing backend-dependent skips provide a canonical home; no generic feature-layer finding is asserted. |
| `docs/ref/databases.txt` | Read the new pool contract and nearby connection/isolation guidance. Verified options-mapping and driver-policy mismatches. |
| `docs/releases/5.1.txt` | Inspected the release entry and reference target. It correctly announces the intended feature; no separate finding. |
| `tests/backends/postgresql/tests.py` | Read the helper and all added pool tests, plus altered direct-connection tests. Checked coverage against callback, invalidation, and option-shape scenarios. |
| `tests/requirements/postgres.txt` | Verified addition of `psycopg-pool>=3.2.0` and compared it with cached dependency versions. No unsupported minimum-version claim. |

Supporting source reads covered `django/db/backends/base/creation.py`,
`django/db/backends/postgresql/operations.py`,
`django/db/backends/postgresql/psycopg_any.py`, `django/db/utils.py`,
`django/db/transaction.py`, `django/test/signals.py`, `django/test/utils.py`,
`django/test/runner.py`, and existing backend health-check and creation tests.
The cached `psycopg_pool/pool.py` was inspected to understand worker admission,
return, and shutdown behavior. These are dependencies of the changed flows,
not an expansion into unrelated repository review.

## Measurements

The committed diff contains 325 additions and 44 deletions in eight files.
Line counts are physical `splitlines()` counts, including comments and blank
lines. `if` counts are Python AST `ast.If` nodes; conditional expressions and
exception branches are not counted, so these are not cyclomatic-complexity
scores. Function counts include nested functions and async functions.

| File | Base lines | Head lines | Base/head `if` nodes | Base/head functions |
| --- | ---: | ---: | ---: | ---: |
| Shared wrapper | 788 | 792 | 43 / 44 | 63 / 63 |
| PostgreSQL wrapper | 516 | 615 | 29 / 41 | 21 / 27 |
| PostgreSQL creation | 86 | 91 | 7 / 7 | 6 / 7 |
| PostgreSQL features | 149 | 163 | 2 / 3 | 7 / 8 |
| Database reference | 1248 | 1273 | — | — |
| Release notes | 469 | 472 | — | — |
| PostgreSQL tests | 439 | 569 | 2 / 2 | 30 / 39 |
| PostgreSQL requirements | 2 | 3 | — | — |

No code file crosses from below to above 1,000 lines. The already-long database
reference is a documentation collection, and the added section does not create
a source-module decomposition concern. The pool lifecycle is the relevant
maintainability regression: process-wide state is queried through an allocating
property from multiple wrapper lifecycle phases. Counts support inspection but
are not a standalone objection or a request to split files solely for size.

The metrics were produced with `git diff --name-only main...review-head`,
`git show main:<path>`, `pathlib.Path(path).read_text()`, and Python's `ast.parse()`
and `ast.walk()`. Raw measurements are retained in `evidence/metrics.json`.
The essential measurement algorithm was:

```python
paths = subprocess.check_output(
    ["git", "diff", "--name-only", "main...review-head"], text=True
).splitlines()
for path in paths:
    base = subprocess.check_output(["git", "show", "main:" + path], text=True)
    head = pathlib.Path(path).read_text()
    base_lines, head_lines = len(base.splitlines()), len(head.splitlines())
    if path.endswith(".py"):
        for source in (base, head):
            tree = ast.parse(source)
            if_nodes = sum(isinstance(node, ast.If) for node in ast.walk(tree))
            functions = sum(
                isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
                for node in ast.walk(tree)
            )
```

## Verification inventory

| Observation | Method | Status |
| --- | --- | --- |
| Role callback reaches wrapper acquisition | Real Django helper chain, mocked raw driver, acquisition sentinel | Verified offline |
| Initial pooled role setup times out | Real installed pool worker, fake physical driver, 0.1-second timeout | Verified offline |
| Identical fake driver works without role | Real installed pool worker, successful get/put control | Verified offline |
| Connected wrapper fails role callback on worker thread | Real wrapper cursor preparation/thread check, mocked raw cursor | Verified offline |
| Timezone adjustment allocates an unused pool only to close it | Real wrapper method, recording fake pool constructor | Verified offline |
| Return after invalidation allocates a new registry pool | Real wrapper close/invalidation, recording fake owner | Verified offline |
| Concurrent close deletion raises `KeyError` | Real `close_pool()`, barrier in fake pool close | Verified offline |
| Test-name switch retains application-target pool | Real `create_test_db()` orchestration, database/command I/O mocked | Verified offline |
| `{}` silently disables pooling | Real pool property, constructor recording | Verified offline |
| psycopg2 policy raises instead of ignoring | Real parameter method, `is_psycopg3=False` branch simulation plus committed test/source | Verified offline; actual psycopg2 driver not run |
| Closed pooled connection cannot reopen inside atomic | Actual `atomic()` enter/close/exit using fake pooled physical connection | Verified offline |
| `keepdb` restoration misses private drop cleanup | Source trace of base destruction and new override | Static verification only |
| Live PostgreSQL role SQL, migrations, cloning, health checks | No PostgreSQL server available | Not executed |
| Pool dependency's minimum supported version | Cache has a newer version and fetching is disallowed | Not executed |

The probes are observations of deliberately triggered scenarios. Their process
exiting 0 means those observations were obtained successfully; it is not a
passing behavioral regression suite for the proposed remedies. The successful
no-role control establishes that the callback timeout is not caused by an
unusable fake driver or a missing server. The sentinel and thread probes provide
separate evidence that the role path crosses the wrapper boundary.

The new base-wrapper guard is justified by a changed lifecycle invariant. A
pooled `_close()` releases the physical connection and clears `self.connection`
inside an atomic block. Without the guard, `ensure_connection()` would call
`connect()`, which resets transaction flags and opens another connection before
the original block exits. The probe obtained the intended `ProgrammingError`
and confirmed that outermost atomic exit restored `in_atomic_block=False`.
Keeping this invariant in the shared wrapper is appropriate; it is not a
PostgreSQL-option check scattered into a generic layer.

## Existing test coverage and proposed acceptance work

The new capacity test exercises pool saturation and reuse. Other additions cover
`True`, timezone selection, health-check callback wiring, forbidden persistent
connections, the generic guard, and psycopg2 rejection. These are useful tests,
but they do not exercise `assume_role` in a pool, pool invalidation with an
outstanding lease, or a preexisting pool during test-name switching.

The helper `no_pool_connection()` disables pooling in eleven existing tests,
including role, isolation, timezone, encoding, and custom-cursor checks. Some
of those tests intentionally require direct independent configurations and
should keep that setup. They do not establish that the corresponding settings
work in a pooled connection. In particular, the altered role test cannot catch
F1. The helper also repeats `copy.deepcopy()` after `connection.copy()` already
deep-copies settings; that small redundancy does not warrant another actionable
finding alongside the ownership defects.

After the structural fixes, targeted acceptance work should exercise a real
pooled role and quoted role name, a callback with no wrapper access, return of
an old-generation lease, idempotent concurrent invalidation, target switching
before migrations and after `keepdb`, mirrors/workers, and `{}` activation.
Coverage should check behavior at these boundaries instead of only private
`pool._check` wiring. Direct-path timezone commit and autocommit expectations
must remain intact. These are validation requirements for the existing findings,
not separate requests to redesign the test suite.

## Reproducibility and checkout integrity

The complete probe source is retained in `evidence/review_probes.py` and also at
`../review_probes.py`. The exact executed command was:

```sh
PYTHONDONTWRITEBYTECODE=1 \
PYTHONPATH=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-002/clone \
/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-002/clone-cache/venv/bin/python \
/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high-selected/att-002/clone-work/review_probes.py
```

The dependency-version check used the cached Python to import `psycopg` and
`psycopg_pool`, print their `__version__` values, and locate `ConnectionPool`
with `inspect.getfile()`. It observed psycopg 3.3.6 and psycopg-pool 3.3.3 under
the supplied virtual environment's Python 3.10 `site-packages`.

Initial and subsequent `git status --porcelain` output was empty. The commands
`git rev-parse HEAD main HEAD^{tree}` confirmed the pinned revisions and tree,
and `git diff --check main...review-head` exited 0. The final integrity check
is retained in `evidence/integrity.txt`. No remedies were applied, and no tests
requiring an unavailable PostgreSQL server were attempted.
