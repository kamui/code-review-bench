# Redis2 directory-index cleanup

## Scope and measurements

Reviewed the committed production change and its new tests at head `6c8fde6642cd428e884cc9c9d88c9aaa189c4bf1`, against merge-base `db5a086d048c5c2d6e51e82bb070d20df04d688d`. The manifest matches `git diff --numstat main...review-head`: production adds 17 lines and tests add 143 lines, with no deletions.

`wc -l weed/filer/redis2/*.go` reports 259 lines for `universal_redis_store.go`, 143 for its tests, and 620 for the six redis2 Go files combined. `git show main:weed/filer/redis2/universal_redis_store.go | wc -l` reports 242. There is no file-size blocker.

Source reads used `nl -ba` for the changed files and `rg` for the client constructors, directory listing wrapper, command implementations, and adjacent Redis stores. Repository guidance was not loaded. Existing packet review judgments were not used as evidence. No network requests or fixture connections were made.

## The ownership and consistency boundary

`UniversalRedis2Store.InsertEntry` writes the value at lines 58–59, then uses `ZAddNX` at line 68. The member and value use different keys: the prefixed full path versus the prefixed directory plus a NUL marker. `UpdateEntry`, lines 92–95, writes only the value. `ListDirectoryEntries`, lines 189–203, enumerates the sorted set and fetches only those paths. `DeleteFolderChildren`, lines 151–161, also depends on membership to discover children.

The new branch at line 208 is reached only for the exact not-found sentinel. `FindEntry`, lines 99–105, produces that sentinel for `redis.Nil` and wraps other errors. Cleanup is therefore not accidentally triggered by an ordinary failed GET. The prefix is applied once to the directory key and once to the value key, correctly. Index restoration uses score zero, matching insertion and lexicographic enumeration.

With successful commands and authoritative reads, the compensation argument is valid. An insert completed before removal is observed by the post-removal probe and restored. An insert whose value write happens after an authoritative absent probe will perform its own member addition after the earlier removal. A concurrent delete can leave a stale member after restoration, which a subsequent listing can remove; that is a different, recoverable outcome from losing membership for a live value.

The helper does not have a trivial-wrapper smell. Its main flaw is that this successful-operation argument is hidden inside a void method and applied to a universal client with stronger failure and routing possibilities.

## Finding 1: destructive cleanup has no recovery contract

The actionable anchor is `weed/filer/redis2/universal_redis_store.go:237–250`, particularly the early return on lines 238–239 and the unchecked final command at line 250. The production listing clears its previous not-found error at line 209 regardless of repair outcome.

A deterministic failing schedule is:

1. A directory member exists but its old value is absent; listing receives `ErrNotFound`.
2. Another insertion completes `SET` and `ZADD NX`. The latter is a successful no-op because the old member remains.
3. Cleanup executes `ZREM` successfully.
4. `EXISTS` correctly reports that the newly inserted value is present.
5. The compensating `ZADD NX` fails after the client's retries are exhausted, without restoring the member.
6. Listing returns nil. The value remains readable; future listings and folder-child enumeration cannot discover it. `UpdateEntry` writes it again but still does not add membership.

The same permanent state follows if step 4 returns an error and step 5 also fails. Treating unknown existence as a reason to restore is reasonable; treating an unsuccessful restoration as completed repair is not.

The earlier `ZREM` error path has the same ownership problem. A transport failure does not prove that Redis performed no mutation. The cached go-redis implementation at `github.com/redis/go-redis/v9@v9.21.0/redis.go:965–1020` writes the command before reading its response and can return the read error; its retry loop at lines 845–893 is finite. Redis can have applied the first removal while the response is lost and subsequent attempts fail. Returning at line 239 skips the value probe and compensation in precisely that ambiguous case.

The issue is introduced by cleanup of physically missing values. This report does not attribute the existing logical-expiry deletion race or existing insertion partial-write behavior to this PR. Here the concurrent insertion itself completed successfully; listing later breaks its invariant.

### Verification status

The unchanged helper was exercised using the scratch overlay at `../probes/overlay.json` and test source `../probes/cleanup_probe_test.go`. The fake embeds `redis.UniversalClient` and implements only `SET`, `GET`, `ZADD NX`, `ZREM`, `EXISTS`, and `ZRANGEBYLEX`. It is a deterministic command-result probe, not a simulated Redis Cluster or a replacement server.

The fake completes the real store's `InsertEntry` immediately before the real helper's `ZRem`, then injects a failed restoration, a failed probe plus failed restoration, or a removal that applied before its reply failed. After clearing failures, each case checks the real store's `FindEntry`, `UpdateEntry`, and another `ListDirectoryEntries`. Direct access and update succeed, while directory listing returns no entries.

All three failure schedules violate the assertion. The successful-recreation control passes. This verifies the store's behavior for plausible Redis command outcomes without depending on thread scheduling or a live server. Actual socket failures and failover were not executed.

