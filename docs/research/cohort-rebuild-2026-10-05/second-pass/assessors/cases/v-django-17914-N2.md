## What goes wrong

Enabling Django's new PostgreSQL connection pool can fail on the first query when psycopg-pool 3.1.9 is already installed. A pool keeps database connections available for reuse. The exact operation is opening a connection with `OPTIONS['pool']=True`, using psycopg 3.1.18 and psycopg-pool 3.1.9. Django maintains the setup instructions and the call into the pool. The Psycopg project maintains the pool constructor and connection-check method.

The cut-off is 2024-03-02 at 14:49:22 UTC. Base is `bcccea3ef31c777b73cba41a6255cd866bf87237`; head is `fad334e1a9b54ea1acb8cce02a25934c5acfe99f`.

## What changed

The pool constructor in `django/db/backends/postgresql/base.py` adds:

```diff
+                check=ConnectionPool.check_connection if enable_checks else None,
```

The same change adds this to `tests/requirements/postgres.txt`:

```diff
+psycopg-pool>=3.2.0
```

Its new user installation paragraph in `docs/ref/databases.txt` says:

```diff
+This option requires ``psycopg[pool]`` or :pypi:`psycopg-pool` to be installed
+and is ignored with ``psycopg2``.
```

The constructor receives `check=None` even when health checks are off. Version 3.1.9 accepts no `check` keyword. With checks on, Django first tries to read `ConnectionPool.check_connection`, which that version also lacks. Both exist in 3.2.0. The import and construction code check no installed pool version.

Before the change, Django had no built-in pool option. The same direct connection worked, but supplying `pool=True` failed as an unknown connection option.

## What was run

These are saved executions, read for this case file. No new probe was run. The original and refresh probes used real PostgreSQL. The refresh used PostgreSQL 16.15, Python 3.10.12 and psycopg 3.1.18.

| Probe | Before the change | At head |
| --- | --- | --- |
| Direct query with pool package 3.1.9 | `(1,)` | `(1,)` |
| `pool=True`, package 3.1.9, health checks off | `ProgrammingError: invalid connection option "pool"` | `TypeError: ConnectionPool.__init__() got an unexpected keyword argument 'check'` |
| `pool=True`, package 3.1.9, health checks on | Same unknown-option error | `AttributeError: type object 'ConnectionPool' has no attribute 'check_connection'` |
| Upgrade only the pool package to 3.2.0, then direct and both pooled modes | Not run in this control | `(1,)` in all three modes |

The original N2 executions are also saved under N3. The refresh's N2 base and head results for 3.1.9 were copied unchanged into N3. N3's 3.2.0 head control was run separately.

Initial refresh attempts had `OperationalError: connection failed: Connection refused` for direct queries. Their pool errors were already as shown above. Later runs with PostgreSQL available supplied the successful direct-query controls. A Python 3.14 dependency setup attempt failed because driver 3.1.18 had no matching binary wheel. Sources: `../../candidates/v-django-17914/probes/N2/result-*.txt`, `probes/N2/refresh/result-*.txt`, the saved `initial-result-*.txt` files and `environment.txt`.

Not run: every pool release below 3.2, a particular user's upgrade or dependency-lock workflow, or a fresh package installation frozen at the historical cut-off. Package release dates and dependency metadata were read. No affected production deployment was inspected.

## Where a promise was looked for

- The project's documentation. Search: `rg -n 'psycopg-pool|psycopg\[pool\]|Connection pool|Stable APIs|APIs marked as internal' docs; read docs/ref/databases.txt and docs/misc/api-stability.txt`. Hits: 4; read: 4. The four hits are matching lines; all were read, along with the database guide and API-stability page. At head, before 2024-03-02, `docs/ref/databases.txt` says: "To use a connection pool with `psycopg`_, you can either set ``"pool"`` in the :setting:`OPTIONS` part of your database configuration in :setting:`DATABASES` to be a dict to be passed to :class:`~psycopg:psycopg_pool.ConnectionPool`, or to ``True`` to use the ``ConnectionPool`` defaults::". It then says "This option requires ``psycopg[pool]`` or :pypi:`psycopg-pool` to be installed". There is no pool-package version in that instruction. Line wrapping is collapsed in these quotations. Saved record: `../../candidates/v-django-17914/upstream/refresh-N2-docs-complete.json`.

