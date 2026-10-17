# C2na: an overwrite that lands between the expiry branch's read and its delete has its new record deleted

Pull request: seaweedfs/seaweedfs #10735, "fix(redis2): remove orphaned directory index members on listing".
Commit before the change ("base"): `db5a086d0`. Reviewed commit ("head"): `6c8fde664`.
Cut-off: the last push, **2026-08-13T14:12:47Z**. Each fact is marked as knowable then, or later.

Each fact is marked **run** (probes `probes/C2/`, extracted in `probes/C2na/`; `probes/grouping/`; `probes/P2/` section 2), **read** or **reported**.

This group is one of two results of its own that the comment of credit question **C2** describes ("A concurrent InsertEntry or UpdateEntry then rewrites the same path"): the overwrite in the first window. Its sibling C2nb is the overwrite in the second window. They are split because their results (record deleted against name lost), lines and smallest fixes differ, as PE-s3's and PE-s2's do (run).

> **What happens:** a listing reads a file that is past its creation time plus time-to-live; an overwrite of the same path (`UpdateEntry`, a `SET` alone) lands after that read and before the listing's `DEL`; the overwrite returns success and its new record is deleted. **Promised: yes**, written: "The Filer Store persists all file metadata and directory information." (wiki); this is PE-s3's promise, which the owner admitted (activation ruling 4). **Delivered: no**, the file is gone by path and from listings after a successful store (run at both commits). **So: duplicate of PE-s3**, the same fault reached by another call (an overwrite instead of a create); a question of widening PE-s3's wording. **Band, decided separately:** none here; PE-s3 is labelled serious.

## Problem

The listing's expiry branch (head lines 214-219, unchanged):

```go
214			if entry.TtlSec > 0 {
215				if entry.Attr.Crtime.Add(time.Duration(entry.TtlSec) * time.Second).Before(time.Now()) {
216					store.Client.Del(ctx, store.getKey(string(path))).Result()
217					store.Client.ZRem(ctx, dirListKey, fileName).Result()
218					continue
```

It decides from the record it read, then deletes whatever the key holds when the `DEL` arrives. PE-s3 is a create landing in that gap. An overwrite, `UpdateEntry` (lines 92-95, `SET` only), landing there is deleted the same way.

How an overwrite reaches it (read, run): `Filer.CreateEntry` of a path that exists calls `Filer.UpdateEntry` and the store's `UpdateEntry` (`weed/filer/filer.go:240-284`); end to end, a second HTTP store of an existing file sends `GET`s and a `SET`, and no `ZADD` (run, `probes/P2/result-end-to-end-*.txt` section 2). For a file already past its time, the filer's own read deletes it first (`filer.go:434-435`), so a later upload creates instead; the overwrite reaches the window when the writer read the file before its time ran out and writes after the listing's read (run, `C2E`, through the filer).

## What changed

Nothing at this place: lines 214-219 and 92-95 are unchanged and identical at base and head (read). The behaviour is the same at both commits (run). As a pre-existing matter, rule Before 2 is not asked.

## Runs

All **run**, real Redis 8.2.1, the real store code of each commit, order forced with a client hook, no error injected. Identical at base and head.

| Case | What is run | Base | Head |
| --- | --- | --- | --- |
| `C2B` | store `UpdateEntry(f)` after the listing's `GET` of `f` (past its time, record in Redis) and before its `DEL` | the update returns no error; afterwards no record, no name; `FindEntry` not found | the same |
| `C2E` | through the filer: a writer reads `f` 1 s before its time runs out; a listing runs after; the writer's `Filer.UpdateEntry(f)` lands after the listing's `GET` and before its `DEL` | the update returns no error; afterwards no record, no name | the same |
| `C2A` (PE-s3 itself, for comparison) | store `InsertEntry(f)` in the same place | the same result | the same |
| grouping, "C2n update before the delete" | as `C2B` | FAILS | FAILS |

The order in `C2B`, the same at both commits:

```text
1. lister:   ZRANGEBYLEX <dir>\x00 - + limit 0 100  ->  [f]
2. lister:   GET <dir>/f  ->  <entry bytes>
3. writer:   SET <dir>/f <entry bytes>  ->  OK
4. writer:   UpdateEntry(f) returned err=<nil>
5. lister:   DEL <dir>/f  ->  1
6. lister:   ZREM <dir>\x00 f  ->  1
```

## Promised?

**The operation:** overwriting a stored file on a `redis2` filer while a listing expires its earlier copy. **Owner:** SeaweedFS. **Classification:** public. **How it is made: written** (rule Promised 4a): the promise the owner found for PE-s3. Evidence against: the maintainer's 2023 "So this may happen" (#4418), which the owner read as an acknowledgement of a fault when admitting PE-s2 and PE-s3.

