# Thermo-nuclear code quality review — seaweedfs#10735

**Range:** `db5a086d0..6c8fde664` (`main...review-head`). Two files: +17 lines in
`weed/filer/redis2/universal_redis_store.go`, plus a new 143-line
`weed/filer/redis2/universal_redis_store_test.go`.

## Verdict

**Approve after light structural changes.** The fix is correct. I walked every
interleaving of the new `ZREM` → `EXISTS` → `ZADD NX` repair against `InsertEntry`,
`UpdateEntry` and `DeleteEntry`, and the protocol converges; the details are in
`01_redis2_listing_cleanup.md`. The single-key design is the right call for
`redis_cluster2`, and no file-size, cluster, or transient-error concern holds up. The PR
is not structurally clean, though. It adds a second, differently shaped cleanup protocol
for "this index member names a dead child" into a loop that already had one, and it hands
the new protocol a mixed prefixed/unprefixed signature. A small code-judo move merges the
two dead-child branches into one path. The repair is also completely silent when it
fails, and its tests run nowhere automatically. None of this is large, and all of it is
cheap to fix now while the code is fresh.

## Findings

**1. Two dead-child protocols eight lines apart; normalize expiry into not-found**
(`weed/filer/redis2/universal_redis_store.go:205-220`). After this PR,
`ListDirectoryEntries` drops a dead index member in two different ways. The not-found
branch at `:207-211` calls the new guarded `removeOrphanedDirectoryListMember`. The
logical-expiry branch at `:214-220` still does an unguarded, results-discarded
`Del` + `ZRem`. Both branches mean the same thing. The expiry branch has the same
index-member recreate race the second commit was written to close, and the loop now has
three levels of nesting around what is really a live / dead / error classification. The
code-judo move is to turn a logically expired entry into `ErrNotFound` right after
`FindEntry` (by `DEL`ing the value and setting `err = filer_pb.ErrNotFound`). That leaves
one dead-child exit through the one helper and a flat `if err != nil { break }`, removes
the `else` ladder and the stray `ZRem` at `:217`, and makes the expiry branch inherit the
restore guard for free. Without concurrency the behavior is identical. The worked loop is
in `01_redis2_listing_cleanup.md`, Finding 1.1.

**2. The helper signature mixes key namespaces and encodes one identity three times**
(`weed/filer/redis2/universal_redis_store.go:237`).
`removeOrphanedDirectoryListMember(ctx, dirListKey string, path util.FullPath, fileName string)`
takes a `dirListKey` that must already carry `keyPrefix`, a `path` that must not (the
helper calls `getKey` on it at `:245`), and a `fileName` that is the last component of
`path`. That mixed contract is exactly how the silent `keyPrefix` no-op the PR body
worries about gets written, and the test already has to rebuild the prefixed key by hand
to call it (`universal_redis_store_test.go:112`). The helper should take
`(dirPath util.FullPath, fileName string)` and derive both keys through `getKey` itself.
Keep `fileName` as the raw ZSET member and do not re-derive it via `DirAndName`, which
sanitizes UTF-8 (`weed/util/fullpath.go:44`). The larger version of the same move gives
the directory index an owner: a `dirListKey(dir)` method used by all six sites in the
file that currently spell out `store.getKey(genDirectoryListKey(...))` inline
(`:68, :121, :136, :151, :166, :178`). Details are in `01_redis2_listing_cleanup.md`,
Finding 1.2.

**3. Every failure in the repair is silent, including the one that reintroduces the fixed
bug** (`weed/filer/redis2/universal_redis_store.go:238-250`). A `ZRem` error returns
without a log, an `Exists` error falls through without a log, and the `ZAddNX` restore
result is discarded outright. If the restore fails, the store ends up with a live value
and no index member, invisible to listings and to `DeleteFolderChildren` until another
`InsertEntry` on that path. That is precisely the state the second commit exists to
prevent, and nothing records it. The PR body's statement that errors "are no longer
discarded outright" holds only for control flow. Log the restore failure with
`glog.ErrorfCtx`, as the filer layer does for its own cleanup at
`weed/filer/filer.go:430`, and optionally log the `ZREM` failure at `V(1)`. Details are in
`01_redis2_listing_cleanup.md`, Finding 1.3.

**4. The regression tests run nowhere automatically, and the recreate test passes against
a no-op helper** (`weed/filer/redis2/universal_redis_store_test.go:19-21, :106-121`). All
tests skip unless `RUN_REDIS_TESTS=1`, and no workflow under `.github` starts Redis or
sets the variable (`git grep -i redis -- .github` is empty). I ran the package offline:
every case reports SKIP. The concurrency guard from the second commit, the part most
likely to be "simplified" away later, is therefore unprotected in CI.
`TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry` also only asserts that the
member survives, which a helper that does nothing satisfies too. It covers the protocol
only together with the orphan-removal test. The fix is to wire a Redis service into an
existing Go test job and make the recreate test also assert that removal happens. At
minimum, document that these tests are manual-only. Details are in `02_redis2_tests.md`,
Finding 2.1.

## Questions (not findings)

**Q1.** The filer already owns TTL expiry: `Filer.FindEntry` (`weed/filer/filer.go:434`)
and `doListDirectoryEntries` (`weed/filer/filer.go:473`) apply the same
`Crtime + TtlSec` check and delete through `Store.DeleteOneEntry`, which runs the store's
own `DEL` + `ZREM`. Is the store-level logical-expiry branch in redis2's
`ListDirectoryEntries` still needed at all, or could it be deleted so the store handles
only the physical not-found case? Finding 1's normalization does not depend on the
answer, but deleting the branch would be the bigger simplification.

## Proposed remediation sequence

1. Change the helper to take `(dirPath, fileName)` and derive its keys through `getKey`
   (Finding 2). Update the one test call site.
2. Normalize logical expiry into not-found and route both dead-child cases through the
   helper, flattening the loop (Finding 1).
3. Log the restore failure at error level, and the `ZREM` failure at `V(1)` (Finding 3).
4. Put the redis2 tests in CI behind a Redis service, and tighten the recreate test so a
   no-op helper fails it (Finding 4).
5. Optional follow-up: introduce a `dirListKey(dir)` owner for the six inline index-key
   derivations, and answer Q1.

## Detail files

- `01_redis2_listing_cleanup.md`: interleaving analysis of the repair protocol,
  Findings 1–3 with worked code, and the items checked and not flagged.
- `02_redis2_tests.md`: offline test run output, CI search, and Finding 4.