### Worked recovery proposal

Returning `error` and checking `.Err()` on `ZAddNX` are necessary contract improvements, but they do not by themselves restore the deleted member. Nor can an ordinary retry on the next listing find an unindexed path. Do not describe either change as a complete repair.

One recoverable design keeps directory-scoped pending repair intent and atomically records that intent with membership removal. A listing drains pending intent before enumerating ordinary members. The value probe is primary-bound. Completing a repair atomically restores membership when needed and clears its intent; an uncertain command result leaves the intent available to the next listing. The directory index and repair ledger must share a cluster slot, but the value key need not. Ledger keys require a tested slot-compatible key derivation, including arbitrary prefixes and braces in paths; blindly appending a suffix does not preserve slots.

The following is protocol pseudocode, not a compiled replacement:

```go
func repairOrphan(ctx context.Context, dir directoryIndex, name string) error {
    // One directory-slot operation records a unique pending token and removes
    // the member. The token is not erased on a lost response.
    token, err := dir.beginRepair(ctx, name)
    if err != nil {
        return err // pending work, if applied, remains discoverable
    }
    return finishRepair(ctx, dir, name, token)
}

func finishRepair(ctx context.Context, dir directoryIndex, name, token string) error {
    present, err := valueExistsOnPrimary(ctx, dir.childPath(name))
    if err != nil {
        return err // leave intent; do not guess that absence was confirmed
    }
    // One directory-slot operation conditionally restores the member and
    // clears only this repair token. A failed/ambiguous completion is retried.
    return dir.completeRepair(ctx, name, token, present)
}
```

Token checks prevent an older completion from consuming newer pending work. All operations should be idempotent; do not expire intent while a live value might remain unindexed. Pending-drain failure must be surfaced rather than returning a successful, incomplete directory listing. If pending repair is applied but its reply is lost, the next listing can still recover it. If completion is applied but its reply is lost, membership is already restored or absence was confirmed. A later insertion after a confirmed-absent probe adds its own member after the earlier removal.

This adds persistent protocol state, so it is an architectural option rather than a claim that a few error checks are sufficient. Another defensible solution changes key ownership so value validation and index removal can be atomic. Under the present layout, fail-safe destructive cleanup cannot assume successful compensation. Choose and document a recovery guarantee before enabling removal on every not-found listing.

## Finding 2: replica absence is not authoritative

The actionable anchor is `weed/filer/redis2/universal_redis_store.go:245–247`. The universal `Exists` call is treated as positive confirmation of absence, but that conclusion is stronger than the configured client guarantees.

`weed/filer/redis2/redis_cluster_store.go:27–41` reads `useReadOnly` and `routeByLatency`, and lines 48–58 pass them to `redis.ClusterOptions.ReadOnly` and `.RouteByLatency`. The cached go-redis `osscluster.go:61–66` documents replica routing and automatic read-only enablement for latency routing. At lines 2389–2405, `cmdNode` routes commands marked read-only to `slotReadOnlyNode`; lines 2438–2452 select the closest node or a replica. Writes route to the slot master. Redis classifies GET and EXISTS as read-only commands; the helper does not override this routing.

A failure schedule does not require a recreate:

1. Insertion finishes on both primaries: the value key's primary holds the entry and the directory-index primary holds its member.
2. The directory-index replica has caught up, so listing sees the member. The value-key replica has not caught up; the keys can be on different shards.
3. Listing's replica-routed GET returns `redis.Nil`.
4. Cleanup removes membership on the directory-index primary.
5. Its replica-routed EXISTS still returns zero, so cleanup does not restore membership.
6. Value replication subsequently catches up, but the entry remains absent from the directory index. No future enumeration knows which path needs repair.

Even when both keys share a shard, concurrent recreation plus replication lag can produce the same false absence. The pre-existing code skipped an absent replica response without changing membership; this change makes that weak read destructive.

### Verification status

The offline `replica_lag` probe starts with a successful real `InsertEntry`, makes value GET and EXISTS report replica absence while directory enumeration remains current, and runs the unchanged listing. Clearing simulated lag does not restore membership. `FindEntry` and `UpdateEntry` succeed, but the next listing returns zero entries. This reproduces the helper's decision for the supplied results. Actual asynchronous replication was not run because Redis and a cluster fixture are unavailable.

The constructor and cached client implementation verify that replica routing is a supported execution path. The temporal replication schedule is an inference from asynchronous replica visibility, not a measurement of a live cluster. It needs no assumed command failure or write failure.

### Worked code-judo proposal: make authority a value-store boundary

Keep ordinary listing reads on their configured route. Give destructive maintenance an explicitly primary-bound probe so the listing code does not grow client-type conditionals. A small dedicated authority boundary earns its abstraction because it distinguishes absence suitable for deletion from potentially stale absence suitable only for display.

