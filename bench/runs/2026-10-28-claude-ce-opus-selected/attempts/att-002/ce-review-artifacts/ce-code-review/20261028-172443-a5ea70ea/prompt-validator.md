You are the independent validation gate for the code-review findings below. Evaluate every finding separately under fresh inspection and under the shared protected-subject policy below. Outside a protected subject, false positives are common; reject a finding when the cited code does not prove it, it predates and is unaffected by this diff, surrounding code handles it, or it is only an unsupported preference. Inside a protected subject, a rejection requires one of the evidence forms the policy names.

Do not let one finding's outcome influence another. Do not invent new findings.

<findings-to-validate>
[
 {
  "#": 1,
  "severity": "P0",
  "confidence": 75,
  "title": "Cascade: _nodb_cursor fallback registers 'default' pool on non-test DB",
  "file": "django/db/backends/postgresql/base.py",
  "line": 534,
  "autofix_class": "manual",
  "owner": "downstream-resolver",
  "requires_verification": true,
  "pre_existing": false,
  "reviewers": [
   "adversarial"
  ],
  "independent_reviewers": [
   "adversarial"
  ],
  "first_evidence": "django/db/backends/postgresql/base.py:529-535 -- conn = self.__class__({**self.settings_dict, \"NAME\": connection.settings_dict[\"NAME\"]}, alias=self.alias)",
  "why_it_matters": "If 'postgres' is unreachable (the managed-hosting case the fallback exists for), the test run migrates, runs tests and flushes the real configured database instead of test_<NAME>. TransactionTestCase truncates its tables, so the data is lost. The fallback builds a wrapper with alias=self.alias ('default'), the real NAME and the inherited OPTIONS['pool']. Its first cursor creates and opens the class-level pool for 'default' with dbname=<real NAME>. conn.close() only returns the connection, so the pool stays registered. create_test_db then sets settings_dict['NAME'] to the test name, but the pool property only builds a pool when the alias is missing, so migrate and every test connection reuse the pool aimed at the real DB. In _clone_test_db the same fallback re-registers a pool on the template DB right after close_pool(); its background min_size connections can make CREATE DATABASE ... TEMPLATE fail with 'source database is being accessed by other users'.",
  "evidence": [
   "django/db/backends/postgresql/base.py:529-535 -- conn = self.__class__({**self.settings_dict, \"NAME\": connection.settings_dict[\"NAME\"]}, alias=self.alias)",
   "django/db/backends/postgresql/base.py:208 -- if self.alias not in self._connection_pools:  (pool only built once per alias; later NAME changes ignored)",
   "django/db/backends/postgresql/base.py:239 -- self._connection_pools.setdefault(self.alias, pool)",
   "django/db/backends/base/creation.py:33-37 -- self._create_test_db(...); self.connection.close(); settings.DATABASES[alias][\"NAME\"] = test_database_name; self.connection.settings_dict[\"NAME\"] = test_database_name  (no close_pool between fallback and migrate)",
   "Offline repro (tmp/adv/repro.py): fallback-style DatabaseWrapper(alias='default', NAME='prod_db').pool -> kwargs dbname 'prod_db'; after main.settings_dict['NAME']='test_prod_db', main.pool is the same pool and still dbname 'prod_db'",
   "django/db/backends/postgresql/creation.py:60-69 -- close(); close_pool(); then with self._nodb_cursor() -> fallback re-registers a 'default' pool on the template DB"
  ],
  "suggested_fix": "In DatabaseWrapper._nodb_cursor's fallback, strip pooling from the temporary wrapper's settings, matching how the base _nodb_cursor avoids pooling via NO_DB_ALIAS: `conn = self.__class__({**self.settings_dict, \"NAME\": connection.settings_dict[\"NAME\"], \"OPTIONS\": {k: v for k, v in self.settings_dict[\"OPTIONS\"].items() if k != \"pool\"}}, alias=self.alias)`. As defence in depth, also call self.connection.close_pool() in create_test_db after NAME is switched (or key _connection_pools by (alias, dbname))."
 },
 {
  "#": 2,
  "severity": "P1",
  "confidence": 100,
  "title": "assume_role with pool: configure callback re-enters the wrapper from a pool thread",
  "file": "django/db/backends/postgresql/base.py",
  "line": 101,
  "autofix_class": "manual",
  "owner": "downstream-resolver",
  "requires_verification": true,
  "pre_existing": false,
  "reviewers": [
   "correctness",
   "adversarial"
  ],
  "independent_reviewers": [
   "correctness",
   "adversarial"
  ],
  "first_evidence": "django/db/backends/postgresql/base.py:101 -- sql = ops.compose_sql(\"SET ROLE %s\", [role_name])",
  "why_it_matters": "With OPTIONS {'pool': ..., 'assume_role': ...} no pooled connection can be created, so every request hangs for the pool timeout (30s by default) and then fails with PoolTimeout. psycopg_pool runs the configure callback in its own worker threads. ensure_role calls ops.compose_sql(), which goes through mogrify(sql, params, self.connection), i.e. the Django wrapper that created the pool. If that wrapper is connected, validate_thread_sharing raises DatabaseError. If it is not connected, the worker calls wrapper.connect(), which calls pool.getconn() from inside the pool's own worker and deadlocks. The pool treats any Exception as a connection failure and keeps retrying with backoff. This breaks the in-code rule 'does not access anything on self aside from variables'. Building the statement from the raw psycopg connection removes the dependency on the wrapper and its thread.",
  "evidence": [
   "django/db/backends/postgresql/base.py:101 -- sql = ops.compose_sql(\"SET ROLE %s\", [role_name])",
   "django/db/backends/postgresql/operations.py:193 -- return mogrify(sql, params, self.connection)",
   "django/db/backends/postgresql/psycopg_any.py:21 -- with connection.cursor() as cursor:  (Django wrapper cursor -> ensure_connection -> connect -> pool.getconn)",
   "psycopg_pool/pool.py:634-635 -- if self._configure: self._configure(conn)  (inside _connect, run by AddConnection worker tasks)",
   "django/db/backends/postgresql/base.py:231 -- configure=self._configure_connection,",
   "Offline repro (tmp/corr/role.py): invoking wrapper._configure_connection(fake_raw_conn) from a separate thread with assume_role set re-entered pool.getconn from that thread and failed: 'OperationalError couldn't get a connection after 0.50 sec'",
   "django/db/backends/postgresql/operations.py:192-193 -- def compose_sql(self, sql, params): return mogrify(sql, params, self.connection)  (self.connection is the Django DatabaseWrapper)",
   "django/db/backends/postgresql/psycopg_any.py:20-22 -- def mogrify(sql, params, connection): with connection.cursor() as cursor: ...  (Django wrapper .cursor() -> validate_thread_sharing / ensure_connection)",
   "django/db/backends/postgresql/base.py:231 -- configure=self._configure_connection (bound to the wrapper instance that first built the pool)",
   "psycopg_pool/pool.py:634-635 (_connect, invoked from AddConnection worker tasks) -- if self._configure: self._configure(conn); psycopg_pool/pool.py:35 CLIENT_EXCEPTIONS = Exception -> _add_connection logs and reschedules, client getconn() waits until PoolTimeout",
   "Offline repro (tmp/adv/repro.py): invoking pool._configure from a second thread with assume_role set -> django.db.utils.DatabaseError: DatabaseWrapper objects created in a thread can only be used in that same thread.",
   "tests/backends/postgresql/tests.py test_connect_role uses no_pool_connection(), so the pool+assume_role combination is never exercised"
  ],
  "suggested_fix": "In ensure_role, build the SQL without going through the Django wrapper. For example (pool is psycopg3-only, but ensure_role is shared): if is_psycopg3, run `cursor.execute(sql.SQL(\"SET ROLE {}\").format(sql.Literal(role_name)))` using psycopg.sql, or `ClientCursor(connection).mogrify(\"SET ROLE %s\", [role_name])` on the raw connection; for psycopg2 keep `cursor.mogrify(...)` on the raw cursor. Add a pooled assume_role test (tests currently use no_pool_connection() for test_connect_role)."
 },
 {
  "#": 3,
  "severity": "P1",
  "confidence": 75,
  "title": "Alias-keyed pool survives create_test_db NAME switch",
  "file": "django/db/backends/postgresql/base.py",
  "line": 239,
  "autofix_class": "gated_auto",
  "owner": "downstream-resolver",
  "requires_verification": true,
  "pre_existing": false,
  "reviewers": [
   "correctness",
   "reliability"
  ],
  "independent_reviewers": [
   "correctness",
   "reliability"
  ],
  "first_evidence": "django/db/backends/postgresql/base.py:239 -- self._connection_pools.setdefault(self.alias, pool)  (pool keyed only by alias; kwargs frozen at creation)",
  "why_it_matters": "If anything opens the pooled alias before the test runner creates the test database (an AppConfig.ready() query, or a query at import time in a module the test suite imports), the per-alias pool is built with the real database's connection kwargs. create_test_db() then closes the connection and rewrites settings_dict['NAME'], but the pool is still in the class-level _connection_pools dict, so every later connect, including migrate, serialization, the tests themselves and TransactionTestCase flushes, goes back to the original (possibly production) database. Without a pool, close() plus the NAME rewrite was enough to retarget the alias. The diff already closes the pool for the same reason in _clone_test_db, _destroy_test_db and ensure_timezone, but not on the create path, the mirror path or setup_worker_connection.",
  "evidence": [
   "django/db/backends/postgresql/base.py:239 -- self._connection_pools.setdefault(self.alias, pool)  (pool keyed only by alias; kwargs frozen at creation)",
   "django/db/backends/postgresql/base.py (_nodb_cursor fallback) -- conn = self.__class__({**self.settings_dict, \"NAME\": connection.settings_dict[\"NAME\"]}, alias=self.alias)",
   "django/db/backends/base/creation.py:64-66 -- self.connection.close(); settings.DATABASES[...][\"NAME\"] = test_database_name; self.connection.settings_dict[\"NAME\"] = test_database_name  (no close_pool)",
   "Offline repro (tmp/corr/stale.py): after fallback-style wrapper created the pool and NAME was renamed, main.get_connection_params dbname=test_prod_db but main.pool is the same pool with kwargs dbname=prod_db",
   "django/db/backends/postgresql/base.py:224 -- connect_kwargs = self.get_connection_params()",
   "django/db/backends/postgresql/base.py:239 -- self._connection_pools.setdefault(self.alias, pool)",
   "django/db/backends/base/creation.py:64-66 -- self.connection.close(); settings.DATABASES[...]['NAME'] = test_database_name; self.connection.settings_dict['NAME'] = test_database_name (no close_pool)",
   "Offline simulation (psycopg_pool 3.3.3): after emulating create_test_db's close()+NAME switch, get_connection_params()['dbname'] == 'test_prod_db' but c.pool.kwargs['dbname'] == 'prod_db' and c.pool is the same object"
  ],
  "suggested_fix": "In django/db/backends/postgresql/creation.py, close the alias pool wherever settings_dict['NAME'] is rewritten: override _create_test_db() as `try: return super()._create_test_db(verbosity, autoclobber, keepdb) finally: self.connection.close_pool()` so the pool is gone before base create_test_db switches NAME, and do the same in set_as_test_mirror() and setup_worker_connection(), following the close_pool() pattern the diff already uses in _clone_test_db/_destroy_test_db. For extra safety, recreate the pool when get_connection_params()['dbname'] differs from pool.kwargs['dbname'] (or key _connection_pools by alias plus dbname). Add a test that builds the pool under one NAME, switches NAME, and asserts the next connection targets the new database."
 },
 {
  "#": 4,
  "severity": "P1",
  "confidence": 75,
  "title": "Threads exiting without close() permanently leak pool slots",
  "file": "django/db/backends/postgresql/base.py",
  "line": 348,
  "autofix_class": "manual",
  "owner": "downstream-resolver",
  "requires_verification": true,
  "pre_existing": false,
  "reviewers": [
   "reliability"
  ],
  "independent_reviewers": [
   "reliability"
  ],
  "first_evidence": "django/db/backends/postgresql/base.py:348 -- connection = self.pool.getconn()",
  "why_it_matters": "With pooling on, a borrowed connection only goes back to the pool when DatabaseWrapper._close() calls putconn. A worker thread (a threading.Thread, an executor job, a management-command helper) that uses the ORM and exits without connection.close() drops its thread-local wrapper, and the psycopg connection is garbage collected without any warning. psycopg_pool does not track checked-out connections and only decrements _nconns on putconn/close, so that slot is lost for good. With 'pool': True the defaults are min_size=4 and max_size=min_size, so four such threads are enough: after that every connection attempt in the process blocks for the 30 s pool timeout and then raises PoolTimeout. Before this change, the same leaked connection was simply closed when it was garbage collected.",
  "evidence": [
   "django/db/backends/postgresql/base.py:348 -- connection = self.pool.getconn()",
   "django/db/backends/postgresql/base.py:395 -- self.connection._pool.putconn(self.connection)  (only return path)",
   "psycopg/_connection_base.py:150 -- if hasattr(self, \"_pool\"): return  (pooled connection GC'd silently)",
   "psycopg_pool/pool.py:_getconn_unchecked -- conn._pool = self; return conn  (pool keeps no reference to checked-out connections; _nconns only decremented on close/shrink/failed reconnect)",
   "django/db/utils.py:145 -- thread_critical = True (connections are threading.local, discarded on thread exit)"
  ],
  "suggested_fix": "Return orphaned connections to the pool. In get_new_connection(), after self.pool.getconn(), register weakref.finalize(self, connection._pool.putconn, connection) and keep the finalizer on the wrapper; call finalizer.detach() in _close() before the explicit putconn. Also add a note to the 'Connection pool' docs section that connections opened outside the request cycle must be closed (connections.close_all()) or they keep a pool slot. Assumption: putconn is thread-safe from a finalizer (psycopg_pool takes its own lock)."
 },
 {
  "#": 5,
  "severity": "P2",
  "confidence": 100,
  "title": "Docs say pool option is ignored with psycopg2; code raises ImproperlyConfigured",
  "file": "docs/ref/databases.txt",
  "line": 271,
  "autofix_class": "gated_auto",
  "owner": "downstream-resolver",
  "requires_verification": false,
  "pre_existing": false,
  "reviewers": [
   "correctness",
   "testing",
   "adversarial",
   "fast-pass"
  ],
  "independent_reviewers": [
   "correctness",
   "testing",
   "adversarial"
  ],
  "first_evidence": "docs/ref/databases.txt:271 -- and is ignored with ``psycopg2``.",
  "why_it_matters": "A project that shares settings between psycopg2 and psycopg3 environments and trusts the docs will not get a silently ignored option. On psycopg2 the first connection fails with ImproperlyConfigured. Other places that touch the pool property (close(), ensure_timezone() via setting_changed, close_if_health_check_failed()) can fail the same way, before any query runs. The new test is named test_connect_pool_setting_ignored_for_psycopg2 but asserts the raise, so the code and docs contradict each other.",
  "evidence": [
   "docs/ref/databases.txt:271 -- and is ignored with ``psycopg2``.",
   "django/db/backends/postgresql/base.py:291-292 -- if pool_options and not is_psycopg3: raise ImproperlyConfigured(\"Database pooling requires psycopg >= 3\")",
   "tests/backends/postgresql/tests.py -- def test_connect_pool_setting_ignored_for_psycopg2 ... with self.assertRaisesMessage(ImproperlyConfigured, msg): new_connection.connect()",
   "django/db/backends/postgresql/base.py:292 -- raise ImproperlyConfigured(\"Database pooling requires psycopg >= 3\")",
   "tests/backends/postgresql/tests.py:349 -- def test_connect_pool_setting_ignored_for_psycopg2(self):  (asserts ImproperlyConfigured)"
  ],
  "suggested_fix": "Assuming raising is the intended behaviour (it is what the test asserts), change the docs sentence to: \"This option requires ``psycopg[pool]`` or :pypi:`psycopg-pool` to be installed and is not supported with ``psycopg2`` (``ImproperlyConfigured`` is raised).\" Rename the test to test_connect_pool_setting_not_supported_for_psycopg2."
 }
]
</findings-to-validate>

