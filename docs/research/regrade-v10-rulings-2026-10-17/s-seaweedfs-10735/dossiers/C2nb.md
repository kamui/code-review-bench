# C2nb: an overwrite that lands between the expiry branch's delete and its removal of the name keeps its record and loses its name

Pull request: seaweedfs/seaweedfs #10735, "fix(redis2): remove orphaned directory index members on listing".
Commit before the change ("base"): `db5a086d0`. Reviewed commit ("head"): `6c8fde664`.
Cut-off: the last push, **2026-08-13T14:12:47Z**. Each fact is marked as knowable then, or later.

Each fact is marked **run** (probes `probes/C2/`, extracted in `probes/C2nb/`; `probes/grouping/`; `probes/P2/` section 2), **read** or **reported**.

This group is the second result of its own that the comment of credit question **C2** describes ("A concurrent InsertEntry or UpdateEntry then rewrites the same path"): the overwrite in the second window. Split from C2na for the reasons given there.

> **What happens:** a listing deletes the record of a file past its creation time plus time-to-live and then removes its name with a separate `ZREM`; an overwrite of the same path (`UpdateEntry`, a `SET` alone) landing between the two returns success, keeps its record and loses its name for good. **Promised: yes**, written: "The Filer Store persists all file metadata and directory information." (wiki); this is PE-s2's promise, which the owner admitted (activation ruling 3), and the head commit itself names this loss with an overwrite ("UpdateEntry does not re-add it, so the loss persisted"). **Delivered: no**, the file reads by path and is in no later listing (run at both commits). **So: duplicate of PE-s2**, the same fault reached by another call; a question of widening PE-s2's "a create". **Band, decided separately:** none here; PE-s2 is labelled serious.

## Problem

The expiry branch (head lines 216-217, unchanged) sends `DEL` of the record and then a separate `ZREM` of the name. PE-s2 is a create landing between them: it finds the name present and adds nothing. An overwrite (`UpdateEntry`, lines 92-95) never adds a name at all, so landing there it ends the same way: record present, name removed.

How an overwrite reaches it (read, run): the filer overwrites through `Filer.UpdateEntry` when it finds the file (`filer.go:240-284`); end to end a second HTTP store of an existing file sends `SET` and no `ZADD` (run, section 2). Because the filer deletes an already expired file on its own read (`filer.go:434-435`), the overwrite reaches the window when the writer read the file before its time ran out (run, `C2F`, through the filer).

## What changed

Nothing at this place: lines 216-217 and 92-95 are unchanged and identical at base and head (read). The head commit repaired the same loss, with an insert, only in the cleanup it added. The behaviour is the same at both commits (run).

## Runs

All **run**, real Redis 8.2.1, the real store code of each commit, order forced with a client hook, no error injected. Identical at base and head.

| Case | What is run | Base | Head |
| --- | --- | --- | --- |
| `C2D` | store `UpdateEntry(f)` after the listing's `DEL` of `f` and before its `ZREM` | the update returns no error; the record exists; the list holds no name; a later listing returns nothing; `FindEntry` finds `f` | the same |
| `C2F` | through the filer: a writer reads `f` 1 s before its time runs out; a listing runs after; the writer's `Filer.UpdateEntry(f)` lands after the listing's `DEL` and before its `ZREM` | the same result | the same |
| `C2C` (PE-s2 itself, for comparison) | store `InsertEntry(f)` in the same place | the same result | the same |
| grouping, "C2n update before the ZREM" | as `C2D` | FAILS | FAILS |

The order in `C2D`, the same at both commits:

```text
1. lister:   ZRANGEBYLEX <dir>\x00 - + limit 0 100  ->  [f]
2. lister:   GET <dir>/f  ->  <entry bytes>
3. lister:   DEL <dir>/f  ->  1
4. writer:   SET <dir>/f <entry bytes>  ->  OK
5. writer:   UpdateEntry(f) returned err=<nil>
6. lister:   ZREM <dir>\x00 f  ->  1
```

## Promised?

**The operation:** overwriting a stored file on a `redis2` filer while a listing expires its earlier copy, and listing its directory afterwards. **Owner:** SeaweedFS. **Classification:** public. **How it is made: written** (rule Promised 4a), the promise the owner found for PE-s2; with a **built** sign, `InsertEntry`'s error when its own name cannot be added ("persisting %s in parent dir: %v", line 69). Evidence against: #4418's "So this may happen", which the owner read as an acknowledgement of a fault (activation 3, A26).