For example, an ordinary EVAL/EVALSHA script over just the value key returns existence while using write routing. It does not touch the directory key and therefore does not require the two existing keys to share a slot:

```go
var existsOnPrimary = redis.NewScript(`return redis.call('EXISTS', KEYS[1])`)

func (store *UniversalRedis2Store) valueExistsOnPrimary(ctx context.Context, path util.FullPath) (bool, error) {
    n, err := existsOnPrimary.Run(ctx, store.Client,
        []string{store.getKey(string(path))}).Int64()
    return n != 0, err
}
```

Use ordinary EVAL/EVALSHA, not their read-only variants. This is a worked API sketch, not an executed patch. Validate actual script routing with the supported client and cluster fixture. Script use also requires deployment permissions for EVAL/EVALSHA; a primary-bound client is an alternative when those commands are unavailable. Neither probe choice fixes finding 1 by itself.

An optional primary check before starting removal can avoid unnecessary destructive work when the original miss was stale. It does not replace the post-removal concurrency check: another insertion may begin after any pre-check. Preserve the successful-interleaving argument while changing what constitutes an authoritative absent result.

## Test structure and simplification

The new 143-line file uses focused helpers for store setup, insertion, listing, and direct index inspection. The prefix table verifies both configured key shapes; the TTL test checks that Redis physically removed the value before exercising listing. Those are useful integration tests. Its recreate test deliberately starts the helper with a live value and confirms successful restoration; it does not exercise a failing command or replica routing.

The structural simplification available immediately is to test the repair protocol through the existing client seam rather than requiring Redis and wall-clock waits for every invariant. Embed the broad client interface in a narrow test fake and implement only the commands that own the protocol. That avoids implementing an entire Redis client, adding a mock dependency, or changing production APIs. Keep TTL expiration as a live-server check because the fake cannot verify Redis expiration.

Turn the repair tests into a table over observed value state, removal outcome, probe outcome, and restore outcome. Include success, genuine orphan, prefix handling, failed restoration, applied removal with a lost response, and stale reads. For a durable recovery implementation, restart the repair owner and prove a subsequent listing drains intent and returns the live entry. These assertions check observable invariants rather than just the number of commands issued.

No dramatic, behavior-preserving reduction to one atomic operation is evident under the unchanged cluster key layout. Migration to per-directory hash-tag ownership could replace remove/check/restore with a check-and-remove script, but requires changes across insertion, update, deletion, lookup, and existing persisted keys. That larger design should be justified separately. Adjacent Redis implementations do not supply a reusable canonical orphan-repair helper; copying their unguarded expiry handling would not solve these findings.

## Commands and results

The environment for each Go command was:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-011/clone-cache/gomodcache
GOCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-011/clone-cache/gocache
GOFLAGS=-mod=mod
GOPROXY=off
GOTOOLCHAIN=local
```

From the clone root, `go test -count=1 -v ./weed/filer/redis2` exited zero and reported 0.022 seconds of package test time. Both orphan prefix subtests, recreation, and TTL expiry skipped under the RUN_REDIS_TESTS gate. The parent orphan test reports PASS despite both children skipping. No live cleanup behavior was exercised.

The focused overlay command was:

```sh
go test -count=1 -v \
  -overlay=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-011/clone-work/probes/overlay.json \
  -run TestCleanupProbeLiveEntryRemainsListed ./weed/filer/redis2
```

It exited one with the following invariant results:

```text
PASS successful_recreate
FAIL restore_fails
FAIL exists_and_restore_fail
FAIL remove_applied_reply_lost
FAIL replica_lag
```

Each failure emitted the same assertion:

```text
live value lost index membership: first listing error=<nil>, member=false, next listing count=0; UpdateEntry did not repair it
```

This expected failure is reproduction evidence, not a failure of the ordinary integration test suite. Each test command ran once for its flag set, offline, within the five-minute allowance. Scratch files exist only under clone-work. No go build over other packages, live Redis tests, race run, upstream lookup, or broad test suite was attempted.

`git diff --check main...review-head` passed. `git status --porcelain=v1`, `git diff --exit-code`, and `git diff --cached --exit-code` showed no changes. Head tree identity is `8e6b61d682e2defba3eaf4e245e11a335ed7582e`; merge-base tree identity is `9b9b42131ca1c254a7097f68a3d3e30b994c2c81`. Final checks confirmed the same clean head. Reports and probes did not alter the checkout.

## Remediation acceptance

Accept the change once cleanup uses authoritative absence and its destructive mutation has a recovery path that survives uncertain command outcomes. Require the offline failure schedules to preserve or recover discoverability after the fixture returns to health. Verify cluster routing when an actual fixture becomes available. Merely passing the successful-recreation helper test or suppressing cleanup errors is insufficient.