<diff>
Staged diff file (Read it): /home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-002/clone-work/ce-review-artifacts/ce-code-review/20261028-172443-a5ea70ea/full.diff
</diff>

<scope-context>
{
 "scope_mode": "standalone",
 "base": "bcccea3ef31c777b73cba41a6255cd866bf87237",
 "diff_a": "bcccea3ef31c777b73cba41a6255cd866bf87237",
 "diff_b": null,
 "head_sha": "fad334e1a9b54ea1acb8cce02a25934c5acfe99f",
 "branch": "review-head",
 "tree_is_reviewed_head": true,
 "checkout": "/home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-002/clone",
 "checkout_read_only": true,
 "remote_refs": null,
 "diff": "/home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-002/clone-work/ce-review-artifacts/ce-code-review/20261028-172443-a5ea70ea/full.diff",
 "files": "/home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-002/clone-work/ce-review-artifacts/ce-code-review/20261028-172443-a5ea70ea/files.txt",
 "pr": {
  "number": 17914,
  "url": "https://github.com/django/django/pull/17914",
  "title": "Refs #33497 -- Added connection pool support for PostgreSQL."
 },
 "environment_limits": [
  "No PostgreSQL server; live database tests are unavailable.",
  "No network.",
  "Scratch writes only under /home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-002/tmp; set PYTHONDONTWRITEBYTECODE=1 when running Python against the checkout."
 ]
}
Reviewed checkout: /home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-002/clone (working tree == reviewed head fad334e1a9; base bcccea3ef3). The checkout is READ-ONLY: never write anything inside it (use PYTHONDONTWRITEBYTECODE=1). Offline checks: PYTHONDONTWRITEBYTECODE=1 PYTHONPATH=<clone> /home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-002/clone-cache/venv/bin/python (psycopg + psycopg_pool installed); scratch under /home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-002/tmp only. No PostgreSQL server; no network; five-minute limit per command. Do not spawn subagents.
</scope-context>

