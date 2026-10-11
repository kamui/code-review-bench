# Redis2 cleanup consistency and architecture

## Scope and measurements

Reviewed the two committed changes at the pinned head, not only the first commit. The production diff adds a helper call in the not-found branch and a 15-line helper; the new test file adds 143 lines. `git diff main...review-head --numstat` reports +17/−0 and +143/−0. `git show main:weed/filer/redis2/universal_redis_store.go | wc -l` reports 242, and `wc -l weed/filer/redis2/*.go` reports 259 for the production file and 143 for the new tests, with 620 lines across the package.

Source evidence was read with `nl -ba` for the production file, test file, and cluster initializer. Related standalone and sentinel initializers and the package KV helpers were also inspected for ownership and routing. No ambient guidance was read. The analysis is based on current source and controlled command responses; the packet's earlier reviews were not evidence for these findings.

The helper is a useful local boundary. It does not require another module or a generic policy framework. Its current void return type, however, hides the distinction between successful pruning, successful compensation, and an unresolved destructive mutation. The parent listing sets its error to nil and continues at lines 208–210 in every case.

## Finding 1: failed compensation loses the repair target

The actionable finding is anchored at `universal_redis_store.go:250`; related new code is at lines 237–251.

`InsertEntry` performs `SET` at line 86 before its parent `ZAddNX` at line 68. Listing enumerates the parent index at lines 189–194 and reads values at line 203. The helper removes the member at line 238, reads existence at line 245, and may restore membership at line 250. `UpdateEntry` at lines 92–95 writes only the value. This establishes that membership is required for subsequent listings and that an ordinary update is not a repair mechanism.

A concrete interleaving is:

1. The directory index contains the name, but the value is absent. Listing reads `redis.Nil` and enters the new helper.
2. A concurrent insert sets a live, non-expiring value and completes `ZADD NX`. That add is a no-op because the old member is still present.
3. Cleanup successfully removes the member and observes `EXISTS = 1`.
4. Restoration receives a final command error, for example a transport failure after the client's retry policy has been exhausted. The helper discards the returned command and its error.
5. The listing clears its original not-found error and returns successfully. A later index scan contains no name, so there is no subsequent helper call to repair this value.

This sequence requires neither malformed data nor a failing insert. A successful insert followed by a failing listing-side compensation is enough. The best-effort principle is asymmetric: leaving an orphan indexed is harmless and retryable; leaving a live value unindexed removes the repair target.

There is a second manifestation of the same recovery defect at lines 238–239. A transport error does not prove that Redis failed to apply `ZREM`. If the server applies it but the response is lost and no retry obtains a result, returning immediately skips even the existence check. An error-aware helper must regard that outcome as potentially destructive, not as proof that state was unchanged. Simply removing the early return is insufficient if the subsequent restoration can also fail.

Verification: the scratch overlay's `restore_failure` and `remove_reply_lost` cases exercise `ListDirectoryEntries`, then perform a second listing. Both finish with a live value, no member, zero callbacks, and nil listing errors. Each result is reproduced with an empty prefix and `sw:`. These are deterministic final command outcomes, not actual transport experiments. The controlled tests establish how the code responds to those outcomes; no live outage was induced.

Remediation must preserve a way to revisit the path. Propagating a repair error makes the failure visible, but alone cannot fix discoverability because retrying a listing still starts from the now-empty index. Bounded retries reduce the probability without satisfying the invariant. Logging alone has the same limitation. Make recovery ownership explicit, or make the conditional mutation atomic so compensation is unnecessary.

## Finding 2: replica absence is not authoritative

The actionable finding is anchored at `universal_redis_store.go:245–247`.

The cluster initializer reads `useReadOnly` and `routeByLatency` at lines 40–41 and passes them into `redis.ClusterOptions.ReadOnly` and `.RouteByLatency` at lines 53–54. These are supported opt-in configurations, although both defaults are false. Standalone and sentinel clients in this package do not enable these cluster read modes.

The pinned dependency is `github.com/redis/go-redis/v9 v9.21.0` (`go.mod:149`). Its cached `osscluster.go` documents that `RouteByLatency` automatically enables read-only routing, sets `ReadOnly = true` at lines 205–206, and routes read-only commands via `slotReadOnlyNode` in `cmdNode` at lines 2399–2402. The latter selects a closest node or a replica at lines 2438–2450 and below. `EXISTS`, like `GET`, is a read-only Redis command; using this client does not make the existence check authoritative. Writes use the slot primary. The entry value and parent index can also be in different slots, so a completed index write cannot establish that the value replica has caught up.

A concrete interleaving starts with an old orphaned index name. An insert recreates the value on its primary and finishes its index add. The value replica has not received the new `SET`, so the initial `FindEntry` still returns not found. The helper then removes the member on the index primary. Its `EXISTS` is routed as a read and returns zero from the same lagging value replica. Lines 246–247 accept that zero and return without compensation. When replication catches up, the value exists but the index no longer contains its name.

Before the change, the same stale `GET` skipped that listing's result while leaving the member available for future scans. The PR converts transient replica lag into persistent omission. This is distinct from finding 1: every command in this sequence can succeed. Better handling of command errors does not solve a successful stale read.

