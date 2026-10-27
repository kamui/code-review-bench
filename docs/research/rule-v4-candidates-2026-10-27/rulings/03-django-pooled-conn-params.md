# Ruling 03-django-pooled-conn-params: v-django-17914

Decisions in this question: `M2` (candidate); `M2` (label).

## `M2`: candidate

Record: [`03-django-pooled-conn-params.M2.before.json`](03-django-pooled-conn-params.M2.before.json)

```text
ASK
- recommender: problem stays with the owner
- assessor-1: problem stays with the owner
- assessor-2: problem stays with the owner
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/v-django-17914/cases/M2.md:

# Pooled connection setup silently ignores parameters selected by a public backend override

The reviewed head is `fad334e1a9b54ea1acb8cce02a25934c5acfe99f`; base is `bcccea3ef31c777b73cba41a6255cd866bf87237`. The last-push cut-off is 2024-03-01T08:02:02Z (*read*, `upstream/pr-timeline.json`). The task packet displays the later merge time; it does not move the cut-off. Its description is absent. The current body is empty and GraphQL reports zero edits (*read*, `upstream/description-edits.json`). All source and documentation facts below are available at the cut-off unless explicitly marked later. The saved executions use only those source revisions. They were made now and test behavior knowable from that source.

## Changed code

*read*: The new pool property constructs kwargs from another get_connection_params call at PostgreSQL base.py:224-234. The pooled get_new_connection branch at 345-348 never consumes its conn_params argument. The direct branch still expands that argument into Database.connect.

The packet title is “Refs #33497 -- Added connection pool support for PostgreSQL.” There is no description text.

```diff
+            connect_kwargs = self.get_connection_params()
+            pool = ConnectionPool(
+                kwargs=connect_kwargs,
...
+            connection = self.pool.getconn()
         else:
             connection = self.Database.connect(**conn_params)
```

Source: `upstream/change.diff`.

## Executed cases

*run*: A DatabaseWrapper subclass sets conn_params["dbname"] to template1 and application_name to subclass_selected immediately before super().get_new_connection. Base and direct head queries return template1/subclass_selected. The pooled head returns postgres/settings_selected. A deterministic get_connection_params override works at both commits, including a head pool. No live settings dictionary is changed by this subclass.

The executions use Python 3.10.12, psycopg 3.1.18, psycopg_pool 3.2.0 and PostgreSQL 16.15. Pool timeouts were shortened to 0.30 seconds. Full outputs, probes and versions: `probes/M2/`. Base has no built-in pool: its comparison is the prior direct connection.

Not run: A third-party application using this exact get_new_connection override; the live minimal subclass is the demonstrated consumer. A production-ready policy for per-call parameter changes after physical sessions already exist.

## Interface records available by the cut-off

> Django comes with built-in database backends. You may subclass an existing database backends to modify its behavior, features, or configuration.

*read*, `upstream/head-docs_ref_databases.txt`, available by the cut-off.

> if any method starts with a single ``_``, it's an internal API.

*read*, `upstream/head-docs_misc_api-stability.txt`, available by the cut-off.

> A `ConnectionPool` object can be used to request connections from multiple concurrent threads.

*read*, `upstream/psycopg-pool-doc.rst`, available by the cut-off.

> to ``True`` to use the ``ConnectionPool`` defaults

*read*, `upstream/head-docs_ref_databases.txt`, available by the cut-off.

*read*: the pool property’s comment says setdefault prevents multiple threads setting the registry in parallel. The new ensure_timezone comment reads "Close the pool so new connections pick up the correct timezone." The change adds test_connect_pool_with_timezone and a finite-capacity acquisition test. Neither test runs concurrent close, per-call get_new_connection parameter edits, or a departed creator’s cached timezone.

*read*: the dependency’s pool constructor accepts kwargs as physical-connect parameters and configure as a callback after physical creation. Its historical close documentation fails waiting and future clients with PoolClosed. These pages and the versioned method source are archived under upstream/.

*read*: GitHub searches and the two original PR discussions were fetched. Django Trac query endpoints returned HTTP 403. Direct retrieval of tickets 33497, 27751 and 35688 succeeded. Public code search samples and their commit dates are archived; counts do not measure frequency.

## Other code paths and patch results

*read/run*: GT-v5 concerns a subclass timezone override skipped by base.py:376. GT-v11 concerns the role override skipped by 380-381 and the removed ensure_role method. GT-v2 concerns the pool registry at 208-239 and _close at 385-399 retaining earlier database parameters across test setup. The parameter patch restores template1/subclass_selected and leaves each other failure. The timezone-only patch restores Asia/Tokyo in its subclass control and leaves postgres/settings_selected. The role-only patch restores the role hook call and leaves postgres/settings_selected. The fallback-only patch restores selected test-database routing and cloning and leaves postgres/settings_selected. Each pair of patches restores both tested behaviors. No QuestDB installation was used in the timezone-dispatch control.

Patches and outputs: `probes/M2/grouping/`. The patches are controlled experiments, not a submitted implementation.

## Impact facts

*run/read*: Backend authors who edit parameters immediately before delegating get_new_connection can silently send queries to a different database or lose their connection options. The probe performs read-only identity queries and demonstrates no damaged data. Disabling pooling or moving deterministic customization into get_connection_params avoids this case. No exact external instance of the per-call trigger was found in the bounded code sample; prevalence is unknown.

No frequency estimate is established by the sampled search hits.

## Known only later

*read*: Django 5.1 was released on 2024-08-07; the release tag retains these pool integration paths. In August 2024, PR 18498 discussed the pool retaining a creator wrapper and restored separate timezone/role hooks. PR 19442 in 2025 explained return-on-close; PR 21795 in 2026 clarified pool configuration. These are after the cut-off.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | problem | medium (no-precedent) | A documented backend subclass that edits conn_params in get_new_connection reaches its chosen database without pooling and silently reaches the settings database with pooling on. |
| assessor-1 | problem | high | Django's documented public-backend subclassing contract covers the get_new_connection override, but pooled setup demonstrably ignores its selected database and connection options through a fault independently fixable from GT-v5, GT-v11 and GT-v2. |
| assessor-2 | problem | high | Django documents overriding nonprivate backend methods, but the pooled branch ignores the public override's connection parameters and demonstrably connects to postgres instead of template1. |

To reconcile before asking:

- None: every party's pick is the recommender's.

## `M2`: label

Record: [`03-django-pooled-conn-params.M2-label.before.json`](03-django-pooled-conn-params.M2-label.before.json)

```text
ASK
- band stays on its existing owner or reconciler route
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/v-django-17914/cards/M2.md:

# Pooled connection setup silently ignores parameters selected by a public backend override

Django supplies a PostgreSQL database backend. Base is the commit before the change; head is the reviewed commit.

Attribution: introduced when the new pool is enabled.

## Executed cases

*run*: A DatabaseWrapper subclass sets conn_params["dbname"] to template1 and application_name to subclass_selected immediately before super().get_new_connection. Base and direct head queries return template1/subclass_selected. The pooled head returns postgres/settings_selected. A deterministic get_connection_params override works at both commits, including a head pool. No live settings dictionary is changed by this subclass.

Python 3.10.12, psycopg 3.1.18, psycopg_pool 3.2.0, PostgreSQL 16.15. Timings arrange a race or limit waiting; they are not a performance measurement. Base uses direct connections because it has no pool option.

## What the affected person sees

Backend authors who edit parameters immediately before delegating get_new_connection can silently send queries to a different database or lose their connection options. The probe performs read-only identity queries and demonstrates no damaged data. Disabling pooling or moving deterministic customization into get_connection_params avoids this case. No exact external instance of the per-call trigger was found in the bounded code sample; prevalence is unknown.

## Impact facts

GT-v5 concerns a subclass timezone override skipped by base.py:376. GT-v11 concerns the role override skipped by 380-381 and the removed ensure_role method. GT-v2 concerns the pool registry at 208-239 and _close at 385-399 retaining earlier database parameters across test setup. The parameter patch restores template1/subclass_selected and leaves each other failure. The timezone-only patch restores Asia/Tokyo in its subclass control and leaves postgres/settings_selected. The role-only patch restores the role hook call and leaves postgres/settings_selected. The fallback-only patch restores selected test-database routing and cloning and leaves postgres/settings_selected. Each pair of patches restores both tested behaviors. No QuestDB installation was used in the timezone-dispatch control.

The new pool property constructs kwargs from another get_connection_params call at PostgreSQL base.py:224-234. The pooled get_new_connection branch at 345-348 never consumes its conn_params argument. The direct branch still expands that argument into Database.connect.

## Evidence limits

A third-party application using this exact get_new_connection override; the live minimal subclass is the demonstrated consumer. A production-ready policy for per-call parameter changes after physical sessions already exist. No population frequency is measured. No production data was written by these probes.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | serious | medium (no-precedent) | Queries silently run against a different database from the one the subclass selected (S4). |
| assessor-1 | serious | medium (no-precedent) | Pooled queries silently use postgres/settings_selected instead of the template1/subclass_selected connection selected by the backend override. |

To reconcile before asking:

- None: every party's pick is the recommender's.

