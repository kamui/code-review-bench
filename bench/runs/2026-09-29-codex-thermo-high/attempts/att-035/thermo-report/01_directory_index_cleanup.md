# Directory-index cleanup

## Scope and measurements

The production diff in `weed/filer/redis2/universal_redis_store.go` adds a call from the `ErrNotFound` path in `ListDirectoryEntries` and a 15-line helper. The helper runs `ZRem` on the directory index, checks the entry key with `Exists`, and uses `ZAddNX` to restore membership when the value is present again or the check errors. The adjacent `InsertEntry` path treats a failed `ZAddNX` as an error and wraps it with the entry path (lines 67–70), while `UpdateEntry` only writes the value (lines 92–95).

The production file is 259 lines after the change; the new test file is 143 lines. Neither approaches the 1,000-line threshold. The changed production logic is localized and does not introduce broad branch growth. I found no additional abstraction, file-size, or decomposition issue in this diff.

## Finding: a failed restoration is silent and can persist

At `weed/filer/redis2/universal_redis_store.go:250`, `store.Client.ZAddNX(...)` is issued without checking its result. This command is the only action that puts the index member back after the preceding `ZRem`. If Redis reports an error, the helper returns normally and `ListDirectoryEntries` continues as though cleanup succeeded. The value may be live, but the directory index no longer contains it; later listings enumerate index members and therefore cannot rediscover this path to repair it. This makes a transient write error a potentially persistent visibility failure.

The helper also returns silently when `ZRem` fails (lines 238–240). That case leaves the stale member and can be retried by a later listing, so its impact differs from a failed restore, but the caller receives no indication of either partial outcome. At minimum the restoration error needs to cross the helper boundary and be surfaced or logged with path context. If cleanup remains best-effort to preserve listing behavior, explicit logging is preferable to pretending the repair completed.

## Finding: `UpdateEntry` can recreate a value after the helper's last check

The recovery logic accounts for `InsertEntry`, whose sequence is SET followed by `ZAddNX`, but not for `UpdateEntry`. `UpdateEntry` calls only `doInsertEntry` (`universal_redis_store.go:92–95`), which SETs the value without touching the index. If cleanup's `Exists` at line 245 returns zero, cleanup exits at line 247; an `UpdateEntry` SET immediately afterward then leaves a live value without its directory member. A later listing scans the index and cannot find this value. This is a concurrency hole in the exact re-creation guarantee the helper is intended to provide.

Make the index invariant hold across every path that writes or recreates an entry. A code-judo option is to put the directory membership write into a shared entry-write path used by both insert and update, while retaining the super-large-directory policy at the appropriate boundary. Alternatively, explicitly constrain and enforce `UpdateEntry` so it cannot recreate a missing value. Add an interleaving test showing that a write after the absence check leaves the entry listable.

## Worked code-judo proposal

Preserve the three-step algorithm required by the non-atomic cross-key constraint, but give it an explicit result contract. Make `removeOrphanedDirectoryListMember` return an error for failed Redis commands; distinguish an unsuccessful initial `ZRem` from a failed repair write if the caller needs different handling. At the listing boundary, log the path, index key, and command failure, or return the error according to the store's intended contract. This removes the hidden success path without adding another state flag or retry layer. A Redis-backed failure test is difficult without a controllable command boundary; if practical, inject a narrow command/client interface in a focused test, otherwise document that the live-Redis test suite cannot exercise this failure mode.

## Verification status

- Read the complete committed diff and the surrounding insert, update, lookup, and listing paths.
- `git diff --check main...review-head`: clean.
- `GOMODCACHE=... GOCACHE=... GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local go test -count=1 ./weed/filer/redis2`: passed (`ok`, 0.018s). Under the task policy no Redis server is available, so opt-in live-Redis cases skip; this run does not verify the restore failure behavior.
- The checkout was not edited.
