# Redis2 orphan-index recovery

## Scope and measurements

The committed range adds orphan cleanup to `UniversalRedis2Store.ListDirectoryEntries` and adds Redis-backed tests. `ListDirectoryEntries` now calls `removeOrphanedDirectoryListMember` when `FindEntry` returns the exact `filer_pb.ErrNotFound` sentinel. The helper removes the sorted-set member, checks whether the value key has reappeared, and restores membership with `ZAddNX` when the key exists or the existence check errors.

The changed production file has 259 lines after the change and 242 at the base; the new test file has 143 lines. The review command `git diff --numstat main...review-head` reports +17 production lines and +143 test lines. This does not create a file-size or decomposition concern.

## Finding 1: Surface failed index restoration

**Evidence:** `weed/filer/redis2/universal_redis_store.go:245-250` checks the value key and, unless it positively confirms absence, issues `ZAddNX`. The result is ignored. Back in `ListDirectoryEntries` at lines 207-210, the original `ErrNotFound` is cleared and iteration continues regardless of whether the repair succeeds.

**Why this matters:** The helper has already successfully run `ZRem` before trying restoration. If a recreated value is present and `ZAddNX` fails (for example, because the listing context is canceled between commands or Redis returns a transient command error), the value remains stored but its name is no longer indexed. The caller receives a successful listing, and later listings cannot encounter this path because they enumerate only the index. This converts a recoverable transient command failure into a persistent visibility loss until another insert re-adds the member.

**Action:** Return an error from `removeOrphanedDirectoryListMember`, check the `ZAddNX` command result, and have the `ErrNotFound` branch return a contextual listing error if repair could not complete. Keep the existing best-effort restoration after an uncertain `Exists` result, but surface failure of that restoration too. This makes the listing contract honest and gives the caller a retry signal rather than claiming convergence.

A concrete shape is:

```go
func (store *UniversalRedis2Store) removeOrphanedDirectoryListMember(
    ctx context.Context, dirListKey string, path util.FullPath, fileName string,
) error {
    if err := store.Client.ZRem(ctx, dirListKey, fileName).Err(); err != nil {
        return fmt.Errorf("remove orphaned member %s: %w", path, err)
    }

    exists, err := store.Client.Exists(ctx, store.getKey(string(path))).Result()
    if err == nil && exists == 0 {
        return nil
    }
    if err := store.Client.ZAddNX(ctx, dirListKey, redis.Z{Score: 0, Member: fileName}).Err(); err != nil {
        return fmt.Errorf("restore directory member %s: %w", path, err)
    }
    return nil
}
```

Then the listing branch should return a wrapped error when this helper fails rather than set `err = nil` and continue. The actual implementation should retain the repository's preferred wrapping style. Since `ZAddNX` is idempotent, a deliberate retry policy could retry restoration before returning, but an ignored result is not safe.

## Code-judo and structural assessment

The new helper is justified: it names the nontrivial recovery operation and keeps the race handling out of the listing loop. A single atomic Redis operation across the value and index keys is not a practical simplification for the stated Redis Cluster layout because the keys do not share a hash slot. The current remove/check/restore sequence is therefore a direct, locally understandable compromise. The worthwhile structural change is to make its success/failure explicit in its return contract, not to introduce another abstraction layer.

The logical-expiry branch still ignores the `DEL` and `ZRem` results, but that behavior predates this patch and is outside the changed lines. It is not counted as a finding against this PR.

## Verification status

Source inspection only. Reviewed the full specified diff, the complete `universal_redis_store.go`, the new integration tests, and numbered lines around the helper. Commands used for measurements: `git diff --numstat main...review-head`, `git show main:weed/filer/redis2/universal_redis_store.go | wc -l`, `wc -l` on the changed files, and `nl -ba` on the changed production file. Tests were not run. The supplied execution policy says a Redis server is unavailable; the review did not attempt to run the package tests. No changes were made to the checkout.