<protected-subject-policy>
Return status "confirmed", "rejected", or "unresolved". Never use lack of disproof as evidence of confirmation.

Set protected_subject to the best-fitting key below, or JSON null when none applies:
- memory-safety: allocation sizes, buffer lengths, index bounds, use-after-free, invalid memory access, or null dereferences.
- concurrency: locks, atomics, data races, ordering, or synchronization whose failure can affect observable behavior.
- data-loss: destructive writes, deletes, truncation, overwrite-in-place, or irreversible migrations and backfills.
- authorization-authentication: identity, permissions, ownership, session/token handling, or privilege boundaries.
- injection: attacker-influenced or untrusted data that can alter SQL, commands, templates, paths, or markup across a trust boundary, including stored input. Text assembly alone is not proof.
- public-contract: an evidenced compatibility concern involving an externally consumed response field, status code, error path, default, message, or published signature. An internal export or intentional contract change alone is not a defect.
- secrets-exposure: hardcoded credentials, API keys, tokens, or private keys in source or configuration; credentials, session tokens, or personal data written to logs, error messages, URLs, or responses; secrets committed to a repository or shipped in a built artifact.
- cryptography: weak or broken algorithms and modes, a fast general-purpose hash used for passwords, static or predictable keys, salts, or IVs, disabled certificate or signature verification, insufficient randomness, or a misused primitive whose failure breaks a security guarantee.