| Place | Query as run | Saved | Hits / read | Found |
| --- | --- | --- | --- | --- |
| `project-docs` | the wiki at `c950d274`: `Filer-Stores.md` lines 1-5 and the Redis rows; `rg -n -i 'ttl'` on `Store-file-with-a-Time-To-Live.md`, `Filer-Server-API.md`, `Directories-and-Files.md`; the documented upload in `Filer-Server-API.md` | `upstream/project-docs.txt` (Q4, Q7, Q8) | 47 / 47 | "The Filer Store persists all file metadata and directory information." `Filer-Server-API.md:23`: "POST /path/to/file?ttl=1d" ("// set file TTL"). Nothing says an overwrite may be lost to a listing. |
| `owner-docs` | Redis 8.2.1 `src/commands/{del,set}.json` | `upstream/owner-docs.txt` | 2 / 2 | `DEL` "Deletes one or more keys."; `SET` "Sets the string value of a key, ignoring its type." Redis owns no part of it. |
| `change` | `rg -i 'UpdateEntry\|expiry branch\|logical\|intended behaviour\|else. branch\|deliberately'` on the description at the cut-off, commit messages and diff | `upstream/change.txt` (Q4) | 11 / 11 | The description calls the expiry branch "the intended behaviour — the code already means to keep the index consistent". The head commit: "UpdateEntry does not re-add it, so the loss persisted." Nothing on a record deleted under a write. |
| `maintainers` | `gh api -X GET search/issues`, 18 queries `created:<2026-08-13T14:12:47Z` (list in `maintainers-read.json`), among them 'redis2 UpdateEntry ttl', 'redis "same transaction"', 'redis2 race' | `upstream/maintainers-read.json`, `issue-4418*.json` | 135 distinct threads / 23 opened, the rest read by title | #4418, `chrislusf`, 2023-04-24: "In redis, the operations to "add a file" and "add the file to the directory" are not in the same transaction. So this may happen." Nothing on the expiry branch or an overwrite. |
| `public-code` | `gh api -X GET search/code '"[redis2]" "enabled = true" filename:filer.toml'`; every file fetched and its enabled store read | `upstream/public-code-read.json` | 26 / 26 | Three repositories enable `redis2` (`ok1996/ok-system-simple`, `lewisl9029/seaweed-async-replication-repro`, `my-piper/piper-swarm`). Ordinary use of the store; no promise. |
| `documented-way` | `sh probes/P2/end-to-end.sh ...`, section 2, at base and head: a file stored with `?ttl=1m`, then stored again | `probes/P2/result-end-to-end-{base,head}.txt` | 1 / 1 | The second store of an existing file sends `GET`s and a `SET`, no `ZADD`: the overwrite path this group needs. With no listing in between, it keeps the file. |

## Delivered?

A store request that returned success leaves no file (run). Not delivered.

## Intended or announced

- At the cut-off (read): nothing on this race.
- **Known only later** (read): the maintainer's #10744 (opened 2026-08-13T17:14:18Z, released in 4.42) changed the branch to delete only if the record still holds the bytes the decision was made on: "deletes through a single-key compare-and-delete script, so the `DEL` fires only if the value still holds the bytes the decision was made on — a recreate that lands mid-flight survives untouched". That shape (in the grouping runs, `fix-PE-s3.patch`, a `WATCH` on the record) cures both a create and an overwrite.

## What the affected person sees

A program that read a file shortly before its time-to-live ran out writes it again while a listing of its directory is expiring the old copy. The write returns success; the file is then not found by its path and is in no listing. Nothing is logged (run, through the store and through the filer). Before the change: the same.

## What the maintainers did

Later (read): fixed the window for any write in #10744, quoted above.

## How each fact is known

| Fact | Known by |
| --- | --- |
| An overwrite between the listing's `GET` and `DEL` is deleted, at both commits, at store and filer level | run |
| An overwrite of an existing file is a `SET` with no `ZADD` | run (end to end), read (`UpdateEntry`) |
| The filer deletes an expired file on its own read, so an overwrite reaches the window only from a read made before the deadline | read (`filer.go:434-435`), run (`C2E`) |
| #10744 | read, later |

## Relation to existing reference families and ruled claims

- **Already decided?** Not exactly: PE-s3 and its claim `CL-s-expiry-deleted-record` name "a create". `already_decided_by`: null.
- **Grouping tests against PE-s3** (run, `probes/grouping/table.json`):
  - *Same lines?* **Yes**: line 216, the `DEL` after the read.
  - *One fix?* **Yes**: PE-s3's smallest fix (`fix-PE-s3.patch`: delete only if the record is still the expired one) cures "PE-s3" and "C2n update before the delete"; no other fix cures either (PE-s2's, P2's, GT-s2's to GT-s5's leave it failing).
  - *One sentence?* **Yes**: this sentence is true of both: *the expiry branch deletes whatever record the key holds when its `DEL` arrives, so a write between its read and its delete is deleted.*
- Against PE-s2: no on all three (another window; PE-s2's fix leaves it failing; PE-s2's sentence is about a name).
- **Clean control:** no. `touches_clean_control`: false.

## Both sides

For joining it to PE-s3: the same lines, the same fix, the same sentence and the same result, by running; only the call differs (an overwrite, which the filer issues when it finds the file).

For keeping it apart: PE-s3's wording says "a create"; an overwrite reaches the window only when the writer read the file before its time ran out (the filer's own read deletes an expired file first), a narrower route than PE-s3's.

## Recommendation

> **What happens:** an overwrite that lands between the expiry branch's read and its `DEL` returns success and is deleted. **Promised: yes**, written: the wiki's persistence sentence, the promise the owner found for PE-s3. **Delivered: no**, the file is gone after a successful store (run at both commits). **So: duplicate of PE-s3**: all three grouping tests yes (run); a question of widening PE-s3's "a create" to any store of the path. **Band, decided separately:** none here; PE-s3 is serious.

**Confidence: medium.** Short of high on one condition, `no-precedent`: the joins listed in rule Before 4 (for example a second `parseBody()` call joined with GT-p1, second-pass 10) are of this shape, but whether they are indexed as made under the current reading was not checked here.

**Strongest argument against:** the overwrite reaches the window only from a read made before the deadline, so most real overwrites of an expired file go through the create path and are already PE-s3; the variant may add nothing an owner would want counted separately, or at all.
