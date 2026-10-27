# Ruling 04-django-pooled-timezone: v-django-17914

Decisions in this question: `M3` (candidate); `M3` (label).

## `M3`: candidate

Record: [`04-django-pooled-timezone.M3.before.json`](04-django-pooled-timezone.M3.before.json)

```text
ASK
- recommender: problem stays with the owner
- assessor-1: problem stays with the owner
- assessor-2: problem stays with the owner
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/v-django-17914/cases/M3.md:

# A timezone override in a fresh thread leaves new physical pooled connections on the departed creator’s timezone

The reviewed head is `fad334e1a9b54ea1acb8cce02a25934c5acfe99f`; base is `bcccea3ef31c777b73cba41a6255cd866bf87237`. The last-push cut-off is 2024-03-01T08:02:02Z (*read*, `upstream/pr-timeline.json`). The task packet displays the later merge time; it does not move the cut-off. Its description is absent. The current body is empty and GraphQL reports zero edits (*read*, `upstream/description-edits.json`). All source and documentation facts below are available at the cut-off unless explicitly marked later. The saved executions use only those source revisions. They were made now and test behavior knowable from that source.

## Changed code

*read*: PostgreSQL base.py:231 binds the pool configure callback to its creating wrapper. That callback reads its cached timezone_name at 376. The unchanged setting_changed receiver, django/test/signals.py:71-82, resets only wrappers initialized in the signaling thread. Before this change there was no pool or bound creator surviving that thread.

The packet title is “Refs #33497 -- Added connection pool support for PostgreSQL.” There is no description text.

```diff
+                configure=self._configure_connection,
...
+        commit_tz = ensure_timezone(connection, self.ops, self.timezone_name)
```

Source: `upstream/change.diff`.

## Executed cases

*run*: A thread creates the pool in UTC, closes its connection and exits. The main thread has zero initialized wrappers. In override_settings(TIME_ZONE="Africa/Nairobi"), with USE_TZ=False, a replacement physical session has a new backend PID but remains UTC at head. Django expects Africa/Nairobi. The same timestamp formats to hour 00 instead of 03. Base opens Africa/Nairobi and returns 03. A current-thread initialized-wrapper control passes at head. Native psql timezone configuration and Python zoneinfo both produce the Nairobi result.

The executions use Python 3.10.12, psycopg 3.1.18, psycopg_pool 3.2.0 and PostgreSQL 16.15. Pool timeouts were shortened to 0.30 seconds. Full outputs, probes and versions: `probes/M3/`. Base has no built-in pool: its comparison is the prior direct connection.

Not run: An ORM write/read round trip or a real application’s timezone-sensitive assertion; the executed wrong SQL hour is the demonstrated consequence. A measurement of the prevalence of threaded timezone overrides.

## Interface records available by the cut-off

> USE_TZ, TIME_ZONE Databases timezone

*read*, `upstream/head-docs_topics_testing_tools.txt`, available by the cut-off.

> Django provides the :data:`django.test.signals.setting_changed` signal that lets you register callbacks to clean up and otherwise reset state when settings are changed.

*read*, `upstream/head-docs_topics_testing_tools.txt`, available by the cut-off.

> You shouldn't alter settings in your applications at runtime.

*read*, `upstream/head-docs_topics_settings.txt`, available by the cut-off.

*read*: the pool property’s comment says setdefault prevents multiple threads setting the registry in parallel. The new ensure_timezone comment reads "Close the pool so new connections pick up the correct timezone." The change adds test_connect_pool_with_timezone and a finite-capacity acquisition test. Neither test runs concurrent close, per-call get_new_connection parameter edits, or a departed creator’s cached timezone.

*read*: the dependency’s pool constructor accepts kwargs as physical-connect parameters and configure as a callback after physical creation. Its historical close documentation fails waiting and future clients with PoolClosed. These pages and the versioned method source are archived under upstream/.

*read*: GitHub searches and the two original PR discussions were fetched. Django Trac query endpoints returned HTTP 403. Direct retrieval of tickets 33497, 27751 and 35688 succeeded. Public code search samples and their commit dates are archived; counts do not measure frequency.

## Other code paths and patch results

*read/run*: GT-v10 describes a borrower changing session state and returning it, with no reset in _close at base.py:385-399 or new setup on checkout. A return-time reset restores the borrower-state control to UTC and leaves the fresh-session override at UTC. The signal-only patch restores Africa/Nairobi and leaves the borrower-state control at Asia/Tokyo. Both patches restore both. The signal visits all aliases in this experiment instead of only initialized local wrappers.

Patches and outputs: `probes/M3/grouping/`. The patches are controlled experiments, not a submitted implementation.

## Impact facts

*run/read*: Threaded test setup followed by a TIME_ZONE override in a thread with no initialized wrapper can get wrong local timestamps even from newly opened physical sessions. The creator has already closed and exited. No borrower changes session state in this case. Frequency is unknown. USE_TZ=False is a documented, nondefault mode; the public-code sample contains that setting but establishes no affected-population count. Native PostgreSQL timezone handling and Python timezone conversion work in this mode.

No frequency estimate is established by the sampled search hits.

## Known only later

*read*: Django 5.1 was released on 2024-08-07; the release tag retains these pool integration paths. In August 2024, PR 18498 discussed the pool retaining a creator wrapper and restored separate timezone/role hooks. PR 19442 in 2025 explained return-on-close; PR 21795 in 2026 clarified pool configuration. These are after the cut-off.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | problem | medium (no-precedent) | The documented TIME_ZONE override does not reach new pooled connections when the thread that created the pool has exited, so SQL returns hour 00 where the override requires 03. |
| assessor-1 | problem | medium (no-precedent) | A documented test-time timezone override produces the wrong SQL hour on a fresh physical session because its callback retains a departed wrapper's timezone, independently of the borrower-state fault GT-v10. |
| assessor-2 | problem | medium (no-precedent) | A documented test-time timezone override produces a fresh physical session in UTC rather than Africa/Nairobi because the new pool retains its departed creator's cached timezone. |

To reconcile before asking:

- None: every party's pick is the recommender's.

## `M3`: label

Record: [`04-django-pooled-timezone.M3-label.before.json`](04-django-pooled-timezone.M3-label.before.json)

```text
ASK
- band stays on its existing owner or reconciler route
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/v-django-17914/cards/M3.md:

# A timezone override in a fresh thread leaves new physical pooled connections on the departed creator’s timezone

Django supplies a PostgreSQL database backend. Base is the commit before the change; head is the reviewed commit.

Attribution: introduced when the new pool is enabled.

## Executed cases

*run*: A thread creates the pool in UTC, closes its connection and exits. The main thread has zero initialized wrappers. In override_settings(TIME_ZONE="Africa/Nairobi"), with USE_TZ=False, a replacement physical session has a new backend PID but remains UTC at head. Django expects Africa/Nairobi. The same timestamp formats to hour 00 instead of 03. Base opens Africa/Nairobi and returns 03. A current-thread initialized-wrapper control passes at head. Native psql timezone configuration and Python zoneinfo both produce the Nairobi result.

Python 3.10.12, psycopg 3.1.18, psycopg_pool 3.2.0, PostgreSQL 16.15. Timings arrange a race or limit waiting; they are not a performance measurement. Base uses direct connections because it has no pool option.

## What the affected person sees

Threaded test setup followed by a TIME_ZONE override in a thread with no initialized wrapper can get wrong local timestamps even from newly opened physical sessions. The creator has already closed and exited. No borrower changes session state in this case. Frequency is unknown. USE_TZ=False is a documented, nondefault mode; the public-code sample contains that setting but establishes no affected-population count. Native PostgreSQL timezone handling and Python timezone conversion work in this mode.

## Impact facts

GT-v10 describes a borrower changing session state and returning it, with no reset in _close at base.py:385-399 or new setup on checkout. A return-time reset restores the borrower-state control to UTC and leaves the fresh-session override at UTC. The signal-only patch restores Africa/Nairobi and leaves the borrower-state control at Asia/Tokyo. Both patches restore both. The signal visits all aliases in this experiment instead of only initialized local wrappers.

PostgreSQL base.py:231 binds the pool configure callback to its creating wrapper. That callback reads its cached timezone_name at 376. The unchanged setting_changed receiver, django/test/signals.py:71-82, resets only wrappers initialized in the signaling thread. Before this change there was no pool or bound creator surviving that thread.

## Evidence limits

An ORM write/read round trip or a real application’s timezone-sensitive assertion; the executed wrong SQL hour is the demonstrated consequence. A measurement of the prevalence of threaded timezone overrides. No population frequency is measured. No production data was written by these probes.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | serious | medium (no-precedent) | SQL returns a wrong time value for the configured timezone, silently (S4). |
| assessor-1 | serious | medium (no-precedent) | A fresh pooled physical session silently returns hour 00 instead of the requested timezone's correct hour 03. |

To reconcile before asking:

- None: every party's pick is the recommender's.