For every subject, confirm only when inspected evidence establishes the issue, the diff introduces or newly exposes it, and surrounding code or applicable runtime guarantees do not prevent it.

A finding that is real in the code may still describe a state that never occurs. For every finding, name the precondition the defect needs (the input, data shape, or ordering) and say what would show it occurs or is reachable: a test, a query against available data, a caller that produces it. When that evidence is in reach with the budget, obtain it; when it is not, confirm on the code alone and state in `reason` that incidence was not measured. Unmeasured incidence does not lower confidence or block confirmation; it is what the reader needs to weigh the severity.

Treat a finding as protected when the actual failure it alleges falls within a subject above. Read its category, title, and body together; keywords only prompt inspection and never establish protection. A naming preference about a token helper is not a token-handling defect. Your classification cannot remove protection established by the claim; the consumer applies this test independently.

On a protected subject, reject only by citing specific evidence that refutes the claim or establishes that it is unrelated pre-existing behavior: quote the file and line number that refutes it, name the version-specific or configuration-specific documentation and the version in force, give short-hash provenance, or cite a discriminating test result. Test evidence must identify the reviewed revision, engine/runtime version, configuration, exercised trigger, assertion, and observed result, and explain why it disproves the exact claim. A general passing suite or a test that did not exercise the alleged trigger is not disproof. An assumed framework guarantee is not evidence. Without one of these evidence forms, return status "unresolved", not "rejected". Inspect existing test evidence or use a read-only reproduction within your authority; do not mutate files or application state to obtain it.