Verification: the `replica_lag` overlay case supplies a zero existence response while the modeled primary value is live. Both prefixes finish with `live=true indexed=false listed=0`. Dependency routing and store options were inspected separately. A real cluster with delayed replication was unavailable; therefore replica topology, lag duration, and network routing were not exercised. The conclusion combines a verified response-handling defect with the supported client's documented source-level routing behavior.

The immediate boundary correction is to read the value's primary for cleanup decisions, not the index's primary and not the ordinary replica-enabled read client. Cached go-redis exposes `ClusterClient.MasterForKey` at `osscluster.go:2481–2488`. Selecting a node for the fully prefixed value key is a possible implementation point; topology changes and routing errors must remain uncertain outcomes rather than being turned into absence. Keep cluster specifics behind the store boundary so `ListDirectoryEntries` does not gain client-type branches. An authoritative preflight before mutation can avoid deleting membership for a value already present, but does not remove the need to handle concurrent inserts and failed compensation after that preflight.

## Worked code-judo proposal: delete compensation by changing ownership

The existing layout uses the full path for a value key and `<dir>\x00` for an index key. In a cluster these keys generally occupy different slots. A two-key Lua script or Redis transaction over those existing keys is not a valid drop-in fix. The store's transaction methods at lines 46–53 are no-ops and cannot supply atomicity.

A versioned layout can make the directory own both its child values and its child index. For example, use a canonical encoded directory identifier as the Redis hash tag, while encoding names outside the tag:

```text
redis2:v2:{encoded-directory}:entry:encoded-name
redis2:v2:{encoded-directory}:children
```

All reads and mutations would derive these keys through the store's canonical key utility, including the configured prefix. Prefixes and brace handling need an explicit compatibility contract. Entry value keys keep the same physical TTL duration. The migration must separately handle existing value keys, old indexes, rolling upgrades, and large-directory policy; it cannot be hidden inside this 17-line fix.

With those two keys in one slot, the cleanup operation becomes a write-routed atomic script:

```lua
if redis.call('EXISTS', KEYS[1]) == 0 then
    return redis.call('ZREM', KEYS[2], ARGV[1])
end
return 0
```

Here `KEYS[1]` is the child value, `KEYS[2]` is the directory index, and `ARGV[1]` is the child name. The write operation executes on the primary for the common slot. This single operation replaces removal, the compensating existence read, and restoration. A recreated value visible before the script prevents removal. If the script removes before a new insert writes its value, that insert's later index add restores membership. If the insert's value write has happened but its index add has not, the script sees the value and does not delete the member. A lost script reply cannot produce the current live-value/unindexed-state through this cleanup interleaving because the check and removal execute together.

This removes a category of states instead of extracting the same unsafe choreography into more helpers. It is the strongest structural simplification, but it is a larger storage-design change and requires migration review and targeted tests. It does not claim to repair unrelated races in deletion or TTL-based updates elsewhere in the existing store.

## Worked current-layout alternative: own reconciliation explicitly

If a schema migration is outside scope, preserve the two-key layout but stop pretending that a void best-effort helper finishes every repair. Establish authoritative value routing and a durable pending-repair owner. A sketch of the contract is:

```text
repair id := persist a unique pending record(value key, index key, name)
if recording fails: leave membership unchanged
attempt removal; any error means removal may have occurred
re-read value on its primary
if definitely absent: finish the record
if present or uncertain: attempt index restoration
if restoration is confirmed: finish the record
otherwise: retain the record for reconciliation after reconnect/restart
```

Create the record before destructive mutation, and use unique repair identities so overlapping listings cannot erase another operation's pending record. A background owner must consume retained records, recheck authoritatively, and restore conservatively on uncertainty; storing an error without any replay consumer is insufficient. Idempotent replay may temporarily reintroduce an orphan when deletion races restoration, which the existing listing cleanup can safely revisit. The design must specify record durability and acknowledgement ordering. Cancellation must not silently abandon a record after removal.

This approach is more machinery than the same-slot script, so it should be chosen explicitly rather than added as scattered retry branches. An interim conservative option is to leave orphan membership untouched in modes where authoritative inspection and recovery cannot be guaranteed. That sacrifices pruning in those modes without introducing permanent invisibility.

## Other audit decisions

No production file approaches 1,000 lines. The helper is not a pass-through wrapper, and splitting these 259 lines into another module would not improve the state model. `getKey` is correctly reused for the directory index at line 178 and for the value at line 245, so the introduced cleanup is prefix-aware. The not-found guard uses the sentinel that `FindEntry` returns only for `redis.Nil`; ordinary initial GET errors do not enter cleanup. New logic does not leak into the filer API or generic KV layer.

The adjacent logical-expiry branch still has older deletion risks, but it is unchanged and is not raised as a new finding. Nor is the superseded first-commit unconditional deletion reported as if it remained at the pinned head. The actionable feedback concerns the current helper's recovery and authority contracts.

The new sequential commands are causally dependent, so parallelizing them is not a valid simplification. The right code-judo move reduces the mutation protocol or defines its recovery owner; a pipeline would not give cross-slot atomicity.
