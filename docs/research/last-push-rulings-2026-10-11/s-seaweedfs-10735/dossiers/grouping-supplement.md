# Grouping supplement: are S1 and S2 one known problem or two

Pull request: seaweedfs/seaweedfs #10735. This file adds runs to `S1.md` and `S2.md`, which stay as written. It answers one question the owner asked after ruling that the problem stays: what do the three tests of rule Before 4 say about S1 and S2? It gives the facts for both readings of each test. It makes no recommendation. Grouping is the owner's.

The case files said, as *read*, that a check or retry of the put-back does not run when the helper has already returned on a removal error, and that removing the early return does not help when the put-back itself fails. Both are now *run*.

## The two sites

Both are in the helper the pull request adds, `removeOrphanedDirectoryListMember` (`weed/filer/redis2/universal_redis_store.go`, head lines 237 to 251). The comments on the right are this file's labels, not the code's.

```go
	if err := store.Client.ZRem(ctx, dirListKey, fileName).Err(); err != nil {   // lines 238 to 240: S2
		return
	}
	...
	store.Client.ZAddNX(ctx, dirListKey, redis.Z{Score: 0, Member: fileName})    // line 250: S1
```

- S1 is line 250. The put-back can fail, and the code does not look at its result.
- S2 is lines 238 to 240. An error from the removal ends the helper, although Redis may have run the removal.
- Line 238 is also the removal itself. Without it neither site can lose a name. The base has no removal on this branch and keeps the name in every scenario (*run*, `probes/S1/result-base.txt`, `probes/S2/result-base.txt`).

## The three patched variants

Each is the smallest change a reviewer of that one site would propose. Each touches only this helper. The patches are in `probes/grouping/`.

- **fix-1, the put-back only** (`fix-1.patch`). The put-back is tried up to three times in total, 100 ms apart. When the third attempt fails, the helper writes one log line, "restore <path>: <error>", and returns. The listing still reports success. The early return on a removal error is left as it is.
- **fix-2, the early return only** (`fix-2.patch`). The `if err ... return` around the removal is gone. The helper always goes on to the re-check and the put-back. The put-back's result is still thrown away.
- **both** (`fix-both.patch`).

The client's own three retries of each command are unchanged in every variant.

## The runs

The real store code of each build ran against a standalone Redis 8.2.1. Every scenario has the path created again between the listing's lookup and its next command. The order of commands was forced, not raced. Outputs: `probes/grouping/result-head.txt`, `result-fix-1.txt`, `result-fix-2.txt`, `result-fix-both.txt`. All *run*.

In every cell the listing returned `names=[] err=<nil>`: no error on any build in any scenario. "Lost" means that after the listing the file's value exists and its name is in no list.

| Scenario | Head | fix-1 | fix-2 | both |
| --- | --- | --- | --- | --- |
| **S1 C.** The put-back fails once (injected) | lost | **kept** | lost | **kept** |
| **S1 D.** Redis unreachable from just after the removal until the listing has returned | lost | lost, one log line | lost | lost, one log line |
| **S1 H.** Redis over its memory limit from the put-back until the listing has returned | lost | lost, one log line | lost | lost, one log line |
| **S2 A.** Redis runs the removal; the client is handed an error for it (injected) | lost | lost | **kept** | **kept** |
| **S2 B2.** Redis runs the removal; its reply is lost once; retries switched off (`max_retries = -1`) | lost | lost | **kept** | **kept** |
| **S2 C.** Redis runs the removal and stops answering until the listing has returned (read timeout 150 ms) | lost | lost | lost | lost, one log line |
| **X1.** Every put-back attempt fails (injected) | lost | lost, one log line | lost | lost, one log line |
| **X2.** Redis unreachable from just after the removal, back after 5 seconds | lost | **kept** | lost | **kept** |
| **X3.** Redis runs the removal, its reply is lost, and Redis is unreachable from then until the listing has returned (default retries) | lost | lost | lost | lost, one log line |

X1 to X3 are new in this run (`probes/grouping/zz_grouping_test.go`). The other six are the saved scenarios, unchanged.

Controls, on all four builds (*run*): with no fault the name is kept (S1 scenario B). With default retries one lost reply is repaired by the client and the name is kept (S2 scenario B). A real orphan with a fault is removed and nothing is lost (S1 scenario G); fix-1 and both write their log line there too, although nothing needed putting back. The pull request's own tests pass.

