# Review summary

## Verdict

Request changes. The cleanup is a compact addition, and the file remains far below the 1,000-line boundary (259 lines after the change). The new path also reuses the prefixed directory key already computed by the listing operation. However, its concurrent-recreation repair only covers `InsertEntry`; another supported write path can still leave a live entry without an index member.

## Finding

**The recreation repair misses `UpdateEntry`.** In `weed/filer/redis2/universal_redis_store.go`, `removeOrphanedDirectoryListMember` removes the member, checks whether the value exists, and restores membership only when that check sees it. If `UpdateEntry` writes the value after `Exists` reports absence, the helper returns without restoring the member, and `UpdateEntry` itself only calls `doInsertEntry` without adding the member. The value is then live but remains invisible to listings until a later `InsertEntry` repairs the index. Make the value-write invariant explicit across both insert and update paths, or otherwise ensure a recreation after the absence check re-adds its member. Full interleaving and a code-judo proposal are in [the Redis2 detail report](01_redis2.md).

## Remediation sequence

First establish one invariant for every successful value write: a normal-directory child has a parent index member. Route `UpdateEntry` through the index-maintaining write path (while retaining the super-large-directory exception), or use an equivalent shared operation that makes the index repair independent of which write API recreated the value. Then extend the existing integration coverage with an `UpdateEntry` recreation case that exercises the boundary after the absence observation. The current recreated-entry test calls the helper with the value already present; it does not cover a value write after the helper has decided the value is absent.

## Verification status

Reviewed the committed range `main...review-head` and surrounding write/list implementations. `git diff --check main...review-head` is clean; the working tree was clean at inspection. No tests were run: this review focused on the source diff, and the packet states that a live Redis server is unavailable.
