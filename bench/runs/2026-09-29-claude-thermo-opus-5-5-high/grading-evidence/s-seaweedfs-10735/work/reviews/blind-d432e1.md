# Review blind-d432e1

### Item 1
Location: weed/filer/redis2/universal_redis_store.go:205-220
Claim: **1. Two dead-child protocols eight lines apart; normalize expiry into not-found**
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
Consequence: —
Fix: —

### Item 2
Location: weed/filer/redis2/universal_redis_store.go:237
Claim: **2. The helper signature mixes key namespaces and encodes one identity three times**
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
Consequence: —
Fix: —

### Item 3
Location: weed/filer/redis2/universal_redis_store.go:238-250
Claim: **3. Every failure in the repair is silent, including the one that reintroduces the fixed
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
Consequence: —
Fix: —

### Item 4
Location: weed/filer/redis2/universal_redis_store_test.go:106-121
Claim: **4. The regression tests run nowhere automatically, and the recreate test passes against
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
Consequence: —
Fix: —

### Item 5
Location: weed/filer/filer.go:434-473
Claim: **Q1.** The filer already owns TTL expiry: `Filer.FindEntry` (`weed/filer/filer.go:434`)
and `doListDirectoryEntries` (`weed/filer/filer.go:473`) apply the same
`Crtime + TtlSec` check and delete through `Store.DeleteOneEntry`, which runs the store's
own `DEL` + `ZREM`. Is the store-level logical-expiry branch in redis2's
`ListDirectoryEntries` still needed at all, or could it be deleted so the store handles
only the physical not-found case? Finding 1's normalization does not depend on the
answer, but deleting the branch would be the bigger simplification.
Consequence: —
Fix: —