What the table shows:

- **Each site's fix cures only its own site's scenarios.** fix-1 keeps the name in S1 C and X2 and changes nothing in S2 A, B2 or C: the helper has returned before the put-back, so the retry never runs and nothing is logged. fix-2 keeps the name in S2 A and B2 and changes nothing in S1 C, D, H, X1 or X2: the put-back runs once, fails, and its result is thrown away.
- **Both together cure every scenario that either cures, and no more.**
- **No variant keeps the name when Redis is still failing when the last attempt runs.** That is S1 D and H, S2 C, X1 and X3. In each the name is lost on all four builds.
- **A later listing cannot repair it, on any build.** In every "lost" cell, two later listings on a healthy Redis returned `names=[]`, and the state after them was unchanged: value present, name absent. No variant leaves anything for a later listing or write to find. With fix-1 or both there is one log line naming the path, when the put-back was reached.
- **How long the fault must last.** With Redis unreachable, the head gives up on the put-back 3.4 seconds after the removal. fix-1 gives up after 7.0 seconds. So fix-1 keeps the name through the 5 second outage of X2 and loses it in S1 D.
- **A full Redis.** In S1 H Redis refuses the put-back with "OOM command not allowed when used memory > 'maxmemory'." on every attempt. The retries do not help while the limit stands.
- **S2 C, exactly.** The removal reports "i/o timeout". On fix-2 the re-check and the put-back then report "i/o timeout" too. Redis never receives them: with replies withheld, each new connection gets as far as its opening `hello` and no further (*run*, `probes/grouping/result-monitor-fix-2-S2C.txt`). So a Redis that has gone silent at the removal defeats both sites in the same request. X3 shows the same with Redis unreachable instead of silent.

`S2.md` and the case file for S2 marked this last point as reasoned, not run. It is now run.

## The three tests of rule Before 4

The rule: "Two symptoms are the same fault when all three hold: the same lines cause both, one fix in the project cures both, and one sentence about the cause is true of both."

### 1. Do the same lines cause both?

It depends on what "the same lines" means.

- **Read as the same statement: no.** S1 is the unchecked `ZAddNX` at line 250. S2 is the `return` at lines 238 to 240. A patch to one does not touch the other, and each patch changes only its own site's scenarios (*run*, the table).
- **Read as the same piece of added code: yes.** Both are in one 15-line helper that the head commit adds whole. Both need the removal at line 238 to have taken effect first, and that removal is new in this pull request. With no removal, neither site loses a name (*run* at the base).

### 2. Does one fix in the project cure both?

It depends on what counts as one fix, and on which scenarios must be cured.

- **Read as one change at one site: no.** fix-1 does not cure S2 A, B2 or C. fix-2 does not cure S1 C, D, H or X2 (*run*).
- **Read as one change to the helper: partly.** The combined patch changes this one function. It cures the scenarios where the fault is over before the last attempt: S1 C, S2 A, S2 B2 and X2. It cures none of the scenarios where the fault lasts: S1 D, S1 H, S2 C, X1 and X3 (*run*).
- **The scenarios no variant cures are shared.** A lasting outage or a full Redis loses the name through the put-back when the removal's reply arrives (S1 D, S1 H), and through the early return when it does not (S2 C, X3). With both fixes applied both kinds still lose it (*run*).
- **A fix that cures all of them was not built.** It would have to stop the helper from depending on commands that come after the removal: for example a single atomic "remove only if the value is absent", or leaving a record that a later listing or write can find. The head commit's message says the atomic form "is not available to the cluster store". Whether such a fix exists for all three stores is not established here.

### 3. Is one sentence about the cause true of both?

It depends on how wide the sentence is.

- **A sentence about the design is true of both.** "The cleanup removes the name first, and the name comes back only if what follows goes right: the removal reports truthfully and the put-back succeeds." S1 is the put-back failing. S2 is the removal not reporting truthfully. The runs agree with it: every lost name follows an applied removal, and no build loses a name without one.
- **A sentence about the line is true of one only.** "The put-back's result is not checked" is true of S1 and not of S2: in S2 A, B2 and C on the head the put-back never runs (*run*). "An error from the removal is taken to mean nothing was removed" is true of S2 and not of S1: in S1 C, D and H the removal reports success (*run*).