- The owning dependency's documentation. Search: `gh api repos/psycopg/psycopg/contents/docs/api/pool.rst?ref=pool-3.1.9; gh api repos/psycopg/psycopg/contents/docs/advanced/pool.rst?ref=pool-3.2.0; read saved pool-3.2.0 docs/news_pool.rst and psycopg-3.1.18 setup.py`. Hits: 4; read: 4. Four records were read. Psycopg owns the `ConnectionPool` constructor, which Django links in its instructions. At tag `pool-3.2.0`, `docs/advanced/pool.rst` labels connection quality `.. versionadded:: 3.2`. Its `docs/news_pool.rst` says "Add `!check` parameter to the pool constructor and `~ConnectionPool.check_connection()` method. (:ticket:`#656`)." Line wrapping is collapsed. The 3.1.9 API documentation lacks that parameter and method. Psycopg 3.1.18's `setup.py` declares `"pool": ["psycopg-pool"]`, with no version restriction. Saved PyPI records date both pool releases to 2023-11-11 and driver 3.1.18 to 2024-02-04, all before the cut-off. Saved record: `../../candidates/v-django-17914/upstream/refresh-owner-docs-read.json`.

- The change's own words. Search: `git diff bcccea3ef31c777b73cba41a6255cd866bf87237 fad334e1a9b54ea1acb8cce02a25934c5acfe99f -- django/db/backends/postgresql/base.py docs/ref/databases.txt docs/releases/5.1.txt tests/backends/postgresql/tests.py tests/requirements/postgres.txt; gh api repos/django/django/pulls/17914`. Hits: 6; read: 6. Five changed files and the PR record were read. PR #17914 opened on 2024-02-28. Its title is "Refs #33497 -- Added connection pool support for PostgreSQL." Its description is empty. The diff adds the unconditional `check` keyword and `psycopg-pool>=3.2.0` to `tests/requirements/postgres.txt`. The installation paragraph has no version minimum. All these facts were available before the cut-off. Saved record: `../../candidates/v-django-17914/upstream/refresh-change-evidence.json`.

- What maintainers said before the cut-off. Search: `gh api -X GET search/issues -f q='repo:django/django "psycopg-pool" created:<=2024-03-02' -f per_page=10; gh api -X GET search/issues -f q='repo:django/django "pool" "3.2" created:<=2024-03-02' -f per_page=10; read saved pre-cutoff PR 17914 and 17594 discussion`. Hits: 9; read: 9. Both searches returned nine result occurrences in total; all nine descriptions and the saved original discussion were read. PR #16881 describes setting the pool option to true or a dictionary. On 2023-12-19, a comment in PR #17594 says "3.1.8 is the minimum supported version, IMO, we don't need to mention it here". Source: https://github.com/django/django/pull/17594#discussion_r1431081615. The comment concerns the psycopg driver, which has a separate version from psycopg-pool. The inspected records are silent on a user-facing pool-package minimum. These are GitHub searches, not a complete search of Django's separate Trac tracker. Saved record: `../../candidates/v-django-17914/upstream/refresh-N2-maintainers-read.json`.

- Public code. Search: `gh api -X GET search/code -f q='"django.db.backends.postgresql" "pool" filename:settings.py' -f per_page=5`. Hits: 3400; read: 5. Five complete snapshots and their dates were read; 3,395 results were not read. In `pythondigest/pythondigest`, `conf/settings.py` at commit `9b8ea82`, dated 2026-09-21, the code says `"pool": True`. This is after the cut-off. The other four inspected files concern other meanings of pool. None of these reads shows a pre-cut-off installation using Django pooling with package 3.1.9. An additional saved web search found a June 2024 demonstration, also after the cut-off. The saved web response has 12 rendered results. Its query string and full-page read count are not recorded in the supplement. No search was repeated for this case file. Saved record: `../../candidates/v-django-17914/upstream/refresh-pool-code-final.json`.

- The documented way to do the same thing. Search: `python3 <scratch>/v-django-17914/refresh-run.py`. Hits: 3; read: 3. The three probe configurations are direct queries, pooling with checks off, and pooling with checks on. The head's documented `pool=True` recipe fails in both pooled modes with package 3.1.9. Replacing only that package with 3.2.0 makes both modes return `(1,)`. Direct queries return `(1,)` with either package. The base rejects `pool=True`. The printed constructor signatures and exceptions appear in the saved result files. Saved record: `../../candidates/v-django-17914/probes/N2/refresh/result-head.txt`.

Django's new tests deliberately exercise pooling and use the test requirement `psycopg-pool>=3.2.0`. They do not exercise 3.1.9. The project's configuration accepts `True` or a dictionary for this option. The dependency's runtime signature in 3.1.9 lists explicit keyword parameters with no `check` and no catch-all keyword parameter. `check_connection` is absent. The 3.2.0 signature includes `check: Optional[ConnectionCB[CT]] = None`, and the method is present. Here `ConnectionCB` is the type for a function called with a connection. Psycopg 3.1.18's optional pool installation names the separately versioned package without a minimum. These code, signature and metadata facts were available before the cut-off.

