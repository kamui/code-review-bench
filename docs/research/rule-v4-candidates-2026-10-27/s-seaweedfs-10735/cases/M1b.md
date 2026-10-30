# Logs while a listing encounters expired records

SeaweedFS pull request 10735. Base: `db5a086d048c5c2d6e51e82bb070d20df04d688d`. Head: `6c8fde6642cd428e884cc9c9d88c9aaa189c4bf1`. Cut-off: **2026-08-13T14:12:47Z**, the last push (task packet section 6).

Facts marked **run** were executed for this packet against the real source of the named commit and a private Redis 8.2.1. **Read** means source or a saved public record. **Reported** means another person's statement, not independently reproduced. Source and executable behaviour at the pinned commits were knowable at the cut-off; the experiments were performed now. Later public facts are separated below. `upstream/source-and-search-index.md` records provenance and search limits.

## Source change

```diff
             glog.V(0).InfofCtx(ctx, "list %s : %v", path, err)
             if err == filer_pb.ErrNotFound {
+                store.removeOrphanedDirectoryListMember(ctx, dirListKey, path, fileName)
                 err = nil
                 continue
```

The added helper, head lines 237–251, removes the name, checks whether the record exists, and restores the name when necessary:

```diff
+    if err := store.Client.ZRem(ctx, dirListKey, fileName).Err(); err != nil {
+        return
+    }
+    exists, err := store.Client.Exists(ctx, store.getKey(string(path))).Result()
+    if err == nil && exists == 0 {
+        return
+    }
+    store.Client.ZAddNX(ctx, dirListKey, redis.Z{Score: 0, Member: fileName})
```

These excerpts use spaces for indentation and omit the helper's existing explanatory comment; tokens and order are unchanged. Complete diff: `upstream/change.diff` (**read**, knowable at cut-off).

The V(0) log call is head line 206 and base line 206, unchanged. It executes before the new helper call. The change edits neither its level nor its message (**read**, diff and source exports).

## Executions

**Run**, `probes/M1b/result-base.txt` and `result-head.txt`: three files are inserted with TtlSec=1, Redis expires them, and the directory is listed twice. At base each listing issues three GETs and prints three lines containing `filer: no entry is found in filer store`; six total. At head the first listing prints three such lines and removes the names; the second prints none. Both calls return an empty result and no error. A live-file control lists `ok` at both commits.

**Run**, exact message suffix:

```text
list /probe/f-00000 : filer: no entry is found in filer store
```

The timestamp and Go source prefix precede this suffix in stderr. The original line existed before this change. No log consumer or log-volume limit was run.

## Public descriptions at the cut-off

The task packet's description at the cut-off says (**read**, section 3, also `upstream/description-at-cutoff.txt`):

> Under a TTL workload the index therefore grows without bound, and two things get worse with it:

> - `LIST` costs one `GET` round trip per name *ever created* in that directory, not per live entry;
> - the skip logs at `glog.V(0)`, which is on by default, so every listing emits one log line per dead member.

Its stated remedy is:

> One line: `ZREM` the member in the not-found branch, mirroring what the expiry branch already does four lines below, against the same `dirListKey` variable.

This is the description supplied by the task, although it describes the earlier revision. `upstream/pr-edits.json` saves the independently fetched GraphQL edit history; edits at 14:14:08Z and 14:15:20Z are after the cut-off. The two later description versions are not quoted here.

**Read**, the change does not state a lower log level or silence on the first encounter. The log call is not removed by its diff. The index cleanup tests concern names left in Redis and do not assert log output. Project-document and dated issue searches are saved under `upstream/M1b-*`; no quotation asserting a quiet initial cleanup was found in the selected material read.

## Related source locations

| Id | What happens; source locations at head | Comparison performed here |
| --- | --- | --- |
| GT-s1 | A stale replica GET and EXISTS make cleanup permanently remove a live file's name; redis2 lines 99–102, 208, 238–250 and cluster routing. | M1a's matrix uses forced stale replies on standalone Redis. A real lagging cluster was not run. |
| GT-s2 | A pending SET-only update writes a value after cleanup has removed its name; lines 86, 92–95, 208, 238–247. | No update interleaving was run; none is needed by these expiry and cost cases. |
| GT-s3 | A failed cleanup or lost removal reply leaves a recreated file without its name; lines 238–250. | No failing-command case was run. All commands in the expiry probes succeeded. |
| GT-s4 | Another directory reader misses a completed insertion while cleanup has temporarily removed its name; lines 238–250, 145–170, 189. | No three-request gap was run. |
| GT-s5 | Removing a missing directory's parent name hides its surviving child index from deletion; lines 208, 237–251, 145–170. | The directory comparison here starts with a present, logically expired record; its separate helper branch was not run. |
| PE-s1 | A page is filled by dead names; the listing skips them without refilling it; lines 189–210. | P2's matrix runs one dead name before one live name with limit 1. |
| PE-s2 / PE-s3 | A write overlapping logical expiry loses its name / its new record; lines 214–219. | No concurrent writer was run. |
| PE-s4 | The documented readOnly setting is not the useReadOnly key read by the cluster store. | No configuration probe was run; different file and operation. |
| PE-s5 | A write between DeleteEntry's record deletion and name removal loses its name; lines 119–143. | No concurrent delete/write was run. |
| PE-s6 | Logical expiry removes a directory and its parent name but leaves its child index; lines 214–219. | P1's matrix runs listing, parent deletion and recreation. |

Statements and causes are **read** from the packet's reference_families and pre_existing_problems, checked against saved source. No production frequency is inferred from this table.

No patch experiment was performed for logging. The log call is separate from every known problem's faulty removal, routing, expiry or pagination operation (**read**, source). The added cleanup makes the second listing stop reaching that call (**run**).

## Impact facts

- **Who:** operators of redis2-family stores with physically missing indexed records.
- **How:** one informational line per encountered missing record remains on the first pass. Repeated output falls from six to three lines in the two-listing example.
- **How stuck:** no operation fails in the run. An operator can filter logs; that workaround was not run.
- **How common:** a TTL workload can produce this path; no deployment prevalence or log cost was measured.
- **Neighbours:** retained names cause repeated GET and log work; the log call itself is unchanged.

## Known only later

No later log-level change was established in the selected fetched records.