## What the maintainer's later follow-up changed at each site

**Later than the last push.** Follow-up #10743 was opened at 2026-08-13T17:12:12Z, 2 h 59 min 25 s after the last push, and merged at 20:08:26Z. It is later evidence and no reason for an answer. Read from the saved `upstream/pr-10743.diff` and from master as fetched on 2026-10-11 (`upstream/master-universal_redis_store.go`, commit `63d2024b`).

- **The put-back line: changed.** The diff:

  ```go
  -	store.Client.ZAddNX(ctx, dirListKey, redis.Z{Score: 0, Member: fileName})
  +	if err := store.Client.ZAddNX(ctx, dirListKey, redis.Z{Score: 0, Member: fileName}).Err(); err != nil {
  +		glog.V(0).InfofCtx(ctx, "restore %s in %s: %v", fileName, dirPath, err)
  +	}
  ```

  The error is now read and logged. It is not retried, and the listing still reports success. Master has the same lines at 267 to 269.

- **The early return: not changed.** In the diff these are context lines, with no `+` or `-`:

  ```go
   	if err := store.Client.ZRem(ctx, dirListKey, fileName).Err(); err != nil {
   		return
   	}
  ```

  Master has the same lines at 250 to 252. Nothing is logged there.

- **One change above both sites.** The follow-up adds, before the removal:

  ```go
  +	// survive the listing request being canceled mid-repair
  +	ctx = context.WithoutCancel(ctx)
  ```

  It applies to all three commands of the helper. At the head a cancelled request already could not reach the helper through the filer (*run*, `probes/S1/result-head.txt`, scenarios F and F2).

- **The follow-up's own words** name one of the two sites. Its description says "The repair was not failure-atomic", names "the restoring `ZAddNX`" and "the failed restore was discarded without a log line", and says "a failed restore is logged". It does not mention an error from the removal.

So the follow-up lists the helper's failure handling under one numbered point, changes the put-back's site, and leaves the early return's site as it was. It is neither fix-1, which retries, nor fix-2. The saved runs of 2026-10-05 at the follow-up's commit, not repeated here, show the name still lost in S1 C, D and H and in S2 A, B2 and C (*reported*).

## How each fact is known

| Fact | Known by | Knowable at the last push? |
| --- | --- | --- |
| The table: name kept or lost, and the listing's return, in nine scenarios on four builds | run (`probes/grouping/result-*.txt`, `table.json`) | yes: the head and three patches to it |
| Two later listings on a healthy Redis return nothing in every "lost" cell | run (the same files) | yes |
| The head gives up 3.4 s after the removal with Redis unreachable; fix-1 after 7.0 s | run (test durations in `result-head.txt` and `result-fix-1.txt`, scenario S1 D) | yes |
| In S2 C on fix-2, Redis receives no re-check and no put-back, only connection handshakes | run (`probes/grouping/result-monitor-fix-2-S2C.txt`) | yes |
| The controls and the pull request's own tests pass on all four builds | run (the same files) | yes |
| The base loses no name in any scenario | run earlier in this round (`probes/S1/result-base.txt`, `probes/S2/result-base.txt`) | yes |
| The two sites and their line numbers; the helper is added whole by the head commit | read (the diff) | yes |
| The head commit's message on the atomic form | read (`upstream/change.json`) | yes, 14:12:32Z |
| What the follow-up changed at each site | read (`upstream/pr-10743.diff`) | **later**, 17:12:12Z |
| Master's lines at each site | read (`upstream/master-universal_redis_store.go`) | **later**, fetched 2026-10-11 |
| The name still lost at the follow-up's commit | reported: the saved run of 2026-10-05 | **later** |

## Not run

- A fix that cures every scenario at both sites. None was built, so "one fix cures both" in the widest sense is answered by reasoning only.
- Other choices for fix-1. One retry scheme was run: three attempts, 100 ms apart. A scheme that retries for longer survives a longer outage and still loses the name when the fault outlasts it or when Redis stays full.
- S2 scenario C with the default 3 second read timeout. The run used 150 ms.
- Real concurrency, `redis_cluster2`, the sentinel store, and a real Redis crash, failover or partition. The command order was forced, one standalone Redis was used, and a TCP relay stood in for a failing server.
- The follow-up's commit and master. Their behaviour is read from the diff and the saved file, and the saved run of 2026-10-05 was not repeated.
