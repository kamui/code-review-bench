# Directory-index write contract

## Scope and judgment

The changed files are `weed/filer/redis2/universal_redis_store.go` and `weed/filer/redis2/universal_redis_store_test.go`. The new helper belongs in the Redis store, which owns the value/index representation. This is a cohesive 17-line production addition rather than scattered feature logic. The new protocol nevertheless assumes a narrower writer contract than the store implements.

The actionable finding carried into the summary is **[P1] Make every value writer maintain directory membership**, anchored to the newly added absence decision at `universal_redis_store.go:245-247`.

## Source evidence

`InsertEntry` at lines 56-73 calls `doInsertEntry` before adding the directory member with `ZAddNX`. It skips index writes for configured super-large directories and for the empty name at the root. `doInsertEntry` at lines 76-89 serializes the entry and performs a Redis `SET` with the requested TTL. The sequence is value first, member second.

`UpdateEntry` at lines 92-94 calls only `doInsertEntry`. Redis `SET` is an upsert here; the store does not check that the previous value still exists. Consequently an update can recreate a value that expired or was evicted between the caller's read and write, without re-adding an index member.

The new `removeOrphanedDirectoryListMember` removes the member at line 238 and returns without restoration when the post-removal `EXISTS` returns zero at lines 245-247. If a value is written later, membership now depends on that writer performing its own `ZAddNX`. This is true for insert and false for update.

The concrete caller is `weed/server/filer_grpc_server.go:643-674`: the update RPC reads an entry, validates it, processes chunks, constructs the replacement entry, and then calls the filer's update method. `weed/filer/filer.go:376-397` preserves identity attributes and passes the replacement to `f.Store.UpdateEntry`; it does not insert the directory member. The path mutation lock in the RPC does not prevent Redis TTL expiry, and the listing cleanup does not take that lock. A metadata update that clears the TTL, or an update to a non-TTL value evicted after the read, can therefore produce a logically live value in the problematic ordering.

The pre-change not-found branch only logged and continued; it did not remove the member. The loss in this ordering is introduced by the PR. The pre-existing logical-expiry deletion race is a different path and is not reported as a finding here.

## Deterministic verification

The scratch probe runs the real `ListDirectoryEntries`, `UpdateEntry`, `FindEntry`, and cleanup helper. Only the Redis command boundary is replaced. The initial state has the name `entry` in the directory index and no value key, representing expiry or eviction. The command double captures a zero result for `EXISTS`, then invokes `UpdateEntry` before returning that result to cleanup. This is equivalent to the update `SET` executing after Redis evaluated the existence check. The replacement entry has `TtlSec` zero and current timestamps, avoiding an unrelated logical-expiry deletion.

The observed command trace was:

```text
ZRANGEBYLEX=[entry]
GET present=false
ZREM applied=1 error=<nil>
EXISTS=0
SET
GET present=true
ZRANGEBYLEX=[]
```

`TestReviewUpdateAfterAbsentCheck` passed assertions that the final value decodes through `FindEntry`, its member is absent, and a second listing returns no callback entries and no error. This is a reproduction assertion, not an assertion that the implementation is correct.

The two successful compensation controls run a concurrent `InsertEntry` before removal and after the absence check respectively. Both retained membership and returned the entry in the next listing. This rules out a blanket claim that the insert compensation itself is incorrect under successful authoritative commands.

## Worked code-judo proposal

Use the store's existing indexed write path for updates:

```go
func (store *UniversalRedis2Store) UpdateEntry(ctx context.Context, entry *filer.Entry) error {
    return store.InsertEntry(ctx, entry)
}
```

This is a proposal only; it was not applied. It replaces the special assumption that updates always find an intact member with one write invariant: every successful value write also ensures its name is indexed. It reuses existing prefix handling, score zero, root handling, super-large-directory handling, and insertion error wrapping. It adds one idempotent index command to updates for ordinary named entries and allows an index failure to make the update return an error, as insert already does. Those are deliberate operational consequences that need validation.

For a write completing before the repair check, the primary existence check restores the name. For a write completing after the repair check, the shared write path adds the name. The proposal closes this finding under successful commands, but it does not close replica-read or failed-compensation findings; those have separate remedies in `02_repair_transport_and_failures.md`.

An extracted shared `persistIndexedEntry` helper is also possible, but routing update through the already canonical insert implementation avoids creating another thin wrapper. Do not copy the membership logic into update independently.

Add a deterministic regression test that pauses the update after its original read, removes/expires the original value, evaluates the cleanup check as absent, and then completes the update with TTL disabled. Assert membership, listing visibility, and a successful update. Repeat with a key prefix and a configured super-large directory to protect the existing write guards. Those tests can use command hooks without requiring a Redis fixture for the interleaving.

## Measurements and commands

`git diff --stat main...review-head` reports two changed files and 160 additions: 17 production lines and 143 test lines. `git show main:weed/filer/redis2/universal_redis_store.go | wc -l` reports 242 lines at the base. `wc -l weed/filer/redis2/*.go` reports 259 production-store lines and 143 test-file lines at the head, with 620 lines across the package. Neither file approaches the 1000-line threshold.

The source was inspected with `git diff main...review-head`, `nl -ba weed/filer/redis2/universal_redis_store.go`, and focused reads of the update RPC, filer update method, and store wrapper. The wrapper removes request cancellation from normal listing and mutation contexts; client-disconnect cancellation is therefore not used as evidence for a production failure in this review.

The baseline check from the clone root was:

```sh
GOMODCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-037/clone-cache/gomodcache \
GOCACHE=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-037/clone-cache/gocache \
GOFLAGS=-mod=mod GOPROXY=off GOTOOLCHAIN=local \
go test -count=1 -v ./weed/filer/redis2
```

It exited zero. All four Redis-dependent leaf cases skipped. No live expiry or insertion behavior was tested by that command.

The reproduction run used the same environment and this invocation:

```sh
go test -count=1 -v -run '^TestReview' \
  -overlay=/home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-037/clone-work/review-probes/overlay.json \
  ./weed/filer/redis2
```

It exited zero in approximately 0.02 seconds of package test execution. Each package/flag set was run once. The scratch source is `../review-probes/probe_test.go`; the overlay maps the existing test file to that source and does not replace production code. No fixture server or network was used.

## Structural audit

There is no justified reason to split a 259-line store solely because this change adds a focused helper. The helper is not an identity wrapper; it names a multi-command repair operation. The new conditional belongs to the existing missing-value branch. No casts, loosely typed payload model, configurable cleanup mode, or cross-package feature leakage was introduced.

The useful structural simplification is at the writer ownership boundary: remove the insert/update membership distinction. Turning the cleanup into additional writer-specific conditionals would make the contract harder to maintain. Extending TTL grace or rewriting the directory representation would not eliminate this update race for non-TTL removals and would change unrelated behavior.