## What the affected person sees

A developer enabling pooling in an environment retaining package 3.1.9 gets an exception before obtaining a pooled connection:

```text
TypeError: ConnectionPool.__init__() got an unexpected keyword argument 'check'
AttributeError: type object 'ConnectionPool' has no attribute 'check_connection'
```

The first occurs with health checks off. The second occurs with them on. Neither message names the required version or tells the developer to upgrade. Turning health checks off does not make pooling work. Upgrading the pool package to 3.2.0 does. Disabling pooling leaves direct database queries working. No data loss was shown.

The base already rejected the pool setting because it lacked this feature. An unconstrained fresh install could select 3.2.0, which was available before the cut-off; the probe did not replay that historical installation.

## What the change announced, and what maintainers did

Before the cut-off: PR #17914 has no description. Its title, added database instructions and Django 5.1 release note announce PostgreSQL pooling. The instructions link the pool constructor and show `pool=True`. They do not name a minimum pool-package version. The test requirement names 3.2.0 and the new tests use pooling. The 2023-12-19 driver-version comment is quoted above. The inspected discussions are silent on a separate pool minimum.

After the cut-off: Django 5.1 shipped on 2024-08-07. The saved current source still supplies `check`, and the saved current database guide still omits the pool minimum. Newer handling allows a custom check callback. PR #21795 merged on 2026-09-15 with "Clarified the docs added in #17914." It discusses persistent connections and health checks, not the minimum examined here. PR #21312, opened 2026-05-18, proposes optional dependency declarations and was unmerged when captured. The inspected later records contain no explicit acknowledgement or fix of these 3.1.9 errors. Sources: `upstream/pr-21795*.json`, `pr-21312*.json`, `current-postgresql.json`, `current-databases.json` and `release-5.1.json` under this target.

## Reference problems already on this pull request

`GT-v4`. The following three passages are verbatim from the packet.

Obligation:

> The documented supported driver configuration must match backend startup behavior.

Trigger:

> A deployment with psycopg2 sets a truthy pool option following guidance that the option is ignored.

Mechanism:

> A configuration described as harmless instead prevents the connection with ImproperlyConfigured.

Same lines: no. The older-pool failure reaches the pool constructor; GT-v4 reaches the separate psycopg2 driver check. One project fix: stating or enforcing the pool-package minimum does not correct the statement that psycopg2 ignores the pool setting. Causes: this case calls APIs absent from psycopg-pool 3.1.9; GT-v4 documents a driver setting as ignored while the backend raises an exception.

`GT-v7`. The following three passages are verbatim from the packet.

Obligation:

> A pool options dictionary, which the new documentation says is passed to ConnectionPool, must not make connecting fail with an unexplained TypeError for parameters that ConnectionPool accepts. Any design that honours these keys, or refuses them with a message naming the keys Django sets itself, satisfies this; the patch shape is not prescribed.

Trigger:

> psycopg 3 with OPTIONS['pool'] set to a dictionary that contains check, configure, open or kwargs; the first query on the alias reaches it. All four keys were run.

Mechanism:

> The pool property in django/db/backends/postgresql/base.py calls ConnectionPool(kwargs=..., open=False, configure=..., check=..., **pool_options), so a user key with one of those four names is a duplicate keyword argument and Python raises TypeError ("got multiple values for keyword argument"), which Django neither catches nor turns into a configuration error. Run at head for each key; a plain dictionary and a reset callback work. The commit before the change has no pool option and rejects it as an invalid connection option.

Same lines: partly. Both reach the `ConnectionPool(...)` call, including its `check` keyword. This case uses `pool=True`, so no user-supplied keyword is repeated. One project fix: merging or checking reserved options does not add the missing APIs to package 3.1.9; stating or checking a minimum does not resolve repeated options on a newer package. Causes: this case uses the 3.2 pool API without a user-facing minimum; GT-v7 supplies the same keyword from both Django and the user's options dictionary.

Candidate N3 covers the same package-version condition. Same lines: yes, both exceptions arise from the added `check=ConnectionPool.check_connection if enable_checks else None` line. One project correction can state or enforce the same minimum for both modes; upgrading the package to 3.2.0 makes both queries work. A version guard alone would explain the setup failure rather than make 3.1.9 pool successfully. Cause of each: Django supplies the 3.2 pool API while its user installation paragraph names no pool-package minimum. With checks off, Python rejects the keyword; with checks on, Python first fails to find the method.