| Place | Query as run | Saved | Hits / read | Found |
| --- | --- | --- | --- | --- |
| `project-docs` | the wiki at `c950d274`: `Filer-Stores.md` lines 1-5 and the Redis rows; `rg -n -i 'ttl'` on `Store-file-with-a-Time-To-Live.md`, `Filer-Server-API.md`, `Directories-and-Files.md`; the documented upload and delete in `Filer-Server-API.md` | `upstream/project-docs.txt` (Q4, Q7, Q8) | 47 / 47 | "The Filer Store persists all file metadata and directory information." `Filer-Server-API.md:23`: "POST /path/to/file?ttl=1d". Nothing says an overwritten file may drop out of its directory. |
| `owner-docs` | Redis 8.2.1 `src/commands/{del,zrem,set}.json` | `upstream/owner-docs.txt` | 3 / 3 | Each command does what Redis documents. |
| `change` | `rg -i 'UpdateEntry\|expiry branch\|logical\|intended behaviour\|else. branch\|deliberately'` on the description at the cut-off, commit messages and diff | `upstream/change.txt` (Q4) | 11 / 11 | The head commit, about the not-found branch: "UpdateEntry does not re-add it, so the loss persisted." The description on the expiry branch: "The second branch is the intended behaviour — the code already means to keep the index consistent." |
| `maintainers` | `gh api -X GET search/issues`, 18 queries `created:<2026-08-13T14:12:47Z`, among them 'redis2 UpdateEntry ttl', 'redis "same transaction"', 'redis2 race' | `upstream/maintainers-read.json`, `issue-4418*.json` | 135 distinct threads / 23 opened, the rest read by title | #4418, `chrislusf`, 2023-04-24: "In redis, the operations to "add a file" and "add the file to the directory" are not in the same transaction. So this may happen." |
| `public-code` | `gh api -X GET search/code '"[redis2]" "enabled = true" filename:filer.toml'`; every file fetched and its enabled store read | `upstream/public-code-read.json` | 26 / 26 | Three repositories enable `redis2` (`ok1996/ok-system-simple`, `lewisl9029/seaweed-async-replication-repro`, `my-piper/piper-swarm`). Ordinary use of the store; no promise. |
| `documented-way` | `sh probes/P2/end-to-end.sh ...`, section 2, at base and head | `probes/P2/result-end-to-end-{base,head}.txt` | 1 / 1 | A second HTTP store of an existing file sends `SET` with no `ZADD`; with no listing in between, the file stays listed. |

## Delivered?

A store request that returned success leaves a file that reads by path and is in no listing (run). Not delivered.

## Intended or announced

- At the cut-off (read): the head commit fixes this loss for the not-found branch and names the overwrite as why it persists; nothing on the expiry branch.
- **Known only later** (read): the author's edit of the description 81 s after the last push: "**Also deliberately not included:** the expiry branch at `:214-219` has a pre-existing instance of the same race, with a worse outcome". The maintainer's #10744 (released in 4.42) "removes the member through `removeOrphanedDirectoryListMember` instead of a bare `ZREM`, which restores the member if an insert races the removal". That shape (`fix-PE-s2.patch`) cures the overwrite too (run).

## What the affected person sees

A program that read a file shortly before its time-to-live ran out writes it again while a listing is expiring the old copy. The write returns success; the file reads by its path and is missing from every later listing; another overwrite does not bring it back (it never adds a name); a recursive delete of the directory would leave its record: the end state, a record without its name, is the one whose aftermath was run for P2 (case A); it was not run again from this case. Before the change: the same.

## What the maintainers did

Later (read): #10744, quoted above.

## How each fact is known

| Fact | Known by |
| --- | --- |
| An overwrite between the expiry branch's `DEL` and `ZREM` keeps its record and loses its name, at both commits, at store and filer level | run |
| An overwrite never adds a name | read (`UpdateEntry`), run (end to end) |
| The overwrite reaches the window only from a read made before the deadline | read (`filer.go:434-435`), run (`C2F`) |
| The head commit's sentence on `UpdateEntry` | read (knowable at the cut-off) |
| The author's later edit and #10744 | read, later |

## Relation to existing reference families and ruled claims

- **Already decided?** Not exactly: PE-s2 and its claim `CL-s-expiry-lost-name` name "a create". `already_decided_by`: null.
- **Grouping tests against PE-s2** (run, `probes/grouping/table.json`):
  - *Same lines?* **Yes**: lines 216-217.
  - *One fix?* **Yes**: PE-s2's smallest fix (`fix-PE-s2.patch`, the branch calls the helper, which puts the name back when the record exists again) cures "PE-s2" and "C2n update before the ZREM"; no other single fix cures either.
  - *One sentence?* **Yes**: this sentence is true of both: *the expiry branch removes the name in a separate command after deleting the record, so a store of the same path between the two keeps its record and loses its name.*
- Against P2 (`DeleteEntry`'s same shape): no on lines and fix (P2's fix leaves this failing; this fix leaves P2 failing), yes on one sentence; against GT-s2: GT-s2's fix (`UpdateEntry` adds the name) does not cure it, because the `ZREM` comes after the overwrite.
- **Clean control:** no. `touches_clean_control`: false.

## Both sides

For joining it to PE-s2: same lines, same fix, same sentence and same result, by running; the head commit itself names the overwrite as the call that cannot repair a lost name.

For keeping it apart: PE-s2's wording says "a create"; an overwrite reaches the window only from a read made before the deadline, a narrower route.

## Recommendation

> **What happens:** an overwrite that lands between the expiry branch's `DEL` and its `ZREM` returns success, keeps its record and loses its name for good. **Promised: yes**, written: the wiki's persistence sentence, the promise the owner found for PE-s2, with the store's create built to fail on a missing name. **Delivered: no**, the file is in no later listing (run at both commits). **So: duplicate of PE-s2**: all three grouping tests yes (run); a question of widening PE-s2's "a create" to any store of the path. **Band, decided separately:** none here; PE-s2 is serious.

**Confidence: medium.** Short of high on one condition, `no-precedent`: the joins listed in rule Before 4 are of this shape (another call reaching the same faulty lines), but whether they are indexed as made under the current reading was not checked here.

**Strongest argument against:** most overwrites of an expired file go through the filer's create path (its own read deletes the expired copy first) and are PE-s2 already; the overwrite route needs a read before the deadline and a write inside one Redis round trip, and may add nothing worth a separate entry.