If a protected claim remains uncertain, return status "unresolved" and state the missing evidence. Low confidence alone does not justify rejecting or confirming it.

Outside protected subjects, keep the ordinary conservative evidence bar: after inspection, reject an unsupported claim and explain why. Missing required inspection is different from inspected-but-unsupported evidence. If the cited file or required context cannot be accessed, return status "unresolved" for any subject, state the access limit, and do not guess.

Classify the claim itself, not its title. Never raise severity or confidence to preserve it. Do not invent findings or propose that uncertainty is a confirmed defect.
</protected-subject-policy>

For local-aligned scope, inspect the cited files, callers, guards, project contracts, and targeted history with read-only tools. For pr-remote or branch-remote scope, use the provided diff and reviewed head ref, never the unrelated workspace copy.

Budget: the batch has 15 minutes of wall clock and about five tool calls per finding. Inspect findings in the order given. When the budget runs out, stop inspecting and give every remaining finding `"status": "unresolved"` with the reason `budget exhausted, uninspected`; never guess a verdict you did not inspect.

Write the JSON object below to `/home/jack/rq-gap/2026-10-28-claude-ce-opus-selected/att-002/clone-work/ce-review-artifacts/ce-code-review/20261028-172443-a5ea70ea/validator-verdicts.json` before you return, then return the same object:
{
  "verdicts": [
    {
      "#": <input stable number>,
      "status": "confirmed" | "rejected" | "unresolved",
      "protected_subject": "<one of the eight policy keys>" | null,
      "reason": "<one sentence grounded in inspected evidence, or naming the evidence you could not obtain>"
    }
  ]
}

Each entry carries exactly those four fields. Return one verdict for every input # exactly once; unknown, duplicate, or missing numbers and invalid status or subject values are malformed output. Do not emit the legacy `validated` boolean. No prose outside JSON. Writing the verdicts file above is the one permitted write; do not edit project files, commit, push, or otherwise mutate the checkout.