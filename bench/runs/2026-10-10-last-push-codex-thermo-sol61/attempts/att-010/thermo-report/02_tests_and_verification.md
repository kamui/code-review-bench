# Tests and verification

## Committed tests

`universal_redis_store_test.go` has 143 lines. `newTestStore` at lines 16–45 gates on `RUN_REDIS_TESTS=1`, creates a standalone Redis client, supplies a unique directory, and registers cleanup. Helpers construct encoded filer entries, collect listing names, and inspect the ZSET directly. The two prefix cases correctly inspect both visible results and the remaining index. The TTL case verifies physical absence before listing, separating it from logical expiry. These are useful integration assertions.

`TestRemoveOrphanedDirectoryListMemberKeepsRecreatedEntry` at lines 106–121 starts with a live entry and invokes the helper directly. It verifies successful compensation when the value exists; it does not force recreation between listing’s miss and removal, inject repair failure, simulate an ambiguous response, or exercise replica routing. Those missing cases matter specifically to the two production findings and are included in their remediation. No separate cosmetic test finding is raised.

The cleanup helper in the test fixture uses existing deletion APIs and does not modify the shared store’s key format. Timing, fixture availability, and cleanup-error reporting were considered but do not justify additional findings for this small integration suite.

## Stock package run

Executed once from the clone root:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-010/clone-cache/gomodcache GOCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-010/clone-cache/gocache GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local go test -count=1 -v ./weed/filer/redis2
```

Result: exit 0, `ok github.com/seaweedfs/seaweedfs/weed/filer/redis2 0.021s`. Both orphan prefix subtests skipped. The recreated-entry and TTL tests also skipped. The parent orphan test appears as PASS because both its children skipped; it provides no executed Redis assertion. No Redis server, module proxy, or network fixture was used. The package compiled under the local toolchain and cached dependencies.

## Offline probes against unchanged production code

Review-only artifacts are outside the clone:

- `../redis2_review_probe_test.go` contains a copy of the committed test file plus a narrowly scripted fake `redis.UniversalClient`.
- `../redis2-review-overlay.json` replaces only the test file for the probe invocation. No production file is overlaid.

The fake embeds the client interface and overrides only `Get`, `ZRangeByLex`, `ZRem`, `Exists`, and `ZAddNX`. It uses the dependency’s normal `NewStringResult`, `NewStringSliceResult`, and `NewIntResult` constructors. The live value is encoded with the real filer entry encoder and read back through the real `FindEntry` decoder. It does not implement Redis transport, replication, expiry, or cluster discovery.

Executed once from the clone root with a distinct flag set:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-010/clone-cache/gomodcache GOCACHE=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-010/clone-cache/gocache GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local go test -count=1 -v -overlay=/home/jack/.t3/bench-runs-lanes/codex-thermo-sol61/2026-10-10-last-push-codex-thermo-sol61/att-010/clone-work/redis2-review-overlay.json -run '^TestReviewProbeLiveEntryBecomesUnlisted$' ./weed/filer/redis2
```

Result: exit 0, `ok github.com/seaweedfs/seaweedfs/weed/filer/redis2 0.020s`. All three subtests executed:

| Probe | Scripted command boundary | Observed result |
| --- | --- | --- |
| `stale_replica` | A live value exists, but initial GET and maintenance EXISTS both report stale absence. | Cleanup removes membership; current direct lookup succeeds; the next listing is empty. |
| `restore_failure` | Recreation completes before removal, EXISTS observes the value, and restoration returns deadline exceeded. | Membership remains absent; direct lookup succeeds; the next listing is empty. |
| `lost_remove_reply` | Recreation completes before removal; ZREM mutates the index but returns deadline exceeded. | The helper returns without repair; direct lookup succeeds; the next listing is empty. |

Each prints: `confirmed live value readable, index member absent, subsequent listing empty, listing returned no error`.

These probes pass because they deliberately assert the defective current behavior. They are reproducibility evidence, not passing correctness regression tests. Tests committed with a fix must invert the membership and subsequent-listing expectations. The fake schedules relevant interleavings synchronously at command boundaries, so no goroutine scheduling or physical clock timing is needed. The stale-read case’s real-world applicability comes separately from the source-verified cluster configuration and routing in `01_directory_index.md`.

## Checks and preservation

`git diff --check main...review-head` exited 0. `git status --porcelain=v1` was empty before review and after both Go invocations. The committed diff still consists only of the production modification and added test file. No fixes were applied. All scratch files, overlays, and reports reside in `clone-work`; all module/build caches use `clone-cache`.

No broader build, vet run, race run, or additional package test was needed to verify these specific command-boundary issues, and unavailable modules/live fixtures were not fetched. No remediation proposal has been compiled or integration-tested. The findings are supported by executed production control flow plus local dependency/source analysis, with live Redis behavior explicitly unverified.
